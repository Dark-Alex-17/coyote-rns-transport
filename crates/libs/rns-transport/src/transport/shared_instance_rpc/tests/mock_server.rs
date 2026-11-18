//! The server side of the handshake from the stdlib spec
//! (`connection.py:513-517`: `deliver_challenge`, then `answer_challenge`),
//! followed by rnsd's one request and one reply (`RNS/Reticulum.py:1277-1334`),
//! on a TCP loopback listener or a unix socket, with the frames the client
//! sent recorded for the tests to inspect. Framing and MACs reuse the
//! client's own `framing` and `handshake` helpers, so a shared mistake would
//! cancel out here; the hand-built header tests and the CPython vectors in
//! `mod.rs` are what keep them honest.

use std::net::SocketAddr;
#[cfg(unix)]
use std::path::Path;

use rand_core::{OsRng, RngCore};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::task::JoinHandle;

use super::super::framing::{recv_frame, send_frame};
use super::super::handshake::{mac, verify_mac, Digest};

const CHALLENGE: &[u8] = b"#CHALLENGE#";
const WELCOME: &[u8] = b"#WELCOME#";
const FAILURE: &[u8] = b"#FAILURE#";
const AUTH_MAX: usize = 256;

/// How the server challenges the client.
#[derive(Debug, Clone, Copy)]
pub(super) enum ServerForm {
    /// `{digest}` + 40 random bytes, the Python 3.12+ server.
    Modern(Digest),
    /// A bare 20 random bytes, the Python 3.11 and older server.
    Legacy,
}

/// How the server answers the client's challenge.
#[derive(Debug, Clone, Copy)]
pub(super) enum Answer {
    Correct,
    /// The right shape with every MAC byte inverted.
    Wrong,
    /// [`Answer::Wrong`], then the socket is closed before the client's
    /// verdict is read, so the client's `#FAILURE#` meets a peer that is
    /// already gone.
    WrongThenHangUp,
    /// A correct MAC in a digest other than the one the client challenged
    /// with, which the stdlib would accept (`connection.py:956-959`).
    OtherDigest(Digest),
}

/// What the server sends after the request.
pub(super) enum Reply {
    /// A well-formed frame carrying the payload.
    Frame(Vec<u8>),
    /// A bare 4-byte header and nothing after it.
    BareHeader(i32),
}

/// What the server does once it has accepted.
pub(super) enum Script {
    /// The full exchange: challenge, verify, answer, serve `reply`, close.
    Exchange { form: ServerForm, key: Vec<u8>, answer: Answer, reply: Reply },
    /// One well-formed frame carrying `payload`, then close.
    Frame(Vec<u8>),
    /// `bytes` written as they are, then close.
    RawBytes(Vec<u8>),
    /// Accept and hold the connection without writing until the client
    /// goes away.
    Silent,
}

/// Every frame the client sent, in order, as the server read them.
#[derive(Debug, Default)]
pub(super) struct Observed {
    pub(super) answer: Option<Vec<u8>>,
    pub(super) challenge: Option<Vec<u8>>,
    pub(super) verdict: Option<Vec<u8>>,
    pub(super) request: Option<Vec<u8>>,
    /// Whether the client hung up once the handshake failed, from one more
    /// read after the `#FAILURE#` went either way: `Some(true)` is EOF,
    /// `Some(false)` is more bytes or an error.
    pub(super) after_failure: Option<bool>,
}

pub(super) struct MockServer {
    addr: Option<SocketAddr>,
    task: JoinHandle<Observed>,
}

impl MockServer {
    pub(super) async fn spawn(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
        let addr = listener.local_addr().expect("listener address");
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            serve(stream, script).await
        });
        Self { addr: Some(addr), task }
    }

    #[cfg(unix)]
    pub(super) fn spawn_unix(path: &Path, script: Script) -> Self {
        let listener = UnixListener::bind(path).expect("bind unix socket");
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            serve(stream, script).await
        });
        Self { addr: None, task }
    }

    pub(super) fn addr(&self) -> SocketAddr {
        self.addr.expect("a tcp listener")
    }

    pub(super) async fn finish(self) -> Observed {
        self.task.await.expect("server task")
    }
}

