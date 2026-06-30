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

/// Detect the dominant language of `text` and return a short code (en/zh/ja/
/// ko/...). Uses `whatlang` (n-gram statistical identification, ~100 languages)
/// instead of a hardcoded character-range heuristic — the old version mislabeled
/// Japanese/Korean as "zh" because their scripts fall in the CJK range it
/// checked. A fast ASCII path short-circuits the common English case, and a
/// script-based fallback covers very short inputs where whatlang abstains.
pub fn detect_language(text: &str) -> &'static str {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "unknown";
    }
    // Fast path: pure-ASCII alphabetic text is overwhelmingly English here, and
    // whatlang is unreliable on short ASCII snippets.
    let has_alpha = trimmed.chars().any(|c| c.is_alphabetic());
    if has_alpha && trimmed.is_ascii() {
        return "en";
    }
    if let Some(info) = whatlang::detect(trimmed) {
        if let Some(code) = lang_short_code(info.lang()) {
            return code;
        }
    }
    // Fallback for inputs whatlang abstains on (e.g. a single CJK word): classify
    // by script so we still distinguish the main CJK languages.
    script_fallback(trimmed)
}

fn lang_short_code(lang: whatlang::Lang) -> Option<&'static str> {
    use whatlang::Lang;
    Some(match lang {
        Lang::Eng => "en",
        Lang::Cmn => "zh",
        Lang::Jpn => "ja",
        Lang::Kor => "ko",
        Lang::Spa => "es",
        Lang::Fra => "fr",
        Lang::Deu => "de",
        Lang::Rus => "ru",
        Lang::Por => "pt",
        Lang::Ita => "it",
        Lang::Ara => "ar",
        Lang::Hin => "hi",
        _ => return None,
    })
}

fn script_fallback(text: &str) -> &'static str {
    if text.chars().any(is_hiragana_katakana) {
        "ja"
    } else if text.chars().any(is_hangul) {
        "ko"
    } else if text.chars().any(is_han) {
        "zh"
    } else if text.chars().any(|c| c.is_alphabetic()) {
        // Some non-Latin alphabetic script whatlang didn't map; "unknown" beats
        // a wrong guess.
        "unknown"
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
    // rule-v2 (ported from the main line): keep the whole message as one
    // context-rich fact instead of splitting on sentence punctuation. Sentence
    // splitting strips referents (e.g. "It taught me ..." loses its subject),
    // hurting retrieval precision. One memory per message preserves self-contained
    // context. (When MEM1_EXTRACT_PROVIDER=openai, the LLM extractor supersedes this.)
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

fn is_han(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch) || ('\u{3400}'..='\u{4dbf}').contains(&ch)
}

fn is_hiragana_katakana(ch: char) -> bool {
    ('\u{3040}'..='\u{30ff}').contains(&ch)
}

fn is_hangul(ch: char) -> bool {
    ('\u{ac00}'..='\u{d7af}').contains(&ch)
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

    #[test]
    fn detect_language_distinguishes_cjk_languages() {
        use super::detect_language;
        // The old heuristic returned "zh" for all of these (CJK char range).
        assert_eq!(
            detect_language("Alice lives in Paris and likes Rust."),
            "en"
        );
        assert_eq!(detect_language("张伟住在北京，喜欢喝茶。"), "zh");
        assert_eq!(
            detect_language("田中さんは東京に住んでいて、コーヒーが好きです。"),
            "ja"
        );
        assert_eq!(
            detect_language("앨리스는 파리에 살고 러스트를 좋아합니다."),
            "ko"
        );
    }

    #[test]
    fn detect_language_short_cjk_word_falls_back_by_script() {
        use super::detect_language;
        // whatlang abstains on very short inputs; the script fallback still
        // separates the CJK languages instead of lumping them as "zh".
        assert_eq!(detect_language("カタカナ"), "ja");
        assert_eq!(detect_language("한글"), "ko");
        assert_eq!(detect_language("中文"), "zh");
    }

    #[test]
    fn detect_language_empty_is_unknown() {
        use super::detect_language;
        assert_eq!(detect_language("   "), "unknown");
        assert_eq!(detect_language("123 !!!"), "unknown");
    }
}
