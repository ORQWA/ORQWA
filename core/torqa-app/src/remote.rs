//! The phone remote (#228): a page on the local network that a phone's browser opens to shift
//! and work the ride's controls, as a shifter's buttons do (#139). Whether it is on, its port
//! and its secret address are this computer's (R78).

pub use torqa_remote::{NAME as REMOTE_NAME, QrCode, Texts as RemoteTexts, WORDS as REMOTE_WORDS};
use torqa_remote::{RemoteServer, RemoteState};
use torqa_storage::computer::{self, PhoneRemote};

use crate::failure::{Failure, Kind};
use crate::{ActiveRide, App, AppError, AppEvent};

/// The remote while it is on.
#[derive(Debug)]
pub(crate) struct RunningRemote {
    server: RemoteServer,
    input: torqa_remote::Remote,
    /// Whether a phone was connected at the last update, to report changes.
    connected: bool,
}

/// How the phone remote is set and doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteStatus {
    /// Whether it is on.
    pub on: bool,
    /// The port it listens on, or will once on.
    pub port: u16,
    /// The address a phone opens, while it is on and this computer is on a network.
    pub address: Option<String>,
    /// Whether a phone has it open.
    pub connected: bool,
}

impl App {
    /// How the phone remote is set and doing.
    #[must_use]
    pub fn phone_remote(&self) -> RemoteStatus {
        let settings = computer::phone_remote(&self.computer);
        let port = self
            .remote
            .as_ref()
            .map_or_else(|| port_of(&settings), |remote| remote.server.port());
        let address = self
            .remote
            .as_ref()
            .and(settings.token.as_deref())
            .zip(torqa_remote::lan_address())
            .map(|(token, ip)| torqa_remote::address(ip, port, token));
        RemoteStatus {
            on: self.remote.is_some(),
            port,
            address,
            connected: self.remote.as_ref().is_some_and(|remote| remote.connected),
        }
    }

    /// The QR code of the phone remote's address, while it has one.
    #[must_use]
    pub fn phone_remote_code(&self) -> Option<QrCode> {
        torqa_remote::qr_code(&self.phone_remote().address?)
    }

    /// Starts the phone remote if the rider left it on, as when Torqa starts.
    ///
    /// # Errors
    /// [`AppError::Reported`] if it cannot listen on its port, e.g. another program has it.
    pub fn resume_phone_remote(&mut self) -> Result<(), AppError> {
        let settings = computer::phone_remote(&self.computer);
        if self.remote.is_some() || !settings.on {
            return Ok(());
        }
        self.start_remote(settings)
    }

    /// Turns the phone remote on or off, for the next time too. Off, phones can no longer
    /// press; their address works again once it is back on.
    ///
    /// # Errors
    /// [`AppError::Reported`] if it cannot listen on its port, when it stays off, or the
    /// choice cannot be saved.
    pub fn set_phone_remote(&mut self, on: bool) -> Result<(), AppError> {
        let mut settings = computer::phone_remote(&self.computer);
        if on && self.remote.is_none() {
            settings.on = true;
            return self.start_remote(settings);
        }
        if !on {
            self.remote = None;
        }
        settings.on = on;
        save(&self.computer, &settings)
    }

    /// Moves the phone remote to `port` (0 for any free one, as tests do): a phone needs its
    /// new address. Should another program have the port, it stays where it was.
    ///
    /// # Errors
    /// [`AppError::Reported`] if it cannot listen on `port`, or the choice cannot be saved.
    pub fn set_phone_remote_port(&mut self, port: u16) -> Result<(), AppError> {
        let settings = computer::phone_remote(&self.computer);
        let moved = PhoneRemote {
            port: Some(port),
            ..settings.clone()
        };
        self.restart_remote(moved, settings)
    }

    /// Gives the phone remote a new secret address: phones paired before can no longer press,
    /// and the open page tells them so.
    ///
    /// # Errors
    /// [`AppError::Reported`] if it cannot listen on its port again, or the new address
    /// cannot be saved.
    pub fn renew_phone_remote(&mut self) -> Result<(), AppError> {
        let settings = computer::phone_remote(&self.computer);
        let renewed = PhoneRemote {
            token: Some(torqa_remote::new_token()),
            ..settings.clone()
        };
        self.restart_remote(renewed, settings)
    }

    /// The phone remote's page says `texts` in the rider's language (R24), from its next load
    /// on.
    pub fn set_phone_remote_texts(&mut self, texts: RemoteTexts) {
        if let Some(remote) = &self.remote {
            remote.server.set_texts(&texts);
        }
        self.remote_texts = texts;
    }

    /// Feeds the ride the shifts asked for on the phone, passes on its other controls and
    /// shows the phone how the ride goes.
    pub(crate) fn poll_phone_remote(&mut self, events: &mut Vec<AppEvent>) {
        let Some(remote) = &mut self.remote else {
            return;
        };
        crate::drive_shift_input(
            &mut remote.input,
            &mut self.ride,
            &mut remote.connected,
            events,
        );
        remote.server.show(remote_state(self.ride.as_ref()));
    }

