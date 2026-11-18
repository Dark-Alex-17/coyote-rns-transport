//! The `multiprocessing.connection` HMAC handshake, client side
//! (`connection.py:785-1006`). The client answers the server's challenge and
//! then delivers its own (`connection.py:557-559`); the server does the
//! reverse (`:513-517`). A challenge body is either the modern
//! `{digest}` + 40 random bytes or a legacy bare 20 bytes from a Python
//! 3.11 or older server, and the answer form follows the challenge form.
//! This client's own challenge mirrors the server's form too, where the
//! stdlib client always challenges with `{sha256}` whatever form the server
//! spoke (`connection.py:970`, `:976`).

use hmac::digest::KeyInit;
use hmac::{Hmac, Mac};
use rand_core::{OsRng, RngCore};
use tokio::io::{AsyncRead, AsyncWrite};

use super::framing::{recv_frame, send_frame};
use super::RpcError;

const CHALLENGE: &[u8] = b"#CHALLENGE#";
const WELCOME: &[u8] = b"#WELCOME#";
const FAILURE: &[u8] = b"#FAILURE#";

/// `MESSAGE_LENGTH` (`connection.py:785`): the random bytes of a modern
/// challenge.
const MESSAGE_LENGTH: usize = 40;
/// `_MD5ONLY_MESSAGE_LENGTH` (`connection.py:886`): a legacy challenge, and
/// the floor `answer_challenge` enforces on any challenge (`:1000-1001`).
const MD5ONLY_MESSAGE_LENGTH: usize = 20;
/// `_MD5_DIGEST_LEN` (`connection.py:887`): a legacy response.
const MD5_DIGEST_LEN: usize = 16;
/// `_MAX_DIGEST_LEN` (`connection.py:880`): the longest allowed digest name.
const MAX_DIGEST_LEN: usize = 8;
/// Every auth frame is read with `recv_bytes(256)` (`connection.py:981`,
/// `:995`, `:1004`).
const AUTH_FRAME_MAX_LEN: usize = 256;

/// Which handshake form the server spoke in its challenge; our own challenge
/// mirrors it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChallengeForm {
    /// A bare 20-byte challenge answered with a raw HMAC-MD5.
    Legacy,
    /// A `{digest}`-prefixed challenge answered with a `{digest}`-prefixed
    /// MAC.
    Modern,
}

/// Python's `_ALLOWED_DIGESTS` (`connection.py:878-879`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Digest {
    Md5,
    Sha256,
    Sha384,
    Sha3_256,
    Sha3_384,
}

impl Digest {
    fn parse(name: &[u8]) -> Option<Self> {
        match name {
            b"md5" => Some(Self::Md5),
            b"sha256" => Some(Self::Sha256),
            b"sha384" => Some(Self::Sha384),
            b"sha3_256" => Some(Self::Sha3_256),
            b"sha3_384" => Some(Self::Sha3_384),
            _ => None,
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha256 => "sha256",
            Self::Sha384 => "sha384",
            Self::Sha3_256 => "sha3_256",
            Self::Sha3_384 => "sha3_384",
        }
    }

    /// The `{digest}` prefix of a modern message.
    pub(super) fn prefix(self) -> Vec<u8> {
        format!("{{{}}}", self.name()).into_bytes()
    }
}

/// `hmac.new(authkey, message, digest).digest()`.
pub(super) fn mac(digest: Digest, key: &[u8], message: &[u8]) -> Result<Vec<u8>, RpcError> {
    match digest {
        Digest::Md5 => tag::<Hmac<md5::Md5>>(key, message),
        Digest::Sha256 => tag::<Hmac<sha2::Sha256>>(key, message),
        Digest::Sha384 => tag::<Hmac<sha2::Sha384>>(key, message),
        Digest::Sha3_256 => tag::<Hmac<sha3::Sha3_256>>(key, message),
        Digest::Sha3_384 => tag::<Hmac<sha3::Sha3_384>>(key, message),
    }
}

/// `hmac.compare_digest` over a fresh MAC (`connection.py:959-967`); a tag of
/// the wrong length fails like a wrong tag.
pub(super) fn verify_mac(
    digest: Digest,
    key: &[u8],
    message: &[u8],
    tag: &[u8],
) -> Result<bool, RpcError> {
    match digest {
        Digest::Md5 => verify::<Hmac<md5::Md5>>(key, message, tag),
        Digest::Sha256 => verify::<Hmac<sha2::Sha256>>(key, message, tag),
        Digest::Sha384 => verify::<Hmac<sha2::Sha384>>(key, message, tag),
        Digest::Sha3_256 => verify::<Hmac<sha3::Sha3_256>>(key, message, tag),
        Digest::Sha3_384 => verify::<Hmac<sha3::Sha3_384>>(key, message, tag),
    }
}

fn keyed<M: Mac + KeyInit>(key: &[u8], message: &[u8]) -> Result<M, RpcError> {
    let mut mac = <M as Mac>::new_from_slice(key).map_err(|_| RpcError::InvalidKeyMaterial)?;
    mac.update(message);
    Ok(mac)
}

fn tag<M: Mac + KeyInit>(key: &[u8], message: &[u8]) -> Result<Vec<u8>, RpcError> {
    Ok(keyed::<M>(key, message)?.finalize().into_bytes().to_vec())
}

fn verify<M: Mac + KeyInit>(key: &[u8], message: &[u8], tag: &[u8]) -> Result<bool, RpcError> {
    Ok(keyed::<M>(key, message)?.verify_slice(tag).is_ok())
}

