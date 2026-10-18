//! The phone remote (#228): Torqa serves a page on the local network, and a phone, tablet or
//! second computer that opens it in a browser shifts the virtual gears and works the ride's
//! other controls a shifter's buttons can be given (#139), seeing the gear and whether the
//! ride is paused. Nothing to install, nothing from the internet (R3).
//!
//! The rider turns it on. It listens on all of this computer's addresses at a fixed port, so
//! a bookmarked address keeps working, and every address starts with a secret token that only
//! the QR code and the address Torqa shows carry: without it, neither others on the network
//! nor a website open in the rider's browser can press for them (ADR 0019).

mod http;

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher, RandomState};
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, PoisonError, RwLock, mpsc};
use std::time::{Duration, Instant};

use fast_qr::{ECL, QRBuilder};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};
use tokio::task::AbortHandle;
use torqa_domain::shifting::{ButtonAction, Control, Shift, ShiftInput};
use tracing::warn;

/// The port the remote listens on until the rider chooses another.
pub const DEFAULT_PORT: u16 = 8571;

// i18n-begin
/// What the rider knows the remote by, as its connection is reported.
pub const NAME: &str = "Phone remote";
// i18n-end

/// The keys of the page's texts ([`Texts::words`]): the actions it offers, by their names,
/// then what else it says.
pub const WORDS: [&str; 20] = [
    "shift_up",
    "shift_down",
    "pause",
    "resume",
    "difficulty_up",
    "difficulty_down",
    "intensity_up",
    "intensity_down",
    "skip_step",
    "next_camera",
    "previous_track",
    "play_pause",
    "next_track",
    "title",
    "gear",
    "paused",
    "not_riding",
    "reconnecting",
    "old_link",
    "keep_awake",
];

/// More connections than a few phones open are turned away.
const MAX_CONNECTIONS: usize = 16;
/// How long a connection may take to send its request. Browsers open connections ahead of
/// time that may never send one.
const REQUEST_TIME: Duration = Duration::from_secs(3);
/// How often an idle event stream says it is still there, so a phone gone without a word
/// ends it.
const HEARTBEAT: Duration = Duration::from_secs(15);
/// The page says it is there every 10 seconds; a phone not heard from for longer than this is
/// gone, e.g. its screen locked, even while its connection lingers.
const QUIET: Duration = Duration::from_secs(25);

const PAGE: &str = include_str!("page.html");

/// What the phone shows of the ride.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemoteState {
    /// A ride is under way: started and not finished.
    pub riding: bool,
    /// The ride is paused.
    pub paused: bool,
    /// The virtual gear, counted from 1, and how many there are; `None` without virtual gears
    /// (R9).
    pub gear: Option<(usize, usize)>,
    /// The ride is a workout: its intensity changes and its steps can be skipped, the trainer
    /// difficulty does not apply (R66).
    pub workout: bool,
}

impl RemoteState {
    /// The state as an event of the stream.
    fn event(&self) -> String {
        let (gear, gears) = self.gear.unzip();
        let data = serde_json::json!({
            "riding": self.riding,
            "paused": self.paused,
            "gear": gear,
            "gears": gears,
            "workout": self.workout,
        });
        format!("data: {data}\n\n")
    }
}

/// What the page says, in the rider's language: the front end translates it (R24).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Texts {
    /// The language's code, e.g. `de`.
    pub language: String,
    /// The texts by key, one for each of [`WORDS`]; a missing one shows its key.
    pub words: BTreeMap<String, String>,
}

/// The phone remote as a shift input: the presses from the page, which shift or ask for the
/// controls (#139). Made with [`start`], beside the server it takes them from.
#[derive(Debug)]
pub struct Remote {
    presses: mpsc::Receiver<ButtonAction>,
    /// Controls pressed for, until [`ShiftInput::controls`] takes them.
    controls: Vec<Control>,
    shared: Arc<Shared>,
}

impl ShiftInput for Remote {
    fn name(&self) -> &str {
        NAME
    }

    fn poll(&mut self) -> Vec<Shift> {
        let mut shifts = Vec::new();
        for action in self.presses.try_iter() {
            shifts.extend_from_slice(action.shifts());
            if let ButtonAction::Control(control) = action {
                self.controls.push(control);
            }
        }
        shifts
    }

    fn controls(&mut self) -> Vec<Control> {
        std::mem::take(&mut self.controls)
    }

    /// Whether a phone has the page open and was heard from lately.
    fn connected(&self) -> bool {
        self.shared.phone_listens()
    }
}

/// The running server. Dropping it stops it: it lets go of the port, and the phones' event
/// streams end.
#[derive(Debug)]
pub struct RemoteServer {
    shared: Arc<Shared>,
    state: watch::Sender<RemoteState>,
    task: AbortHandle,
    port: u16,
}

