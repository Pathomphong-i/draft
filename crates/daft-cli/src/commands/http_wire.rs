//! Pure-Rust HTTP/1.1 Wire Protocol & Transport for Draft ('dft') Remote Sync & Server.

use crate::error::CliError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WireObject {
    pub oid: String,
    pub object_type: String,
    pub data_hex: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HasObjectsRequest {
    pub oids: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HasObjectsResponse {
    pub missing: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FetchPackRequest {
    pub have: Vec<String>,
    #[serde(default)]
    pub wants: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FetchPackResponse {
    pub head: String,
    pub default_branch: String,
    pub refs: BTreeMap<String, String>,
    pub objects: Vec<WireObject>,
    #[serde(default)]
    pub territory: Option<serde_json::Value>,
    #[serde(default)]
    pub agents: Vec<serde_json::Value>,
    #[serde(default)]
    pub entanglements: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PushPackRequest {
    pub branch: String,
    pub commit_id: String,
    #[serde(default)]
    pub refs: BTreeMap<String, String>,
    pub objects: Vec<WireObject>,
    #[serde(default)]
    pub territory: Option<serde_json::Value>,
    #[serde(default)]
    pub agents: Vec<serde_json::Value>,
    #[serde(default)]
    pub entanglements: Vec<serde_json::Value>,
    #[serde(default)]
    pub pusher: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PushPackResponse {
    pub ok: bool,
    pub message: String,
    #[serde(default)]
    pub updated_refs: Vec<String>,
    #[serde(default)]
    pub objects_received: usize,
    #[serde(default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub compare_url: Option<String>,
}

pub fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Normalizes a Draft remote URL into `<scheme>://<host>[:<port>]/<owner>/<repo>` (without trailing `/` or `.dft`).
pub fn normalize_repo_http_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    let trimmed = trimmed
        .strip_suffix(".dft")
        .or_else(|| trimmed.strip_suffix(".git"))
        .unwrap_or(trimmed);
    trimmed.to_string()
}

/// Extracts repository directory name from URL or local path.
pub fn extract_repo_name(raw: &str) -> String {
    let clean = raw
        .trim()
        .trim_end_matches('/')
        .strip_suffix(".dft")
        .or_else(|| raw.trim().trim_end_matches('/').strip_suffix(".git"))
        .unwrap_or_else(|| raw.trim().trim_end_matches('/'));
    clean
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("cloned_repo")
        .to_string()
}

#[derive(Debug, Clone)]
pub struct ParsedHttpUrl {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub userinfo: Option<String>,
}

pub fn parse_http_url(url: &str) -> Result<ParsedHttpUrl, CliError> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("http://") {
        ("http".to_string(), r)
    } else if let Some(r) = url.strip_prefix("https://") {
        ("https".to_string(), r)
    } else {
        return Err(CliError::General(format!("Unsupported URL scheme: {}", url)));
    };

    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], rest[idx..].to_string()),
        None => (rest, "/".to_string()),
    };

    let (userinfo, hostport) = match authority.rfind('@') {
        Some(idx) => (Some(authority[..idx].to_string()), &authority[idx + 1..]),
        None => (None, authority),
    };

    let default_port = if scheme == "https" { 443 } else { 80 };
    let (host, port) = if hostport.starts_with('[') {
        // IPv6 literal
        if let Some(end_bracket) = hostport.find(']') {
            let h = &hostport[1..end_bracket];
            let p = hostport[end_bracket + 1..]
                .strip_prefix(':')
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(default_port);
            (h.to_string(), p)
        } else {
            return Err(CliError::General(format!("Malformed IPv6 host in URL: {}", url)));
        }
    } else if let Some((h, p_str)) = hostport.rsplit_once(':') {
        let p = p_str.parse::<u16>().map_err(|_| {
            CliError::General(format!("Invalid port '{}' in URL '{}'", p_str, url))
        })?;
        (h.to_string(), p)
    } else {
        (hostport.to_string(), default_port)
    };

    Ok(ParsedHttpUrl {
        scheme,
        host,
        port,
        path,
        userinfo,
    })
}

/// Resolves optional CLI auth header from URL credentials, env vars, or local dft.toml.
pub fn resolve_auth_header(parsed: &ParsedHttpUrl, dft_dir: Option<&Path>) -> Option<String> {
    if let Some(ref ui) = parsed.userinfo {
        if !ui.contains(':') && ui.starts_with("dft_pat_") {
            return Some(format!("Bearer {}", ui));
        }
        return Some(format!("Basic {}", base64_encode(ui.as_bytes())));
    }
    if let Ok(tok) = env::var("DFT_TOKEN").or_else(|_| env::var("DRAFT_TOKEN")) {
        if !tok.trim().is_empty() {
            return Some(format!("Bearer {}", tok.trim()));
        }
    }
    if let Some(dir) = dft_dir {
        if let Ok(cfg) = std::fs::read_to_string(dir.join("dft.toml")) {
            if let Ok(val) = toml::from_str::<toml::Value>(&cfg) {
                if let Some(tok) = val
                    .get("credential")
                    .and_then(|c| c.get("token"))
                    .and_then(|t| t.as_str())
                {
                    if !tok.trim().is_empty() {
                        return Some(format!("Bearer {}", tok.trim()));
                    }
                }
            }
        }
    }
    None
}

pub fn http_post_json<Req: Serialize, Resp: DeserializeOwned>(
    url: &str,
    payload: &Req,
    dft_dir: Option<&Path>,
) -> Result<Resp, CliError> {
    let body_bytes = serde_json::to_vec(payload)
        .map_err(|e| CliError::General(format!("Failed to serialize wire request: {}", e)))?;
    let parsed = parse_http_url(url)?;
    let auth = resolve_auth_header(&parsed, dft_dir);

    let (status, resp_bytes) = if parsed.scheme == "https" {
        curl_request("POST", url, auth.as_deref(), Some(&body_bytes))?
    } else {
        tcp_http_request(&parsed, "POST", auth.as_deref(), Some(&body_bytes))?
    };

    if !(200..300).contains(&status) {
        let msg = String::from_utf8_lossy(&resp_bytes);
        return Err(CliError::General(format!(
            "Remote HTTP {} error from {}: {}",
            status,
            url,
            msg.trim()
        )));
    }

    serde_json::from_slice(&resp_bytes).map_err(|e| {
        CliError::General(format!(
            "Invalid JSON response from {} ({}): {}",
            url,
            e,
            String::from_utf8_lossy(&resp_bytes[..resp_bytes.len().min(200)])
        ))
    })
}

fn curl_request(
    method: &str,
    url: &str,
    auth: Option<&str>,
    body: Option<&[u8]>,
) -> Result<(u16, Vec<u8>), CliError> {
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "-X", method, "-H", "Content-Type: application/json", "-w", "\n__DFT_HTTP_CODE__:%{http_code}"]);
    if let Some(a) = auth {
        cmd.args(["-H", &format!("Authorization: {}", a)]);
    }
    let tmp_file = if let Some(b) = body {
        let path = env::temp_dir().join(format!("dft-req-{}-{}.json", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        std::fs::write(&path, b)?;
        cmd.args(["--data-binary", &format!("@{}", path.display())]);
        Some(path)
    } else {
        None
    };
    cmd.arg(url);

    let out = cmd.output().map_err(|e| {
        CliError::General(format!("Failed to invoke curl for HTTPS transport: {}", e))
    })?;
    if let Some(p) = tmp_file {
        let _ = std::fs::remove_file(p);
    }
    if !out.status.success() {
        return Err(CliError::General(format!(
            "HTTPS transport failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }

    let marker = b"\n__DFT_HTTP_CODE__:";
    if let Some(pos) = out
        .stdout
        .windows(marker.len())
        .rposition(|w| w == marker)
    {
        let code_str = String::from_utf8_lossy(&out.stdout[pos + marker.len()..]);
        let status = code_str.trim().parse::<u16>().unwrap_or(500);
        Ok((status, out.stdout[..pos].to_vec()))
    } else {
        Ok((200, out.stdout))
    }
}

fn tcp_http_request(
    parsed: &ParsedHttpUrl,
    method: &str,
    auth: Option<&str>,
    body: Option<&[u8]>,
) -> Result<(u16, Vec<u8>), CliError> {
    let addr_str = format!("{}:{}", parsed.host, parsed.port);
    let mut addrs = addr_str.to_socket_addrs().map_err(|e| {
        CliError::General(format!("Could not resolve remote host '{}': {}", addr_str, e))
    })?;
    let sock_addr = addrs.next().ok_or_else(|| {
        CliError::General(format!("No socket addresses resolved for '{}'", addr_str))
    })?;

    let mut stream = TcpStream::connect_timeout(&sock_addr, Duration::from_secs(10)).map_err(|e| {
        CliError::General(format!("Failed to connect to {}: {}", addr_str, e))
    })?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(120)));

    let payload = body.unwrap_or(&[]);
    let host_hdr = if parsed.port == 80 {
        parsed.host.clone()
    } else {
        format!("{}:{}", parsed.host, parsed.port)
    };

    let mut req_head = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: dft/{}\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        method,
        parsed.path,
        host_hdr,
        env!("CARGO_PKG_VERSION"),
        payload.len()
    );
    if let Some(a) = auth {
        req_head.push_str(&format!("Authorization: {}\r\n", a));
    }
    req_head.push_str("\r\n");

    stream
        .write_all(req_head.as_bytes())
        .and_then(|_| {
            if !payload.is_empty() {
                stream.write_all(payload)
            } else {
                Ok(())
            }
        })
        .and_then(|_| stream.flush())
        .map_err(|e| CliError::General(format!("Failed sending HTTP request: {}", e)))?;

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .map_err(|e| CliError::General(format!("Failed reading HTTP status line: {}", e)))?;

    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| {
            CliError::General(format!("Malformed HTTP status line: '{}'", status_line.trim()))
        })?;

    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| CliError::General(format!("Failed reading HTTP headers: {}", e)))?;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    let is_chunked = headers
        .get("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);

    let resp_body = if is_chunked {
        read_chunked_body(&mut reader)?
    } else if let Some(cl_str) = headers.get("content-length") {
        let len = cl_str.parse::<usize>().unwrap_or(0);
        let mut buf = vec![0u8; len];
        reader
            .read_exact(&mut buf)
            .map_err(|e| CliError::General(format!("Incomplete HTTP response body: {}", e)))?;
        buf
    } else {
        let mut buf = Vec::new();
        reader
            .read_to_end(&mut buf)
            .map_err(|e| CliError::General(format!("Failed reading HTTP body: {}", e)))?;
        buf
    };

    Ok((status_code, resp_body))
}

fn read_chunked_body<R: BufRead>(reader: &mut R) -> Result<Vec<u8>, CliError> {
    let mut out = Vec::new();
    loop {
        let mut size_line = String::new();
        reader
            .read_line(&mut size_line)
            .map_err(|e| CliError::General(format!("Chunk header read error: {}", e)))?;
        let hex_part = size_line
            .trim()
            .split(';')
            .next()
            .unwrap_or("0")
            .trim();
        let chunk_size = usize::from_str_radix(hex_part, 16).unwrap_or(0);
        if chunk_size == 0 {
            let mut trailer = String::new();
            let _ = reader.read_line(&mut trailer);
            break;
        }
        let mut chunk = vec![0u8; chunk_size];
        reader
            .read_exact(&mut chunk)
            .map_err(|e| CliError::General(format!("Chunk payload read error: {}", e)))?;
        out.extend_from_slice(&chunk);
        let mut crlf = [0u8; 2];
        let _ = reader.read_exact(&mut crlf);
    }
    Ok(out)
}

// ============================================================================
// Server-Side HTTP/1.1 Request Reader (supports large bodies up to 512 MiB)
// ============================================================================

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn query_param(&self, key: &str) -> Option<String> {
        for pair in self.query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let mut kv = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                if url_decode(k) == key {
                    return Some(url_decode(v));
                }
            }
        }
        None
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        let raw = self.headers.get("cookie")?;
        for part in raw.split(';') {
            let trimmed = part.trim();
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == name {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }

    pub fn json<T: DeserializeOwned>(&self) -> Result<T, String> {
        serde_json::from_slice(&self.body).map_err(|e| format!("Invalid JSON body: {}", e))
    }
}