/// The two message shapes `_get_digest_name_and_payload` tells apart
/// (`connection.py:891-914`).
enum Message<'a> {
    /// 16 or 20 bytes: a legacy challenge or a legacy raw MD5 response.
    Legacy,
    /// `{digest}` and the rest.
    Prefixed { digest: Digest, payload: &'a [u8] },
}

enum PrefixError {
    /// No `{digest}` prefix where one was required.
    Missing,
    /// A `{digest}` prefix naming a digest outside the allowed set.
    Unsupported(String),
}

/// `_get_digest_name_and_payload` (`connection.py:891-914`): the legacy
/// lengths first, then a `{digest}` whose closing brace sits within the
/// first ten bytes (`message.find(b'}', 1, _MAX_DIGEST_LEN + 2)`).
fn parse_message(message: &[u8]) -> Result<Message<'_>, PrefixError> {
    if message.len() == MD5ONLY_MESSAGE_LENGTH || message.len() == MD5_DIGEST_LEN {
        return Ok(Message::Legacy);
    }
    if message.first() != Some(&b'{') {
        return Err(PrefixError::Missing);
    }
    let close = message
        .iter()
        .skip(1)
        .take(MAX_DIGEST_LEN + 1)
        .position(|byte| *byte == b'}')
        .map(|offset| offset + 1)
        .ok_or(PrefixError::Missing)?;
    let name = &message[1..close];
    match Digest::parse(name) {
        Some(digest) => Ok(Message::Prefixed { digest, payload: &message[close + 1..] }),
        None => Err(PrefixError::Unsupported(name.escape_ascii().to_string())),
    }
}

/// `answer_challenge` (`connection.py:991-1006`) with `_create_response`
/// (`:917-942`): the MAC covers the whole challenge body, `{digest}` prefix
/// included, and the reply carries the same prefix; a legacy challenge gets
/// a raw HMAC-MD5. Anything but `#WELCOME#` back is
/// [`RpcError::Authentication`].
pub(super) async fn answer_challenge<S>(
    stream: &mut S,
    key: &[u8],
) -> Result<ChallengeForm, RpcError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let message = recv_frame(stream, AUTH_FRAME_MAX_LEN).await?;
    let body = message.strip_prefix(CHALLENGE).ok_or_else(|| {
        RpcError::Protocol(format!("expected a challenge, got a {}-byte frame", message.len()))
    })?;
    if body.len() < MD5ONLY_MESSAGE_LENGTH {
        return Err(RpcError::Protocol(format!("challenge too short: {} bytes", body.len())));
    }
    let (form, response) = match parse_message(body) {
        Ok(Message::Legacy) => (ChallengeForm::Legacy, mac(Digest::Md5, key, body)?),
        Ok(Message::Prefixed { digest, .. }) => {
            let mut response = digest.prefix();
            response.extend(mac(digest, key, body)?);
            (ChallengeForm::Modern, response)
        }
        Err(PrefixError::Missing) => {
            return Err(RpcError::Protocol(format!(
                "challenge of {} bytes carries no digest prefix",
                body.len()
            )))
        }
        Err(PrefixError::Unsupported(name)) => return Err(RpcError::UnsupportedDigest(name)),
    };
    send_frame(stream, &response).await?;
    let welcome = recv_frame(stream, AUTH_FRAME_MAX_LEN).await?;
    if welcome != WELCOME {
        return Err(RpcError::Authentication);
    }
    Ok(form)
}

/// `deliver_challenge` (`connection.py:970-988`) with `_verify_challenge`
/// (`:945-967`), in the form the server's own challenge used: a legacy server
/// gets a bare 20-byte challenge and must answer with a raw HMAC-MD5; a
/// modern one gets `{sha256}` + 40 bytes and must answer `{sha256}` + the
/// HMAC-SHA256 over the whole body, prefix included. Python would also
/// accept a response in a different allowed digest; this client holds the
/// server to the digest it was challenged with. A response that does not
/// verify is answered with `#FAILURE#` and is [`RpcError::Authentication`].
pub(super) async fn deliver_challenge<S>(
    stream: &mut S,
    key: &[u8],
    form: ChallengeForm,
) -> Result<(), RpcError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (digest, message) = match form {
        ChallengeForm::Legacy => {
            let mut message = vec![0u8; MD5ONLY_MESSAGE_LENGTH];
            OsRng.fill_bytes(&mut message);
            (Digest::Md5, message)
        }
        ChallengeForm::Modern => {
            let mut random = [0u8; MESSAGE_LENGTH];
            OsRng.fill_bytes(&mut random);
            let mut message = Digest::Sha256.prefix();
            message.extend_from_slice(&random);
            (Digest::Sha256, message)
        }
    };
    let mut frame = CHALLENGE.to_vec();
    frame.extend_from_slice(&message);
    send_frame(stream, &frame).await?;
    let response = recv_frame(stream, AUTH_FRAME_MAX_LEN).await?;
    let verified = match (form, parse_message(&response)) {
        (ChallengeForm::Legacy, Ok(Message::Legacy)) => {
            verify_mac(Digest::Md5, key, &message, &response)?
        }
        (ChallengeForm::Modern, Ok(Message::Prefixed { digest: answered, payload }))
            if answered == digest =>
        {
            verify_mac(digest, key, &message, payload)?
        }
        _ => false,
    };
    if !verified {
        send_frame(stream, FAILURE).await?;
        return Err(RpcError::Authentication);
    }
    send_frame(stream, WELCOME).await
}
