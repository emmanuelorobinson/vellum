//! Device authorization over HTTPS. All errors are fixed text; no vendor errors, codes or tokens are logged.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use vellum_core::ports::SecretString;

#[derive(Clone)]
pub struct AccountUrl(url::Url);
impl AccountUrl {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let url = url::Url::parse(text).map_err(|_| "Invalid account URL")?;
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if (url.scheme() != "https" && !(url.scheme() == "http" && local))
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err("Use an HTTPS account origin (HTTP is allowed only on loopback)");
        }
        Ok(Self(url))
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    pub fn verification_url(&self, code: &str) -> String {
        let mut url = self.0.join("device").expect("fixed relative URL");
        url.query_pairs_mut().append_pair("user_code", code);
        url.to_string()
    }
}
#[derive(Clone, Copy)]
pub enum PollStatus {
    Pending,
    SlowDown,
    Denied,
    Expired,
}
pub struct PollState {
    deadline: u64,
    next: u64,
    interval: u64,
}
impl PollState {
    pub fn new(now: u64, expires: u64, interval: u64) -> Result<Self, &'static str> {
        if expires == 0 || expires > 600 || !(1..=60).contains(&interval) {
            return Err("Invalid device response");
        }
        let interval = interval * 1000;
        Ok(Self {
            deadline: now.saturating_add(expires * 1000),
            next: now.saturating_add(interval),
            interval,
        })
    }
    pub fn wait_ms(&self, now: u64) -> Result<u64, &'static str> {
        if now >= self.deadline || self.next >= self.deadline {
            return Err("Device code expired; run login again");
        }
        Ok(self.next.saturating_sub(now))
    }
    pub fn update(&mut self, status: PollStatus, now: u64) -> Result<(), &'static str> {
        match status {
            PollStatus::Pending => {}
            PollStatus::SlowDown => self.interval = self.interval.saturating_add(5000),
            PollStatus::Denied => return Err("Device request denied"),
            PollStatus::Expired => return Err("Device code expired; run login again"),
        }
        self.next = now.saturating_add(self.interval);
        Ok(())
    }
}
pub struct StoredSession {
    pub origin: AccountUrl,
    pub token: SecretString,
}
impl StoredSession {
    pub fn new(origin: AccountUrl, token: SecretString) -> Result<Self, &'static str> {
        if token.expose().is_empty()
            || token.expose().len() > 4096
            || !token.expose().bytes().all(|b| b.is_ascii_graphic())
        {
            return Err("Invalid session response");
        }
        Ok(Self { origin, token })
    }
    pub fn encode(&self) -> Result<SecretString, &'static str> {
        serde_json::to_string(&json!({"origin":self.origin.as_str(),"token":self.token.expose()}))
            .map(SecretString::new)
            .map_err(|_| "Session unavailable")
    }
    pub fn decode(value: SecretString) -> Result<Self, &'static str> {
        #[derive(Deserialize)]
        struct Wire {
            origin: String,
            token: String,
        }
        let wire: Wire =
            serde_json::from_str(value.expose()).map_err(|_| "Stored session unavailable")?;
        Self::new(
            AccountUrl::parse(&wire.origin)?,
            SecretString::new(wire.token),
        )
    }
}
pub struct DeviceCode {
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
    device_code: SecretString,
}
pub enum TokenReply {
    Waiting(PollStatus),
    Ready(SecretString),
}
#[derive(Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
}
pub struct AccountClient {
    origin: AccountUrl,
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
}
impl AccountClient {
    pub fn new(origin: AccountUrl) -> Result<Self, &'static str> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Account transport unavailable")?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Account transport unavailable")?;
        Ok(Self {
            origin,
            client,
            runtime,
        })
    }
    fn request(
        &self,
        path: &str,
        body: Option<Value>,
        token: Option<&SecretString>,
    ) -> Result<(u16, Value), &'static str> {
        self.runtime.block_on(async {
            let url = self
                .origin
                .0
                .join(path)
                .map_err(|_| "Account service unavailable")?;
            let mut request = if let Some(body) = body {
                self.client.post(url).json(&body)
            } else {
                self.client.get(url)
            };
            if let Some(token) = token {
                request = request.bearer_auth(token.expose());
            }
            let mut response = request
                .send()
                .await
                .map_err(|_| "Account service unavailable")?;
            let status = response.status().as_u16();
            if response.content_length().is_some_and(|n| n > 64 * 1024) {
                return Err("Invalid account response");
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| "Account service unavailable")?
            {
                if bytes.len() + chunk.len() > 64 * 1024 {
                    return Err("Invalid account response");
                }
                bytes.extend_from_slice(&chunk);
            }
            let value = serde_json::from_slice(&bytes).map_err(|_| "Invalid account response")?;
            Ok((status, value))
        })
    }
    pub fn start(&self) -> Result<DeviceCode, &'static str> {
        let (status, value) = self.request(
            "api/auth/device/code",
            Some(json!({"client_id":"vellum-cli"})),
            None,
        )?;
        if status != 200 {
            return Err("Account service refused device login");
        }
        #[derive(Deserialize)]
        struct Wire {
            device_code: String,
            user_code: String,
            expires_in: u64,
            interval: u64,
        }
        let wire: Wire = serde_json::from_value(value).map_err(|_| "Invalid device response")?;
        if wire.user_code.is_empty()
            || wire.user_code.len() > 191
            || !wire
                .user_code
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("Invalid device response");
        }
        PollState::new(0, wire.expires_in, wire.interval)?;
        let secret =
            StoredSession::new(self.origin.clone(), SecretString::new(wire.device_code))?.token;
        Ok(DeviceCode {
            verification_url: self.origin.verification_url(&wire.user_code),
            user_code: wire.user_code,
            device_code: secret,
            expires_in: wire.expires_in,
            interval: wire.interval,
        })
    }
    pub fn poll(&self, device: &DeviceCode) -> Result<TokenReply, &'static str> {
        let (status,value)=self.request("api/auth/device/token",Some(json!({"client_id":"vellum-cli","device_code":device.device_code.expose(),"grant_type":"urn:ietf:params:oauth:grant-type:device_code"})),None)?;
        if status == 200 {
            let token = value
                .get("access_token")
                .and_then(Value::as_str)
                .ok_or("Invalid session response")?;
            return Ok(TokenReply::Ready(
                StoredSession::new(self.origin.clone(), SecretString::new(token))?.token,
            ));
        }
        let state = match value.get("error").and_then(Value::as_str) {
            Some("authorization_pending") => PollStatus::Pending,
            Some("slow_down") => PollStatus::SlowDown,
            Some("access_denied") => PollStatus::Denied,
            Some("expired_token") => PollStatus::Expired,
            _ => return Err("Account service refused device login"),
        };
        Ok(TokenReply::Waiting(state))
    }
    pub fn me(&self, token: &SecretString) -> Result<User, &'static str> {
        let (status, value) = self.request("v1/me", None, Some(token))?;
        if status != 200 {
            return Err("Session expired or revoked; run login again");
        }
        let field = |name| -> Result<String, &'static str> {
            let value = value
                .get("user")
                .and_then(|u| u.get(name))
                .and_then(Value::as_str)
                .ok_or("Invalid account response")?;
            if value.len() > 512
                || value.chars().any(char::is_control)
                || value.contains(token.expose())
            {
                return Err("Invalid account response");
            }
            Ok(value.to_owned())
        };
        Ok(User {
            id: field("id")?,
            email: field("email")?,
            name: field("name")?,
        })
    }
    pub fn revoke(&self, token: &SecretString) -> Result<(), &'static str> {
        let (status, _) = self.request("api/auth/sign-out", Some(json!({})), Some(token))?;
        if status != 200 {
            return Err("Remote session revocation unconfirmed");
        }
        Ok(())
    }
}

