//! Eight-rider star lobby with bounded controls, coalesced state and owned background tasks.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use orqwa_domain::units::Kilograms;
use quinn::{RecvStream, SendStream};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::{Instant, interval, timeout, timeout_at};

use crate::code::{Invite, Reply, RouteHash, valid_name};
use crate::link::{Connection, Host, HostEvent, Joiner, LinkError};
use crate::state::{
    MAX_RIDERS, Membership, RiderId, RiderState, STATE_EVERY, StateError, StateInbox,
};
use crate::voice::{self, VoiceHandle, VoiceOwner, VoicePort};
use crate::wire::{self, Message};

mod activity;
use activity::Activities;
pub use activity::SharedActivity;

const CONTROL_CAPACITY: usize = 32;
const WRITE_WAIT: Duration = Duration::from_secs(2);
type States = [Option<RiderState>; MAX_RIDERS];
/// A fixed-size roster, indexed by the host-assigned rider number.
pub type Roster = [Option<Rider>; MAX_RIDERS];

/// Public rider metadata. Heart rate and peer addresses are never part of the roster.
#[derive(Clone, Debug, PartialEq)]
pub struct Rider {
    /// Bounded printable UTF-8 name.
    pub name: String,
    /// Rider plus bicycle mass in kilograms.
    pub mass: Kilograms,
}
impl Rider {
    pub(crate) fn validate(&self) -> Result<(), LobbyError> {
        if !valid_name(&self.name)
            || !self.mass.0.is_finite()
            || !(20.0..=300.0).contains(&self.mass.0)
        {
            return Err(LobbyError::Protocol);
        }
        Ok(())
    }
}

/// Reliable membership/countdown events and disposable latest-state updates.
#[derive(Clone, Debug)]
pub enum LobbyEvent {
    /// The host selected new content; previous local preparation is obsolete.
    Selection(crate::activity::Selection),
    /// One current participant verified/prepared this revision, or withdrew readiness.
    Readiness {
        /// Authenticated roster slot, including the local rider.
        id: RiderId,
        /// Selection this acknowledgment applies to.
        revision: u64,
        /// Whether local preparation completed.
        ready: bool,
    },
    /// An admitted rider joined the roster.
    Joined {
        /// Assigned number.
        id: RiderId,
        /// Printable metadata.
        rider: Rider,
    },
    /// A rider disconnected; the local ride continues.
    Left {
        /// Departing number.
        id: RiderId,
    },
    /// Common countdown target on this computer's monotonic clock.
    Start {
        /// Estimated start, compensating for half the measured RTT at joiners.
        at: Instant,
    },
    /// One validated latest sample, at most five times per second per rider.
    State {
        /// Authenticated sender assigned by the host.
        id: RiderId,
        /// Physical state without heart rate.
        state: RiderState,
    },
    /// A pending reply expired.
    Expired,
    /// The host or networking task ended. The local ride continues.
    HostLeft,
}