    /// Starts the remote as `settings` say, with a secret address if it has none yet, and
    /// remembers them once it runs.
    fn start_remote(&mut self, mut settings: PhoneRemote) -> Result<(), AppError> {
        let token = settings
            .token
            .get_or_insert_with(torqa_remote::new_token)
            .clone();
        let port = port_of(&settings);
        let (server, input) =
            torqa_remote::start(self.runtime.handle(), port, &token, &self.remote_texts).map_err(
                |error| AppError::Reported(Failure::new(Kind::StartRemote, error).arg(port)),
            )?;
        self.remote = Some(RunningRemote {
            server,
            input,
            connected: false,
        });
        settings.on = true;
        save(&self.computer, &settings)
    }

    /// Runs the remote as `wanted` says if it is on, else as `before` said; remembers `wanted`
    /// once it runs, or at once while it is off.
    fn restart_remote(&mut self, wanted: PhoneRemote, before: PhoneRemote) -> Result<(), AppError> {
        if self.remote.is_none() {
            return save(&self.computer, &wanted);
        }
        // The new one may need the port the old one has.
        self.remote = None;
        self.start_remote(wanted).inspect_err(|_| {
            if let Err(error) = self.start_remote(before) {
                tracing::warn!(%error, "the phone remote is off");
            }
        })
    }
}

/// The port the remote listens on as `settings` say.
fn port_of(settings: &PhoneRemote) -> u16 {
    settings.port.unwrap_or(torqa_remote::DEFAULT_PORT)
}

fn save(file: &std::path::Path, settings: &PhoneRemote) -> Result<(), AppError> {
    computer::set_phone_remote(file, settings)
        .map_err(|e| AppError::Reported(Failure::new(Kind::SaveSetting, e)))
}