impl RemoteServer {
    /// The port it listens on.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Shows the phones `state`; nothing goes out if it has not changed.
    pub fn show(&self, state: RemoteState) {
        self.state.send_if_modified(|shown| {
            let changed = *shown != state;
            *shown = state;
            changed
        });
    }

    /// The page says `texts` from its next load on.
    pub fn set_texts(&self, texts: &Texts) {
        *self
            .shared
            .page
            .write()
            .unwrap_or_else(PoisonError::into_inner) = render(texts);
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// What the server's connections share with the shift input.
#[derive(Debug)]
struct Shared {
    token: String,
    presses: mpsc::Sender<ButtonAction>,
    page: RwLock<Vec<u8>>,
    state: watch::Receiver<RemoteState>,
    /// The event streams open.
    phones: AtomicUsize,
    /// When a phone was last heard from, in milliseconds since `started`.
    heard: AtomicU64,
    started: Instant,
}

impl Shared {
    fn now(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn hear(&self) {
        self.heard.store(self.now(), Ordering::Relaxed);
    }

    fn phone_listens(&self) -> bool {
        let quiet = u64::try_from(QUIET.as_millis()).unwrap_or(u64::MAX);
        self.phones.load(Ordering::Relaxed) > 0
            && self
                .now()
                .saturating_sub(self.heard.load(Ordering::Relaxed))
                <= quiet
    }

    /// The rest of `path` after the token, e.g. `press/shift_up`; `None` without the token.
    fn below_token<'p>(&self, path: &'p str) -> Option<&'p str> {
        let path = path.strip_prefix('/')?;
        let (token, rest) = path.split_once('/').unwrap_or((path, ""));
        // As long whatever the token's first wrong character, so timing gives none away.
        let differs = token.len() != self.token.len()
            || token
                .bytes()
                .zip(self.token.bytes())
                .fold(0, |differs, (a, b)| differs | (a ^ b))
                != 0;
        (!differs).then_some(rest)
    }
}

/// Starts the remote on `port` (0 for any free one) of all of this computer's addresses,
/// with addresses below `token` and the page saying `texts`, its connections served on
/// `runtime`: the server, and the shift input its presses come to.
///
/// # Errors
/// If the port cannot be listened on, e.g. another program has it.
pub fn start(
    runtime: &tokio::runtime::Handle,
    port: u16,
    token: &str,
    texts: &Texts,
) -> std::io::Result<(RemoteServer, Remote)> {
    let listener = bind(port)?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let listener = {
        let _entered = runtime.enter();
        TcpListener::from_std(listener)?
    };
    let (presses_tx, presses) = mpsc::channel();
    let (state, state_rx) = watch::channel(RemoteState::default());
    let shared = Arc::new(Shared {
        token: token.to_owned(),
        presses: presses_tx,
        page: RwLock::new(render(texts)),
        state: state_rx,
        phones: AtomicUsize::new(0),
        heard: AtomicU64::new(0),
        started: Instant::now(),
    });
    let task = runtime
        .spawn(serve(listener, Arc::clone(&shared)))
        .abort_handle();
    Ok((
        RemoteServer {
            shared: Arc::clone(&shared),
            state,
            task,
            port,
        },
        Remote {
            presses,
            controls: Vec::new(),
            shared,
        },
    ))
}

/// Listens on `port`. A remote stopped a moment ago may still hold it, until its task is
/// dropped on the runtime's next turn: that is waited for, briefly.
fn bind(port: u16) -> std::io::Result<std::net::TcpListener> {
    let mut tries = 0;
    loop {
        match std::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) {
            Err(error) if error.kind() == ErrorKind::AddrInUse && tries < 20 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(25));
            }
            result => return result,
        }
    }
}

/// A new secret for the remote's addresses: 64 random bits, as 16 hexadecimal digits.
#[must_use]
pub fn new_token() -> String {
    // The standard library's hasher keys come from the system's randomness.
    format!("{:016x}", RandomState::new().build_hasher().finish())
}

/// This computer's address on the local network, as a phone there reaches it: the one its
/// way out goes through; `None` without a network. The socket only picks the address: a UDP
/// socket sends nothing when it connects.
#[must_use]
pub fn lan_address() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // TEST-NET-1 (RFC 5737): never anyone's, routed as the internet is.
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()? {
        SocketAddr::V4(address)
            if !address.ip().is_loopback() && !address.ip().is_unspecified() =>
        {
            Some(*address.ip())
        }
        _ => None,
    }
}

/// The address a phone opens: the page of the remote at `ip` and `port` with `token`.
#[must_use]
pub fn address(ip: Ipv4Addr, port: u16, token: &str) -> String {
    format!("http://{ip}:{port}/{token}/")
}

