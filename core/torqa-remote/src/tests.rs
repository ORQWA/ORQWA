use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use super::*;

const TOKEN: &str = "0123456789abcdef";

fn texts() -> Texts {
    Texts {
        language: "de".to_owned(),
        words: WORDS
            .iter()
            .map(|&key| (key.to_owned(), format!("{key} auf Deutsch")))
            .collect(),
    }
}

/// A remote on a free port.
fn remote() -> (RemoteServer, Remote) {
    start(&tokio::runtime::Handle::current(), 0, TOKEN, &texts()).unwrap()
}

/// Sends a request for `path` and reads the whole answer.
async fn request(port: u16, method: &str, path: &str) -> String {
    let mut socket = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    socket
        .write_all(format!("{method} {path} HTTP/1.1\r\nHost: torqa\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut answer = String::new();
    socket.read_to_string(&mut answer).await.unwrap();
    answer
}

/// Opens the event stream; the reader is past the stream's head.
async fn listen(port: u16) -> BufReader<TcpStream> {
    let mut socket = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    socket
        .write_all(format!("GET /{TOKEN}/events HTTP/1.1\r\nHost: torqa\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut reader = BufReader::new(socket);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    assert_eq!(line, "HTTP/1.1 200 OK\r\n");
    while line != "\r\n" {
        line.clear();
        reader.read_line(&mut line).await.unwrap();
    }
    reader
}

/// The next event's data.
async fn next_event(reader: &mut BufReader<TcpStream>) -> serde_json::Value {
    let mut line = String::new();
    loop {
        line.clear();
        assert!(
            reader.read_line(&mut line).await.unwrap() > 0,
            "the stream ended"
        );
        if let Some(data) = line.strip_prefix("data: ") {
            return serde_json::from_str(data.trim_end()).unwrap();
        }
    }
}

#[tokio::test]
async fn a_press_on_the_phone_shifts_or_asks_for_its_control() {
    let (server, mut remote) = remote();

    for action in ["shift_up", "shift_up_two", "pause", "skip_step"] {
        let answer = request(server.port(), "POST", &format!("/{TOKEN}/press/{action}")).await;
        assert!(answer.starts_with("HTTP/1.1 204"), "{answer}");
    }

    assert_eq!(remote.poll(), [Shift::Up, Shift::Up, Shift::Up]);
    assert_eq!(remote.controls(), [Control::Pause, Control::SkipStep]);
    assert_eq!(remote.poll(), [], "each press once");
    assert_eq!(remote.controls(), []);
}

#[tokio::test]
async fn without_the_token_nothing_is_there_and_nothing_is_pressed() {
    let (server, mut remote) = remote();

    for path in [
        "/press/shift_up".to_owned(),
        "/fedcba9876543210/press/shift_up".to_owned(),
        format!("/{}/press/shift_up", &TOKEN[..15]),
        format!("/{TOKEN}0/press/shift_up"),
        "/".to_owned(),
        "/fedcba9876543210/".to_owned(),
    ] {
        for method in ["GET", "POST"] {
            let answer = request(server.port(), method, &path).await;
            assert!(
                answer.starts_with("HTTP/1.1 404"),
                "{method} {path}: {answer}"
            );
        }
    }

    assert_eq!(remote.poll(), []);
    assert_eq!(remote.controls(), []);
}

#[tokio::test]
async fn an_unknown_action_presses_nothing() {
    let (server, mut remote) = remote();

    for path in ["press/launch", "press/", "press", "shift_up"] {
        let answer = request(server.port(), "POST", &format!("/{TOKEN}/{path}")).await;
        assert!(answer.starts_with("HTTP/1.1 404"), "{path}: {answer}");
    }
    let answer = request(server.port(), "GET", &format!("/{TOKEN}/press/shift_up")).await;
    assert!(
        answer.starts_with("HTTP/1.1 404"),
        "a press is posted: {answer}"
    );

    assert_eq!(remote.poll(), []);
}

#[tokio::test]
async fn the_page_speaks_the_riders_language_and_loads_nothing_from_elsewhere() {
    let (server, _remote) = remote();

    for path in [
        format!("/{TOKEN}/"),
        format!("/{TOKEN}"),
        format!("/{TOKEN}/?from=qr"),
    ] {
        let page = request(server.port(), "GET", &path).await;
        assert!(page.starts_with("HTTP/1.1 200"), "{path}: {page}");
        assert!(page.contains(r#"<html lang="de">"#));
        assert!(page.contains(r#""shift_up":"shift_up auf Deutsch""#));
        assert!(page.contains("Content-Security-Policy: default-src 'none';"));
        assert!(page.contains("Referrer-Policy: no-referrer"));
    }

    let mut odd = texts();
    odd.language = "de\"><script>".to_owned();
    odd.words
        .insert("title".to_owned(), "</script><script>alert(1)".to_owned());
    server.set_texts(&odd);
    let page = request(server.port(), "GET", &format!("/{TOKEN}/")).await;
    assert!(
        page.contains(r#"<html lang="descript">"#),
        "only a language code"
    );
    assert!(
        page.contains(r"\u003c/script>\u003cscript>alert(1)"),
        "a text cannot end the page's script"
    );
    assert_eq!(page.matches("</script>").count(), 2, "the page's own two");
}

#[tokio::test]
async fn the_phone_sees_the_gear_and_the_pause_as_they_change() {
    let (server, remote) = remote();
    assert!(!remote.connected(), "no phone yet");

    let mut phone = listen(server.port()).await;
    let first = next_event(&mut phone).await;
    assert_eq!(first["riding"], false);
    assert!(first["gear"].is_null());
    assert!(remote.connected(), "a phone listens");

    server.show(RemoteState {
        riding: true,
        paused: false,
        gear: Some((7, 24)),
        workout: false,
    });
    let riding = next_event(&mut phone).await;
    assert_eq!(riding["riding"], true);
    assert_eq!(riding["gear"], 7);
    assert_eq!(riding["gears"], 24);

    // The same state again sends nothing: the next event is the pause.
    server.show(RemoteState {
        riding: true,
        paused: false,
        gear: Some((7, 24)),
        workout: false,
    });
    server.show(RemoteState {
        riding: true,
        paused: true,
        gear: Some((7, 24)),
        workout: true,
    });
    let paused = next_event(&mut phone).await;
    assert_eq!(paused["paused"], true);
    assert_eq!(paused["workout"], true);

    drop(phone);
    tokio::time::timeout(Duration::from_secs(5), async {
        while remote.connected() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the phone that closed the page is gone");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stopped_remote_ends_the_phones_streams_and_frees_its_port() {
    let (server, remote) = remote();
    let port = server.port();
    let mut phone = listen(port).await;
    next_event(&mut phone).await;

    drop(server);
    let mut rest = String::new();
    tokio::time::timeout(Duration::from_secs(5), phone.read_to_string(&mut rest))
        .await
        .expect("the stream ends")
        .unwrap();
    assert!(!remote.connected());

    let again = start(&tokio::runtime::Handle::current(), port, TOKEN, &texts());
    assert!(again.is_ok(), "the port is free again: {:?}", again.err());
}

#[tokio::test]
async fn a_port_another_program_has_is_an_error() {
    let taken = std::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    let port = taken.local_addr().unwrap().port();

    let error = start(&tokio::runtime::Handle::current(), port, TOKEN, &texts()).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::AddrInUse);
}

#[tokio::test]
async fn an_endless_request_is_turned_away() {
    let (server, mut remote) = remote();
    let mut socket = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port()))
        .await
        .unwrap();
    let head = format!(
        "POST /{TOKEN}/press/shift_up HTTP/1.1\r\nX: {}",
        "a".repeat(9000)
    );

    // The server may close before it has taken it all.
    let _ = socket.write_all(head.as_bytes()).await;
    let mut buffer = [0; 64];
    let read = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buffer))
        .await
        .expect("closed, not left waiting");

    assert!(matches!(read, Ok(0) | Err(_)), "no answer: {read:?}");
    assert_eq!(remote.poll(), []);
}

#[test]
fn tokens_are_new_each_time() {
    let token = new_token();
    assert_eq!(token.len(), 16);
    assert!(token.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(new_token(), token);
}

#[test]
fn the_address_leads_to_the_page() {
    assert_eq!(
        address(Ipv4Addr::new(192, 168, 1, 20), 8571, TOKEN),
        "http://192.168.1.20:8571/0123456789abcdef/"
    );
}

#[test]
fn the_qr_code_is_square_with_its_three_finder_patterns() {
    let code = qr_code(&address(Ipv4Addr::new(192, 168, 1, 20), 8571, TOKEN)).unwrap();
    let size = code.size;
    assert_eq!(code.dark.len(), size * size);
    assert_eq!((size - 17) % 4, 0, "a QR code's side: {size}");
    let dark = |row: usize, column: usize| code.dark[row * size + column];
    // Each finder: a dark ring, a light ring, a dark 3×3 centre; the fourth corner has none.
    for (top, left) in [(0, 0), (0, size - 7), (size - 7, 0)] {
        for i in 0..7 {
            assert!(dark(top, left + i) && dark(top + 6, left + i));
            assert!(dark(top + i, left) && dark(top + i, left + 6));
        }
        assert!(!dark(top + 1, left + 1) && !dark(top + 5, left + 5));
        assert!(dark(top + 3, left + 3));
    }
    assert!(
        !(0..7).all(|i| dark(size - 1, size - 7 + i)),
        "no finder bottom right"
    );
}
