use std::collections::HashSet;
use tst::TSTMap;

/// Basic text search map.
/// 
/// # Does 
/// * Tokenizes words on whitespace boundaries
/// * Ignores case
/// * searches prefixes
/// 
/// # Should do
/// * Better tokenization of words wrt punctuation
/// * Fuzzy search (particularly for ¨, ß, etc.)
/// * Weighting of the results based on closeness of fuzzy search
/// * Ordering results by closeness of fuzzy search
/// * Ordering results by importance of stations
pub struct Suggester<T> {
    map: TSTMap<HashSet<T>>,
}

impl<T: std::hash::Hash + Eq + Copy> Suggester<T> {
    pub fn new() -> Suggester<T> {
        Suggester {
            map: TSTMap::new(),
        }
    }

    pub fn insert(&mut self, key: &str, value: T) {
        for word in key.split_whitespace() {
            if word.len() > 3 {
                let v = self.map.entry(&word.to_lowercase()).or_insert(HashSet::new());
                v.insert(value);
            }
        }
    }

    pub fn num_words(&self) -> usize { self.map.len() }

    pub fn prefix_iter(&self, prefix: &str) -> impl Iterator<Item = (String, &HashSet<T>)> {
        self.map.prefix_iter(&prefix.to_lowercase())
    }

    pub fn search(&self, query: &str) -> impl IntoIterator<Item = T> {
        let query: Vec<_> = query.split_whitespace().collect();
        let mut results: HashSet<_> = self.prefix_iter(query[0]).map(|(_, s)| s).flatten().map(|i| *i).collect();
        for part in &query[1..] {
            let previous_results = results;
            results = self.prefix_iter(&part).map(|(_, s)| s).flatten().map(|i| *i).filter(|val| previous_results.contains(val)).collect();
        }
        results
    }
}
