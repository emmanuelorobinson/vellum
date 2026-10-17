use std::collections::BTreeMap;

use serde_json::json;
use vellum_core::ports::*;

/// An importer for the text `memory:<name>` (a rectangle) and `memory:<name>:image` (an image with an asset).
pub struct MemoryImporter;

impl Importer for MemoryImporter {
    fn id(&self) -> &'static str {
        "memory"
    }
    fn recognizes(&self, input: &ImportInput<'_>) -> bool {
        input.bytes.starts_with(b"memory:")
    }
    fn import(&self, input: &ImportInput<'_>) -> Result<Imported, ImportError> {
        let text = std::str::from_utf8(input.bytes).map_err(|_| ImportError::new("not text"))?;
        let rest = text
            .strip_prefix("memory:")
            .ok_or_else(|| ImportError::new("not a memory file"))?;
        let (name, image) = match rest.strip_suffix(":image") {
            Some(name) => (name, true),
            None => (rest, false),
        };
        if name.is_empty() {
            return Err(ImportError::new("the memory file has no name"));
        }
        if image {
            Ok(Imported {
                nodes: vec![
                    json!({"type":"image","name":name,"width":4,"height":4,"asset":asset_ref(0)}),
                ],
                assets: vec![ImportedAsset {
                    mime: "image/png".into(),
                    bytes: vec![1, 2, 3],
                }],
                notes: vec![],
            })
        } else {
            Ok(Imported {
                nodes: vec![json!({"type":"rect","name":name,"width":4,"height":4})],
                ..Imported::default()
            })
        }
    }
}

/// Six icons in two styles of use: stroke or fill, with or without a filled variant.
pub struct MemoryIcons {
    id: String,
    style: IconStyle,
    fill: bool,
    icons: Vec<(String, Vec<String>)>,
}

impl MemoryIcons {
    pub fn new(id: &str, style: IconStyle, fill: bool) -> Self {
        let icons = [
            ("arrow-right", vec!["direction", "next"]),
            ("arrow-left", vec!["direction", "back"]),
            ("house", vec!["home", "building"]),
            ("star", vec!["favourite", "rating"]),
            ("trash", vec!["delete", "bin"]),
            ("user", vec!["person", "account"]),
        ];
        Self {
            id: id.to_owned(),
            style,
            fill,
            icons: icons
                .into_iter()
                .map(|(n, t)| (n.to_owned(), t.into_iter().map(str::to_owned).collect()))
                .collect(),
        }
    }
}

impl IconLibrary for MemoryIcons {
    fn id(&self) -> &str {
        &self.id
    }
    fn label(&self) -> &str {
        "Memory icons"
    }
    fn style(&self) -> IconStyle {
        self.style
    }
    fn weights(&self) -> Vec<u16> {
        vec![200, 400, 600]
    }
    fn supports_fill(&self) -> bool {
        self.fill
    }
    fn len(&self) -> usize {
        self.icons.len()
    }
    fn search(&self, query: &str, limit: usize) -> Vec<IconInfo> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut hits: Vec<(u8, &(String, Vec<String>))> = self
            .icons
            .iter()
            .filter_map(|icon| {
                let mut best = u8::MAX;
                for word in &words {
                    let rank = if icon.0 == *word {
                        0
                    } else if icon.0.split('-').any(|p| p == word) || icon.1.contains(word) {
                        1
                    } else if icon.0.contains(word.as_str()) {
                        2
                    } else {
                        return None;
                    };
                    best = if best == u8::MAX {
                        rank
                    } else {
                        best.max(rank)
                    };
                }
                Some((if words.is_empty() { 0 } else { best }, icon))
            })
            .collect();
        hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.0.cmp(&b.1.0)));
        hits.into_iter()
            .take(limit)
            .map(|(_, (name, tags))| IconInfo {
                name: name.clone(),
                tags: tags.clone(),
            })
            .collect()
    }
    fn glyph(&self, name: &str, weight: u16, fill: bool) -> Option<IconGlyph> {
        self.icons.iter().find(|(n, _)| n == name)?;
        let weights = self.weights();
        let weight = *weights
            .iter()
            .min_by_key(|w| w.abs_diff(weight))
            .expect("not empty");
        Some(IconGlyph {
            library: self.id.clone(),
            name: name.to_owned(),
            view_box: [0., 0., 24., 24.],
            path: format!("M0 0 L24 {}", name.len()),
            style: self.style,
            weight,
            fill: fill && self.fill,
            stroke: (self.style == IconStyle::Stroke).then_some(IconStroke {
                width: f64::from(weight) / 200.,
                round: true,
            }),
        })
    }
}

/// An image source over a fixed list of pictures.
pub struct MemoryImages;

fn attribution() -> Attribution {
    Attribution {
        provider: "memory".into(),
        author: "TEST DATA Sam".into(),
        author_url: "https://example.com/sam".into(),
        source_url: "https://example.com/pictures/7".into(),
        licence: "Test licence".into(),
        licence_url: "https://example.com/licence".into(),
    }
}

impl ImageSource for MemoryImages {
    fn id(&self) -> &'static str {
        "memory"
    }
    fn origin(&self) -> &'static str {
        "https://example.com"
    }
    fn search(&self, query: &ImageQuery) -> Result<ImagePage, ImageError> {
        if query.query.trim().is_empty() {
            return Err(ImageError::Rejected("the query is blank".into()));
        }
        let hits: Vec<ImageHit> = (0..5)
            .map(|i| ImageHit {
                id: format!("{}", 7 + i),
                description: format!("TEST DATA picture of {}", query.query),
                width: 640,
                height: 480,
                preview_url: format!("https://example.com/p/{i}-small.jpg"),
                download_url: format!("https://example.com/p/{i}.jpg"),
                attribution: attribution(),
            })
            .take(query.per_page as usize)
            .collect();
        Ok(ImagePage { hits, total: 5 })
    }
    fn fetch(&self, hit: &ImageHit) -> Result<FetchedImage, ImageError> {
        Ok(FetchedImage {
            mime: "image/jpeg".into(),
            bytes: hit.id.clone().into_bytes(),
            attribution: hit.attribution.clone(),
        })
    }
}

/// An HTTP client that answers from a table and applies the address rules every real one must.
#[derive(Default)]
pub struct MemoryFetch {
    pages: BTreeMap<String, FetchResponse>,
}

impl MemoryFetch {
    pub fn with(mut self, url: &str, content_type: &str, body: &[u8]) -> Self {
        self.pages.insert(
            url.to_owned(),
            FetchResponse {
                status: 200,
                content_type: content_type.to_owned(),
                body: body.to_vec(),
            },
        );
        self
    }
}

impl HttpFetch for MemoryFetch {
    fn get(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        let url = request.url.as_str();
        let rest = url.strip_prefix("https://").ok_or(FetchError::Blocked)?;
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        if authority.contains('@')
            || authority.is_empty()
            || authority.starts_with('[')
            || authority.starts_with("localhost")
            || authority.starts_with("127.")
            || authority.starts_with("192.168.")
            || authority.starts_with("10.")
            || authority.starts_with("169.254.")
            || authority.starts_with("172.16.")
        {
            return Err(FetchError::Blocked);
        }
        let page = self.pages.get(url).ok_or(FetchError::Network)?;
        if page.body.len() > request.max_bytes {
            return Err(FetchError::TooLarge);
        }
        Ok(page.clone())
    }
}
