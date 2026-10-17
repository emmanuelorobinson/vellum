//! A token index over icon names and tags: every word of a query must match the start of some token.
use std::collections::BTreeMap;

/// Lower-case words of `text`: runs of letters and digits, so `arrow-right`, `arrow_right` and `Arrow Right` agree.
pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

pub struct Entry {
    pub name: String,
    pub name_words: Vec<String>,
    pub tag_words: Vec<String>,
}

#[derive(Default)]
pub struct Index {
    /// Token to the icons (by position) that have it, as a name word or a tag word.
    tokens: BTreeMap<String, Vec<u32>>,
}

impl Index {
    pub fn build(entries: &[Entry]) -> Self {
        let mut tokens: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for (i, e) in entries.iter().enumerate() {
            for w in e.name_words.iter().chain(&e.tag_words) {
                let list = tokens.entry(w.clone()).or_default();
                if list.last() != Some(&(i as u32)) {
                    list.push(i as u32);
                }
            }
        }
        Self { tokens }
    }

    /// Icons with a token starting with `word`, as sorted positions.
    fn with_prefix(&self, word: &str) -> Vec<u32> {
        let mut found: Vec<u32> = self
            .tokens
            .range(word.to_owned()..)
            .take_while(|(token, _)| token.starts_with(word))
            .flat_map(|(_, list)| list.iter().copied())
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    }

    /// Positions matching every word, best first, at most `limit`.
    pub fn search(&self, entries: &[Entry], query: &str, limit: usize) -> Vec<u32> {
        let query = words(query);
        if limit == 0 {
            return vec![];
        }
        if query.is_empty() {
            let mut all: Vec<u32> = (0..entries.len() as u32).collect();
            all.sort_by(|a, b| entries[*a as usize].name.cmp(&entries[*b as usize].name));
            all.truncate(limit);
            return all;
        }
        let mut hits: Option<Vec<u32>> = None;
        for word in &query {
            let these = self.with_prefix(word);
            hits = Some(match hits {
                None => these,
                Some(so_far) => so_far
                    .into_iter()
                    .filter(|i| these.binary_search(i).is_ok())
                    .collect(),
            });
        }
        let mut hits = hits.unwrap_or_default();
        let key = |i: u32| {
            let e = &entries[i as usize];
            // 0: the name is exactly the query's words. Then by how well each word matches, worst word first.
            let exact = u8::from(e.name_words != query);
            let worst = query
                .iter()
                .map(|w| {
                    if e.name_words.iter().any(|n| n == w) {
                        0u8
                    } else if e.name_words.iter().any(|n| n.starts_with(w.as_str())) {
                        1
                    } else if e.tag_words.iter().any(|t| t == w) {
                        2
                    } else {
                        3
                    }
                })
                .max()
                .unwrap_or(0);
            (exact, worst, e.name_words.len(), e.name.clone())
        };
        hits.sort_by_cached_key(|i| key(*i));
        hits.truncate(limit);
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, tags: &[&str]) -> Entry {
        Entry {
            name: name.into(),
            name_words: words(name),
            tag_words: tags.iter().flat_map(|t| words(t)).collect(),
        }
    }

    #[test]
    fn words_ignore_case_and_separators() {
        assert_eq!(words("Arrow_right-2 Up"), ["arrow", "right", "2", "up"]);
        assert!(words("---").is_empty());
    }

    #[test]
    fn exact_names_then_name_matches_then_tags() {
        let entries = vec![
            entry("home", &["house"]),
            entry("house", &["home", "building"]),
            entry("house-plus", &[]),
            entry("warehouse", &["house"]),
            entry("hot-tub", &[]),
        ];
        let index = Index::build(&entries);
        let names = |q: &str| {
            index
                .search(&entries, q, 10)
                .into_iter()
                .map(|i| entries[i as usize].name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names("house"), ["house", "house-plus", "home", "warehouse"]);
        assert_eq!(names("hous"), ["house", "house-plus", "home", "warehouse"]);
        assert_eq!(names("house plus"), ["house-plus"]);
        assert_eq!(names("HOUSE-PLUS"), ["house-plus"]);
        assert_eq!(names("zzz"), Vec::<String>::new());
        assert_eq!(index.search(&entries, "house", 2).len(), 2);
        assert_eq!(names("")[0], "home");
    }
}
