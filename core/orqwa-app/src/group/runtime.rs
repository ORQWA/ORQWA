//! One owned app group task; content and audio work never run in the trainer loop.

use std::path::{Path, PathBuf};
use std::time::Duration;

use orqwa_domain::workout::Plan;
use orqwa_net::activity::{Assistance, ContentId, ContentKind, Selection};
use orqwa_net::code::{Invite, Reply};
use orqwa_net::content::{self, Source};
use orqwa_net::link::{ConnectOptions, Host, Joiner};
use orqwa_net::lobby::{HostLobby, JoinerLobby, LobbyEvent, Rider, SharedActivity};
use orqwa_net::state::{RiderId, RiderState};
use orqwa_voice::{VoiceChat, device::DeviceOptions};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use super::{Phase, Snapshot, VoiceControls};
use std::sync::{Arc, atomic::Ordering};

pub(super) enum Command {
    Admit(String),
    Select(Option<PathBuf>, Option<Plan>),
    Prepared(u64),
    Ready(bool),
    Start,
    Remove(RiderId),
    Retry,
    Failure(String),
    Voice(u64, bool, bool, bool, f32),
    Volume(f32),
    Devices,
    ChooseDevices(String, String),
    Mute(RiderId, bool),
}
pub(super) enum Session {
    Host(HostLobby),
    Join(JoinerLobby),
}
impl Session {
    async fn next(&mut self) -> Option<LobbyEvent> {
        match self {
            Self::Host(l) => l.next().await,
            Self::Join(l) => l.next().await,
        }
    }
    async fn ready(&self, revision: u64, ready: bool) -> Result<(), String> {
        match self {
            Self::Host(l) => l.ready(revision, ready).await.map_err(|e| e.to_string()),
            Self::Join(l) => l.ready(revision, ready).map_err(|e| e.to_string()),
        }
    }
    fn state(&self, state: RiderState) {
        let _ = match self {
            Self::Host(l) => l.send_state(state),
            Self::Join(l) => l.send_state(state),
        };
    }
}
enum Prepared {
    Host(SharedActivity),
    Join(Option<PathBuf>, Option<PathBuf>),
}
enum Audio {
    Chat(u64, VoiceChat),
    Devices(Vec<(String, String, bool, bool)>),
}

pub(super) struct Request {
    pub(super) voice_controls: Arc<VoiceControls>,
    pub(super) invite: Option<String>,
    pub(super) rider: Rider,
    pub(super) options: ConnectOptions,
    pub(super) data: PathBuf,
    pub(super) temporary: PathBuf,
}
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) async fn run(
    request: Request,
    mut commands: mpsc::Receiver<Command>,
    updates: watch::Sender<Snapshot>,
    mut local: watch::Receiver<Option<RiderState>>,
) {
    let mut snapshot = Snapshot {
        phase: Phase::Connecting,
        ..Snapshot::default()
    };
    updates.send_replace(snapshot.clone());
    if let Err(error) = run_inner(request, &mut commands, &updates, &mut snapshot, &mut local).await
    {
        snapshot.phase = Phase::Failed;
        snapshot.error = error;
        snapshot.invite.clear();
        snapshot.reply.clear();
        updates.send_replace(snapshot);
    }
}

