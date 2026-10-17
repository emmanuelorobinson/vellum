//! ImageSource: stock images from Pixabay (ADR-009).
//!
//! The person's own Pixabay key lives in the keychain under [`KEY_NAME`]; nothing ships a key. Search results are
//! kept for 24 hours, as Pixabay's API terms ask. A chosen picture is downloaded once and stored in the repository
//! as an ordinary asset, never linked, and its author, page and licence are returned so they can be written into
//! the node's metadata. Downloads are only taken from `pixabay.com` addresses that a search result named.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use vellum_core::ports::{
    Attribution, Clock, FetchError, FetchRequest, FetchedImage, HttpFetch, ImageError, ImageHit,
    ImagePage, ImageQuery, ImageSource, Orientation, SecretStore,
};

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The keychain entry the key is stored under.
pub const KEY_NAME: &str = "stock.pixabay";

const API: &str = "https://pixabay.com/api/";
const LICENCE: &str = "Pixabay Content License";
const LICENCE_URL: &str = "https://pixabay.com/service/license-summary/";
/// Pixabay's largest size without special approval.
const LARGEST: f64 = 1280.0;
const CACHE_MS: i64 = 24 * 60 * 60 * 1000;
const CACHE_ENTRIES: usize = 64;
const MAX_ANSWER: usize = 2 * 1024 * 1024;
const MAX_PICTURE: usize = 12 * 1024 * 1024;

pub struct PixabaySource {
    fetch: Arc<dyn HttpFetch>,
    secrets: Arc<dyn SecretStore>,
    clock: Arc<dyn Clock + Send + Sync>,
    cache: Mutex<BTreeMap<String, (i64, ImagePage)>>,
}

impl PixabaySource {
    pub fn new(
        fetch: Arc<dyn HttpFetch>,
        secrets: Arc<dyn SecretStore>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            fetch,
            secrets,
            clock,
            cache: Mutex::new(BTreeMap::new()),
        }
    }
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

fn map_fetch(error: FetchError) -> ImageError {
    match error {
        FetchError::Blocked => ImageError::Rejected("the address is not allowed".into()),
        FetchError::TooLarge => ImageError::Rejected("the answer is larger than allowed".into()),
        FetchError::Network | FetchError::Timeout => ImageError::Network,
    }
}

fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// A downloadable address: https on pixabay.com or one of its subdomains, with no user name or port.
fn pixabay_address(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    !authority.contains(['@', ':'])
        && (authority == "pixabay.com" || authority.ends_with(".pixabay.com"))
}

fn hit(v: &Value) -> Option<ImageHit> {
    let id = v.get("id")?.as_u64()?.to_string();
    let download = string(v, "largeImageURL").or_else(|| string(v, "webformatURL"))?;
    let preview = string(v, "webformatURL")
        .or_else(|| string(v, "previewURL"))
        .unwrap_or_else(|| download.clone());
    if !pixabay_address(&download) || !pixabay_address(&preview) {
        return None;
    }
    let (w, h) = (
        v.get("imageWidth")?.as_f64()?,
        v.get("imageHeight")?.as_f64()?,
    );
    if !(w >= 1. && h >= 1.) {
        return None;
    }
    // The stored file is the large version: at most 1280 on its longer side.
    let shrink = (LARGEST / w.max(h)).min(1.);
    let user = string(v, "user")?;
    let user_id = v.get("user_id").and_then(Value::as_u64);
    Some(ImageHit {
        id,
        description: string(v, "tags").unwrap_or_default(),
        width: (w * shrink).round() as u32,
        height: (h * shrink).round() as u32,
        preview_url: preview,
        download_url: download,
        attribution: Attribution {
            provider: "pixabay".into(),
            author_url: match user_id {
                Some(id) => format!("https://pixabay.com/users/{}-{id}/", encode(&user)),
                None => "https://pixabay.com/".into(),
            },
            author: user,
            source_url: string(v, "pageURL").filter(|u| pixabay_address(u))?,
            licence: LICENCE.into(),
            licence_url: LICENCE_URL.into(),
        },
    })
}