async fn serve<S>(mut stream: S, script: Script) -> Observed
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut observed = Observed::default();
    match script {
        Script::Exchange { form, key, answer, reply } => {
            exchange(&mut stream, form, &key, answer, reply, &mut observed).await;
        }
        Script::Frame(payload) => {
            send_frame(&mut stream, &payload).await.expect("send frame");
            stream.shutdown().await.expect("shutdown after frame");
            drain(&mut stream).await;
        }
        Script::RawBytes(bytes) => {
            stream.write_all(&bytes).await.expect("write raw bytes");
            stream.shutdown().await.expect("shutdown after raw bytes");
            drain(&mut stream).await;
        }
        Script::Silent => drain(&mut stream).await,
    }
    observed
}

/// Reads until the client closes, so the client sees no reset while it is
/// still writing.
async fn drain<S>(stream: &mut S)
where
    S: AsyncRead + Unpin,
{
    let mut sink = [0u8; 256];
    while let Ok(read) = stream.read(&mut sink).await {
        if read == 0 {
            break;
        }
    }
}

async fn client_hung_up<S>(stream: &mut S) -> bool
where
    S: AsyncRead + Unpin,
{
    let mut sink = [0u8; 16];
    matches!(stream.read(&mut sink).await, Ok(0))
}

async fn exchange<S>(
    stream: &mut S,
    form: ServerForm,
    key: &[u8],
    answer: Answer,
    reply: Reply,
    observed: &mut Observed,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // deliver_challenge (connection.py:970-988)
    let (digest, message) = match form {
        ServerForm::Modern(digest) => {
            let mut message = digest.prefix();
            message.extend_from_slice(&random(40));
            (digest, message)
        }
        ServerForm::Legacy => (Digest::Md5, random(20)),
    };
    let mut frame = CHALLENGE.to_vec();
    frame.extend_from_slice(&message);
    send_frame(stream, &frame).await.expect("send challenge");
    let response = recv_frame(stream, AUTH_MAX).await.expect("recv answer");
    observed.answer = Some(response.clone());
    let tag = match form {
        ServerForm::Modern(digest) => response.strip_prefix(digest.prefix().as_slice()),
        ServerForm::Legacy => Some(response.as_slice()),
    };
    let verified = match tag {
        Some(tag) => verify_mac(digest, key, &message, tag),
        None => false,
    };
    if !verified {
        send_frame(stream, FAILURE).await.expect("send failure");
        observed.after_failure = Some(client_hung_up(stream).await);
        return;
    }
    send_frame(stream, WELCOME).await.expect("send welcome");

    // answer_challenge (connection.py:991-1006) with _create_response (:917-942)
    let challenge = recv_frame(stream, AUTH_MAX).await.expect("recv challenge");
    observed.challenge = Some(challenge.clone());
    let body = challenge.strip_prefix(CHALLENGE).expect("challenge prefix");
    let challenged = if body.len() == 20 {
        Digest::Md5
    } else {
        let close = body.iter().position(|byte| *byte == b'}').expect("digest prefix");
        match &body[1..close] {
            b"sha256" => Digest::Sha256,
            other => panic!("unexpected client digest {:?}", String::from_utf8_lossy(other)),
        }
    };
    let digest = match answer {
        Answer::OtherDigest(other) => other,
        Answer::Correct | Answer::Wrong | Answer::WrongThenHangUp => challenged,
    };
    let mut tag = mac(digest, key, body);
    if let Answer::Wrong | Answer::WrongThenHangUp = answer {
        for byte in &mut tag {
            *byte = !*byte;
        }
    }
    let mut response = if digest == Digest::Md5 { Vec::new() } else { digest.prefix() };
    response.extend(tag);
    send_frame(stream, &response).await.expect("send answer");
    if let Answer::WrongThenHangUp = answer {
        stream.shutdown().await.expect("shutdown before the verdict");
        return;
    }
    let verdict = recv_frame(stream, AUTH_MAX).await.expect("recv verdict");
    observed.verdict = Some(verdict.clone());
    if verdict != WELCOME {
        observed.after_failure = Some(client_hung_up(stream).await);
        return;
    }

    // rpc_loop (RNS/Reticulum.py:1277-1334)
    let request = recv_frame(stream, 1 << 20).await.expect("recv request");
    observed.request = Some(request);
    match reply {
        Reply::Frame(payload) => send_frame(stream, &payload).await.expect("send reply"),
        Reply::BareHeader(header) => {
            stream.write_all(&header.to_be_bytes()).await.expect("write reply header");
            stream.shutdown().await.expect("shutdown after reply header");
        }
    }
}

fn random(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    OsRng.fill_bytes(&mut bytes);
    bytes
}
