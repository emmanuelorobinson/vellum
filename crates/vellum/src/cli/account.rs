//! Explicit account commands only. Local commands never read a cloud session or call the account service.
use super::commands::Output;
use clap::Args;
use serde_json::json;
use std::{io, time::Instant};
use vellum_account_http::{AccountClient, AccountUrl, PollState, StoredSession, TokenReply};
use vellum_core::ports::SecretStore;
use vellum_secrets_keyring::Keychain;
const SLOT: &str = "cloud-session";
#[derive(Args)]
pub struct LoginArgs {
    /// Your account service origin. Also accepted from VELLUM_ACCOUNT_URL; no service is contacted by default.
    #[arg(long)]
    pub account_url: Option<String>,
}
fn secrets() -> Keychain {
    Keychain::new(vellum_types::brand::KEYCHAIN_SERVICE)
}
fn load(store: &dyn SecretStore) -> Result<Option<StoredSession>, &'static str> {
    store
        .get(SLOT)
        .map_err(|_| "OS keychain unavailable")?
        .map(StoredSession::decode)
        .transpose()
}
pub fn login(args: LoginArgs, out: &Output) -> io::Result<bool> {
    let attempt = || -> Result<(), &'static str> {
        let store = secrets();
        let origin = match args
            .account_url
            .or_else(|| std::env::var("VELLUM_ACCOUNT_URL").ok())
        {
            Some(url) => AccountUrl::parse(&url)?,
            None => load(&store)?
                .map(|s| s.origin)
                .ok_or("Set --account-url or VELLUM_ACCOUNT_URL to your account service")?,
        };
        let client = AccountClient::new(origin.clone())?;
        let device = client.start()?;
        let notice = json!({"state":"waiting","user_code":device.user_code,"verification_url":device.verification_url,"expires_in":device.expires_in});
        out.result("login",notice,&format!("Approve in your browser: {}\nCode: {}\nThis connects the CLI; it does not share your repo.",device.verification_url,device.user_code)).map_err(|_|"Output unavailable")?;
        let clock = Instant::now();
        let now = || u64::try_from(clock.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut poll = PollState::new(now(), device.expires_in, device.interval)?;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(poll.wait_ms(now())?));
            match client.poll(&device)? {
                TokenReply::Waiting(status) => poll.update(status, now())?,
                TokenReply::Ready(token) => {
                    let session = StoredSession::new(origin, token)?;
                    store
                        .set(SLOT, session.encode()?)
                        .map_err(|_| "OS keychain refused the session")?;
                    break;
                }
            }
        }
        Ok(())
    };
    match attempt() {
        Ok(()) => {
            out.result("login", json!({"signed_in":true}), "Signed in")?;
            Ok(true)
        }
        Err(message) => {
            out.error("login", message);
            Ok(false)
        }
    }
}
pub fn whoami(out: &Output) -> io::Result<bool> {
    let attempt = || -> Result<_, &'static str> {
        let session = load(&secrets())?.ok_or("Signed out")?;
        AccountClient::new(session.origin)?.me(&session.token)
    };
    match attempt() {
        Ok(user) => {
            out.result("whoami", json!({"user":user}), "Signed in")?;
            Ok(true)
        }
        Err(message) => {
            out.error("whoami", message);
            Ok(false)
        }
    }
}
pub fn logout(out: &Output) -> io::Result<bool> {
    let store = secrets();
    let session = match load(&store) {
        Ok(session) => session,
        Err(message) => {
            out.error("logout", message);
            return Ok(false);
        }
    };
    // Delete on this computer even when the service is offline; never keep a token the person signed out of.
    if store.delete(SLOT).is_err() {
        out.error("logout", "OS keychain refused sign-out");
        return Ok(false);
    }
    let revoked = session.is_none_or(|session| {
        AccountClient::new(session.origin)
            .and_then(|client| client.revoke(&session.token))
            .is_ok()
    });
    out.result(
        "logout",
        json!({"signed_in":false,"remote_revoked":revoked}),
        if revoked {
            "Signed out"
        } else {
            "Signed out on this computer; remote session revocation unconfirmed"
        },
    )?;
    Ok(true)
}