/// Bounded lobby failures, without copying peer-supplied diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum LobbyError {
    /// Activity state, membership or revision mismatch.
    #[error(transparent)]
    Activity(#[from] crate::activity::ActivityError),
    /// Discovery, transport or admission failed.
    #[error(transparent)]
    Link(#[from] LinkError),
    /// Invalid physical state or a state flood.
    #[error(transparent)]
    State(#[from] StateError),
    /// Invalid rider metadata, frame or protocol transition.
    #[error("The peer sent an invalid lobby message or rider description.")]
    Protocol,
    /// A queue is full, or the networking owner disappeared.
    #[error("The lobby is busy or disconnected.")]
    Lost,
    /// The countdown has begun; no further riders can join.
    #[error("The ride has already started; late joining is disabled.")]
    Started,
    /// Countdown outside the supported range.
    #[error("Choose a countdown from one to sixty seconds.")]
    Countdown,
}

struct Events {
    controls: mpsc::Receiver<LobbyEvent>,
    states: watch::Receiver<States>,
    pending: States,
    me: RiderId,
    ended: bool,
    active: [bool; MAX_RIDERS],
}
impl Events {
    fn control(&mut self, event: LobbyEvent) -> LobbyEvent {
        match &event {
            LobbyEvent::Joined { id, .. } => self.active[usize::from(id.value())] = true,
            LobbyEvent::Left { id } => {
                self.active[usize::from(id.value())] = false;
                self.pending[usize::from(id.value())] = None;
            }
            _ => {}
        }
        event
    }
    async fn next(&mut self) -> Option<LobbyEvent> {
        loop {
            match self.controls.try_recv() {
                Ok(event) => return Some(self.control(event)),
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    if self.ended {
                        return None;
                    }
                    self.ended = true;
                    return Some(LobbyEvent::HostLeft);
                }
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
            for (id, state) in self.pending.iter_mut().enumerate() {
                let id = rider_id(id);
                if let Some(state) = state
                    .take()
                    .filter(|_| id != self.me && self.active[usize::from(id.value())])
                {
                    return Some(LobbyEvent::State { id, state });
                }
            }
            tokio::select! {
                control = self.controls.recv() => if let Some(event) = control { return Some(self.control(event)); },
                changed = self.states.changed() => if changed.is_ok() {
                    self.pending = *self.states.borrow_and_update();
                },
            }
        }
    }
}

fn rider_id(index: usize) -> RiderId {
    RiderId::new(u8::try_from(index).expect("fixed roster index")).expect("fixed roster index")
}

fn events(
    me: RiderId,
    controls: mpsc::Receiver<LobbyEvent>,
    states: watch::Receiver<States>,
) -> Events {
    Events {
        controls,
        states,
        pending: [None; MAX_RIDERS],
        me,
        ended: false,
        active: [false; MAX_RIDERS],
    }
}

enum Command {
    Admit(Reply, oneshot::Sender<Result<(), LobbyError>>),
    Start(Duration, oneshot::Sender<Result<Instant, LobbyError>>),
    Select(SharedActivity, oneshot::Sender<Result<(), LobbyError>>),
    Ready(u64, bool, oneshot::Sender<Result<(), LobbyError>>),
    PeerReady(Membership, u64, bool),
    Remove(RiderId),
}

/// The host's lobby owner. Dropping it aborts its actor, peer tasks and transport listeners.
pub struct HostLobby {
    invite: Invite,
    commands: mpsc::Sender<Command>,
    state: watch::Sender<Option<RiderState>>,
    events: Events,
    tasks: JoinSet<()>,
    voice: Option<VoicePort>,
}
impl HostLobby {
    /// Starts a lobby from an already-open transport. Metadata is validated before spawning.
    ///
    /// # Errors
    /// Returns [`LobbyError::Protocol`] for invalid metadata or a host-name mismatch.
    pub fn open(host: Host, me: Rider) -> Result<Self, LobbyError> {
        me.validate()?;
        if me.name != host.invite().host {
            return Err(LobbyError::Protocol);
        }
        let invite = host.invite().clone();
        let (commands, receive) = mpsc::channel(8);
        let (control, controls) = mpsc::channel(CONTROL_CAPACITY);
        let (states, updates) = watch::channel([None; MAX_RIDERS]);
        let (state, local) = watch::channel(None);
        let (voice, owner, _changes) = voice::channel(RiderId::HOST);
        let mut tasks = JoinSet::new();
        tasks.spawn(host_actor(
            host,
            me,
            receive,
            control,
            states,
            local,
            owner,
            commands.clone(),
        ));
        Ok(Self {
            invite,
            commands,
            state,
            events: events(RiderId::HOST, controls, updates),
            tasks,
            voice: Some(voice),
        })
    }
    /// The private-channel invite for this host lifetime.
    #[must_use]
    pub fn invite(&self) -> &Invite {
        &self.invite
    }
    /// Takes the local voice port once. It is disabled and microphone-muted initially.
    pub fn take_voice(&mut self) -> Option<VoicePort> {
        self.voice.take()
    }
    /// Explicitly admits a reply before the countdown.
    ///
    /// # Errors
    /// Returns [`LobbyError`] on mismatch, expiry/replay, exhaustion, start or disconnection.
    pub async fn admit(&self, reply: &Reply) -> Result<(), LobbyError> {
        let (send, receive) = oneshot::channel();
        self.commands
            .try_send(Command::Admit(reply.clone(), send))
            .map_err(|_| LobbyError::Lost)?;
        receive.await.map_err(|_| LobbyError::Lost)?
    }
    /// Replaces an app lobby's activity before countdown and invalidates all readiness.
    ///
    /// # Errors
    /// Returns [`LobbyError`] for invalid descriptors, frozen selection or disconnected actor.
    pub async fn select(&self, activity: SharedActivity) -> Result<(), LobbyError> {
        let (send, receive) = oneshot::channel();
        self.commands
            .try_send(Command::Select(activity, send))
            .map_err(|_| LobbyError::Lost)?;
        receive.await.map_err(|_| LobbyError::Lost)?
    }
    /// Sets the host's readiness after local preparation.
    ///
    /// # Errors
    /// Returns [`LobbyError`] for obsolete selection or a disconnected actor.
    pub async fn ready(&self, revision: u64, ready: bool) -> Result<(), LobbyError> {
        let (send, receive) = oneshot::channel();
        self.commands
            .try_send(Command::Ready(revision, ready, send))
            .map_err(|_| LobbyError::Lost)?;
        receive.await.map_err(|_| LobbyError::Lost)?
    }
    /// Disconnects one joiner so an unavailable participant need not block countdown.
    ///
    /// # Errors
    /// Returns [`LobbyError`] for the host slot or a full/stopped command queue.
    pub fn remove(&self, id: RiderId) -> Result<(), LobbyError> {
        if id == RiderId::HOST {
            return Err(LobbyError::Protocol);
        }
        self.commands
            .try_send(Command::Remove(id))
            .map_err(|_| LobbyError::Lost)
    }
    /// Starts once, sealing pending admissions and returning the local countdown target.
    ///
    /// # Errors
    /// Returns [`LobbyError`] for invalid duration, repeated starts or disconnection.
    pub async fn start(&self, countdown: Duration) -> Result<Instant, LobbyError> {
        if !(Duration::from_secs(1)..=Duration::from_secs(60)).contains(&countdown) {
            return Err(LobbyError::Countdown);
        }
        let (send, receive) = oneshot::channel();
        self.commands
            .try_send(Command::Start(countdown, send))
            .map_err(|_| LobbyError::Lost)?;
        receive.await.map_err(|_| LobbyError::Lost)?
    }
    /// Coalesces the local rider's state; no per-call task or event is allocated.
    ///
    /// # Errors
    /// Returns [`LobbyError`] for invalid physical values.
    pub fn send_state(&self, state: RiderState) -> Result<(), LobbyError> {
        state.datagram(RiderId::HOST)?;
        self.state.send_replace(Some(state));
        Ok(())
    }
    /// Next control or latest-state event; a stopped owner yields `HostLeft` then `None`.
    pub async fn next(&mut self) -> Option<LobbyEvent> {
        self.events.next().await
    }
    /// Stops the actor and waits for cancellation. Socket shutdown has a 100 ms close grace.
    pub async fn close(mut self) {
        self.tasks.abort_all();
        while self.tasks.join_next().await.is_some() {}
    }
}

/// The joiner's lobby owner. Losing it closes its connection and owned endpoint.
pub struct JoinerLobby {
    me: RiderId,
    state: watch::Sender<Option<RiderState>>,
    events: Events,
    tasks: JoinSet<()>,
    voice: Option<VoicePort>,
    connection: Connection,
    readiness: mpsc::Sender<(u64, bool)>,
    app_lobby: bool,
}
impl JoinerLobby {
    /// Connects, describes the rider and validates the welcome within the original deadline.
    ///
    /// # Errors
    /// Returns [`LobbyError`] for invalid metadata, transport, timeout or protocol failures.
    pub async fn join(joiner: Joiner, rider: Rider) -> Result<Self, LobbyError> {
        rider.validate()?;
        let route = joiner.reply().route;
        let connection = joiner.connect().await?;
        let (me, roster, send, recv) = timeout_at(connection.admission_deadline(), async {
            let (mut send, mut recv) = connection.open_bi().await?;
            wire::write(&mut send, &Message::Hello(route, rider.clone())).await?;
            let Message::Welcome(me, roster) = wire::read(&mut recv).await? else {
                return Err(LobbyError::Protocol);
            };
            if roster[usize::from(me.value())].as_ref() != Some(&rider) {
                return Err(LobbyError::Protocol);
            }
            Ok::<_, LobbyError>((me, roster, send, recv))
        })
        .await
        .map_err(|_| LinkError::Timeout)??;
        let (control, controls) = mpsc::channel(CONTROL_CAPACITY);
        for (id, rider) in roster
            .iter()
            .enumerate()
            .filter_map(|(id, r)| r.clone().map(|r| (rider_id(id), r)))
        {
            if id != me {
                control
                    .try_send(LobbyEvent::Joined { id, rider })
                    .map_err(|_| LobbyError::Lost)?;
            }
        }
        let (states, updates) = watch::channel([None; MAX_RIDERS]);
        let (state, local) = watch::channel(None);
        let (voice, owner, changes) = voice::channel(me);
        let mut tasks = JoinSet::new();
        let (readiness, ready) = mpsc::channel(8);
        let content_connection = connection.clone();
        tasks.spawn(async move {
            let _ = client_actor(
                connection,
                me,
                roster,
                send,
                recv,
                control,
                states,
                local,
                owner,
                changes,
                ready,
                route == RouteHash::LOBBY,
            )
            .await;
        });
        Ok(Self {
            me,
            state,
            events: events(me, controls, updates),
            tasks,
            voice: Some(voice),
            connection: content_connection,
            readiness,
            app_lobby: route == RouteHash::LOBBY,
        })
    }
    /// This connection's host-assigned number.
    #[must_use]
    pub const fn me(&self) -> RiderId {
        self.me
    }
    /// An authenticated content link for one locally owned download task in app-lobby mode.
    #[must_use]
    pub fn content_link(&self) -> Option<Connection> {
        self.app_lobby.then(|| self.connection.clone())
    }
    /// Announces local readiness for a selected app activity through a bounded control queue.
    ///
    /// # Errors
    /// Returns [`LobbyError`] in legacy mode or if the control queue is full/stopped.
    pub fn ready(&self, revision: u64, ready: bool) -> Result<(), LobbyError> {
        if !self.app_lobby {
            return Err(LobbyError::Protocol);
        }
        self.readiness
            .try_send((revision, ready))
            .map_err(|_| LobbyError::Lost)
    }
    /// Takes the local, disabled voice port once, without opening audio hardware.
    pub fn take_voice(&mut self) -> Option<VoicePort> {
        self.voice.take()
    }
    /// Replaces one latest local state, published at most five times per second.
    ///
    /// # Errors
    /// Returns [`LobbyError`] for invalid physical values.
    pub fn send_state(&self, state: RiderState) -> Result<(), LobbyError> {
        state.datagram(self.me)?;
        self.state.send_replace(Some(state));
        Ok(())
    }
    /// Next control or coalesced state; losing the host never stops local ride logic.
    pub async fn next(&mut self) -> Option<LobbyEvent> {
        self.events.next().await
    }
    /// Closes this connection and stops its actor and readers.
    pub async fn close(mut self) {
        self.tasks.abort_all();
        while self.tasks.join_next().await.is_some() {}
    }
}

impl Drop for JoinerLobby {
    fn drop(&mut self) {
        self.connection.close();
    }
}

struct Mailbox {
    inbox: StateInbox,
    start: Option<Instant>,
}
type Shared = Arc<Mutex<Mailbox>>;
fn lock(shared: &Shared) -> MutexGuard<'_, Mailbox> {
    shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn elapsed(start: Option<Instant>) -> Option<Duration> {
    start
        .filter(|at| *at <= Instant::now())
        .map(|at| Instant::now() - at)
}

#[allow(clippy::large_enum_variant)] // fixed sixteen-entry peer queue; inline controls avoid allocations
enum Control {
    Message(Message),
    Start(Instant),
}
struct Peer {
    connection: Connection,
    member: Membership,
    control: mpsc::Sender<Control>,
}
enum Done {
    Hello(Result<(Connection, SendStream, RecvStream, Rider), LobbyError>),
    Left(Membership),
}

fn event(control: &mpsc::Sender<LobbyEvent>, event: LobbyEvent) -> Result<(), LobbyError> {
    control.try_send(event).map_err(|_| LobbyError::Lost)
}
fn broadcast(peers: &[Option<Peer>; MAX_RIDERS], message: &Message) {
    for peer in peers.iter().flatten() {
        if peer
            .control
            .try_send(Control::Message(message.clone()))
            .is_err()
        {
            peer.connection.close();
        }
    }
}

async fn hello(
    connection: Connection,
    route: RouteHash,
) -> Result<(Connection, SendStream, RecvStream, Rider), LobbyError> {
    let (send, recv, rider) = timeout_at(connection.admission_deadline(), async {
        let (send, mut recv) = connection.accept_bi().await?;
        let Message::Hello(received, rider) = wire::read(&mut recv).await? else {
            return Err(LobbyError::Protocol);
        };
        if received != route {
            return Err(LobbyError::Protocol);
        }
        Ok::<_, LobbyError>((send, recv, rider))
    })
    .await
    .map_err(|_| LinkError::Timeout)??;
    Ok((connection, send, recv, rider))
}

#[allow(clippy::too_many_arguments)] // actor channels and the lifetime guard that invalidates audio ports
async fn host_actor(
    mut host: Host,
    me: Rider,
    mut commands: mpsc::Receiver<Command>,
    controls: mpsc::Sender<LobbyEvent>,
    states: watch::Sender<States>,
    mut local: watch::Receiver<Option<RiderState>>,
    voice: VoiceOwner,
    command_sender: mpsc::Sender<Command>,
) {
    let _ = host_loop(
        &mut host,
        me,
        &mut commands,
        &controls,
        &states,
        &mut local,
        &voice.0,
        &command_sender,
    )
    .await;
}

#[allow(clippy::too_many_arguments)] // bounded actor channels and independent voice mailbox
#[allow(clippy::too_many_lines)] // one bounded host transition table owns admission, selection and teardown
async fn host_loop(
    host: &mut Host,
    me: Rider,
    commands: &mut mpsc::Receiver<Command>,
    controls: &mpsc::Sender<LobbyEvent>,
    states: &watch::Sender<States>,
    local: &mut watch::Receiver<Option<RiderState>>,
    voice: &VoiceHandle,
    command_sender: &mpsc::Sender<Command>,
) -> Result<(), LobbyError> {
    let shared = Arc::new(Mutex::new(Mailbox {
        inbox: StateInbox::new(Instant::now().into_std()),
        start: None,
    }));
    let member = lock(&shared)
        .inbox
        .join(RiderId::HOST, Instant::now().into_std())?;
    voice.join(member, None);
    let mut roster: Roster = std::array::from_fn(|_| None);
    roster[0] = Some(me);
    let mut peers: [Option<Peer>; MAX_RIDERS] = std::array::from_fn(|_| None);
    let mut activities = Activities::new(host.invite().route == RouteHash::LOBBY)?;
    let mut tasks = JoinSet::new();
    let mut ticker = interval(STATE_EVERY);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Admit(reply, result)) => {
                    let answer = if lock(&shared).start.is_some() { Err(LobbyError::Started) } else { host.admit(&reply).map_err(Into::into) };
                    let _ = result.send(answer);
                }
                Some(Command::Start(countdown, result)) => {
                    if lock(&shared).start.is_some() { let _ = result.send(Err(LobbyError::Started)); continue; }
                    if let Err(error) = activities.start() { let _ = result.send(Err(error)); continue; }
                    let at = Instant::now() + countdown;
                    lock(&shared).start = Some(at);
                    host.seal();
                    for peer in peers.iter().flatten() {
                        if peer.control.try_send(Control::Start(at)).is_err() { peer.connection.close(); }
                    }
                    event(controls, LobbyEvent::Start { at })?;
                    let _ = result.send(Ok(at));
                }
                Some(Command::Select(selected,result)) => {
                    let answer = activities.select(selected.clone());
                    if answer.is_ok() {
                        broadcast(&peers,&Message::Selection(selected.selection.clone()));
                        event(controls,LobbyEvent::Selection(selected.selection))?;
                    }
                    let _ = result.send(answer);
                }
                Some(Command::Ready(revision,ready,result)) => {
                    let answer = activities.ready(RiderId::HOST,revision,ready);
                    if answer.is_ok() {
                        broadcast(&peers,&Message::Readiness(RiderId::HOST,revision,ready));
                        event(controls,LobbyEvent::Readiness {id:RiderId::HOST,revision,ready})?;
                    }
                    let _ = result.send(answer);
                }
                Some(Command::PeerReady(member,revision,ready)) => {
                    let id = member.rider();
                    if peers[usize::from(id.value())].as_ref().is_some_and(|p| p.member==member)
                        && activities.ready(id,revision,ready).is_ok() {
                        broadcast(&peers,&Message::Readiness(id,revision,ready));
                        event(controls,LobbyEvent::Readiness {id,revision,ready})?;
                    }
                }
                Some(Command::Remove(id)) => if let Some(peer) = &peers[usize::from(id.value())] {peer.connection.close();},
                None => return Ok(()),
            },
            incoming = host.next() => match incoming {
                Some(HostEvent::Joined(connection)) if lock(&shared).start.is_none() && tasks.len() < 16 => {
                    let route = host.invite().route;
                    tasks.spawn(async move { Done::Hello(hello(connection, route).await) });
                }
                Some(HostEvent::Joined(connection)) => connection.close(),
                Some(HostEvent::Expired) => event(controls, LobbyEvent::Expired)?,
                None => return Err(LobbyError::Lost),
            },
            done = tasks.join_next(), if !tasks.is_empty() => match done {
                Some(Ok(Done::Hello(Ok((connection, send, recv, rider))))) if lock(&shared).start.is_none() => {
                    let Some(slot) = roster.iter().position(Option::is_none) else { connection.close(); continue; };
                    let id = rider_id(slot);
                    activities.joined(id)?;
                    let member = lock(&shared).inbox.join(id, Instant::now().into_std())?;
                    voice.join(member,Some(connection.clone()));
                    broadcast(&peers, &Message::Joined(id, rider.clone()));
                    broadcast(&peers, &Message::VoiceRoster(voice.epochs()));
                    event(controls, LobbyEvent::Joined { id, rider: rider.clone() })?;
                    roster[slot] = Some(rider);
                    let (control, receive) = mpsc::channel(16);
                    peers[slot] = Some(Peer { connection: connection.clone(), member, control });
                    let shared = shared.clone(); let welcome = Message::Welcome(id, roster.clone()); let updates = states.subscribe();
                    let voice = voice.clone();
                    let epochs = voice.epochs();
                    let initial = activities.initial(); let selection = activities.selected.subscribe(); let commands = command_sender.clone();
                    tasks.spawn(async move { let _ = serve_peer(&connection, send, recv, welcome, member, shared, receive, updates, voice, epochs,initial,selection,commands).await; connection.close(); Done::Left(member) });
                }
                Some(Ok(Done::Left(member))) => {
                    let slot = usize::from(member.rider().value());
                    if peers[slot].as_ref().is_some_and(|p| p.member == member) {
                        voice.leave(member);
                        activities.left(member.rider());
                        peers[slot] = None; roster[slot] = None; lock(&shared).inbox.remove(member);
                        broadcast(&peers, &Message::Left(member.rider()));
                        broadcast(&peers, &Message::VoiceRoster(voice.epochs()));
                        event(controls, LobbyEvent::Left { id: member.rider() })?;
                    }
                }
                _ => {}
            },
            _ = ticker.tick() => {
                let mut mailbox = lock(&shared);
                if let Some(elapsed) = elapsed(mailbox.start) {
                    if local.has_changed().unwrap_or(false)
                        && let Some(state) = *local.borrow_and_update() {
                            mailbox.inbox.ingest(member, &state.datagram(RiderId::HOST)?, elapsed, Instant::now().into_std())?;
                    }
                    states.send_replace(mailbox.inbox.drain(Instant::now().into_std()));
                }
            },
        }
    }
}

