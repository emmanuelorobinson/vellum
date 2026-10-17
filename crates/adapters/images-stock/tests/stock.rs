//! TEST DATA. The JSON is shaped like Pixabay's documented search answer; the pictures are made up.
use std::sync::{Arc, Mutex};

use serde_json::json;
use vellum_contracts::fakes::{ManualClock, MemorySecrets};
use vellum_core::ports::*;
use vellum_images_stock::{KEY_NAME, PixabaySource};

#[derive(Default)]
struct Scripted {
    log: Mutex<Vec<FetchRequest>>,
    search: Mutex<Option<FetchResponse>>,
    picture: Mutex<Option<FetchResponse>>,
}

impl HttpFetch for Scripted {
    fn get(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        self.log.lock().unwrap().push(request.clone());
        let slot = if request.url.starts_with("https://pixabay.com/api/") {
            &self.search
        } else {
            &self.picture
        };
        slot.lock().unwrap().clone().ok_or(FetchError::Network)
    }
}

fn answer(status: u16, content_type: &str, body: &[u8]) -> Option<FetchResponse> {
    Some(FetchResponse {
        status,
        content_type: content_type.into(),
        body: body.to_vec(),
    })
}

fn results() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "total": 400, "totalHits": 400,
        "hits": [
            {"id": 7, "pageURL": "https://pixabay.com/photos/kitchen-7/", "tags": "kitchen, cooking, TEST DATA",
             "previewURL": "https://cdn.pixabay.com/photo/p7_150.jpg", "webformatURL": "https://pixabay.com/get/p7_640.jpg",
             "largeImageURL": "https://pixabay.com/get/p7_1280.jpg", "imageWidth": 5000, "imageHeight": 2500,
             "user": "TEST DATA Sam", "user_id": 1234},
            {"id": 8, "pageURL": "https://pixabay.com/photos/pan-8/", "tags": "pan",
             "webformatURL": "https://pixabay.com/get/p8_640.jpg", "largeImageURL": "https://evil.example.com/p8.jpg",
             "imageWidth": 800, "imageHeight": 600, "user": "x", "user_id": 1},
            {"id": 9, "pageURL": "https://pixabay.com/photos/cup-9/", "tags": "cup",
             "webformatURL": "https://pixabay.com/get/p9_640.jpg", "imageWidth": 600, "imageHeight": 900,
             "user": "Ana", "user_id": 5},
        ]
    }))
    .unwrap()
}

struct Rig {
    fetch: Arc<Scripted>,
    clock: Arc<ManualClock>,
    source: PixabaySource,
}

fn rig(key: Option<&str>) -> Rig {
    let fetch = Arc::new(Scripted::default());
    *fetch.search.lock().unwrap() = answer(200, "application/json", &results());
    *fetch.picture.lock().unwrap() = answer(200, "image/jpeg", b"jpeg bytes");
    let secrets = Arc::new(MemorySecrets::default());
    if let Some(key) = key {
        secrets.set(KEY_NAME, SecretString::new(key)).unwrap();
    }
    let clock = Arc::new(ManualClock::new(1_700_000_000_000));
    let source = PixabaySource::new(fetch.clone(), secrets, clock.clone());
    Rig {
        fetch,
        clock,
        source,
    }
}

fn query(q: &str) -> ImageQuery {
    ImageQuery {
        query: q.into(),
        page: 1,
        per_page: 3,
        orientation: None,
    }
}

#[test]
fn passes_the_image_source_contract() {
    let r = rig(Some("k"));
    vellum_contracts::suites::import::image_source(&r.source, "TEST DATA kitchen");
}

#[test]
fn a_search_sends_the_key_as_a_placeholder_and_never_in_the_url() {
    let r = rig(Some("sekret"));
    r.source
        .search(&ImageQuery {
            query: "ripe tomato & basil".into(),
            page: 2,
            per_page: 500,
            orientation: Some(Orientation::Landscape),
        })
        .unwrap();
    let log = r.fetch.log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(
        log[0].url,
        "https://pixabay.com/api/?key={secret}&q=ripe%20tomato%20%26%20basil&image_type=photo&safesearch=true&page=2&per_page=200&orientation=horizontal"
    );
    assert_eq!(
        log[0].secret.as_ref().map(|s| s.expose().to_owned()),
        Some("sekret".into())
    );
    assert!(!log[0].url.contains("sekret"));
}

#[test]
fn hits_carry_sizes_attribution_and_only_pixabay_addresses() {
    let r = rig(Some("k"));
    let page = r.source.search(&query("kitchen")).unwrap();
    assert_eq!(page.total, 400);
    let ids: Vec<_> = page.hits.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(
        ids,
        ["7", "9"],
        "a hit that points off pixabay.com is dropped"
    );
    let first = &page.hits[0];
    assert_eq!(
        (first.width, first.height),
        (1280, 640),
        "sized as the stored 1280 version"
    );
    assert_eq!(first.download_url, "https://pixabay.com/get/p7_1280.jpg");
    assert_eq!(first.preview_url, "https://pixabay.com/get/p7_640.jpg");
    assert_eq!(first.description, "kitchen, cooking, TEST DATA");
    assert_eq!(
        first.attribution,
        Attribution {
            provider: "pixabay".into(),
            author: "TEST DATA Sam".into(),
            author_url: "https://pixabay.com/users/TEST%20DATA%20Sam-1234/".into(),
            source_url: "https://pixabay.com/photos/kitchen-7/".into(),
            licence: "Pixabay Content License".into(),
            licence_url: "https://pixabay.com/service/license-summary/".into(),
        }
    );
    assert_eq!(
        page.hits[1].download_url, "https://pixabay.com/get/p9_640.jpg",
        "no large version: the medium one"
    );
}