/// A QR code: `size` rows of `size` modules, row by row, `true` where dark; without the quiet
/// zone around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrCode {
    /// Modules per side.
    pub size: usize,
    /// The modules, row by row.
    pub dark: Vec<bool>,
}

/// The QR code of `text`, e.g. the remote's address; `None` if it is too long for one.
#[must_use]
pub fn qr_code(text: &str) -> Option<QrCode> {
    // Medium error correction: a phone reads it off a screen at an angle, not off paper.
    let code = QRBuilder::new(text).ecl(ECL::M).build().ok()?;
    let dark = (0..code.size)
        .flat_map(|row| code[row].iter().map(|module| module.value()))
        .collect();
    Some(QrCode {
        size: code.size,
        dark,
    })
}

/// The page saying `texts`.
fn render(texts: &Texts) -> Vec<u8> {
    let language: String = texts
        .language
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    // Inside the page's script element: no `<` may end it early.
    let words = serde_json::to_string(&texts.words)
        .unwrap_or_else(|_| "{}".to_owned())
        .replace('<', "\\u003c");
    PAGE.replace("{{LANGUAGE}}", &language)
        .replace("{{WORDS}}", &words)
        .into_bytes()
}

async fn serve(listener: TcpListener, shared: Arc<Shared>) {
    let connections = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        let socket = match listener.accept().await {
            Ok((socket, _)) => socket,
            Err(error) => {
                // E.g. out of file handles: let others close theirs first.
                warn!(%error, "the phone remote cannot take a connection");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let Ok(permit) = Arc::clone(&connections).try_acquire_owned() else {
            continue;
        };
        let shared = Arc::clone(&shared);
        tokio::spawn(async move {
            answer(socket, shared).await;
            drop(permit);
        });
    }
}

/// Answers a connection's request. A phone that went meanwhile does not hear the answer; it
/// presses again.
async fn answer(mut socket: TcpStream, shared: Arc<Shared>) {
    let Ok(Some(request)) =
        tokio::time::timeout(REQUEST_TIME, http::read_request(&mut socket)).await
    else {
        return;
    };
    // Without the token, nothing is there: the same answer as for a wrong path.
    let Some(rest) = shared.below_token(&request.path) else {
        let _ = http::reply_empty(&mut socket, "404 Not Found").await;
        return;
    };
    let _ = match (request.method.as_str(), rest) {
        ("GET", "") => {
            let page = shared
                .page
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            http::reply(&mut socket, "200 OK", "text/html; charset=utf-8", &page).await
        }
        ("GET", "events") => {
            stream(socket, &shared).await;
            return;
        }
        ("POST", "alive") => {
            shared.hear();
            http::reply_empty(&mut socket, "204 No Content").await
        }
        ("POST", rest) => match rest
            .strip_prefix("press/")
            .and_then(ButtonAction::from_name)
        {
            Some(action) => {
                shared.hear();
                // Only fails once the remote's input is gone, when it is being stopped.
                let _ = shared.presses.send(action);
                http::reply_empty(&mut socket, "204 No Content").await
            }
            None => http::reply_empty(&mut socket, "404 Not Found").await,
        },
        _ => http::reply_empty(&mut socket, "404 Not Found").await,
    };
}

/// Counts an event stream open for as long as it lives.
struct Listening<'s>(&'s Shared);

impl<'s> Listening<'s> {
    fn new(shared: &'s Shared) -> Self {
        shared.phones.fetch_add(1, Ordering::Relaxed);
        shared.hear();
        Self(shared)
    }
}

impl Drop for Listening<'_> {
    fn drop(&mut self) {
        self.0.phones.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Sends the phone the state now and whenever it changes, until it goes or the server stops.
async fn stream(mut socket: TcpStream, shared: &Shared) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    if http::start_events(&mut socket).await.is_err() {
        return;
    }
    let _listening = Listening::new(shared);
    let mut state = shared.state.clone();
    let mut next = Some(state.borrow_and_update().event());
    let mut buffer = [0; 64];
    loop {
        if let Some(event) = next.take()
            && socket.write_all(event.as_bytes()).await.is_err()
        {
            return;
        }
        tokio::select! {
            changed = state.changed() => {
                // The server stopped.
                if changed.is_err() {
                    return;
                }
                next = Some(state.borrow_and_update().event());
            }
            () = tokio::time::sleep(HEARTBEAT) => next = Some(": still here\n\n".to_owned()),
            // A phone sends nothing more on the stream; it ends when the phone closes it.
            read = socket.read(&mut buffer) => if matches!(read, Ok(0) | Err(_)) {
                return;
            },
        }
    }
}

#[cfg(test)]
mod tests;