#[allow(clippy::too_many_arguments)] // one bounded peer worker owns both stream halves and mailboxes
async fn serve_peer(
    connection: &Connection,
    mut send: SendStream,
    mut recv: RecvStream,
    welcome: Message,
    member: Membership,
    shared: Shared,
    mut controls: mpsc::Receiver<Control>,
    mut states: watch::Receiver<States>,
    voice: VoiceHandle,
    epochs: [u64; MAX_RIDERS],
    initial: Vec<Message>,
    selected: watch::Receiver<Option<SharedActivity>>,
    commands: mpsc::Sender<Command>,
) -> Result<(), LobbyError> {
    timeout_at(
        connection.admission_deadline(),
        wire::write(&mut send, &welcome),
    )
    .await
    .map_err(|_| LinkError::Timeout)??;
    timeout_at(
        connection.admission_deadline(),
        wire::write(&mut send, &Message::VoiceRoster(epochs)),
    )
    .await
    .map_err(|_| LinkError::Timeout)??;
    for message in initial {
        timeout(WRITE_WAIT, wire::write(&mut send, &message))
            .await
            .map_err(|_| LobbyError::Lost)??;
    }
    let mut readers = JoinSet::new();
    let mut transfers = JoinSet::new();
    let mut transfer_budget = voice::IncomingBudget::controls();
    let receiver_voice = voice.clone();
    readers.spawn(async move {
        let mut budget = voice::IncomingBudget::controls();
        loop {
            let message = wire::read(&mut recv).await?;
            budget.charge().map_err(|_| LobbyError::Protocol)?;
            match message {
                Message::VoiceEnabled(enabled) => receiver_voice
                    .consent(member, enabled)
                    .map_err(|_| LobbyError::Protocol)?,
                Message::Ready(revision, ready) => commands
                    .try_send(Command::PeerReady(member, revision, ready))
                    .map_err(|_| LobbyError::Lost)?,
                _ => return Err(LobbyError::Protocol),
            }
        }
        #[allow(unreachable_code)]
        Ok::<(), LobbyError>(())
    });
    loop {
        tokio::select! {
            control = controls.recv() => {
                let message = match control {
                    Some(Control::Message(message)) => message,
                    Some(Control::Start(at)) => Message::Start(u32::try_from(at.saturating_duration_since(Instant::now()).as_millis()).map_err(|_| LobbyError::Protocol)?),
                    None => return Ok(()),
                };
                timeout(WRITE_WAIT, wire::write(&mut send, &message)).await.map_err(|_| LobbyError::Lost)??;
            },
            changed = states.changed() => {
                changed.map_err(|_| LobbyError::Lost)?;
                let snapshot = *states.borrow_and_update();
                for (id, state) in snapshot.iter().enumerate().filter_map(|(id,state)| state.map(|state| (rider_id(id), state))) {
                    if id != member.rider() { connection.send_datagram(&state.datagram(id)?)?; }
                }
            },
            bytes = connection.read_datagram() => {
                let bytes = bytes?;
                if bytes.first() == Some(&voice::TAG) {
                    voice.ingest(member,&bytes).map_err(|_| LobbyError::Protocol)?;
                    continue;
                }
                let mut mailbox = lock(&shared);
                let elapsed = elapsed(mailbox.start).ok_or(LobbyError::Protocol)?;
                mailbox.inbox.ingest(member, &bytes, elapsed, Instant::now().into_std())?;
            },
            _ = readers.join_next() => return Err(LobbyError::Protocol),
            stream = connection.accept_bi() => {
                transfer_budget.charge().map_err(|_|LobbyError::Protocol)?;
                let (send,recv) = stream?;
                if transfers.try_join_next().is_none() && !transfers.is_empty() {return Err(LobbyError::Protocol);}
                let mut changes = selected.clone();
                let selection = changes.borrow_and_update().clone().ok_or(LobbyError::Protocol)?;
                transfers.spawn(async move {tokio::select! {result = activity::transfer(send,recv,selection) => result,_ = changes.changed() => Err(crate::content::ContentError::Interrupted)}});
            },
            _ = transfers.join_next(), if !transfers.is_empty() => {},
        }
    }
}

