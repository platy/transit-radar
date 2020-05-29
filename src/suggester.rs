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
    exact: TSTMap<HashSet<T>>,
    lowercase_words: TSTMap<HashSet<T>>,
}

impl<T: std::hash::Hash + Eq + Copy> Suggester<T> {
    pub fn new() -> Suggester<T> {
        Suggester {
            lowercase_words: TSTMap::new(),
            exact: TSTMap::new(),
        }
    }

    pub fn insert(&mut self, key: &str, value: T) {
        let v = self
            .exact
            .entry(key)
            .or_insert(HashSet::new());
        v.insert(value);

        for word in key.split_whitespace() {
            if word.len() > 2 {
                let v = self
                    .lowercase_words
                    .entry(&word.to_lowercase())
                    .or_insert(HashSet::new());
                v.insert(value);
            }
        }
    }

    pub fn num_words(&self) -> usize {
        self.lowercase_words.len()
    }

    pub fn prefix_iter(&self, prefix: &str) -> impl Iterator<Item = (String, &HashSet<T>)> {
        self.lowercase_words.prefix_iter(&prefix.to_lowercase())
    }

    pub fn search(&self, query: &str) -> impl IntoIterator<Item = T> {
        if let Some(results) = self.exact.get(query) {
            return results.clone();
        }
        let query: Vec<_> = query.split_whitespace().collect();
        let mut results: Option<HashSet<T>> = None;
        for part in query {
            let filter: Box<dyn Fn(&T) -> bool> = if let Some(results) = results {
                let previous_results = results;
                Box::new(move |val| previous_results.contains(val))
            } else {
                Box::new(|_| true)
            };
            results = Some(
                self.prefix_iter(&part)
                    .map(|(_, s)| s)
                    .flatten()
                    .map(|i| *i)
                    .filter(filter)
                    .collect(),
            );
        }
        results.unwrap_or_default()
    }
}

#[cfg(test)]
mod test {
    use super::Suggester;
    use std::collections::HashSet;

    fn suggester() -> Suggester<u32> {
        let mut suggester = Suggester::new();
        suggester.insert("Foo Bar", 1);
        suggester.insert("Foo Baz", 2);
        suggester.insert("Bar Baz", 3);
        suggester.insert("bar baz", 4);
        suggester
    }

    fn assert_search_results<'i>(query: &str, expected: impl IntoIterator<Item = &'i u32>) {
        let results: HashSet<_> = suggester().search(query).into_iter().collect();
        assert_eq!(results, expected.into_iter().copied().collect());
    }

    #[test]
    fn word_count() {
        let suggester = suggester();
        assert_eq!(suggester.num_words(), 3);
    }

    #[test]
    fn exact_match_case_sensitive() {
        assert_search_results("Bar Baz", &[3]);
        assert_search_results("bar baz", &[4]);
    }

    #[test]
    fn one_word_matchcase() {
        assert_search_results("Foo", &[1, 2]);
    }

    #[test]
    fn two_word_matchcase() {
        assert_search_results("Foo Bar", &[1]);
    }

    #[test]
    fn one_word_offcase() {
        assert_search_results("foo", &[1, 2]);
    }

    #[test]
    fn two_word_offcase() {
        assert_search_results("foo bar", &[1]);
    }
}
