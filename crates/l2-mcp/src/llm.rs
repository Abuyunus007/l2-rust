//! The language model: an OpenAI-compatible chat endpoint (`POST /v1/chat/completions`) such as
//! llama.cpp's `llama-server` or Ollama, on this machine.
//!
//! The HTTP is written by hand (HTTP/1.1, no TLS): the model runs locally, and a client library
//! would be the only new dependency of the project for one request type (D-28).

use std::future::Future;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// How long a model may think about one answer. A small model on a GPU needs seconds; on a CPU a
/// long answer can take minutes.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(600);

/// A chat model. A trait so the tests can stand in for it.
pub trait Llm: Send + Sync {
    /// One chat completion: `request` is the OpenAI request body, the answer its response body.
    fn complete(&self, request: Value) -> impl Future<Output = Result<Value, String>> + Send;
}

/// An OpenAI-compatible endpoint at `http://host:port` (a path in the URL is put in front of
/// `/v1/...`).
pub struct HttpLlm {
    host: String,
    port: u16,
    /// The path before `/v1`, empty or like `/llm`.
    base: String,
}

impl HttpLlm {
    /// `url` like `http://127.0.0.1:8080`. Only `http`: the model runs on this machine or the
    /// local network.
    pub fn new(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| format!("the model URL must start with http:// ({url})"))?;
        let (authority, base) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => (
                host,
                port.parse::<u16>()
                    .map_err(|_| format!("bad port in the model URL ({url})"))?,
            ),
            None => (authority, 80),
        };
        if host.is_empty() {
            return Err(format!("no host in the model URL ({url})"));
        }
        let base = base.trim_end_matches('/');
        let base = if base.is_empty() {
            String::new()
        } else {
            format!("/{base}")
        };
        Ok(Self {
            host: host.to_owned(),
            port,
            base,
        })
    }

    /// The models the endpoint offers (`GET /v1/models`), by name.
    pub async fn models(&self) -> Result<Vec<String>, String> {
        let answer = self.call("GET", "/v1/models", &[]).await?;
        Ok(answer
            .get("data")
            .and_then(Value::as_array)
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| m.get("id").and_then(Value::as_str).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// One request and its JSON answer.
    async fn call(&self, method: &str, path: &str, body: &[u8]) -> Result<Value, String> {
        let raw = tokio::time::timeout(ANSWER_TIMEOUT, self.send(method, path, body))
            .await
            .map_err(|_| "the model did not answer in time".to_owned())??;
        let (status, body) = parse_response(&raw)?;
        let text = String::from_utf8_lossy(&body);
        if !(200..300).contains(&status) {
            return Err(format!("the model answered HTTP {status}: {}", text.trim()));
        }
        serde_json::from_str(&text).map_err(|e| format!("the model's answer is not JSON: {e}"))
    }

    async fn send(&self, method: &str, path: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        let mut stream = TcpStream::connect((self.host.as_str(), self.port))
            .await
            .map_err(|e| format!("cannot reach the model at {}:{}: {e}", self.host, self.port))?;
        let head = format!(
            "{method} {}{path} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\n\
             Accept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.base,
            self.host,
            self.port,
            body.len()
        );
        stream
            .write_all(head.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stream.write_all(body).await.map_err(|e| e.to_string())?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .map_err(|e| format!("the model's answer broke off: {e}"))?;
        Ok(response)
    }
}

impl Llm for HttpLlm {
    async fn complete(&self, request: Value) -> Result<Value, String> {
        let body = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        self.call("POST", "/v1/chat/completions", &body).await
    }
}

/// The status and the body of a raw HTTP/1.1 response (a `Content-Length` body, a chunked one,
/// or one that ends with the connection).
pub fn parse_response(raw: &[u8]) -> Result<(u16, Vec<u8>), String> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("the model's answer has no HTTP header")?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let body = &raw[split + 4..];
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or("the model's answer has no HTTP status")?;
    let header = |name: &str| {
        head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim()
                .eq_ignore_ascii_case(name)
                .then(|| value.trim().to_owned())
        })
    };
    let chunked = header("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
    if chunked {
        return Ok((status, dechunk(body)?));
    }
    let body = match header("content-length").and_then(|v| v.parse::<usize>().ok()) {
        Some(length) => body
            .get(..length)
            .ok_or("the model's answer is cut short")?
            .to_vec(),
        None => body.to_vec(),
    };
    Ok((status, body))
}

/// A `Transfer-Encoding: chunked` body put together.
fn dechunk(mut body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let line_end = body
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("a chunk of the model's answer has no size")?;
        let size_text = String::from_utf8_lossy(&body[..line_end]);
        let size_text = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|_| "a chunk size is not a number")?;
        body = &body[line_end + 2..];
        if size == 0 {
            return Ok(out);
        }
        out.extend_from_slice(
            body.get(..size)
                .ok_or("a chunk of the model's answer is cut short")?,
        );
        body = body.get(size + 2..).unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn the_url_gives_host_port_and_path() {
        let llm = HttpLlm::new("http://127.0.0.1:8080").unwrap();
        assert_eq!(
            (llm.host.as_str(), llm.port, llm.base.as_str()),
            ("127.0.0.1", 8080, "")
        );
        let llm = HttpLlm::new("http://localhost:11434/").unwrap();
        assert_eq!((llm.port, llm.base.as_str()), (11434, ""));
        let llm = HttpLlm::new("http://box/llm/").unwrap();
        assert_eq!((llm.port, llm.base.as_str()), (80, "/llm"));
        assert!(HttpLlm::new("https://x:1").is_err());
        assert!(HttpLlm::new("http://:1").is_err());
        assert!(HttpLlm::new("http://x:port").is_err());
    }

    #[test]
    fn a_body_with_a_length_a_chunked_one_and_one_to_the_end() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}junk";
        assert_eq!(parse_response(raw).unwrap(), (200, b"{}".to_vec()));
        let raw =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\n{\"a\r\n4;x=1\r\n\":1}\r\n0\r\n\r\n";
        assert_eq!(parse_response(raw).unwrap(), (200, b"{\"a\":1}".to_vec()));
        let raw = b"HTTP/1.0 500 Internal Server Error\r\n\r\nboom";
        assert_eq!(parse_response(raw).unwrap(), (500, b"boom".to_vec()));
        assert!(parse_response(b"garbage").is_err());
        assert!(parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n{}").is_err());
    }
}
