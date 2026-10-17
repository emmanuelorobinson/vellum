//! Import and asset contracts (ARCHITECTURE §3.6): an importer is deterministic and total, an icon library finds
//! what it holds and resolves weights and fills honestly, an image source returns stored-able pictures with their
//! attribution, and the HTTP fetch refuses addresses it must not touch.
use serde_json::Value;
use vellum_core::ports::*;

/// `valid` is a small input the importer reads in full. Garbage must be refused, and nothing may panic, however
/// much of `valid` is cut off.
pub fn importer(importer: &dyn Importer, valid: &[u8]) {
    assert!(!importer.id().is_empty(), "Importer: the id is not empty");
    let input = ImportInput {
        bytes: valid,
        name: "TEST DATA.file",
    };
    assert!(
        importer.recognizes(&input),
        "Importer: it does not recognize its own format"
    );
    for junk in [&b""[..], b"\x00\x01\x02 not a document", b"{\"a\":1}"] {
        assert!(
            !importer.recognizes(&ImportInput {
                bytes: junk,
                name: "junk"
            }),
            "Importer: it recognizes {junk:?}"
        );
        assert!(
            importer
                .import(&ImportInput {
                    bytes: junk,
                    name: "junk"
                })
                .is_err(),
            "Importer: junk {junk:?} was imported"
        );
    }
    let first = importer
        .import(&input)
        .expect("Importer: it cannot import its own format");
    assert!(
        !first.nodes.is_empty(),
        "Importer: a valid input gave no nodes"
    );
    for node in &first.nodes {
        assert!(
            node.get("type").and_then(Value::as_str).is_some(),
            "Importer: a node has no type: {node}"
        );
    }
    for index in first.used_assets() {
        let asset = first.assets.get(index);
        assert!(
            asset.is_some(),
            "Importer: a node names asset {index} that was not returned"
        );
        let asset = asset.expect("checked");
        assert!(
            asset.mime.starts_with("image/") && !asset.bytes.is_empty(),
            "Importer: asset {index} is not an image"
        );
    }
    assert_eq!(
        first,
        importer
            .import(&input)
            .expect("Importer: second import failed"),
        "Importer: the same input gave different results"
    );
    for cut in (0..valid.len()).step_by((valid.len() / 48).max(1)) {
        let _ = importer.import(&ImportInput {
            bytes: &valid[..cut],
            name: "cut",
        });
    }
    // Damaged input of every kind: bytes changed, cut and repeated, from a fixed seed so a failure repeats.
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..400 {
        let mut bytes = valid.to_vec();
        for _ in 0..=(next() % 4) {
            let at = (next() as usize) % bytes.len().max(1);
            if let Some(b) = bytes.get_mut(at) {
                *b = next() as u8;
            }
        }
        if next() % 5 == 0 {
            bytes.truncate((next() as usize) % (bytes.len() + 1));
        }
        let _ = importer.import(&ImportInput {
            bytes: &bytes,
            name: "mutated",
        });
        let _ = importer.recognizes(&ImportInput {
            bytes: &bytes,
            name: "mutated",
        });
    }
}

/// `name` is an icon the library holds, and `word` a word that finds it besides the whole name.
pub fn icon_library(library: &dyn IconLibrary, name: &str, word: &str) {
    assert!(
        !library.id().is_empty() && !library.label().is_empty(),
        "IconLibrary: id and label"
    );
    assert!(!library.is_empty(), "IconLibrary: it holds no icons");
    let weights = library.weights();
    assert!(
        !weights.is_empty()
            && weights.windows(2).all(|w| w[0] < w[1])
            && weights.iter().all(|w| (100..=700).contains(w)),
        "IconLibrary: weights are ascending and 100 to 700: {weights:?}"
    );
    let all = library.search("", 5);
    assert!(all.len() <= 5, "IconLibrary: search ignored its limit");
    assert_eq!(
        all,
        library.search("", 5),
        "IconLibrary: a blank search changed between calls"
    );
    assert!(
        all.windows(2).all(|w| w[0].name <= w[1].name),
        "IconLibrary: a blank search is by name"
    );
    let found = library.search(name, 10);
    assert_eq!(
        found.first().map(|i| i.name.as_str()),
        Some(name),
        "IconLibrary: the exact name is not the first result"
    );
    assert!(
        library.search(word, 10).iter().any(|i| i.name == name),
        "IconLibrary: {word:?} does not find {name:?}"
    );
    assert!(
        library.search(&format!("{name} zzzzqqqq"), 10).is_empty(),
        "IconLibrary: every word of a query must match"
    );
    assert!(
        library.search("zzzzqqqq", 10).is_empty(),
        "IconLibrary: nonsense found something"
    );
    assert_eq!(
        library.search(name, 0).len(),
        0,
        "IconLibrary: a limit of 0 returned icons"
    );
    for fill in [false, true] {
        let glyph = library
            .glyph(name, 400, fill)
            .expect("IconLibrary: it cannot draw an icon it finds");
        assert_eq!(
            (glyph.library.as_str(), glyph.name.as_str()),
            (library.id(), name)
        );
        assert!(
            glyph.view_box[2] > 0. && glyph.view_box[3] > 0.,
            "IconLibrary: empty view box"
        );
        assert!(
            !glyph.path.trim().is_empty(),
            "IconLibrary: the icon has no path"
        );
        assert_eq!(glyph.style, library.style(), "IconLibrary: style");
        assert!(
            weights.contains(&glyph.weight),
            "IconLibrary: it used a weight it does not list"
        );
        assert_eq!(
            glyph.stroke.is_some(),
            library.style() == IconStyle::Stroke,
            "IconLibrary: a stroke is given exactly for stroke icons"
        );
        if let Some(stroke) = glyph.stroke {
            assert!(
                stroke.width > 0.0 && stroke.width.is_finite(),
                "IconLibrary: stroke width"
            );
        }
        if !library.supports_fill() {
            assert!(
                !glyph.fill,
                "IconLibrary: it claims a fill it does not have"
            );
        } else {
            assert_eq!(glyph.fill, fill, "IconLibrary: fill was not honoured");
        }
    }
    let thin = library
        .glyph(name, 1, false)
        .expect("IconLibrary: weight 1");
    let heavy = library
        .glyph(name, 999, false)
        .expect("IconLibrary: weight 999");
    assert_eq!(
        thin.weight, weights[0],
        "IconLibrary: a low weight takes the lightest"
    );
    assert_eq!(
        heavy.weight,
        *weights.last().expect("not empty"),
        "IconLibrary: a high weight takes the heaviest"
    );
    assert!(
        library.glyph("zzzzqqqq-not-an-icon", 400, false).is_none(),
        "IconLibrary: invented an icon"
    );
}

