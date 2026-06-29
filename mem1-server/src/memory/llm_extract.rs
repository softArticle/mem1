//! LLM-based fact extraction (optional, env-gated).
//!
//! When `MEM1_EXTRACT_PROVIDER=openai` is set, the add path rewrites raw
//! conversation turns into normalized, self-contained atomic facts via an
//! OpenAI-compatible chat endpoint — resolving pronouns, attributing the
//! speaker, and dropping contentless chit-chat. This addresses the recall
//! ceiling of the deterministic `rule-v1` splitter: filler like "Wow!" no
//! longer becomes a standalone memory, and a fact's wording is normalized so
//! a query and the stored memory land in the same lexical/semantic space.
//!
//! Defensive by design (mem0 lesson: never trust an LLM to honor a contract):
//! the response is parsed leniently and any failure degrades to `None` so the
//! caller falls back to rule-based extraction rather than dropping the write.

use crate::memory::extraction::{detect_language, ExtractedFact, SourceText};

pub const EXTRACTOR_VERSION: &str = "llm-v1";

#[derive(Clone)]
pub struct LlmExtractor {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl LlmExtractor {
    /// Build from environment. Returns `None` when extraction is not configured
    /// (so the caller keeps using rule-based extraction).
    pub fn from_env() -> Option<Self> {
        let provider = std::env::var("MEM1_EXTRACT_PROVIDER").unwrap_or_default();
        if provider != "openai" {
            return None;
        }
        let api_key = std::env::var("MEM1_EXTRACT_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .ok()?;
        let base_url = std::env::var("MEM1_EXTRACT_BASE_URL")
            .or_else(|_| std::env::var("OPENAI_BASE_URL"))
            .ok()?
            .trim_end_matches('/')
            .to_string();
        let model =
            std::env::var("MEM1_EXTRACT_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        // Bound each extraction call: a slow turn degrades to rule-based extraction
        // rather than stalling the whole add pipeline (mem0 lesson: match timeouts
        // to reality and keep auxiliary enrichment fail-open).
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(90))
            .build()
            .ok()?;
        Some(Self {
            api_key,
            base_url,
            model,
            client,
        })
    }

    /// Extract normalized atomic facts from the conversation turns. On any error
    /// (network, bad JSON, empty), returns `None` so the caller falls back.
    pub async fn extract(&self, sources: &[SourceText]) -> Option<Vec<ExtractedFact>> {
        if sources.is_empty() {
            return None;
        }
        let convo = render_conversation(sources);
        let prompt = build_prompt(&convo);

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": prompt}
            ]
        });

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "llm extract: non-200, falling back");
            return None;
        }
        let data: serde_json::Value = resp.json().await.ok()?;
        let content = data
            .get("choices")?
            .get(0)?
            .get("message")?
            .get("content")?
            .as_str()?
            .to_string();

        let facts = parse_facts(&content);
        if facts.is_empty() {
            return None;
        }

        // Map each normalized fact back to a representative source turn so role
        // attribution and source_index stay meaningful (used by graph/context).
        let primary = &sources[0];
        let source_text = sources
            .iter()
            .map(|s| s.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        Some(
            facts
                .into_iter()
                .map(|content| ExtractedFact {
                    language: detect_language(&content).to_string(),
                    content,
                    source_text: source_text.clone(),
                    source_role: normalized_role(&primary.role),
                    source_index: primary.index,
                })
                .collect(),
        )
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

fn render_conversation(sources: &[SourceText]) -> String {
    sources
        .iter()
        .map(|s| {
            let role = s.role.trim();
            let text = s.text.trim();
            if role.is_empty() {
                text.to_string()
            } else {
                format!("{role}: {text}")
            }
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

const SYSTEM_PROMPT: &str = "You extract durable memories from conversation turns. \
Return only a JSON object with a single key \"facts\" whose value is an array of strings.";

fn build_prompt(conversation: &str) -> String {
    format!(
        "From the conversation below, extract the durable, self-contained facts worth \
remembering about the speakers (preferences, relationships, events, plans, attributes, dates).\n\n\
Rules:\n\
- Resolve pronouns and references; each fact must stand alone without the surrounding turns.\n\
- Attribute the fact to the named speaker when known (e.g. \"Caroline moved from Sweden\").\n\
- Keep concrete details (names, places, dates, numbers) verbatim.\n\
- Omit pure chit-chat, greetings, acknowledgements, and opinions with no durable content.\n\
- Prefer several short atomic facts over one compound sentence.\n\
- If nothing is worth remembering, return an empty array.\n\n\
Return ONLY JSON: {{\"facts\": [\"...\", \"...\"]}}\n\n\
Conversation:\n{conversation}"
    )
}

/// Lenient parse: strip code fences, find the JSON object, read `facts[]`.
/// Falls back to line-splitting if the model ignored the JSON contract.
fn parse_facts(content: &str) -> Vec<String> {
    let cleaned = strip_code_fences(content);
    if let Some(facts) = parse_json_facts(cleaned) {
        return facts;
    }
    // Fallback: some models emit a bullet/line list despite instructions.
    cleaned
        .lines()
        .map(|l| l.trim().trim_start_matches(['-', '*', '•']).trim())
        .filter(|l| l.len() > 2 && !l.starts_with('{') && !l.starts_with('}'))
        .map(|l| l.trim_matches('"').to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn strip_code_fences(s: &str) -> &str {
    let s = s.trim();
    let s = s
        .strip_prefix("```json")
        .or_else(|| s.strip_prefix("```"))
        .unwrap_or(s);
    s.strip_suffix("```").unwrap_or(s).trim()
}

fn parse_json_facts(s: &str) -> Option<Vec<String>> {
    // Locate the outermost JSON object even if surrounded by prose.
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end <= start {
        return None;
    }
    let obj: serde_json::Value = serde_json::from_str(&s[start..=end]).ok()?;
    let arr = obj.get("facts")?.as_array()?;
    let facts: Vec<String> = arr
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some(facts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_facts() {
        let out = parse_facts("{\"facts\": [\"Caroline is from Sweden\", \"Caroline is single\"]}");
        assert_eq!(out, vec!["Caroline is from Sweden", "Caroline is single"]);
    }

    #[test]
    fn parses_fenced_json() {
        let out = parse_facts("```json\n{\"facts\": [\"Melanie has kids\"]}\n```");
        assert_eq!(out, vec!["Melanie has kids"]);
    }

    #[test]
    fn parses_json_with_prose_around() {
        let out = parse_facts("Here are the facts:\n{\"facts\": [\"A likes tea\"]}\nDone.");
        assert_eq!(out, vec!["A likes tea"]);
    }

    #[test]
    fn falls_back_to_line_list() {
        let out = parse_facts("- Caroline moved from Sweden\n- She has a necklace");
        assert_eq!(
            out,
            vec!["Caroline moved from Sweden", "She has a necklace"]
        );
    }

    #[test]
    fn empty_facts_array() {
        assert!(parse_facts("{\"facts\": []}").is_empty());
    }
}