pub fn read_http_request(stream: &TcpStream) -> Option<HttpRequest> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let mut reader = BufReader::new(stream);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).ok()? == 0 {
        return None;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_uppercase();
    let raw_uri = parts.next().unwrap_or("/");

    let (raw_path, query) = match raw_uri.split_once('?') {
        Some((p, q)) => (p, q.to_string()),
        None => (raw_uri, String::new()),
    };
    let path = url_decode(raw_path);

    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    const MAX_BODY_BYTES: usize = 512 * 1024 * 1024; // 512 MiB
    let is_chunked = headers
        .get("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);

    let body = if is_chunked {
        read_chunked_body(&mut reader).ok()?
    } else if let Some(cl_str) = headers.get("content-length") {
        let len = cl_str.parse::<usize>().ok()?;
        if len > MAX_BODY_BYTES {
            return None;
        }
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).ok()?;
        buf
    } else {
        Vec::new()
    };

    Some(HttpRequest {
        method,
        path,
        query,
        headers,
        body,
    })
}

pub fn url_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let raw = input.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            if let Ok(hex_str) = std::str::from_utf8(&raw[i + 1..i + 3]) {
                if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                    bytes.push(val);
                    i += 3;
                    continue;
                }
            }
        } else if raw[i] == b'+' {
            bytes.push(b' ');
            i += 1;
            continue;
        }
        bytes.push(raw[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).to_string()
}

const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if clean.len() % 4 != 0 {
        return None;
    }
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' => Some(0),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let v0 = val(chunk[0])?;
        let v1 = val(chunk[1])?;
        let v2 = val(chunk[2])?;
        let v3 = val(chunk[3])?;
        let triple = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
        out.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }
    }
    Some(out)
}