/// `query` finds pictures in the source.
pub fn image_source(source: &dyn ImageSource, query: &str) {
    assert!(!source.id().is_empty(), "ImageSource: id");
    assert!(
        source.origin().starts_with("https://") && !source.origin().ends_with('/'),
        "ImageSource: the origin is an https address without a trailing slash"
    );
    let page = source
        .search(&ImageQuery {
            query: query.to_owned(),
            page: 1,
            per_page: 3,
            orientation: None,
        })
        .expect("ImageSource: search failed");
    assert!(
        !page.hits.is_empty(),
        "ImageSource: {query:?} found nothing"
    );
    assert!(
        page.hits.len() <= 3,
        "ImageSource: more results than asked for"
    );
    assert!(
        page.total >= page.hits.len() as u64,
        "ImageSource: total is below the results"
    );
    for hit in &page.hits {
        assert!(!hit.id.is_empty(), "ImageSource: a hit has no id");
        assert!(
            hit.download_url.starts_with("https://"),
            "ImageSource: downloads are https"
        );
        assert!(
            hit.width > 0 && hit.height > 0,
            "ImageSource: a hit has no size"
        );
        let a = &hit.attribution;
        assert!(
            !a.provider.is_empty()
                && !a.author.is_empty()
                && !a.source_url.is_empty()
                && !a.licence.is_empty(),
            "ImageSource: attribution is incomplete: {a:?}"
        );
    }
    let hit = &page.hits[0];
    let picture = source.fetch(hit).expect("ImageSource: fetch failed");
    assert!(
        picture.mime.starts_with("image/") && !picture.bytes.is_empty(),
        "ImageSource: not a picture"
    );
    assert_eq!(
        picture.attribution, hit.attribution,
        "ImageSource: the picture lost its attribution"
    );
}

/// Addresses a fetch must refuse before any request is made: not https, a user name, this computer, a private
/// network, an IP literal. Needs no network.
pub fn http_fetch_refuses_bad_addresses(fetch: &dyn HttpFetch) {
    let request = |url: &str| FetchRequest {
        url: url.to_owned(),
        secret: None,
        max_bytes: 1 << 20,
    };
    for blocked in [
        "http://example.com/a.png",
        "https://user:pass@example.com/a.png",
        "https://localhost/a.png",
        "https://127.0.0.1/a.png",
        "https://192.168.1.4/a.png",
        "https://[::1]/a.png",
        "https://169.254.169.254/latest/meta-data",
        "ftp://example.com/a.png",
        "not a url",
    ] {
        assert_eq!(
            fetch.get(&request(blocked)).err(),
            Some(FetchError::Blocked),
            "HttpFetch: {blocked:?} must be blocked"
        );
    }
}

/// `good` is an https address the fetch can reach (a fake answers it).
pub fn http_fetch(fetch: &dyn HttpFetch, good: &str) {
    let request = |url: &str| FetchRequest {
        url: url.to_owned(),
        secret: None,
        max_bytes: 1 << 20,
    };
    let ok = fetch
        .get(&request(good))
        .expect("HttpFetch: the good address failed");
    assert_eq!(ok.status, 200, "HttpFetch: status");
    http_fetch_refuses_bad_addresses(fetch);
    let tiny = FetchRequest {
        url: good.to_owned(),
        secret: None,
        max_bytes: 1,
    };
    assert_eq!(
        fetch.get(&tiny).err(),
        Some(FetchError::TooLarge),
        "HttpFetch: max_bytes"
    );
}