/// What the phone shows of `ride`.
fn remote_state(ride: Option<&ActiveRide>) -> RemoteState {
    let Some(active) = ride.filter(|active| !active.finished) else {
        return RemoteState::default();
    };
    let state = active.ride.state();
    RemoteState {
        riding: true,
        paused: active.paused,
        gear: state.gear.map(|gear| (gear.number, gear.of)),
        workout: state.workout.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{Ipv4Addr, TcpStream};
    use std::time::Duration;

    use torqa_devices::fake::FakeRider;
    use torqa_domain::profile::Drivetrain;
    use torqa_domain::shifting::Control;
    use torqa_domain::units::{Percent, Rpm, Watts};
    use torqa_physics::DescentMode;

    use super::*;
    use crate::tests::{run_until, temp_dir, write_route};
    use crate::{GhostChoice, TrainerChoice};

    /// An app whose phone remote, once on, listens on any free port.
    fn app(name: &str) -> (std::path::PathBuf, App) {
        let dir = temp_dir(name);
        let mut app = App::new(dir.join("data"), dir.join("cache")).unwrap();
        app.set_phone_remote_port(0).unwrap();
        (dir, app)
    }

    fn token(app: &App) -> String {
        computer::phone_remote(&app.computer).token.unwrap()
    }

    /// Presses `action` as the phone does, with `token`; the answer's status line.
    fn press(port: u16, token: &str, action: &str) -> String {
        let mut socket = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        write!(
            socket,
            "POST /{token}/press/{action} HTTP/1.1\r\nHost: torqa\r\n\r\n"
        )
        .unwrap();
        let mut answer = String::new();
        socket.read_to_string(&mut answer).unwrap();
        answer.lines().next().unwrap_or_default().to_owned()
    }

    /// Opens the phone's event stream; the reader is past its head.
    fn listen(port: u16, token: &str) -> BufReader<TcpStream> {
        let mut socket = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(
            socket,
            "GET /{token}/events HTTP/1.1\r\nHost: torqa\r\n\r\n"
        )
        .unwrap();
        let mut reader = BufReader::new(socket);
        let mut line = String::from("start");
        while line != "\r\n" {
            line.clear();
            reader.read_line(&mut line).unwrap();
        }
        reader
    }

    /// Reads the phone's events until one `wanted`.
    fn wait_for(phone: &mut BufReader<TcpStream>, wanted: impl Fn(&serde_json::Value) -> bool) {
        let mut line = String::new();
        loop {
            line.clear();
            assert!(phone.read_line(&mut line).unwrap() > 0, "the stream ended");
            if let Some(data) = line.strip_prefix("data: ")
                && wanted(&serde_json::from_str(data.trim_end()).unwrap())
            {
                return;
            }
        }
    }

    #[test]
    fn the_remote_is_off_until_turned_on_and_comes_back_on_with_its_address() {
        let (dir, mut app) = app("remote-on");
        let off = app.phone_remote();
        assert!(!off.on);
        assert_eq!(off.address, None, "no address while off");

        app.set_phone_remote(true).unwrap();
        let on = app.phone_remote();
        assert!(on.on);
        assert_ne!(on.port, 0, "listening on a port of its own");
        let token = token(&app);
        assert_eq!(
            press(on.port, &token, "shift_up"),
            "HTTP/1.1 204 No Content"
        );
        if let Some(address) = &on.address {
            assert!(
                address.ends_with(&format!(":{}/{token}/", on.port)),
                "{address}"
            );
            assert!(app.phone_remote_code().is_some(), "a QR code of it");
        }
        app.shutdown();
        drop(app);

        let mut next = App::new(dir.join("data"), dir.join("cache")).unwrap();
        next.resume_phone_remote().unwrap();
        assert!(next.phone_remote().on, "left on");
        assert_eq!(
            super::tests::token(&next),
            token,
            "paired phones keep working"
        );

        next.set_phone_remote(false).unwrap();
        let port = next.phone_remote().port;
        assert!(!next.phone_remote().on);
        assert!(
            TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err(),
            "nothing listens"
        );
        next.shutdown();
        drop(next);
        let mut last = App::new(dir.join("data"), dir.join("cache")).unwrap();
        last.resume_phone_remote().unwrap();
        assert!(!last.phone_remote().on, "left off");
        last.shutdown();
    }

    #[test]
    fn the_phone_shifts_the_ride_asks_for_its_controls_and_sees_the_gear() {
        let (dir, mut app) = app("remote-ride");
        let mut profile = app.profile().profile.clone();
        profile.drivetrain = Drivetrain::SingleCog {
            chainring: 50,
            cog: 14,
        };
        let id = app.profile().id.clone();
        app.save_profile(Some(&id), profile).unwrap();
        app.load_route(write_route(&dir), true);
        run_until(&mut app, |e| matches!(e, AppEvent::RouteLoaded(_)));
        app.connect_trainer(TrainerChoice::Fake(FakeRider {
            power: Watts(200.0),
            cadence: Rpm(90.0),
            heart: None,
        }))
        .unwrap();
        app.start_ride(Percent(50.0), DescentMode::Coast, &GhostChoice::None)
            .unwrap();
        app.set_phone_remote(true).unwrap();
        let (port, token) = (app.phone_remote().port, token(&app));
        let before = app.ride_state().unwrap().gear.unwrap();

        let mut phone = listen(port, &token);
        assert_eq!(press(port, &token, "shift_up"), "HTTP/1.1 204 No Content");
        assert_eq!(press(port, &token, "pause"), "HTTP/1.1 204 No Content");
        let events = app.update(Duration::from_millis(16));

        assert_eq!(
            app.ride_state().unwrap().gear.unwrap().number,
            before.number + 1
        );
        assert!(
            events.contains(&AppEvent::Control(Control::Pause)),
            "{events:?}"
        );
        assert!(
            events.contains(&AppEvent::Connected(REMOTE_NAME.to_owned())),
            "{events:?}"
        );
        assert!(app.phone_remote().connected);
        wait_for(&mut phone, |state| {
            state["riding"] == true
                && state["gear"] == before.number + 1
                && state["gears"] == before.of
        });

        app.set_paused(true);
        app.update(Duration::from_millis(16));
        wait_for(&mut phone, |state| state["paused"] == true);
        app.shutdown();
    }

    #[test]
    fn a_new_address_locks_out_the_phones_paired_before() {
        let (_dir, mut app) = app("remote-renew");
        app.set_phone_remote(true).unwrap();
        let old = token(&app);
        let mut phone = listen(app.phone_remote().port, &old);

        app.renew_phone_remote().unwrap();

        let (port, new) = (app.phone_remote().port, token(&app));
        assert_ne!(new, old);
        assert_eq!(press(port, &old, "shift_up"), "HTTP/1.1 404 Not Found");
        assert_eq!(press(port, &new, "shift_up"), "HTTP/1.1 204 No Content");
        let mut rest = String::new();
        // With the read timeout, a stream left open is an error here.
        assert!(
            phone.read_to_string(&mut rest).is_ok(),
            "the old page's stream ends"
        );
        app.shutdown();
    }

    #[test]
    fn a_port_another_program_has_says_so_and_changes_nothing() {
        let taken = std::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
        let port = taken.local_addr().unwrap().port();
        let (_dir, mut app) = app("remote-taken");

        app.set_phone_remote_port(port).unwrap();
        let Err(AppError::Reported(failure)) = app.set_phone_remote(true) else {
            panic!("the port is taken");
        };
        assert_eq!(failure.kind, Kind::StartRemote);
        assert_eq!(failure.args, [port.to_string()]);
        assert!(!app.phone_remote().on);
        assert!(!computer::phone_remote(&app.computer).on, "not left on");

        app.set_phone_remote_port(0).unwrap();
        app.set_phone_remote(true).unwrap();
        assert!(app.set_phone_remote_port(port).is_err());
        assert!(app.phone_remote().on, "still on where it was");
        assert_eq!(computer::phone_remote(&app.computer).port, Some(0));
        app.shutdown();
    }
}
