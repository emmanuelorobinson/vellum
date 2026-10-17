use std::collections::BTreeMap;

use serde_json::Value;
use vellum_core::ports::{IconGlyph, IconInfo, IconLibrary, IconStroke, IconStyle};

use crate::search::{Entry, Index, words};

/// How one icon is drawn.
#[derive(Debug, Clone, PartialEq)]
pub enum Art {
    /// One drawing at every weight (stroke sets): the weight changes the line.
    One(String),
    /// A drawing per weight, and optionally a filled one.
    Weighted {
        by_weight: BTreeMap<u16, String>,
        fill: Option<String>,
    },
}

/// One icon as a set is made from: a name, words that find it, and its art.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub name: String,
    pub tags: Vec<String>,
    pub art: Art,
}

/// What a set is.
#[derive(Debug, Clone, PartialEq)]
pub struct SetInfo {
    pub id: String,
    pub label: String,
    pub style: IconStyle,
    pub view_box: [f64; 4],
    pub weights: Vec<u16>,
    pub fill: bool,
}

pub struct Library {
    info: SetInfo,
    records: Vec<Record>,
    entries: Vec<Entry>,
    index: Index,
    by_name: BTreeMap<String, usize>,
}

impl Library {
    pub fn from_records(info: SetInfo, mut records: Vec<Record>) -> Self {
        records.sort_by(|a, b| a.name.cmp(&b.name));
        records.dedup_by(|a, b| a.name == b.name);
        let entries: Vec<Entry> = records
            .iter()
            .map(|r| Entry {
                name: r.name.clone(),
                name_words: words(&r.name),
                tag_words: r.tags.iter().flat_map(|t| words(t)).collect(),
            })
            .collect();
        let index = Index::build(&entries);
        let by_name = records
            .iter()
            .enumerate()
            .map(|(i, r)| (r.name.clone(), i))
            .collect();
        Self {
            info,
            records,
            entries,
            index,
            by_name,
        }
    }

    /// Reads a generated data file. `Err` names what is wrong; the shipped files never are.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let split = text
            .find(",\"icons\":[")
            .ok_or("the icon file has no icons list")?;
        let head: Value =
            serde_json::from_str(&format!("{}}}", &text[..split])).map_err(|e| e.to_string())?;
        let all: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let s = |k: &str| {
            head.get(k)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(format!("missing {k}"))
        };
        let view_box = head
            .get("viewBox")
            .and_then(Value::as_array)
            .filter(|v| v.len() == 4)
            .map(|v| {
                [
                    v[0].as_f64().unwrap_or(0.),
                    v[1].as_f64().unwrap_or(0.),
                    v[2].as_f64().unwrap_or(0.),
                    v[3].as_f64().unwrap_or(0.),
                ]
            })
            .ok_or("missing viewBox")?;
        let info = SetInfo {
            id: s("id")?,
            label: s("label")?,
            style: if s("style")? == "stroke" {
                IconStyle::Stroke
            } else {
                IconStyle::Fill
            },
            view_box,
            weights: head
                .get("weights")
                .and_then(Value::as_array)
                .map(|w| {
                    w.iter()
                        .filter_map(|w| w.as_u64().map(|w| w as u16))
                        .collect()
                })
                .unwrap_or_default(),
            fill: head.get("fill").and_then(Value::as_bool).unwrap_or(false),
        };
        let mut records = Vec::new();
        for icon in all
            .get("icons")
            .and_then(Value::as_array)
            .ok_or("no icons")?
        {
            let [name, tags, art] = icon
                .as_array()
                .map(Vec::as_slice)
                .ok_or("an icon is not a list")?
            else {
                return Err("an icon is not [name, tags, art]".into());
            };
            let art = match art {
                Value::String(d) => Art::One(d.clone()),
                Value::Object(map) => {
                    let mut by_weight = BTreeMap::new();
                    let mut fill = None;
                    for (key, d) in map {
                        let d = d.as_str().ok_or("art is text")?.to_owned();
                        match key.parse::<u16>() {
                            Ok(w) => {
                                by_weight.insert(w, d);
                            }
                            Err(_) if key == "fill" => fill = Some(d),
                            Err(_) => return Err(format!("unknown variant {key}")),
                        }
                    }
                    Art::Weighted { by_weight, fill }
                }
                _ => return Err("art is text or an object".into()),
            };
            records.push(Record {
                name: name.as_str().ok_or("a name is text")?.to_owned(),
                tags: tags
                    .as_array()
                    .map(|t| {
                        t.iter()
                            .filter_map(|t| t.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
                art,
            });
        }
        Ok(Self::from_records(info, records))
    }

    pub fn info(&self) -> &SetInfo {
        &self.info
    }
}

fn nearest(weights: &[u16], want: u16) -> u16 {
    weights
        .iter()
        .copied()
        .min_by_key(|w| w.abs_diff(want))
        .unwrap_or(400)
}

impl IconLibrary for Library {
    fn id(&self) -> &str {
        &self.info.id
    }
    fn label(&self) -> &str {
        &self.info.label
    }
    fn style(&self) -> IconStyle {
        self.info.style
    }
    fn weights(&self) -> Vec<u16> {
        self.info.weights.clone()
    }
    fn supports_fill(&self) -> bool {
        self.info.fill
    }
    fn len(&self) -> usize {
        self.records.len()
    }

    fn search(&self, query: &str, limit: usize) -> Vec<IconInfo> {
        self.index
            .search(&self.entries, query, limit)
            .into_iter()
            .map(|i| {
                let r = &self.records[i as usize];
                IconInfo {
                    name: r.name.clone(),
                    tags: r.tags.clone(),
                }
            })
            .collect()
    }

    fn glyph(&self, name: &str, weight: u16, fill: bool) -> Option<IconGlyph> {
        let record = &self.records[*self.by_name.get(name)?];
        let (path, weight, filled) = match &record.art {
            Art::One(d) => (d.clone(), nearest(&self.info.weights, weight), false),
            Art::Weighted {
                by_weight,
                fill: filled,
            } => match filled {
                Some(d) if fill => (
                    d.clone(),
                    nearest(&by_weight.keys().copied().collect::<Vec<_>>(), weight),
                    true,
                ),
                _ => {
                    let have: Vec<u16> = by_weight.keys().copied().collect();
                    let w = nearest(&have, weight);
                    (by_weight.get(&w)?.clone(), w, false)
                }
            },
        };
        let stroke = (self.info.style == IconStyle::Stroke).then(|| IconStroke {
            width: f64::from(weight) / 200.,
            round: true,
        });
        Some(IconGlyph {
            library: self.info.id.clone(),
            name: record.name.clone(),
            view_box: self.info.view_box,
            path,
            style: self.info.style,
            weight,
            fill: filled,
            stroke,
        })
    }
}
