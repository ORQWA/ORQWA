//! Credential-bound stable identities for one prepared successor listener.

use std::time::Instant;

use crate::code::{AdmissionSecret, Reply};
use crate::handover::{Context, Plan, Proof};
use crate::link::{Connection, Host};
use crate::state::{MAX_RIDERS, RiderId};

use super::{LobbyError, Rider, Roster, SharedActivity};

/// Sources and authority proof prepared for a consenting successor. Constructing this value
/// opens no sockets and does not activate a new organizer.
pub struct Replacement {
    /// Exact old authority commitment, checked against the current group before construction.
    pub plan: Plan,
    /// Frozen current roster; connection credentials retain these rider identities.
    pub roster: Roster,
    /// Already verified local sources for the frozen selection.
    pub activity: SharedActivity,
    /// Original shared start on this computer's monotonic clock.
    pub start: Instant,
}

pub(super) struct Prepared {
    pub(super) replacement: Replacement,
    pub(super) active: bool,
    credentials: [Option<AdmissionSecret>; MAX_RIDERS],
}
impl Prepared {
    pub(super) fn new(replacement: Replacement, host: &Host) -> Result<Self, LobbyError> {
        let plan = &replacement.plan;
        let context = Context::new(
            plan.context.root,
            plan.context.epoch,
            plan.context.predecessor,
            plan.context.successor,
            &replacement.roster,
            &replacement.activity.selection,
        )?;
        // The local listener was already opened under its own destination policy. The plan
        // must pin that exact listener; parsing a plan never widens network permission.
        if plan.context != context || host.invite() != &plan.invite {
            return Err(LobbyError::Protocol);
        }
        plan.clock(replacement.start)?;
        Ok(Self {
            replacement,
            active: false,
            credentials: std::array::from_fn(|_| None),
        })
    }
    pub(super) fn admit(
        &mut self,
        host: &mut Host,
        roster: &Roster,
        id: RiderId,
        reply: &Reply,
    ) -> Result<(), LobbyError> {
        let slot = usize::from(id.value());
        if self.active
            || id == self.replacement.plan.context.successor
            || self.replacement.roster[slot].is_none()
            || roster[slot].is_some()
            || self.credentials[slot].is_some()
        {
            return Err(LobbyError::Protocol);
        }
        host.admit(reply)?;
        self.credentials[slot] = Some(reply.secret.clone());
        Ok(())
    }
    pub(super) fn assign(
        &mut self,
        connection: &Connection,
        rider: &Rider,
        roster: &Roster,
    ) -> Result<RiderId, LobbyError> {
        let credential = connection.admission().ok_or(LobbyError::Protocol)?;
        let slot = self
            .credentials
            .iter()
            .position(|candidate| candidate.as_ref() == Some(credential))
            .ok_or(LobbyError::Protocol)?;
        if roster[slot].is_some() || self.replacement.roster[slot].as_ref() != Some(rider) {
            return Err(LobbyError::Protocol);
        }
        self.credentials[slot] = None;
        Ok(RiderId::new(
            u8::try_from(slot).map_err(|_| LobbyError::Protocol)?,
        )?)
    }
    pub(super) fn activate(
        &mut self,
        epoch: u64,
        proof: &Proof,
        roster: &Roster,
    ) -> Result<bool, LobbyError> {
        let members = roster.iter().enumerate().fold(0_u8, |mask, (slot, rider)| {
            if rider.is_some() {
                mask | (1 << slot)
            } else {
                mask
            }
        });
        if epoch != self.replacement.plan.context.epoch
            || !self.replacement.plan.accepts(proof)
            || members != self.replacement.plan.context.members
        {
            return Err(LobbyError::Protocol);
        }
        let changed = !self.active;
        self.active = true;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::{Assistance, ContentId, ContentKind, Selection};
    use crate::discovery::GatherOptions;
    use crate::handover::{Message, WorkoutPosition};
    use crate::link::{ConnectOptions, Joiner};
    use crate::lobby::{HostLobby, JoinerLobby, LobbyEvent, RideProgress};
    use crate::policy::AddressPolicy;
    use crate::voice::{Playout, VoiceFrame};
    use orqwa_domain::units::{Kilograms, Meters};
    use std::time::Duration;
    use tokio::time::timeout;

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
    async fn client_event(
        client: &mut JoinerLobby,
        matches: impl Fn(&LobbyEvent) -> bool,
    ) -> LobbyEvent {
        timeout(Duration::from_secs(2), async {
            loop {
                let event = client.next().await.unwrap();
                if matches(&event) {
                    return event;
                }
            }
        })
        .await
        .unwrap()
    }
    async fn host_event(host: &mut HostLobby, matches: impl Fn(&LobbyEvent) -> bool) {
        timeout(Duration::from_secs(2), async {
            loop {
                if matches(&host.next().await.unwrap()) {
                    return;
                }
            }
        })
        .await
        .unwrap();
    }
    async fn join(
        host: &HostLobby,
        id: RiderId,
        plan: &Plan,
        roster: &Roster,
        selection: &Selection,
    ) -> JoinerLobby {
        let joiner = Joiner::lobby(plan.invite.clone(), &options())
            .await
            .unwrap();
        host.admit_as(joiner.reply(), id).await.unwrap();
        JoinerLobby::prepare(
            joiner,
            roster[usize::from(id.value())].clone().unwrap(),
            id,
            plan.clone(),
            roster.clone(),
            selection.clone(),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // real stable-ID replacement, commit barrier, voice forwarding, late admission and socket cleanup share one owned listener
    async fn prepared_links_require_the_authorized_proof_and_preserve_ids_voice_and_late_admission()
    {
        let directory = std::env::temp_dir().join(format!(
            "orqwa-handover-{:x?}",
            crate::crypto::random::<8>().unwrap()
        ));
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("workout.oqw");
        std::fs::write(&file, b"frozen workout").unwrap();
        let workout_id = ContentId::of(ContentKind::Workout, b"frozen workout").unwrap();
        let mut roster: Roster = std::array::from_fn(|_| None);
        for (slot, name) in [(0, "Original"), (3, "Successor"), (5, "Friend")] {
            roster[slot] = Some(Rider {
                name: name.into(),
                mass: Kilograms(83.0),
            });
        }
        let selection = Selection {
            revision: 7,
            name: "Together".into(),
            course: None,
            workout: Some(workout_id),
            assistance: Assistance::default(),
        };
        let organizer = RiderId::new(3).unwrap();
        let friend = RiderId::new(5).unwrap();
        let transport = Host::lobby("Successor", &options()).await.unwrap();
        let addresses: Vec<_> = transport
            .invite()
            .candidates
            .iter()
            .map(|candidate| candidate.addr())
            .collect();
        let root = crate::code::RideId([1; 16]);
        let context = Context::new(root, 2, RiderId::HOST, organizer, &roster, &selection).unwrap();
        let workout = Some(WorkoutPosition {
            elapsed: Duration::from_mins(10),
            paused: true,
            sequence: 4,
        });
        let (plan, proof) = Plan::prepare(
            context.clone(),
            transport.invite().clone(),
            Duration::from_mins(5),
            workout,
        )
        .unwrap();
        let (_, wrong) = Plan::prepare(
            context,
            transport.invite().clone(),
            Duration::from_mins(5),
            workout,
        )
        .unwrap();
        let mut host = HostLobby::prepare(
            transport,
            Replacement {
                plan: plan.clone(),
                roster: roster.clone(),
                activity: SharedActivity {
                    selection: selection.clone(),
                    course: None,
                    workout: Some(file),
                },
                start: Instant::now().checked_sub(Duration::from_mins(5)).unwrap(),
            },
        )
        .unwrap();
        let host_voice = host.take_voice().unwrap();
        assert!(!host_voice.alive());
        let mut first = join(&host, RiderId::HOST, &plan, &roster, &selection).await;
        assert_eq!(first.me(), RiderId::HOST);
        assert_eq!(first.organizer(), organizer);
        assert_eq!(first.root(), root);
        assert_eq!(first.epoch(), 2);
        first
            .handover(Message::Commit {
                epoch: 2,
                proof: proof.clone(),
            })
            .unwrap();
        client_event(&mut first, |event| matches!(event, LobbyEvent::HostLeft)).await;
        host_event(
            &mut host,
            |event| matches!(event, LobbyEvent::Left {id} if *id == RiderId::HOST),
        )
        .await;
        assert!(
            !host_voice.alive(),
            "proof cannot activate before all required links exist"
        );
        first.close().await;
        let mut first = join(&host, RiderId::HOST, &plan, &roster, &selection).await;
        let mut other = join(&host, friend, &plan, &roster, &selection).await;
        other
            .handover(Message::Commit {
                epoch: 2,
                proof: wrong,
            })
            .unwrap();
        client_event(&mut other, |event| matches!(event, LobbyEvent::HostLeft)).await;
        host_event(
            &mut host,
            |event| matches!(event, LobbyEvent::Left {id} if *id == friend),
        )
        .await;
        assert!(
            !host_voice.alive(),
            "a prepared successor has no usable early commit proof"
        );
        other.close().await;
        let mut other = join(&host, friend, &plan, &roster, &selection).await;
        let first_voice = first.take_voice().unwrap();
        let other_voice = other.take_voice().unwrap();
        assert!(!first_voice.alive() && !other_voice.alive());
        first.handover(Message::Commit { epoch: 2, proof }).unwrap();
        client_event(&mut first, |event| {
            matches!(
                event,
                LobbyEvent::Handover {
                    message: Message::Commit { .. },
                    ..
                }
            )
        })
        .await;
        client_event(&mut other, |event| {
            matches!(
                event,
                LobbyEvent::Handover {
                    message: Message::Commit { .. },
                    ..
                }
            )
        })
        .await;
        host_event(&mut host, |event| {
            matches!(
                event,
                LobbyEvent::Handover {
                    message: Message::Commit { .. },
                    ..
                }
            )
        })
        .await;
        assert!(host_voice.alive() && first_voice.alive() && other_voice.alive());
        host_voice.enable(true);
        first_voice.enable(true);
        other_voice.enable(true);
        let spurt = first_voice.microphone(true).unwrap();
        timeout(Duration::from_secs(2), async {
            let (mut host_heard, mut other_heard) = (false, false);
            for sequence in 0..80 {
                first_voice
                    .send(VoiceFrame::new(RiderId::HOST, spurt, sequence, b"speech").unwrap())
                    .unwrap();
                tokio::time::sleep(Duration::from_millis(20)).await;
                host_heard |= matches!(
                    host_voice.playout(Instant::now())[0].action,
                    Playout::Frame(_)
                );
                other_heard |= matches!(
                    other_voice.playout(Instant::now())[0].action,
                    Playout::Frame(_)
                );
                if host_heard && other_heard {
                    return;
                }
            }
            panic!("rider zero must send uplink and the nonzero organizer must forward");
        })
        .await
        .unwrap();
        first_voice.microphone(false).unwrap();
        let joiner = Joiner::lobby(plan.invite.clone(), &options())
            .await
            .unwrap();
        host.admit(joiner.reply()).await.unwrap();
        let mut late = JoinerLobby::join(
            joiner,
            Rider {
                name: "Late".into(),
                mass: Kilograms(75.0),
            },
        )
        .await
        .unwrap();
        assert_eq!(late.root(), root);
        assert_eq!(late.epoch(), 2);
        assert_eq!(late.organizer(), organizer);
        client_event(&mut late, |event| {
            matches!(event, LobbyEvent::Running { .. })
        })
        .await;
        let downloaded =
            crate::content::fetch(&late.content_link().unwrap(), workout_id, &directory, None)
                .await
                .unwrap();
        assert_eq!(std::fs::read(downloaded.path()).unwrap(), b"frozen workout");
        drop(downloaded);
        host.progress(Some(RideProgress {
            distance: Meters(1200.0),
            workout: Some(Duration::from_mins(10)),
        }))
        .unwrap();
        late.ready(7, true).unwrap();
        late.enter(7).unwrap();
        assert!(matches!(
            client_event(&mut late, |event| matches!(event, LobbyEvent::Entry { .. })).await,
            LobbyEvent::Entry {
                progress: Some(RideProgress {
                    distance: Meters(1200.0),
                    ..
                }),
                ..
            }
        ));
        late.close().await;
        other.close().await;
        first.close().await;
        host.close().await;
        timeout(Duration::from_secs(2), async {
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
        std::fs::remove_dir_all(directory).unwrap();
    }
}
