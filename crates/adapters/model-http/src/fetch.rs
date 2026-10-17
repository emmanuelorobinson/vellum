//! Plain HTTPS GET for [`HttpFetch`] (stock image search and download). Same rules as the model transport, and
//! stricter about where it connects: no IP literals, no names that resolve to this computer or a private
//! network (checked on the addresses the connection will actually use), no redirects, bounded size and time.
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use vellum_core::ports::{FetchError, FetchRequest, FetchResponse, HttpFetch};

pub struct HttpFetcher {
    timeout: Duration,
    /// For tests and a local mirror: plain http and loopback addresses are allowed.
    loopback: bool,
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            loopback: false,
        }
    }
}

impl HttpFetcher {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            ..Self::default()
        }
    }

    /// Allows `http://` and loopback addresses. Only for tests and a mirror the person runs themselves.
    #[must_use]
    pub fn allowing_loopback(mut self) -> Self {
        self.loopback = true;
        self
    }
}

/// Whether `ip` is somewhere the open internet is: not this computer, not a private or link-local network.
fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || o[0] == 0
                || (o[0] == 100 && (64..128).contains(&o[1])) // carrier-grade NAT
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public(IpAddr::V4(v4));
            }
            let s = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // unique local
                || (s[0] & 0xffc0) == 0xfe80 // link local
                || (s[0] == 0x64 && s[1] == 0xff9b)) // NAT64
        }
    }
}

struct Target {
    url: url::Url,
    host: String,
    port: u16,
}

fn check(request: &FetchRequest, loopback: bool) -> Result<Target, FetchError> {
    let text = match &request.secret {
        Some(secret) => request.url.replace("{secret}", &encode(secret.expose())),
        None => request.url.clone(),
    };
    let url = url::Url::parse(&text).map_err(|_| FetchError::Blocked)?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    let scheme_ok = url.scheme() == "https" || (loopback && local && url.scheme() == "http");
    if !scheme_ok
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(FetchError::Blocked);
    }
    let host = match url.host() {
        Some(url::Host::Domain(d)) => {
            let d = d.to_ascii_lowercase();
            if !loopback
                && (d == "localhost"
                    || d.ends_with(".localhost")
                    || d.ends_with(".local")
                    || d.ends_with(".internal"))
            {
                return Err(FetchError::Blocked);
            }
            d
        }
        Some(url::Host::Ipv4(ip)) if loopback && ip.is_loopback() => ip.to_string(),
        _ => return Err(FetchError::Blocked),
    };
    let port = url.port_or_known_default().ok_or(FetchError::Blocked)?;
    Ok(Target { url, host, port })
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn runtime() -> Result<&'static tokio::runtime::Runtime, FetchError> {
    static RUNTIME: std::sync::OnceLock<Result<tokio::runtime::Runtime, ()>> =
        std::sync::OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .map_err(|_| ())
        })
        .as_ref()
        .map_err(|_| FetchError::Network)
}

async fn run(
    target: Target,
    max_bytes: usize,
    timeout: Duration,
    loopback: bool,
) -> Result<FetchResponse, FetchError> {
    // Resolve here and connect to exactly the addresses that passed, so a name cannot answer differently later.
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((target.host.as_str(), target.port))
        .await
        .map_err(|_| FetchError::Network)?
        .collect();
    if addrs.is_empty() || (!loopback && !addrs.iter().all(|a| public(a.ip()))) {
        return Err(FetchError::Blocked);
    }
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(&target.host, &addrs)
        .build()
        .map_err(|_| FetchError::Network)?;
    let mut response = client.get(target.url).send().await.map_err(|e| {
        if e.is_timeout() {
            FetchError::Timeout
        } else {
            FetchError::Network
        }
    })?;
    if response
        .content_length()
        .is_some_and(|n| n > max_bytes as u64)
    {
        return Err(FetchError::TooLarge);
    }
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        if e.is_timeout() {
            FetchError::Timeout
        } else {
            FetchError::Network
        }
    })? {
        if body.len() + chunk.len() > max_bytes {
            return Err(FetchError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(FetchResponse {
        status,
        content_type,
        body,
    })
}

impl HttpFetch for HttpFetcher {
    fn get(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        let target = check(request, self.loopback)?;
        let (sender, receiver) = std::sync::mpsc::channel();
        let (max, timeout, loopback) = (request.max_bytes, self.timeout, self.loopback);
        runtime()?.spawn(async move {
            let _ = sender.send(run(target, max, timeout, loopback).await);
        });
        // The task has its own timeout; this one only guards a runtime that has stopped.
        receiver
            .recv_timeout(self.timeout + Duration::from_secs(15))
            .map_err(|_| FetchError::Timeout)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_open_internet_is_public() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "224.0.0.1",
            "::1",
            "fe80::1",
            "fc00::1",
            "::ffff:10.0.0.1",
            "64:ff9b::1",
        ] {
            assert!(!public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "104.18.2.3", "2606:4700::1"] {
            assert!(public(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn the_secret_is_substituted_encoded_and_only_where_marked() {
        let request = FetchRequest {
            url: "https://example.com/api?key={secret}&q=%7Bsecret%7D".into(),
            secret: Some(vellum_core::ports::SecretString::new("a b&c")),
            max_bytes: 1,
        };
        let target = check(&request, false).unwrap();
        assert_eq!(
            target.url.as_str(),
            "https://example.com/api?key=a%20b%26c&q=%7Bsecret%7D"
        );
        assert_eq!((target.host.as_str(), target.port), ("example.com", 443));
    }
}
