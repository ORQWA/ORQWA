use super::*;
use crate::{App, TrainerChoice};
use orqwa_devices::fake::FakeRider;
use orqwa_domain::units::{Rpm, Watts};
use std::time::Duration;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let token = orqwa_net::code::RideId::generate().unwrap();
        let path = std::env::temp_dir().join(format!("orqwa-group-test-{:x?}", token.0));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn app(&self) -> App {
        App::new(self.0.join("data"), self.0.join("cache")).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn options() -> ConnectOptions {
    let mut options = ConnectOptions::default();
    options.gather.policy.loopback = true;
    options.gather.stun_servers.clear();
    options
}
fn wait(host: &mut App, peer: &mut App, condition: impl Fn(&App, &App) -> bool) {
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until {
        host.update(Duration::from_millis(16));
        peer.update(Duration::from_millis(16));
        if condition(host, peer) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "group timed out: host {} {}, peer {} {}",
        host.group().phase.key(),
        host.group().error,
        peer.group().phase.key(),
        peer.group().error
    );
}
#[test]
fn app_lobby_downloads_a_workout_starts_locally_and_survives_host_loss() {
    let from = Directory::new();
    let to = Directory::new();
    let mut host = from.app();
    let mut peer = to.app();
    host.connect_trainer(TrainerChoice::Fake(FakeRider {
        power: Watts(180.0),
        cadence: Rpm(85.0),
        heart: None,
    }))
    .unwrap();
    peer.connect_trainer(TrainerChoice::Fake(FakeRider {
        power: Watts(300.0),
        cadence: Rpm(90.0),
        heart: None,
    }))
    .unwrap();
    peer.profile.profile.ftp = Watts(300.0);
    host.group_open(None, options()).unwrap();
    wait(&mut host, &mut peer, |h, _| !h.group().invite.is_empty());
    peer.group_open(Some(host.group().invite), options())
        .unwrap();
    wait(&mut host, &mut peer, |_, p| !p.group().reply.is_empty());
    host.group_admit(peer.group().reply).unwrap();
    wait(&mut host, &mut peer, |h, p| {
        h.group().roster.iter().flatten().count() == 2 && p.group().phase == Phase::Waiting
    });
    assert!(
        host.start_workout(
            host.structured_workout("builtin:recovery-30").unwrap(),
            false,
            DescentMode::Coast,
            "Solo"
        )
        .is_err()
    );
    host.group_select(None, Some("builtin:recovery-30"))
        .unwrap();
    wait(&mut host, &mut peer, |h, p| {
        h.group().prepared && p.group().prepared && h.trainer_connected() && p.trainer_connected()
    });
    assert!(
        peer.workouts()
            .iter()
            .any(|w| w.id.contains("shared-workouts"))
    );
    assert!(host.group_start().is_err());
    host.group_ready(true).unwrap();
    peer.group_ready(true).unwrap();
    wait(&mut host, &mut peer, |h, _| h.group().can_start());
    host.group_start().unwrap();
    wait(&mut host, &mut peer, |h, p| {
        h.ride_state().is_some() && p.ride_state().is_some()
    });
    let a = host.ride_state().unwrap().workout.unwrap();
    let b = peer.ride_state().unwrap().workout.unwrap();
    let a = a.target_power.unwrap().0;
    let b = b.target_power.unwrap().0;
    assert!(
        (b - a * 1.5).abs() < 1.0,
        "FTP-relative targets must follow each rider: {a}, {b}"
    );
    host.group_leave();
    wait(&mut host, &mut peer, |_, p| p.group().phase == Phase::Ended);
    let before = peer.ride_state().unwrap().elapsed;
    peer.update(Duration::from_secs(1));
    assert!(peer.ride_state().unwrap().elapsed > before);
    peer.group_leave();
    assert!(
        peer.workouts()
            .iter()
            .any(|w| w.id.contains("shared-workouts"))
    );
}

#[test]
fn a_late_lobby_admission_receives_the_already_selected_course_without_a_local_copy() {
    let from = Directory::new();
    let to = Directory::new();
    let mut host = from.app();
    let mut peer = to.app();
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../orqwa-gd/courses/twann-chasseral.tqc");
    host.group_open(None, options()).unwrap();
    wait(&mut host, &mut peer, |h, _| !h.group().invite.is_empty());
    host.group_select(Some(source.clone()), None).unwrap();
    wait(&mut host, &mut peer, |h, _| h.group().selection.is_some());
    peer.group_open(Some(host.group().invite), options())
        .unwrap();
    wait(&mut host, &mut peer, |_, p| !p.group().reply.is_empty());
    host.group_admit(peer.group().reply).unwrap();
    wait(&mut host, &mut peer, |_, p| p.group().course.is_some());
    let downloaded = peer.group().course.unwrap();
    assert!(downloaded.starts_with(peer.courses_dir()));
    assert_eq!(
        std::fs::read(&source).unwrap(),
        std::fs::read(downloaded).unwrap()
    );
    peer.group_leave();
    host.group_leave();
}

#[test]
fn voice_off_closes_the_transmission_gate_synchronously_without_opening_audio_devices() {
    let from = Directory::new();
    let to = Directory::new();
    let mut host = from.app();
    let mut idle = to.app();
    host.group_open(None, options()).unwrap();
    wait(&mut host, &mut idle, |h, _| {
        h.group().phase == Phase::Waiting
    });
    let port = host
        .group
        .as_ref()
        .unwrap()
        .voice_controls
        .port
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    port.enable(true);
    port.microphone(true).unwrap();
    assert!(port.transmitting().is_some());
    host.group_voice(false, false, false, 0.6).unwrap();
    assert_eq!(
        port.transmitting(),
        None,
        "off must gate transmission before the actor processes its command"
    );
    host.group_leave();
    assert_eq!(port.transmitting(), None);
}
