//! Private S3-compatible canonical snapshots. Credentials are injected, never serialized or returned.
use object_store::{
    ClientOptions, ObjectStore, ObjectStoreExt, PutMode, PutOptions, RetryConfig, aws::AmazonS3,
    aws::AmazonS3Builder, path::Path,
};
use sha2::{Digest, Sha256};
use std::{sync::Mutex, time::Duration};
use vellum_core::ports::{
    SecretString,
    sync::{Error, SnapshotStore},
};
use vellum_types::sync::MAX_SNAPSHOT_BYTES;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub struct R2Snapshots {
    store: AmazonS3,
    runtime: Mutex<tokio::runtime::Runtime>,
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn path(key: &str) -> Result<Path, Error> {
    if key.len() != 64
        || !key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::NotFound);
    }
    Ok(Path::from(format!("sync/snapshots/{key}.vellum")))
}
impl R2Snapshots {
    pub fn connect(
        endpoint: &str,
        bucket: &str,
        access: SecretString,
        secret: SecretString,
    ) -> Result<Self, Error> {
        let url = url::Url::parse(endpoint).map_err(|_| Error::InvalidUpdate)?;
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if (url.scheme() != "https" && !(local && url.scheme() == "http"))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(Error::InvalidUpdate);
        }
        let store = AmazonS3Builder::new()
            .with_endpoint(endpoint)
            .with_region("auto")
            .with_bucket_name(bucket)
            .with_access_key_id(access.expose())
            .with_secret_access_key(secret.expose())
            .with_virtual_hosted_style_request(false)
            .with_disable_tagging(true)
            .with_client_options(
                ClientOptions::new()
                    .with_timeout(Duration::from_secs(15))
                    .with_connect_timeout(Duration::from_secs(5)),
            )
            .with_allow_http(local)
            .with_retry(RetryConfig {
                max_retries: 2,
                retry_timeout: Duration::from_secs(20),
                ..Default::default()
            })
            .build()
            .map_err(|_| Error::Unavailable)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            store,
            runtime: Mutex::new(runtime),
        })
    }
}
impl SnapshotStore for R2Snapshots {
    fn put(&self, bytes: &[u8]) -> Result<String, Error> {
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(Error::Limit);
        }
        let key = digest(bytes);
        let path = path(&key)?;
        let result = self
            .runtime
            .lock()
            .map_err(|_| Error::Unavailable)?
            .block_on(self.store.put_opts(
                &path,
                bytes.to_vec().into(),
                PutOptions {
                    mode: PutMode::Create,
                    ..Default::default()
                },
            ));
        match result {
            Ok(_) => {}
            Err(
                object_store::Error::AlreadyExists { .. }
                | object_store::Error::Precondition { .. },
            ) => {
                self.get(&key)?;
            }
            Err(_) => return Err(Error::Unavailable),
        }
        Ok(key)
    }
    fn get(&self, key: &str) -> Result<Vec<u8>, Error> {
        let path = path(key)?;
        let bytes = self
            .runtime
            .lock()
            .map_err(|_| Error::Unavailable)?
            .block_on(async {
                let result = self
                    .store
                    .get(&path)
                    .await
                    .map_err(|_| Error::Unavailable)?;
                if result.meta.size > MAX_SNAPSHOT_BYTES as u64 {
                    return Err(Error::Limit);
                }
                result.bytes().await.map_err(|_| Error::Unavailable)
            })?;
        if bytes.len() > MAX_SNAPSHOT_BYTES || digest(&bytes) != key {
            return Err(Error::InvalidUpdate);
        }
        Ok(bytes.to_vec())
    }
}
