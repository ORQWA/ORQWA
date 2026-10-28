//! Small, length-prefixed lobby controls. No general-purpose peer-controlled deserializer.

use std::time::Duration;

use quinn::{RecvStream, SendStream};
use tokio::time::timeout;

use crate::code::RouteHash;
use crate::lobby::{LobbyError, Rider, Roster};
use crate::state::{MAX_RIDERS, RiderId};

const MAX_FRAME: usize = 1024;

#[derive(Clone, Debug)]
pub(crate) enum Message {
    Hello(RouteHash, Rider),
    Welcome(RiderId, Roster),
    Joined(RiderId, Rider),
    Left(RiderId),
    Start(u32),
    VoiceEnabled(bool),
    VoiceRoster([u64; MAX_RIDERS]),
    Selection(crate::activity::Selection),
    Ready(u64, bool),
    Readiness(RiderId, u64, bool),
}

pub(crate) async fn write(send: &mut SendStream, message: &Message) -> Result<(), LobbyError> {
    let bytes = encode(message)?;
    send.write_all(
        &u16::try_from(bytes.len())
            .map_err(|_| LobbyError::Protocol)?
            .to_be_bytes(),
    )
    .await
    .map_err(|_| LobbyError::Lost)?;
    send.write_all(&bytes).await.map_err(|_| LobbyError::Lost)
}

// Waiting for a whole control is allowed while in the lobby. Once the first byte
// arrives, a partial frame must complete in two seconds, including its length header.
pub(crate) async fn read(recv: &mut RecvStream) -> Result<Message, LobbyError> {
    let mut first = [0];
    recv.read_exact(&mut first)
        .await
        .map_err(|_| LobbyError::Lost)?;
    timeout(Duration::from_secs(2), async {
        let mut second = [0];
        recv.read_exact(&mut second)
            .await
            .map_err(|_| LobbyError::Lost)?;
        let len = usize::from(u16::from_be_bytes([first[0], second[0]]));
        if len == 0 || len > MAX_FRAME {
            return Err(LobbyError::Protocol);
        }
        let mut bytes = [0; MAX_FRAME];
        recv.read_exact(&mut bytes[..len])
            .await
            .map_err(|_| LobbyError::Lost)?;
        decode(&bytes[..len])
    })
    .await
    .map_err(|_| LobbyError::Lost)?
}

fn rider(bytes: &mut Vec<u8>, rider: &Rider) -> Result<(), LobbyError> {
    rider.validate()?;
    bytes.push(u8::try_from(rider.name.len()).map_err(|_| LobbyError::Protocol)?);
    bytes.extend_from_slice(rider.name.as_bytes());
    bytes.extend_from_slice(&rider.mass.0.to_be_bytes());
    Ok(())
}

