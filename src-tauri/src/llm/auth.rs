//! Connect OpenRouter in one click (design-ux U1, Decision D-39): OAuth PKCE
//! through a loopback callback. The browser asks the person, OpenRouter
//! redirects to `http://localhost:<port>/callback?code=..`, and the code is
//! swapped for a key the person controls. The verifier and the key stay in
//! Rust and are never logged.
//!
//! Checked against <https://openrouter.ai/docs/guides/overview/auth/oauth>
//! on 2026-11-08: `https://openrouter.ai/auth` takes `callback_url`,
//! `code_challenge`, `code_challenge_method` (`S256`) and `state`; localhost
//! callbacks work on any port; `POST /api/v1/auth/keys` takes `code`,
//! `code_verifier`, `code_challenge_method` and answers `{key}`.

use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;
use url::Url;

use super::client::Transport;
use super::wire::{HttpRequest, Method};

const AUTH_URL: &str = "https://openrouter.ai/auth";
const KEYS_URL: &str = "https://openrouter.ai/api/v1/auth/keys";

/// How long the person has to approve in the browser.
pub const CONNECT_WAIT: Duration = Duration::from_secs(5 * 60);

/// A local connection gets this long to send its request line.
const REQUEST_WAIT: Duration = Duration::from_secs(3);
const EXCHANGE_WAIT: Duration = Duration::from_secs(20);
const MAX_REQUEST: usize = 8 * 1024;

const CONNECTED_PAGE: &str = "Aevaleron is connected. You can close this tab.";
const FAILED_PAGE: &str = "Aevaleron couldn't connect. Go back to it and try again.";

const TIMED_OUT: &str = "Didn't hear back from OpenRouter. Try again, or paste a key instead.";
const FAILED: &str = "OpenRouter didn't finish connecting. Try again, or paste a key instead.";
const NO_BROWSER: &str = "Couldn't open your browser. Paste a key instead.";

/// Why a connect ended without a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectError {
    TimedOut,
    Failed,
    NoBrowser,
    /// Another connect replaced this one.
    Cancelled,
}

impl ConnectError {
    /// The sentence the person reads.
    pub fn text(self) -> &'static str {
        match self {
            ConnectError::TimedOut => TIMED_OUT,
            ConnectError::Failed | ConnectError::Cancelled => FAILED,
            ConnectError::NoBrowser => NO_BROWSER,
        }
    }
}