impl ImageSource for PixabaySource {
    fn id(&self) -> &'static str {
        "stock"
    }

    fn origin(&self) -> &'static str {
        "https://pixabay.com"
    }

    fn search(&self, query: &ImageQuery) -> Result<ImagePage, ImageError> {
        let q = query.query.trim();
        if q.is_empty() || q.chars().count() > 100 {
            return Err(ImageError::Rejected(
                "a search is 1 to 100 characters".into(),
            ));
        }
        let per_page = query.per_page.clamp(3, 200);
        let key = format!(
            "q={}&image_type=photo&safesearch=true&page={}&per_page={per_page}{}",
            encode(q),
            query.page.max(1),
            match query.orientation {
                Some(Orientation::Landscape) => "&orientation=horizontal",
                Some(Orientation::Portrait) => "&orientation=vertical",
                None => "",
            }
        );
        let now = self.clock.now().0;
        if let Some((at, page)) = self.cache.lock().expect("not poisoned").get(&key)
            && now - *at < CACHE_MS
        {
            return Ok(page.clone());
        }
        let secret = self
            .secrets
            .get(KEY_NAME)
            .map_err(|_| ImageError::Rejected("the keychain could not be read".into()))?
            .ok_or(ImageError::NotConfigured)?;
        let answer = self
            .fetch
            .get(&FetchRequest {
                url: format!("{API}?key={{secret}}&{key}"),
                secret: Some(secret),
                max_bytes: MAX_ANSWER,
            })
            .map_err(map_fetch)?;
        match answer.status {
            200 => {}
            401 | 403 => return Err(ImageError::Authentication),
            429 => return Err(ImageError::RateLimited),
            400 => {
                // Pixabay says 400 for a bad key as well as a bad query, in words.
                let body = String::from_utf8_lossy(&answer.body).to_lowercase();
                return Err(if body.contains("api key") {
                    ImageError::Authentication
                } else {
                    ImageError::Rejected("the search was not accepted".into())
                });
            }
            other => return Err(ImageError::Rejected(format!("it answered {other}"))),
        }
        let json: Value = serde_json::from_slice(&answer.body)
            .map_err(|_| ImageError::Rejected("the answer was not JSON".into()))?;
        let hits = json
            .get("hits")
            .and_then(Value::as_array)
            .ok_or_else(|| ImageError::Rejected("the answer has no results list".into()))?;
        let page = ImagePage {
            hits: hits.iter().filter_map(hit).collect(),
            total: json
                .get("totalHits")
                .and_then(Value::as_u64)
                .unwrap_or(hits.len() as u64),
        };
        let mut cache = self.cache.lock().expect("not poisoned");
        cache.retain(|_, (at, _)| now - *at < CACHE_MS);
        while cache.len() >= CACHE_ENTRIES {
            let oldest = cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(k, _)| k.clone());
            match oldest {
                Some(k) => cache.remove(&k),
                None => break,
            };
        }
        cache.insert(key, (now, page.clone()));
        Ok(page)
    }

    fn fetch(&self, hit: &ImageHit) -> Result<FetchedImage, ImageError> {
        if !pixabay_address(&hit.download_url) {
            return Err(ImageError::Rejected(
                "only Pixabay pictures can be downloaded".into(),
            ));
        }
        let answer = self
            .fetch
            .get(&FetchRequest {
                url: hit.download_url.clone(),
                secret: None,
                max_bytes: MAX_PICTURE,
            })
            .map_err(map_fetch)?;
        if answer.status != 200 {
            return Err(ImageError::Rejected(format!(
                "the download answered {}",
                answer.status
            )));
        }
        let mime = match answer.content_type.as_str() {
            m @ ("image/jpeg" | "image/png" | "image/webp") => m.to_owned(),
            other => {
                return Err(ImageError::Rejected(format!(
                    "the download is {other:?}, not a picture"
                )));
            }
        };
        if answer.body.is_empty() {
            return Err(ImageError::Rejected("the download was empty".into()));
        }
        Ok(FetchedImage {
            mime,
            bytes: answer.body,
            attribution: hit.attribution.clone(),
        })
    }
}
