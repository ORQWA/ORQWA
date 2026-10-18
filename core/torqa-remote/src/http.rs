//! Just enough HTTP/1.1 for the remote's page: a request per connection and its answer, or,
//! for the event stream, answers for as long as the phone listens. Browsers send nothing
//! else to it, and anything else is turned away.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The most a request's head may take; a phone's is well under 2 kB.
const MAX_HEAD: usize = 8 * 1024;

/// What a request asks for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Request {
    /// E.g. `GET`.
    pub method: String,
    /// The path, without any query.
    pub path: String,
}

/// Reads the head of a request; `None` if the connection ends first, or what comes is not an
/// HTTP/1 request or too long for one.
pub(crate) async fn read_request(socket: &mut TcpStream) -> Option<Request> {
    let mut head = Vec::new();
    let mut buffer = [0; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        if head.len() > MAX_HEAD {
            return None;
        }
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        head.extend_from_slice(&buffer[..read]);
    }
    let line = head.split(|&b| b == b'\r').next()?;
    let mut parts = std::str::from_utf8(line).ok()?.split(' ');
    let (method, target, version) = (parts.next()?, parts.next()?, parts.next()?);
    if !version.starts_with("HTTP/1.") || parts.next().is_some() {
        return None;
    }
    let path = target.split('?').next()?;
    Some(Request {
        method: method.to_owned(),
        path: path.to_owned(),
    })
}

/// Answers with `body`, then closes the connection. The page loads nothing from elsewhere
/// and never passes its address on: it holds the token. Its one video, the clip that keeps
/// the screen on, is embedded in it as a `data:` address.
pub(crate) async fn reply(
    socket: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n\
         Content-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; \
         style-src 'unsafe-inline'; media-src data:; connect-src 'self'; \
         frame-ancestors 'none'\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).await?;
    socket.write_all(body).await?;
    socket.shutdown().await
}

/// Answers with `status` alone, e.g. `204 No Content`, then closes the connection.
pub(crate) async fn reply_empty(socket: &mut TcpStream, status: &str) -> std::io::Result<()> {
    // A 204 must not say how long its (absent) body is; the others say it is empty.
    let length = if status.starts_with("204") {
        ""
    } else {
        "Content-Length: 0\r\n"
    };
    let head = format!(
        "HTTP/1.1 {status}\r\n{length}Cache-Control: no-store\r\nConnection: close\r\n\r\n"
    );
    socket.write_all(head.as_bytes()).await?;
    socket.shutdown().await
}

/// Starts an event stream (Server-Sent Events): the connection stays open for its events.
/// The browser tries again two seconds after losing it.
pub(crate) async fn start_events(socket: &mut TcpStream) -> std::io::Result<()> {
    socket
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\
              Connection: keep-alive\r\n\r\nretry: 2000\n\n",
        )
        .await
}
