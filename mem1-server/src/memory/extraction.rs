pub const EXTRACTOR_VERSION: &str = "rule-v2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceText {
    pub text: String,
    pub role: String,
    pub index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractedFact {
    pub content: String,
    pub source_text: String,
    pub source_role: String,
    pub source_index: usize,
    pub language: String,
}

pub fn extract_facts(sources: &[SourceText]) -> Vec<ExtractedFact> {
    sources
        .iter()
        .flat_map(|source| {
            let source_text = normalize_whitespace(&source.text);
            split_fact_candidates(&source_text)
                .into_iter()
                .filter(|candidate| is_fact_like(candidate))
                .map(move |content| ExtractedFact {
                    language: detect_language(&content).to_string(),
                    content,
                    source_text: source_text.clone(),
                    source_role: normalized_role(&source.role),
                    source_index: source.index,
                })
        })
        .collect()
}

pub fn detect_language(text: &str) -> &'static str {
    if text.chars().any(is_cjk) {
        "zh"
    } else if text.chars().any(|c| c.is_ascii_alphabetic()) {
        "en"
    } else {
        "unknown"
    }
}

fn normalized_role(role: &str) -> String {
    let role = role.trim();
    if role.is_empty() {
        "message".to_string()
    } else {
        role.to_string()
    }
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_fact_candidates(text: &str) -> Vec<String> {
    // rule-v2: keep the whole message as one context-rich fact instead of
    // splitting on sentence punctuation. Sentence-level splitting strips
    // referents (e.g. "It taught me ..." loses its subject), hurting retrieval
    // precision. One memory per message preserves self-contained context.
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if ch == '\n' {
            push_candidate(&mut out, &mut current);
        }
    }
    push_candidate(&mut out, &mut current);
    out
}

fn push_candidate(out: &mut Vec<String>, current: &mut String) {
    let candidate = normalize_whitespace(current);
    if !candidate.is_empty() {
        out.push(candidate);
    }
    current.clear();
}

fn is_fact_like(text: &str) -> bool {
    text.chars().any(|c| c.is_alphanumeric())
}

fn is_cjk(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch)
        || ('\u{3400}'..='\u{4dbf}').contains(&ch)
        || ('\u{3040}'..='\u{30ff}').contains(&ch)
        || ('\u{ac00}'..='\u{d7af}').contains(&ch)
}

#[cfg(test)]
mod tests {
    use super::{extract_facts, SourceText, EXTRACTOR_VERSION};

    #[test]
    fn extract_facts_keeps_whole_message_as_one_fact() {
        let facts = extract_facts(&[SourceText {
            text: " Alice likes Rust. Alice lives in Paris. ".to_string(),
            role: "content".to_string(),
            index: 0,
        }]);

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].content, "Alice likes Rust. Alice lives in Paris.");
        assert_eq!(
            facts[0].source_text,
            "Alice likes Rust. Alice lives in Paris."
        );
        assert_eq!(facts[0].source_role, "content");
        assert_eq!(facts[0].source_index, 0);
        assert_eq!(facts[0].language, "en");
        assert_eq!(EXTRACTOR_VERSION, "rule-v2");
    }

    #[test]
    fn extract_facts_keeps_message_source_role_and_index() {
        let facts = extract_facts(&[
            SourceText {
                text: "I prefer tea.".to_string(),
                role: "user".to_string(),
                index: 0,
            },
            SourceText {
                text: "Noted.".to_string(),
                role: "assistant".to_string(),
                index: 1,
            },
        ]);

        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].source_role, "user");
        assert_eq!(facts[0].source_index, 0);
        assert_eq!(facts[1].source_role, "assistant");
        assert_eq!(facts[1].source_index, 1);
        assert_eq!(facts[1].source_text, "Noted.");
    }

    #[test]
    fn extract_facts_returns_empty_for_punctuation_only_input() {
        let facts = extract_facts(&[SourceText {
            text: " ... ".to_string(),
            role: "content".to_string(),
            index: 0,
        }]);

        assert!(facts.is_empty());
    }
}