impl AccountClient {
    /// Snapshot protocol only. The daemon owns credentials; the renderer receives public progress only.
    pub fn share(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
        token: &SecretString,
    ) -> Result<Value, &'static str> {
        if !path.starts_with("v1/shares")
            || path.len() > 512
            || path.contains("..")
            || !path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-".contains(&b))
            || body.len() > 512 * 1024
        {
            return Err("Invalid share request");
        }
        self.runtime.block_on(async {
            let url = self.origin.0.join(path).map_err(|_| "Share unavailable")?;
            let method = reqwest::Method::from_bytes(method.as_bytes())
                .map_err(|_| "Invalid share request")?;
            let mut response = self
                .client
                .request(method, url)
                .bearer_auth(token.expose())
                .header(
                    "content-type",
                    if path.contains("/parts/") {
                        "application/octet-stream"
                    } else {
                        "application/json"
                    },
                )
                .body(body.to_vec())
                .send()
                .await
                .map_err(|_| "Upload interrupted; retry to resume")?;
            let status = response.status();
            if !status.is_success() {
                return Err(match status.as_u16() {
                    401 => "Sign in required",
                    403 => "Pro required",
                    404 => "Share unavailable",
                    _ => "Share changed or upload interrupted; retry",
                });
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| "Share unavailable")? {
                if bytes.len() + chunk.len() > 256 * 1024 {
                    return Err("Invalid share response");
                }
                bytes.extend_from_slice(&chunk);
            }
            serde_json::from_slice(&bytes).map_err(|_| "Invalid share response")
        })
    }
}
