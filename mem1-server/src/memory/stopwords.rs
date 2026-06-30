//! Stop-word filtering for retrieval, backed by the `stop-words` crate instead
//! of a hand-maintained English array.
//!
//! Two near-duplicate hardcoded English lists used to live in storage/memory.rs
//! and api/handlers.rs. They were English-only and drifted apart. This module
//! centralizes them: the word list comes from the `stop-words` crate (NLTK list,
//! ~100 languages) and is cached in a `HashSet` for O(1) lookup. The language is
//! selectable via `MEM1_STOPWORD_LANG` (ISO code or English name, default
//! "english") so non-English deployments can switch without code changes.

use std::collections::HashSet;
use std::sync::OnceLock;

static STOPWORDS: OnceLock<HashSet<String>> = OnceLock::new();

fn load() -> HashSet<String> {
    // Default to English via the typed enum (never panics). `MEM1_STOPWORD_LANG`,
    // when set, must be an ISO 639-1 code the `stop-words` crate recognizes
    // (e.g. "en", "zh", "fr", "de", "ja") — an unrecognized value panics at
    // startup, which is the right place to surface a misconfiguration.
    let list: &'static [&'static str] = match std::env::var("MEM1_STOPWORD_LANG") {
        Ok(code) if !code.trim().is_empty() => stop_words::get(code.trim()),
        _ => stop_words::get(stop_words::LANGUAGE::English),
    };
    list.iter().map(|w| w.to_string()).collect()
}

/// True if `word` (already lowercased by the caller) is a stop word in the
/// configured language. Words shorter than 2 chars are not stop words here —
/// callers apply their own length thresholds.
pub fn is_stopword(word: &str) -> bool {
    STOPWORDS.get_or_init(load).contains(word)
}

#[cfg(test)]
mod tests {
    use super::is_stopword;

    #[test]
    fn common_english_function_words_are_stopwords() {
        for w in ["the", "and", "this", "that", "what", "where", "with"] {
            assert!(is_stopword(w), "{w} should be a stopword");
        }
    }

    #[test]
    fn content_words_are_not_stopwords() {
        for w in ["paris", "rust", "camped", "melanie"] {
            assert!(!is_stopword(w), "{w} should not be a stopword");
        }
    }
}