#[allow(clippy::too_many_arguments)] // the actor owns the one connection and its bounded channels
#[allow(clippy::too_many_lines)] // ordered reliable controls and datagrams share one connection owner
async fn client_actor(
    connection: Connection,
    me: RiderId,
    mut roster: Roster,
    mut send: SendStream,
    mut recv: RecvStream,
    controls: mpsc::Sender<LobbyEvent>,
    states: watch::Sender<States>,
    mut local: watch::Receiver<Option<RiderState>>,
    voice: VoiceOwner,
    mut voice_changes: watch::Receiver<bool>,
    mut readiness: mpsc::Receiver<(u64, bool)>,
    app_lobby: bool,
) -> Result<(), LobbyError> {
    voice.0.uplink(connection.clone());
    let mut readers = JoinSet::new();
    let (messages, mut incoming) = mpsc::channel(16);
    readers.spawn(async move {
        loop {
            let message = wire::read(&mut recv).await?;
            messages.try_send(message).map_err(|_| LobbyError::Lost)?;
        }
        #[allow(unreachable_code)]
        Ok::<(), LobbyError>(())
    });
    let mut inbox = StateInbox::new(Instant::now().into_std());
    let mut members = [None; MAX_RIDERS];
    for (id, rider) in roster.iter().enumerate() {
        if rider.is_some() && rider_id(id) != me {
            members[id] = Some(inbox.join(rider_id(id), Instant::now().into_std())?);
            voice.0.join(members[id].ok_or(LobbyError::Protocol)?, None);
        }
    }
    let mut start = None;
    let mut selection_revision = 0;
    let mut count = 0_u8;
    let mut control_period = Instant::now();
    let mut ticker = interval(STATE_EVERY);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // A host may forward eight independent 5 Hz streams. Charge even unknown/stale packets.
    let mut packets = 80_u8;
    let mut voice_packets = voice::IncomingBudget::new();
    let mut voice_control_open = true;
    let mut refill = Instant::now();
    loop {
        tokio::select! {
            Some((revision,ready)) = readiness.recv() => {
                timeout(WRITE_WAIT,wire::write(&mut send,&Message::Ready(revision,ready))).await.map_err(|_| LobbyError::Lost)??;
            },
            changed = voice_changes.changed(), if voice_control_open => {
                let enabled = if changed.is_ok() { *voice_changes.borrow_and_update() } else { voice_control_open = false; false };
                timeout(WRITE_WAIT,wire::write(&mut send,&Message::VoiceEnabled(enabled))).await.map_err(|_| LobbyError::Lost)??;
            },
            message = incoming.recv() => {
                if app_lobby && control_period.elapsed()>=Duration::from_secs(1) {count=0;control_period=Instant::now();}
                count = count.checked_add(1).filter(|n| *n <= 64).ok_or(LobbyError::Protocol)?;
                match message.ok_or(LobbyError::Lost)? {
                    Message::Selection(selection) if app_lobby && start.is_none() && selection.revision>selection_revision => {
                        selection_revision = selection.revision; event(&controls,LobbyEvent::Selection(selection))?;
                    }
                    Message::Readiness(id,revision,ready) if app_lobby && revision==selection_revision && roster[usize::from(id.value())].is_some() => {
                        event(&controls,LobbyEvent::Readiness {id,revision,ready})?;
                    }
                    Message::VoiceRoster(epochs) => voice.0.install_epochs(epochs).map_err(|_| LobbyError::Protocol)?,
                    Message::Joined(id, rider) if start.is_none() && roster[usize::from(id.value())].is_none() && id != me => {
                        let index = usize::from(id.value()); members[index] = Some(inbox.join(id, Instant::now().into_std())?);
                        voice.0.join(members[index].ok_or(LobbyError::Protocol)?,None);
                        roster[index] = Some(rider.clone()); event(&controls, LobbyEvent::Joined { id, rider })?;
                    }
                    Message::Left(id) if id != RiderId::HOST && id != me => {
                        let index = usize::from(id.value()); let member = members[index].take().ok_or(LobbyError::Protocol)?;
                        voice.0.leave(member);
                        inbox.remove(member); roster[index] = None; event(&controls, LobbyEvent::Left { id })?;
                    }
                    Message::Start(millis) if start.is_none() => {
                        let at = Instant::now() + Duration::from_millis(u64::from(millis)).saturating_sub(connection.rtt()/2);
                        start = Some(at); event(&controls, LobbyEvent::Start { at })?;
                    }
                    _ => return Err(LobbyError::Protocol),
                }
            },
            bytes = connection.read_datagram() => {
                let bytes = bytes?;
                if bytes.first() == Some(&voice::TAG) {
                    voice_packets.charge().map_err(|_| LobbyError::Protocol)?;
                    let id = RiderId::new(*bytes.get(2).ok_or(LobbyError::Protocol)?)?;
                    if id == me { return Err(LobbyError::Protocol); }
                    if let Some(member) = members[usize::from(id.value())] {
                        voice.0.ingest(member,&bytes).map_err(|_| LobbyError::Protocol)?;
                    }
                    continue;
                }
                let added = u8::try_from((Instant::now() - refill).as_millis()/25).unwrap_or(80);
                if added > 0 {
                    packets = packets.saturating_add(added).min(80);
                    let now = Instant::now();
                    let remainder = (now-refill).as_nanos() % Duration::from_millis(25).as_nanos();
                    refill = now - Duration::from_nanos(u64::try_from(remainder).expect("under 25 ms"));
                }
                if packets == 0 { return Err(StateError::Flood.into()); } packets -= 1;
                let id = RiderId::new(*bytes.get(1).ok_or(LobbyError::Protocol)?)?;
                let elapsed = elapsed(start).ok_or(LobbyError::Protocol)?;
                if id == me { return Err(LobbyError::Protocol); }
                // A datagram sent before a reliable departure may arrive after it.
                if let Some(member) = members[usize::from(id.value())] { inbox.ingest(member, &bytes, elapsed, Instant::now().into_std())?; }
            },
            _ = ticker.tick() => if let Some(elapsed) = elapsed(start) {
                if local.has_changed().unwrap_or(false)
                    && let Some(state) = *local.borrow_and_update() {
                        let bytes = state.datagram(me)?;
                        RiderState::from_datagram(&bytes, me, elapsed)?;
                        connection.send_datagram(&bytes)?;
                }
                states.send_replace(inbox.drain(Instant::now().into_std()));
            },
            _ = readers.join_next() => return Err(LobbyError::Lost),
            _ = connection.accept_bi() => return Err(LobbyError::Protocol),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::GatherOptions;
    use crate::link::ConnectOptions;
    use crate::policy::AddressPolicy;
    use orqwa_domain::units::{Meters, Watts};

    #[tokio::test]
    async fn empty_app_lobbies_distribute_selected_content_and_gate_countdown_on_readiness() {
        use crate::activity::{Assistance, ContentKind, Selection};
        use crate::content::{self, Source};
        let path = std::env::temp_dir().join(format!(
            "orqwa-app-lobby-{:x?}",
            crate::crypto::random::<8>().unwrap()
        ));
        std::fs::create_dir(&path).unwrap();
        let file = path.join("source.oqw");
        std::fs::write(&file, b"normalized workout bytes").unwrap();
        let source = Source::open(&file, ContentKind::Workout).await.unwrap();
        let mut host = HostLobby::open(
            Host::lobby("Host", &options()).await.unwrap(),
            rider("Host"),
        )
        .unwrap();
        let mut client = join(&host, "Friend").await;
        assert!(matches!(
            host.start(Duration::from_secs(1)).await,
            Err(LobbyError::Activity(_))
        ));
        let selection = Selection {
            revision: 1,
            name: "Together".into(),
            course: None,
            workout: Some(source.id()),
            assistance: Assistance::default(),
        };
        host.select(SharedActivity {
            selection: selection.clone(),
            course: None,
            workout: Some(file),
        })
        .await
        .unwrap();
        client_event(&mut client, |e| matches!(e, LobbyEvent::Selection(_))).await;
        let link = client.content_link().unwrap();
        let received = content::fetch(&link, source.id(), &path, None)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(received.path()).unwrap(),
            b"normalized workout bytes"
        );
        drop(received);
        host.ready(1, true).await.unwrap();
        assert!(host.start(Duration::from_secs(1)).await.is_err());
        client.ready(1, true).unwrap();
        timeout(Duration::from_secs(2),async {loop {if matches!(host.next().await,Some(LobbyEvent::Readiness {id,ready:true,..}) if id==client.me()) {break;}}}).await.unwrap();
        host.start(Duration::from_secs(1)).await.unwrap();
        client_event(&mut client, |e| matches!(e, LobbyEvent::Start { .. })).await;
        assert!(
            host.select(SharedActivity {
                selection,
                course: None,
                workout: Some(path.join("source.oqw"))
            })
            .await
            .is_err()
        );
        drop(client);
        drop(host);
        std::fs::remove_dir_all(path).unwrap();
    }

    fn options() -> ConnectOptions {
        ConnectOptions {
            gather: GatherOptions {
                policy: AddressPolicy {
                    lan: false,
                    loopback: true,
                },
                stun_servers: vec![],
                ..GatherOptions::default()
            },
            window: Duration::from_secs(2),
        }
    }
    fn rider(name: &str) -> Rider {
        Rider {
            name: name.into(),
            mass: Kilograms(83.0),
        }
    }
    async fn host() -> HostLobby {
        HostLobby::open(
            Host::open("Host", "Test course", RouteHash([7; 32]), &options())
                .await
                .unwrap(),
            rider("Host"),
        )
        .unwrap()
    }
    async fn join(host: &HostLobby, name: &str) -> JoinerLobby {
        let joiner = Joiner::open(host.invite().clone(), host.invite().route, &options())
            .await
            .unwrap();
        host.admit(joiner.reply()).await.unwrap();
        JoinerLobby::join(joiner, rider(name))
            .await
            .unwrap_or_else(|error| panic!("join {name}: {error}"))
    }
    async fn client_event(
        client: &mut JoinerLobby,
        predicate: impl Fn(&LobbyEvent) -> bool,
    ) -> LobbyEvent {
        timeout(Duration::from_secs(2), async {
            loop {
                let event = client.next().await.expect("client event");
                if predicate(&event) {
                    return event;
                }
            }
        })
        .await
        .unwrap()
    }
    async fn host_event(
        host: &mut HostLobby,
        predicate: impl Fn(&LobbyEvent) -> bool,
    ) -> LobbyEvent {
        timeout(Duration::from_secs(2), async {
            loop {
                let event = host.next().await.expect("host event");
                if predicate(&event) {
                    return event;
                }
            }
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn riders_share_countdown_latest_states_and_departures_without_remote_ride_control() {
        let mut host = host().await;
        let mut anna = join(&host, "Anna").await;
        let mut ben = join(&host, "Ben").await;
        let at = host.start(Duration::from_secs(1)).await.unwrap();
        let LobbyEvent::Start { at: anna_at } =
            client_event(&mut anna, |e| matches!(e, LobbyEvent::Start { .. })).await
        else {
            panic!("start");
        };
        assert!(anna_at.max(at) - anna_at.min(at) < Duration::from_millis(100));
        client_event(&mut ben, |e| matches!(e, LobbyEvent::Start { .. })).await;
        tokio::time::sleep_until(at + Duration::from_millis(5)).await;
        // High-frequency local updates replace one watch value before the 5 Hz publisher.
        for distance in 0..1000 {
            anna.send_state(RiderState {
                at: Duration::from_millis(1),
                distance: Meters(f64::from(distance)),
                power: Watts(222.0),
                ..RiderState::default()
            })
            .unwrap();
        }
        let LobbyEvent::State { id, state } =
            client_event(&mut ben, |e| matches!(e, LobbyEvent::State { .. })).await
        else {
            panic!("state");
        };
        assert_eq!(id, anna.me());
        assert_eq!(state.distance, Meters(999.0));
        assert_eq!(state.power, Watts(222.0));
        assert!(
            timeout(Duration::from_millis(250), ben.next())
                .await
                .is_err(),
            "no duplicate backlog"
        );
        anna.close().await;
        client_event(&mut ben, |e| matches!(e, LobbyEvent::Left { .. })).await;
        host_event(&mut host, |e| matches!(e, LobbyEvent::Left { .. })).await;
        drop(host);
        client_event(&mut ben, |e| matches!(e, LobbyEvent::HostLeft)).await;
        assert!(ben.next().await.is_none());
        // Local ride code can continue producing state even after all networking ends.
        ben.send_state(RiderState::default()).unwrap();
        ben.close().await;
    }

    #[tokio::test]
    async fn full_rosters_and_started_lobbies_cannot_admit_late_riders() {
        let host = host().await;
        let mut clients = Vec::new();
        for number in 1..MAX_RIDERS {
            clients.push(join(&host, &format!("Rider {number}")).await);
        }
        let ninth = Joiner::open(host.invite().clone(), host.invite().route, &options())
            .await
            .unwrap();
        host.admit(ninth.reply()).await.unwrap();
        assert!(JoinerLobby::join(ninth, rider("Ninth")).await.is_err());
        assert_eq!(clients.len(), MAX_RIDERS - 1);
        host.start(Duration::from_secs(1)).await.unwrap();
        assert!(matches!(
            host.start(Duration::from_secs(1)).await,
            Err(LobbyError::Started)
        ));
        let late = Joiner::open(host.invite().clone(), host.invite().route, &options())
            .await
            .unwrap();
        assert!(matches!(
            host.admit(late.reply()).await,
            Err(LobbyError::Started)
        ));
        drop(late);
        drop(clients);
        host.close().await;
    }

    async fn raw_join(host: &HostLobby) -> (Connection, SendStream, RecvStream, RiderId) {
        let joiner = Joiner::open(host.invite().clone(), host.invite().route, &options())
            .await
            .unwrap();
        host.admit(joiner.reply()).await.unwrap();
        let connection = joiner.connect().await.unwrap();
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        wire::write(
            &mut send,
            &Message::Hello(host.invite().route, rider("Peer")),
        )
        .await
        .unwrap();
        let Message::Welcome(id, _) = wire::read(&mut recv).await.unwrap() else {
            panic!("welcome");
        };
        (connection, send, recv, id)
    }

    #[tokio::test]
    async fn forged_senders_invalid_physics_future_timestamps_and_floods_disconnect_only_the_offender()
     {
        for attack in 0..4 {
            let mut host = host().await;
            let (connection, _send, _recv, id) = raw_join(&host).await;
            let at = host.start(Duration::from_secs(1)).await.unwrap();
            tokio::time::sleep_until(at).await;
            let mut bytes = RiderState::default().datagram(id).unwrap();
            match attack {
                0 => bytes[1] = RiderId::HOST.value(),
                1 => bytes[6..10].copy_from_slice(&f32::NAN.to_be_bytes()),
                2 => bytes[2..6].copy_from_slice(&100_000_u32.to_be_bytes()),
                _ => {}
            }
            for _ in 0..if attack == 3 { 20 } else { 1 } {
                let _ = connection.send_datagram(&bytes);
            }
            timeout(Duration::from_secs(1), connection.closed())
                .await
                .unwrap();
            host_event(&mut host, |e| matches!(e, LobbyEvent::Left { .. })).await;
            host.send_state(RiderState::default()).unwrap();
            assert!(!host.tasks.is_empty());
            host.close().await;
        }
    }

    #[tokio::test]
    async fn a_pinned_host_cannot_extend_the_join_deadline_by_stalling_its_lobby_welcome() {
        let mut settings = options();
        settings.window = Duration::from_millis(250);
        let mut host = Host::open("Host", "Test course", RouteHash([7; 32]), &settings)
            .await
            .unwrap();
        let joiner = Joiner::open(host.invite().clone(), host.invite().route, &settings)
            .await
            .unwrap();
        host.admit(joiner.reply()).unwrap();
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let result = JoinerLobby::join(joiner, rider("Anna")).await;
            assert!(matches!(result, Err(LobbyError::Link(LinkError::Timeout))));
        });
        let Some(HostEvent::Joined(connection)) = host.next().await else {
            panic!("connection");
        };
        let (_send, mut recv) = connection.accept_bi().await.unwrap();
        assert!(matches!(
            wire::read(&mut recv).await,
            Ok(Message::Hello(_, _))
        ));
        timeout(Duration::from_millis(500), tasks.join_next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        host.close().await;
    }

    #[tokio::test]
    async fn dropping_lobby_owners_releases_their_actual_udp_ports() {
        let host = host().await;
        let mut addresses: Vec<_> = host
            .invite()
            .candidates
            .iter()
            .map(|candidate| candidate.addr())
            .collect();
        let joiner = Joiner::open(host.invite().clone(), host.invite().route, &options())
            .await
            .unwrap();
        addresses.extend(
            joiner
                .reply()
                .candidates
                .iter()
                .map(|candidate| candidate.addr()),
        );
        host.admit(joiner.reply()).await.unwrap();
        let client = JoinerLobby::join(joiner, rider("Anna")).await.unwrap();
        drop(client);
        drop(host);
        timeout(Duration::from_secs(1), async {
            loop {
                if addresses
                    .iter()
                    .all(|address| std::net::UdpSocket::bind(address).is_ok())
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn slow_event_consumers_coalesce_updates_and_do_not_resurrect_departed_riders() {
        let (send, receive) = mpsc::channel(CONTROL_CAPACITY);
        let (states, updates) = watch::channel([None; MAX_RIDERS]);
        let mut events = events(RiderId::HOST, receive, updates);
        let id = rider_id(1);
        send.try_send(LobbyEvent::Joined {
            id,
            rider: rider("Anna"),
        })
        .unwrap();
        assert!(matches!(
            events.next().await,
            Some(LobbyEvent::Joined { .. })
        ));
        for sample in 0..1000 {
            let mut snapshot = [None; MAX_RIDERS];
            snapshot[1] = Some(RiderState {
                distance: Meters(f64::from(sample)),
                ..RiderState::default()
            });
            states.send_replace(snapshot);
        }
        let Some(LobbyEvent::State { state, .. }) = events.next().await else {
            panic!("state");
        };
        assert_eq!(state.distance, Meters(999.0));
        states.send_replace([Some(RiderState::default()); MAX_RIDERS]);
        send.try_send(LobbyEvent::Left { id }).unwrap();
        assert!(matches!(events.next().await, Some(LobbyEvent::Left { .. })));
        assert!(
            timeout(Duration::from_millis(50), events.next())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn cancelling_lobby_admission_releases_the_joiners_socket_while_the_host_stalls() {
        let mut host = Host::open("Host", "Test course", RouteHash([7; 32]), &options())
            .await
            .unwrap();
        let joiner = Joiner::open(host.invite().clone(), host.invite().route, &options())
            .await
            .unwrap();
        let addresses: Vec<_> = joiner
            .reply()
            .candidates
            .iter()
            .map(|candidate| candidate.addr())
            .collect();
        host.admit(joiner.reply()).unwrap();
        let mut tasks = JoinSet::new();
        tasks.spawn(JoinerLobby::join(joiner, rider("Anna")));
        let Some(HostEvent::Joined(connection)) = host.next().await else {
            panic!("connection");
        };
        let (_send, mut recv) = connection.accept_bi().await.unwrap();
        assert!(matches!(
            wire::read(&mut recv).await,
            Ok(Message::Hello(_, _))
        ));
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        timeout(Duration::from_secs(1), async {
            while !addresses
                .iter()
                .all(|address| std::net::UdpSocket::bind(address).is_ok())
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        host.close().await;
    }

    #[tokio::test]
    async fn voice_sender_forgery_packet_floods_and_control_floods_close_only_the_offending_peer() {
        for attack in 0..3 {
            let mut host = host().await;
            let (connection, mut send, _recv, id) = raw_join(&host).await;
            if attack == 2 {
                for _ in 0..12 {
                    let _ = wire::write(&mut send, &Message::VoiceEnabled(true)).await;
                }
            } else {
                let source = if attack == 0 { RiderId::HOST } else { id };
                let frame = voice::VoiceFrame::new(source, 1, 0, &[1]).unwrap();
                let (bytes, len) = frame.datagram();
                for _ in 0..if attack == 1 { 20 } else { 1 } {
                    let _ = connection.send_datagram(&bytes[..len]);
                }
            }
            timeout(Duration::from_secs(1), connection.closed())
                .await
                .unwrap();
            host_event(&mut host, |event| matches!(event, LobbyEvent::Left { .. })).await;
            assert!(!host.tasks.is_empty());
            host.close().await;
        }
    }
}