/// 32 random bytes, base64url: a PKCE verifier (43 characters), or a `state`.
fn secret() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// PKCE S256: `base64url(sha256(verifier))`.
fn challenge_of(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn authorize_url(port: u16, challenge: &str, state: &str) -> String {
    let callback = format!("http://localhost:{port}/callback");
    let params = [
        ("callback_url", callback.as_str()),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
    ];
    Url::parse_with_params(AUTH_URL, params)
        .map(String::from)
        .unwrap_or_default()
}

/// The `code` in a request line, when it is the callback for this `state`.
/// A local page can reach the port, so anything else is turned away and the
/// listener keeps waiting for the real one.
fn code_in(request_line: &str, state: &str) -> Option<String> {
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("GET") {
        return None;
    }
    let url = Url::parse(&format!("http://localhost{}", parts.next()?)).ok()?;
    if url.path() != "/callback" {
        return None;
    }
    let field = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    if field("state").as_deref() != Some(state) {
        return None;
    }
    field("code").filter(|c| !c.trim().is_empty())
}

async fn request_line(stream: &mut TcpStream) -> Option<String> {
    let mut seen = Vec::new();
    let mut chunk = [0u8; 1024];
    while !seen.contains(&b'\n') && seen.len() < MAX_REQUEST {
        let n = tokio::time::timeout(REQUEST_WAIT, stream.read(&mut chunk))
            .await
            .ok()?
            .ok()?;
        if n == 0 {
            break;
        }
        seen.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8_lossy(&seen);
    text.lines().next().map(str::to_string)
}

async fn answer(stream: &mut TcpStream, status: &str, page: &str) {
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    let _ = stream.write_all(reply.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// `POST /api/v1/auth/keys`: the code for a key.
async fn exchange(
    transport: &dyn Transport,
    code: &str,
    verifier: &str,
    cancel: &CancellationToken,
) -> Option<String> {
    let req = HttpRequest {
        method: Method::Post,
        url: KEYS_URL.to_string(),
        headers: vec![("Content-Type".into(), "application/json".into())],
        body: Some(json!({
            "code": code,
            "code_verifier": verifier,
            "code_challenge_method": "S256",
        })),
        timeout: Some(EXCHANGE_WAIT),
    };
    let reply = transport.send(req, cancel).await.ok()?;
    if !reply.ok() {
        return None;
    }
    let bytes = reply.bytes(cancel).await.ok()?;
    let body: Value = serde_json::from_slice(&bytes).ok()?;
    let key = body["key"].as_str()?.trim();
    (!key.is_empty()).then(|| key.to_string())
}

/// Waits for the callback, swaps its code for a key and answers the tab.
async fn serve(
    listener: TcpListener,
    state: &str,
    verifier: &str,
    transport: &dyn Transport,
    cancel: &CancellationToken,
) -> Result<String, ConnectError> {
    loop {
        let accepted = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(ConnectError::Cancelled),
            a = listener.accept() => a,
        };
        let Ok((mut stream, _)) = accepted else {
            continue;
        };
        let line = request_line(&mut stream).await;
        let Some(code) = line.and_then(|l| code_in(&l, state)) else {
            answer(&mut stream, "404 Not Found", "Not found.").await;
            continue;
        };
        return match exchange(transport, &code, verifier, cancel).await {
            Some(key) => {
                answer(&mut stream, "200 OK", CONNECTED_PAGE).await;
                Ok(key)
            }
            None => {
                answer(&mut stream, "502 Bad Gateway", FAILED_PAGE).await;
                Err(ConnectError::Failed)
            }
        };
    }
}

/// The whole flow. `open` shows the approval page in the default browser;
/// the result is a key to store.
pub async fn connect(
    transport: &dyn Transport,
    open: impl FnOnce(&str) -> bool,
    cancel: &CancellationToken,
    wait: Duration,
) -> Result<String, ConnectError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|_| ConnectError::Failed)?;
    let port = listener
        .local_addr()
        .map_err(|_| ConnectError::Failed)?
        .port();
    let verifier = secret();
    let state = secret();
    let url = authorize_url(port, &challenge_of(&verifier), &state);
    if !open(&url) {
        return Err(ConnectError::NoBrowser);
    }
    let served = serve(listener, &state, &verifier, transport, cancel);
    match tokio::time::timeout(wait, served).await {
        Ok(result) => result,
        Err(_) => Err(ConnectError::TimedOut),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::client::Reply;
    use crate::llm::errors::ApiError;
    use async_trait::async_trait;
    use parking_lot::Mutex;
    use std::sync::Arc;

    #[derive(Default)]
    struct TokenEndpoint {
        sent: Mutex<Vec<HttpRequest>>,
    }

    #[async_trait]
    impl Transport for TokenEndpoint {
        async fn send(&self, req: HttpRequest, _: &CancellationToken) -> Result<Reply, ApiError> {
            self.sent.lock().push(req);
            Ok(Reply::full(200, r#"{"key":"sk-or-v1-fake"}"#))
        }
    }

    #[test]
    fn pkce_challenge_is_s256_of_verifier() {
        // RFC 7636, appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let want = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(challenge_of(verifier), want);
        let made = secret();
        assert_eq!(made.len(), 43);
        assert_ne!(made, secret());
    }

    #[test]
    fn the_approval_url_has_the_documented_parameters() {
        let url = Url::parse(&authorize_url(51423, "chal", "st")).unwrap();
        assert_eq!(url.host_str(), Some("openrouter.ai"));
        assert_eq!(url.path(), "/auth");
        let field = |name: &str| {
            url.query_pairs()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.into_owned())
        };
        let callback = field("callback_url");
        assert_eq!(callback.as_deref(), Some("http://localhost:51423/callback"));
        assert_eq!(field("code_challenge").as_deref(), Some("chal"));
        assert_eq!(field("code_challenge_method").as_deref(), Some("S256"));
        assert_eq!(field("state").as_deref(), Some("st"));
    }

    async fn browser(port: u16, target: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let req = format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n");
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    #[tokio::test]
    async fn callback_parses_code_and_rejects_bad_requests() {
        assert_eq!(
            code_in("GET /callback?code=abc&state=s HTTP/1.1", "s").as_deref(),
            Some("abc")
        );
        for bad in [
            "POST /callback?code=abc&state=s HTTP/1.1",
            "GET /favicon.ico HTTP/1.1",
            "GET /callback?code=abc&state=other HTTP/1.1",
            "GET /callback?code=abc HTTP/1.1",
            "GET /callback?state=s HTTP/1.1",
            "GET /callback?code=&state=s HTTP/1.1",
            "",
        ] {
            assert_eq!(code_in(bad, "s"), None, "{bad}");
        }

        // The round trip: strangers are turned away, the callback gets a key.
        let endpoint = Arc::new(TokenEndpoint::default());
        let opened = Arc::new(Mutex::new(String::new()));
        let cancel = CancellationToken::new();
        let wait = Duration::from_secs(20);
        let flow = {
            let (endpoint, opened, cancel) = (endpoint.clone(), opened.clone(), cancel.clone());
            tokio::spawn(async move {
                let open = |url: &str| {
                    *opened.lock() = url.to_string();
                    true
                };
                connect(&*endpoint, open, &cancel, wait).await
            })
        };
        while opened.lock().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let url = Url::parse(&opened.lock().clone()).unwrap();
        let field = |name: &str| {
            url.query_pairs()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.into_owned())
                .unwrap()
        };
        let callback = Url::parse(&field("callback_url")).unwrap();
        let port = callback.port().unwrap();
        let state = field("state");

        assert!(browser(port, "/favicon.ico").await.contains("404"));
        let wrong = browser(port, "/callback?code=evil&state=nope").await;
        assert!(wrong.contains("404"));
        assert!(endpoint.sent.lock().is_empty(), "a stranger gets no key");

        let good = format!("/callback?code=the-code&state={state}");
        let page = browser(port, &good).await;
        assert!(page.starts_with("HTTP/1.1 200"));
        assert!(page.ends_with(CONNECTED_PAGE));
        assert_eq!(flow.await.unwrap().unwrap(), "sk-or-v1-fake");

        let sent = endpoint.sent.lock();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].url, KEYS_URL);
        let body = sent[0].body.as_ref().unwrap();
        assert_eq!(body["code"], "the-code");
        assert_eq!(body["code_challenge_method"], "S256");
        // The verifier sent is the one the challenge in the URL was made of.
        let verifier = body["code_verifier"].as_str().unwrap();
        assert_eq!(challenge_of(verifier), field("code_challenge"));
    }

    #[tokio::test]
    async fn no_answer_times_out_with_the_paste_hint() {
        let endpoint = TokenEndpoint::default();
        let cancel = CancellationToken::new();
        let wait = Duration::from_millis(30);
        let got = connect(&endpoint, |_| true, &cancel, wait).await;
        assert_eq!(got, Err(ConnectError::TimedOut));
        let want = "Didn't hear back from OpenRouter. Try again, or paste a key instead.";
        assert_eq!(ConnectError::TimedOut.text(), want);
        let closed = connect(&endpoint, |_| false, &cancel, wait).await;
        assert_eq!(closed, Err(ConnectError::NoBrowser));
    }
}