fn encode(message: &Message) -> Result<Vec<u8>, LobbyError> {
    let mut bytes = vec![crate::code::VERSION];
    match message {
        Message::Hello(route, who) => {
            bytes.push(1);
            bytes.extend_from_slice(&route.0);
            rider(&mut bytes, who)?;
        }
        Message::Welcome(me, roster) => {
            bytes.extend_from_slice(&[2, me.value()]);
            bytes.push(
                u8::try_from(roster.iter().flatten().count()).map_err(|_| LobbyError::Protocol)?,
            );
            for (id, who) in roster
                .iter()
                .enumerate()
                .filter_map(|(id, who)| who.as_ref().map(|who| (id, who)))
            {
                bytes.push(u8::try_from(id).map_err(|_| LobbyError::Protocol)?);
                rider(&mut bytes, who)?;
            }
        }
        Message::Joined(id, who) => {
            bytes.extend_from_slice(&[3, id.value()]);
            rider(&mut bytes, who)?;
        }
        Message::Left(id) => bytes.extend_from_slice(&[4, id.value()]),
        Message::Start(millis) => {
            bytes.push(5);
            bytes.extend_from_slice(&millis.to_be_bytes());
        }
        Message::VoiceEnabled(enabled) => bytes.extend_from_slice(&[6, u8::from(*enabled)]),
        Message::VoiceRoster(epochs) => {
            bytes.push(7);
            for epoch in epochs {
                bytes.extend_from_slice(&epoch.to_be_bytes());
            }
        }
        Message::Selection(selection) => {
            bytes.push(8);
            bytes.extend_from_slice(&selection.encode().map_err(|_| LobbyError::Protocol)?);
        }
        Message::Ready(revision, ready) => {
            bytes.extend_from_slice(&[9, u8::from(*ready)]);
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
        Message::Readiness(id, revision, ready) => {
            bytes.extend_from_slice(&[10, id.value(), u8::from(*ready)]);
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
    }
    if bytes.len() > MAX_FRAME {
        return Err(LobbyError::Protocol);
    }
    Ok(bytes)
}

struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], LobbyError> {
        let bytes = self.0.get(..N).ok_or(LobbyError::Protocol)?;
        let result = bytes.try_into().map_err(|_| LobbyError::Protocol)?;
        self.0 = &self.0[N..];
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, LobbyError> {
        Ok(self.take::<1>()?[0])
    }
    fn id(&mut self) -> Result<RiderId, LobbyError> {
        Ok(RiderId::new(self.byte()?)?)
    }
    fn rider(&mut self) -> Result<Rider, LobbyError> {
        let len = usize::from(self.byte()?);
        if len == 0 || len > crate::code::MAX_NAME_BYTES {
            return Err(LobbyError::Protocol);
        }
        let name = std::str::from_utf8(self.0.get(..len).ok_or(LobbyError::Protocol)?)
            .map_err(|_| LobbyError::Protocol)?;
        self.0 = &self.0[len..];
        let rider = Rider {
            name: name.into(),
            mass: orqwa_domain::units::Kilograms(f64::from_be_bytes(self.take()?)),
        };
        rider.validate()?;
        Ok(rider)
    }
}

fn decode(bytes: &[u8]) -> Result<Message, LobbyError> {
    if bytes.len() > MAX_FRAME {
        return Err(LobbyError::Protocol);
    }
    let mut reader = Reader(bytes);
    if reader.byte()? != crate::code::VERSION {
        return Err(LobbyError::Protocol);
    }
    let message = match reader.byte()? {
        1 => Message::Hello(RouteHash(reader.take()?), reader.rider()?),
        2 => {
            let me = reader.id()?;
            let count = usize::from(reader.byte()?);
            if !(2..=MAX_RIDERS).contains(&count) || me == RiderId::HOST {
                return Err(LobbyError::Protocol);
            }
            let mut roster: Roster = std::array::from_fn(|_| None);
            for _ in 0..count {
                let id = usize::from(reader.id()?.value());
                if roster[id].is_some() {
                    return Err(LobbyError::Protocol);
                }
                roster[id] = Some(reader.rider()?);
            }
            if roster[0].is_none() || roster[usize::from(me.value())].is_none() {
                return Err(LobbyError::Protocol);
            }
            Message::Welcome(me, roster)
        }
        3 => Message::Joined(reader.id()?, reader.rider()?),
        4 => Message::Left(reader.id()?),
        5 => {
            let millis = u32::from_be_bytes(reader.take()?);
            if millis > 60_000 {
                return Err(LobbyError::Protocol);
            }
            Message::Start(millis)
        }
        6 => Message::VoiceEnabled(match reader.byte()? {
            0 => false,
            1 => true,
            _ => return Err(LobbyError::Protocol),
        }),
        7 => {
            let mut epochs = [0; MAX_RIDERS];
            for epoch in &mut epochs {
                *epoch = u64::from_be_bytes(reader.take()?);
            }
            Message::VoiceRoster(epochs)
        }
        8 => {
            let selection =
                crate::activity::Selection::decode(reader.0).map_err(|_| LobbyError::Protocol)?;
            reader.0 = &[];
            Message::Selection(selection)
        }
        9 => {
            let ready = match reader.byte()? {
                0 => false,
                1 => true,
                _ => return Err(LobbyError::Protocol),
            };
            Message::Ready(u64::from_be_bytes(reader.take()?), ready)
        }
        10 => {
            let id = reader.id()?;
            let ready = match reader.byte()? {
                0 => false,
                1 => true,
                _ => return Err(LobbyError::Protocol),
            };
            Message::Readiness(id, u64::from_be_bytes(reader.take()?), ready)
        }
        _ => return Err(LobbyError::Protocol),
    };
    if !reader.0.is_empty() {
        return Err(LobbyError::Protocol);
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use orqwa_domain::units::Kilograms;

    #[test]
    fn untrusted_controls_reject_truncation_trailing_data_duplicate_ids_and_invalid_riders() {
        let who = Rider {
            name: "Anna".into(),
            mass: Kilograms(83.0),
        };
        let valid = encode(&Message::Hello(RouteHash([1; 32]), who.clone())).unwrap();
        assert!(matches!(decode(&valid), Ok(Message::Hello(_, _))));
        for len in 0..valid.len() {
            assert!(decode(&valid[..len]).is_err());
        }
        let mut extra = valid.clone();
        extra.push(0);
        assert!(decode(&extra).is_err());
        for mass in [f64::NAN, f64::INFINITY, -1.0, 1000.0] {
            let mut invalid = valid.clone();
            let at = invalid.len() - 8;
            invalid[at..].copy_from_slice(&mass.to_be_bytes());
            assert!(decode(&invalid).is_err());
        }
        let mut roster: Roster = std::array::from_fn(|_| None);
        roster[0] = Some(who.clone());
        roster[1] = Some(who);
        let welcome = encode(&Message::Welcome(RiderId::new(1).unwrap(), roster)).unwrap();
        assert!(decode(&welcome).is_ok());
        let mut duplicate = welcome;
        duplicate[18] = 0;
        assert!(decode(&duplicate).is_err());
        assert!(decode(&vec![0; MAX_FRAME + 1]).is_err());
        assert!(decode(&[crate::code::VERSION, 5, 0, 1, 0, 0]).is_err());
    }
}
