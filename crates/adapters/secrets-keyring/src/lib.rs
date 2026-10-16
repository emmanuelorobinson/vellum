//! OS-only writes, explicit read-only CI environment fallback, payload-free errors.
use std::sync::Arc;
use vellum_core::ports::{SecretError, SecretStore, SecretString};
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

pub struct Keychain {
    service: String,
}
impl Keychain {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
    fn entry(&self, key: &str) -> Result<keyring::Entry, SecretError> {
        validate_key(key)?;
        keyring::Entry::new(&self.service, key).map_err(classify)
    }
}
fn classify(error: keyring::Error) -> SecretError {
    // Do not retain, format or chain a vendor error: some variants carry secrets.
    match error {
        keyring::Error::Invalid(_, _) => SecretError::InvalidKey,
        keyring::Error::NoDefaultStore => SecretError::Unavailable,
        _ => SecretError::Denied,
    }
}
impl SecretStore for Keychain {
    fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError> {
        match self.entry(key)?.get_password() {
            Ok(value) => Ok(Some(SecretString::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(classify(error)),
        }
    }
    fn set(&self, key: &str, value: SecretString) -> Result<(), SecretError> {
        self.entry(key)?
            .set_password(value.expose())
            .map_err(classify)
    }
    fn delete(&self, key: &str) -> Result<(), SecretError> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(classify(error)),
        }
    }
}
fn validate_key(key: &str) -> Result<(), SecretError> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(SecretError::InvalidKey);
    }
    Ok(())
}

type EnvLookup = dyn Fn(&str) -> Option<String> + Send + Sync;
pub struct EnvSecrets {
    lookup: Box<EnvLookup>,
}
impl EnvSecrets {
    /// CI fallback is opt-in, not enabled by being linked into a host.
    pub fn for_ci() -> Result<Self, SecretError> {
        if !std::env::var("CI").is_ok_and(|value| value == "true" || value == "1") {
            return Err(SecretError::Denied);
        }
        Ok(Self::with_lookup(|name| std::env::var(name).ok()))
    }
    pub fn with_lookup(lookup: impl Fn(&str) -> Option<String> + Send + Sync + 'static) -> Self {
        Self {
            lookup: Box::new(lookup),
        }
    }
}
impl SecretStore for EnvSecrets {
    fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError> {
        validate_key(key)?;
        let name = format!(
            "VELLUM_{}_API_KEY",
            key.replace('-', "_").to_ascii_uppercase()
        );
        Ok((self.lookup)(&name)
            .filter(|v| !v.is_empty())
            .map(SecretString::new))
    }
    fn set(&self, _: &str, _: SecretString) -> Result<(), SecretError> {
        Err(SecretError::ReadOnly)
    }
    fn delete(&self, _: &str) -> Result<(), SecretError> {
        Err(SecretError::ReadOnly)
    }
}
pub struct FallbackSecrets {
    primary: Arc<dyn SecretStore>,
    fallback: Arc<dyn SecretStore>,
}
impl FallbackSecrets {
    pub fn new(primary: Arc<dyn SecretStore>, fallback: Arc<dyn SecretStore>) -> Self {
        Self { primary, fallback }
    }
}
impl SecretStore for FallbackSecrets {
    fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError> {
        match self.primary.get(key)? {
            Some(value) => Ok(Some(value)),
            None => self.fallback.get(key),
        }
    }
    fn set(&self, key: &str, value: SecretString) -> Result<(), SecretError> {
        self.primary.set(key, value)
    }
    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.primary.delete(key)
    }
}