#[test]
fn results_are_kept_for_a_day_per_search() {
    let r = rig(Some("k"));
    r.source.search(&query("kitchen")).unwrap();
    r.source.search(&query("kitchen")).unwrap();
    assert_eq!(
        r.fetch.log.lock().unwrap().len(),
        1,
        "the second search is answered from the keep"
    );
    r.source.search(&query("garden")).unwrap();
    r.source
        .search(&ImageQuery {
            page: 2,
            ..query("kitchen")
        })
        .unwrap();
    assert_eq!(
        r.fetch.log.lock().unwrap().len(),
        3,
        "another query or page is another search"
    );
    r.clock.advance(24 * 60 * 60 * 1000 - 1);
    r.source.search(&query("kitchen")).unwrap();
    assert_eq!(r.fetch.log.lock().unwrap().len(), 3);
    r.clock.advance(1);
    r.source.search(&query("kitchen")).unwrap();
    assert_eq!(
        r.fetch.log.lock().unwrap().len(),
        4,
        "after 24 hours it asks again"
    );
}

#[test]
fn without_a_key_nothing_is_sent() {
    let r = rig(None);
    assert_eq!(
        r.source.search(&query("kitchen")).unwrap_err(),
        ImageError::NotConfigured
    );
    assert!(r.fetch.log.lock().unwrap().is_empty());
}

#[test]
fn the_providers_refusals_become_plain_errors_that_never_hold_the_key() {
    let cases = [
        (answer(429, "text/plain", b"slow"), ImageError::RateLimited),
        (answer(401, "text/plain", b""), ImageError::Authentication),
        (
            answer(400, "text/plain", b"[ERROR 400] Invalid or missing API key"),
            ImageError::Authentication,
        ),
        (
            answer(400, "text/plain", b"[ERROR 400] \"q\" is too long"),
            ImageError::Rejected("the search was not accepted".into()),
        ),
        (
            answer(503, "text/plain", b""),
            ImageError::Rejected("it answered 503".into()),
        ),
        (
            answer(200, "application/json", b"not json"),
            ImageError::Rejected("the answer was not JSON".into()),
        ),
        (None, ImageError::Network),
    ];
    for (response, expected) in cases {
        let r = rig(Some("sekret"));
        *r.fetch.search.lock().unwrap() = response;
        let error = r.source.search(&query("kitchen")).unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("sekret"));
    }
}

#[test]
fn blank_and_overlong_searches_are_refused_before_any_request() {
    let r = rig(Some("k"));
    assert!(r.source.search(&query("   ")).is_err());
    assert!(r.source.search(&query(&"a".repeat(101))).is_err());
    assert!(r.fetch.log.lock().unwrap().is_empty());
}

#[test]
fn a_download_must_be_a_picture_from_pixabay() {
    let r = rig(Some("k"));
    let hit = r.source.search(&query("kitchen")).unwrap().hits.remove(0);
    let picture = r.source.fetch(&hit).unwrap();
    assert_eq!(
        (picture.mime.as_str(), picture.bytes.as_slice()),
        ("image/jpeg", &b"jpeg bytes"[..])
    );
    assert_eq!(picture.attribution, hit.attribution);
    let log = r.fetch.log.lock().unwrap().clone();
    assert_eq!(log.last().unwrap().url, hit.download_url);
    assert!(
        log.last().unwrap().secret.is_none(),
        "the key is never sent with a download"
    );

    let off_site = ImageHit {
        download_url: "https://evil.example.com/a.jpg".into(),
        ..hit.clone()
    };
    assert!(r.source.fetch(&off_site).is_err());
    let sneaky = ImageHit {
        download_url: "https://pixabay.com.evil.example/a.jpg".into(),
        ..hit.clone()
    };
    assert!(r.source.fetch(&sneaky).is_err());
    let with_user = ImageHit {
        download_url: "https://pixabay.com@evil.example/a.jpg".into(),
        ..hit.clone()
    };
    assert!(r.source.fetch(&with_user).is_err());
    let calls = r.fetch.log.lock().unwrap().len();
    assert_eq!(calls, log.len(), "refused addresses are not requested");

    *r.fetch.picture.lock().unwrap() = answer(200, "text/html", b"<html>");
    assert!(
        r.source
            .fetch(&hit)
            .unwrap_err()
            .to_string()
            .contains("not a picture")
    );
    *r.fetch.picture.lock().unwrap() = answer(404, "image/jpeg", b"x");
    assert!(r.source.fetch(&hit).is_err());
    *r.fetch.picture.lock().unwrap() = answer(200, "image/png", b"");
    assert!(r.source.fetch(&hit).is_err());
}