#[allow(clippy::too_many_lines)] // one owner makes all state/transfer/audio cancellation paths explicit
async fn run_inner(
    request: Request,
    commands: &mut mpsc::Receiver<Command>,
    updates: &watch::Sender<Snapshot>,
    snapshot: &mut Snapshot,
    local: &mut watch::Receiver<Option<RiderState>>,
) -> Result<(), String> {
    let policy = request.options.gather.policy;
    let temp = Directory(request.temporary);
    tokio::fs::create_dir_all(&temp.0)
        .await
        .map_err(|e| e.to_string())?;
    let mut session = if let Some(invite) = request.invite {
        let invite = Invite::from_code(&invite, policy).map_err(|e| e.to_string())?;
        let joiner = Joiner::lobby(invite, &request.options)
            .await
            .map_err(|e| e.to_string())?;
        snapshot.reply = joiner.reply().to_code(policy).map_err(|e| e.to_string())?;
        updates.send_replace(snapshot.clone());
        let lobby = JoinerLobby::join(joiner, request.rider.clone())
            .await
            .map_err(|e| e.to_string())?;
        snapshot.me = lobby.me().value();
        Session::Join(lobby)
    } else {
        let host = Host::lobby(&request.rider.name, &request.options)
            .await
            .map_err(|e| e.to_string())?;
        snapshot.invite = host.invite().to_code(policy).map_err(|e| e.to_string())?;
        snapshot.host = true;
        Session::Host(HostLobby::open(host, request.rider.clone()).map_err(|e| e.to_string())?)
    };
    snapshot.roster[usize::from(snapshot.me)] = Some(request.rider);
    snapshot.phase = Phase::Waiting;
    snapshot.reply.clear();
    let port = match &mut session {
        Session::Host(l) => l.take_voice(),
        Session::Join(l) => l.take_voice(),
    };
    request
        .voice_controls
        .port
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone_from(&port);
    let mut voice: Option<VoiceChat> = None;
    let mut device_options = DeviceOptions::default();
    let mut voice_volume = 0.6;
    let mut audio = JoinSet::new();
    let mut jobs = JoinSet::new();
    let mut next_revision = 0u64;
    let (progress, mut latest_progress) = watch::channel(content::Progress {
        received: 0,
        total: 0,
    });
    let mut ticker = tokio::time::interval(Duration::from_millis(200));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        updates.send_replace(snapshot.clone());
        tokio::select! {
            event=voice_event(&mut voice)=>{
                match event {
                    Some(orqwa_voice::VoiceEvent::PeerMuted {peer})=>{snapshot.muted[usize::from(peer.value())]=true;snapshot.error="A speaker sent invalid audio and was muted locally.".into();}
                    Some(orqwa_voice::VoiceEvent::DeviceUnavailable)=>{voice=None;snapshot.voice=false;snapshot.microphone=false;snapshot.error="The audio device is unavailable. Choose another device and enable voice again.".into();}
                    Some(orqwa_voice::VoiceEvent::Stopped)|None=>{voice=None;snapshot.voice=false;snapshot.microphone=false;}
                }
            },
            command = commands.recv() => {
                let Some(command) = command else {return Ok(());};
                let result = match command {
                    Command::Admit(text) => match &session {Session::Host(l)=>match Reply::from_code(&text,policy) {Ok(reply)=>l.admit(&reply).await.map_err(|e|e.to_string()),Err(e)=>Err(e.to_string())},Session::Join(_)=>Err("Only the host can admit riders.".into())},
                    Command::Select(course,plan) => {
                        if snapshot.start.is_some() || !snapshot.host {continue;}
                        jobs.abort_all(); while jobs.join_next().await.is_some() {}
                        next_revision = next_revision.checked_add(1).ok_or("Activity revision exhausted.")?;
                        let revision = next_revision; let dir = temp.0.clone();
                        snapshot.phase = Phase::Downloading; snapshot.error.clear(); snapshot.prepared = false;
                        jobs.spawn(async move {chosen(course,plan,dir,revision).await.map(Prepared::Host)}); Ok(())
                    }
                    Command::Prepared(revision) => {if snapshot.selection.as_ref().is_some_and(|s|s.revision==revision) {snapshot.prepared = true; snapshot.phase = Phase::Ready;} Ok(())}
                    Command::Ready(ready) => {
                        if !snapshot.prepared {continue;}
                        session.ready(snapshot.selection.as_ref().ok_or("Select an activity first.")?.revision,ready).await
                    }
                    Command::Start => match &session {Session::Host(l)=>l.start(Duration::from_secs(5)).await.map(|_|()).map_err(|e|e.to_string()),Session::Join(_)=>Err("Only the host starts the ride.".into())},
                    Command::Remove(id) => match &session {Session::Host(l)=>l.remove(id).map_err(|e|e.to_string()),Session::Join(_)=>Err("Only the host removes riders.".into())},
                    Command::Retry => {
                        if let (Session::Join(l),Some(selection)) = (&session,&snapshot.selection) {
                            jobs.abort_all(); while jobs.join_next().await.is_some() {}
                            let link = l.content_link().ok_or("The group connection ended.")?;
                            let selection = selection.clone(); let data = request.data.clone();
                            snapshot.phase = Phase::Downloading; snapshot.error.clear();
                            let progress=progress.clone();jobs.spawn(async move {download(&link,&selection,&data,&progress).await.map(|(c,w)|Prepared::Join(c,w))});
                        } Ok(())
                    }
                    Command::Failure(error) => {snapshot.error = error; snapshot.prepared = false; snapshot.phase = Phase::Waiting; Ok(())}
                    Command::Devices => {
                        if audio.is_empty() {audio.spawn(async {tokio::task::spawn_blocking(||orqwa_voice::device::devices().map(|devices|Audio::Devices(devices.into_iter().take(32).map(|d|(d.id,d.name,d.input,d.output)).collect())).map_err(|e|e.to_string())).await.map_err(|e|e.to_string())?});} Ok(())
                    }
                    Command::ChooseDevices(input,output) => {
                        if !snapshot.voice && audio.is_empty() {device_options=DeviceOptions {input:(!input.is_empty()).then_some(input),output:(!output.is_empty()).then_some(output)};} Ok(())
                    }
                    Command::Mute(id,muted) => {if let Some(chat)=&voice {chat.mute_peer(id,muted);snapshot.muted[usize::from(id.value())]=muted;} Ok(())}
                    Command::Volume(volume)=>{voice_volume=volume;if let Some(chat)=&voice {let _=chat.volume(volume);}Ok(())}
                    Command::Voice(generation,on,mic,deaf,volume) => {
                        // Serialize generation checking and transmission changes with the UI's
                        // immediate mute gate; an older queued unmute cannot reopen it later.
                        let _gate=request.voice_controls.port.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                        if generation!=request.voice_controls.generation.load(Ordering::Acquire){continue;}
                        voice_volume=volume;
                        if !on {audio.abort_all(); voice = None; snapshot.voice = false; snapshot.voice_pending=false; snapshot.microphone = false; snapshot.deafened = false;}
                        else if let Some(chat) = &voice {
                            let configured = (|| {chat.deafen(deaf); chat.volume(volume).map_err(|e|e.to_string())?;
                            chat.microphone(mic && !deaf).map_err(|e|e.to_string())?; Ok::<(),String>(())})();
                            if let Err(error) = configured {snapshot.error=error;}
                            snapshot.microphone = chat.microphone_enabled(); snapshot.deafened = deaf;
                        } else if audio.is_empty() && let Some(port) = &port {
                            snapshot.voice_pending=true;
                            let port = port.clone();let options=device_options.clone(); audio.spawn(async move {VoiceChat::open(port,options).await.map(|chat|Audio::Chat(generation,chat)).map_err(|e|e.to_string())});
                        } Ok(())
                    }
                };
                if let Err(error) = result {snapshot.error = error;}
            },
            event = session.next() => match event {
                Some(LobbyEvent::Joined {id,rider}) => {snapshot.roster[usize::from(id.value())] = Some(rider); snapshot.ready[usize::from(id.value())] = false;}
                Some(LobbyEvent::Left {id}) => {let index = usize::from(id.value()); snapshot.roster[index] = None; snapshot.ready[index] = false; snapshot.states[index] = None;}
                Some(LobbyEvent::Readiness {id,revision,ready}) => if snapshot.selection.as_ref().is_some_and(|s|s.revision==revision) {snapshot.ready[usize::from(id.value())] = ready;},
                Some(LobbyEvent::Selection(selection)) => {
                    if snapshot.selection.as_ref().is_some_and(|s|s.revision==selection.revision) {continue;}
                    jobs.abort_all(); while jobs.join_next().await.is_some() {}
                    snapshot.selection = Some(selection.clone()); snapshot.ready.fill(false); snapshot.prepared = false; snapshot.error.clear();
                    snapshot.course = None; snapshot.workout = None; snapshot.plan=None;snapshot.workout_id.clear();
                    if let Session::Join(l) = &session {
                        let link = l.content_link().ok_or("The group connection ended.")?; let data = request.data.clone();
                        snapshot.phase = Phase::Downloading;
                        progress.send_replace(content::Progress {received:0,total:0});let progress=progress.clone();jobs.spawn(async move {download(&link,&selection,&data,&progress).await.map(|(c,w)|Prepared::Join(c,w))});
                    }
                }
                Some(LobbyEvent::Start {at}) => {snapshot.start = Some(at.into_std()); snapshot.phase = Phase::Countdown;}
                Some(LobbyEvent::State {id,state}) => {snapshot.states[usize::from(id.value())] = Some((state,std::time::Instant::now()));}
                Some(LobbyEvent::Expired) => snapshot.error = "An admission reply expired. Exchange fresh codes.".into(),
                Some(LobbyEvent::HostLeft)|None => {jobs.abort_all(); audio.abort_all(); drop(voice); snapshot.voice = false; snapshot.microphone = false; snapshot.start = None; snapshot.phase = Phase::Ended; snapshot.states.fill(None); updates.send_replace(snapshot.clone()); return Ok(());}
            },
            changed=latest_progress.changed()=>{if changed.is_ok(){let p=*latest_progress.borrow_and_update();snapshot.received=p.received;snapshot.total=p.total;}},
            result = jobs.join_next(), if !jobs.is_empty() => {
                match result {
                    Some(Ok(Ok(Prepared::Host(selected)))) => {
                        if let Session::Host(l) = &session {l.select(selected.clone()).await.map_err(|e|e.to_string())?;}
                        snapshot.selection = Some(selected.selection); snapshot.course = selected.course; snapshot.workout = selected.workout;
                        snapshot.plan = load_plan(snapshot.workout.as_ref()).await?;
                        snapshot.ready.fill(false); snapshot.phase = Phase::Preparing;
                    }
                    Some(Ok(Ok(Prepared::Join(course,workout)))) => {snapshot.course = course; snapshot.workout_id=workout.as_ref().map_or(String::new(),|p|p.display().to_string()); snapshot.workout = workout; snapshot.plan=load_plan(snapshot.workout.as_ref()).await?; snapshot.phase = Phase::Preparing;}
                    Some(Ok(Err(error))) => {snapshot.phase = Phase::Waiting; snapshot.error = error;}
                    Some(Err(error)) if !error.is_cancelled() => {snapshot.phase = Phase::Waiting; snapshot.error = "Activity preparation stopped. Retry the download.".into();}
                    _=>{}
                }
            },
            result = audio.join_next(), if !audio.is_empty() => match result {
                Some(Ok(Ok(Audio::Chat(generation,chat)))) if generation==request.voice_controls.generation.load(Ordering::Acquire) => {let _=chat.volume(voice_volume);voice = Some(chat); snapshot.voice = true;snapshot.voice_pending=false; snapshot.microphone = false; snapshot.deafened = false;snapshot.muted.fill(false);}
                Some(Ok(Ok(Audio::Devices(devices)))) => snapshot.devices=devices,
                Some(Ok(Err(error))) => {snapshot.voice_pending=false;snapshot.error = error;},
                _ => snapshot.voice_pending=false,
            },
            _ = ticker.tick() => {
                if let Some(start) = snapshot.start && std::time::Instant::now()>=start {
                    snapshot.phase = Phase::Riding;
                    if let Some(state) = *local.borrow_and_update() {session.state(state);}
                }
                if let Some(chat) = &voice {snapshot.microphone = chat.microphone_enabled();}
            }
        }
    }
}

async fn voice_event(voice: &mut Option<VoiceChat>) -> Option<orqwa_voice::VoiceEvent> {
    if let Some(chat) = voice {
        chat.next().await
    } else {
        std::future::pending().await
    }
}

async fn load_plan(path: Option<&PathBuf>) -> Result<Option<std::sync::Arc<Plan>>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
    Ok(Some(std::sync::Arc::new(
        orqwa_net::workout::decode(&bytes).map_err(|e| e.to_string())?,
    )))
}

async fn chosen(
    course: Option<PathBuf>,
    plan: Option<Plan>,
    directory: PathBuf,
    revision: u64,
) -> Result<SharedActivity, String> {
    let mut name = String::new();
    let course_id = if let Some(path) = &course {
        let file = path.clone();
        let manifest =
            tokio::task::spawn_blocking(move || orqwa_storage::course::shared::validate(&file))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
        name = manifest.name;
        Some(
            Source::open(path, ContentKind::Course)
                .await
                .map_err(|e| e.to_string())?
                .id(),
        )
    } else {
        None
    };
    let (workout, workout_id) = if let Some(plan) = plan {
        if name.is_empty() {
            name.clone_from(&plan.name);
        }
        let bytes = orqwa_net::workout::encode(&plan).map_err(|e| e.to_string())?;
        let id = ContentId::of(ContentKind::Workout, &bytes).map_err(|e| e.to_string())?;
        let path = directory.join(format!("workout-{revision}.oqw"));
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|e| e.to_string())?;
        (Some(path), Some(id))
    } else {
        (None, None)
    };
    while name.len() > 64 {
        name.pop();
    }
    let selection = Selection {
        revision,
        name,
        course: course_id,
        workout: workout_id,
        assistance: Assistance {
            drafting: false,
            keep_together: false,
        },
    };
    selection.validate().map_err(|e| e.to_string())?;
    Ok(SharedActivity {
        selection,
        course,
        workout,
    })
}

async fn download(
    connection: &orqwa_net::link::Connection,
    selection: &Selection,
    data: &Path,
    progress: &watch::Sender<content::Progress>,
) -> Result<(Option<PathBuf>, Option<PathBuf>), String> {
    let mut files = [None, None];
    for (index, id) in [selection.course, selection.workout]
        .into_iter()
        .enumerate()
    {
        let Some(id) = id else {
            continue;
        };
        let library = data.join(if id.kind() == ContentKind::Course {
            "courses"
        } else {
            "shared-workouts"
        });
        tokio::fs::create_dir_all(&library)
            .await
            .map_err(|e| e.to_string())?;
        let path = library.join(id.basename());
        if path.is_file()
            && Source::open(&path, id.kind())
                .await
                .is_ok_and(|s| s.id() == id)
        {
            files[index] = Some(path);
            continue;
        }
        let received = content::fetch(connection, id, &library, Some(progress))
            .await
            .map_err(|e| e.to_string())?;
        let validated = received
            .validate(move |path| match id.kind() {
                ContentKind::Course => orqwa_storage::course::shared::validate(path).is_ok(),
                ContentKind::Workout => std::fs::read(path)
                    .is_ok_and(|bytes| orqwa_net::workout::decode(&bytes).is_ok()),
            })
            .await
            .map_err(|e| e.to_string())?;
        files[index] = Some(
            validated
                .publish(&library)
                .await
                .map_err(|e| e.to_string())?,
        );
    }
    let [course, workout] = files;
    Ok((course, workout))
}
