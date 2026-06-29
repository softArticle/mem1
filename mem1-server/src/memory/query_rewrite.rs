//! LLM multi-query rewriting (optional, env-gated).
//!
//! When `MEM1_QUERY_REWRITE=openai` is set, the search path expands a single
//! retrieval query into a small set of focused sub-queries via an
//! OpenAI-compatible chat endpoint. Each sub-query is retrieved independently
//! (full keyword+vector+graph RRF) and the runs are fused, so a compound
//! question whose answer is scattered across several facts (the multi-hop
//! recall-miss failure mode) pulls all the relevant facts into the candidate
//! pool instead of only what a single query happens to hit.
//!
//! Defensive by design (mem0 lesson: never trust an LLM to honor a contract):
//! the original query is always included, and any failure (network, bad JSON,
//! empty) degrades to `vec![query]` so search behaves exactly as if rewriting
//! were off.

const MAX_SUBQUERIES: usize = 4;

#[derive(Clone)]
pub struct QueryRewriter {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl QueryRewriter {
    /// Build from environment. Returns `None` when not configured (so search
    /// keeps using the single query unchanged).
    pub fn from_env() -> Option<Self> {
        let provider = std::env::var("MEM1_QUERY_REWRITE").unwrap_or_default();
        if provider != "openai" {
            return None;
        }
        let api_key = std::env::var("MEM1_REWRITE_API_KEY")
            .or_else(|_| std::env::var("MEM1_EXTRACT_API_KEY"))
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .ok()?;
        let base_url = std::env::var("MEM1_REWRITE_BASE_URL")
            .or_else(|_| std::env::var("MEM1_EXTRACT_BASE_URL"))
            .or_else(|_| std::env::var("OPENAI_BASE_URL"))
            .ok()?
            .trim_end_matches('/')
            .to_string();
        let model = std::env::var("MEM1_REWRITE_MODEL")
            .or_else(|_| std::env::var("MEM1_EXTRACT_MODEL"))
            .unwrap_or_else(|_| "gpt-4o-mini".to_string());
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

    /// Expand `query` into focused sub-queries. The original query is always the
    /// first element. On any failure returns `vec![query]` (fail-open).
    pub async fn rewrite(&self, query: &str) -> Vec<String> {
        let fallback = vec![query.to_string()];
        let subs = match self.call(query).await {
            Some(s) => s,
            None => return fallback,
        };
        // Always lead with the original query, then append distinct rewrites.
        let mut out: Vec<String> = vec![query.to_string()];
        let orig_norm = query.trim().to_lowercase();
        for s in subs {
            let t = s.trim();
            if t.is_empty() || t.to_lowercase() == orig_norm {
                continue;
            }
            if out.iter().any(|e| e.trim().eq_ignore_ascii_case(t)) {
                continue;
            }
            out.push(t.to_string());
            if out.len() >= MAX_SUBQUERIES {
                break;
            }
        }
        out
    }

    async fn call(&self, query: &str) -> Option<Vec<String>> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": build_prompt(query)}
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
            tracing::warn!(status = %resp.status(), "query rewrite: non-200, using original query");
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
        let subs = parse_queries(&content);
        if subs.is_empty() {
            None
        } else {
            Some(subs)
        }
    }
}

const SYSTEM_PROMPT: &str = "You rewrite a memory-retrieval query into focused sub-queries. \
Return only a JSON object with a single key \"queries\" whose value is an array of strings.";

fn build_prompt(query: &str) -> String {
    format!(
        "Split the retrieval query below into the SMALLEST set of independent sub-queries, \
each targeting one fact that can be retrieved on its own.\n\n\
Rules:\n\
- A simple, single-fact query needs no split: return it unchanged as one element.\n\
- A compound query (multiple people, events, steps, or \"and\"/\"then\"/\"before\" clauses) \
should become one sub-query per fact.\n\
- Keep concrete details (names, places, dates) verbatim in each sub-query.\n\
- Do not invent facts not implied by the query. At most 4 sub-queries.\n\n\
Return ONLY JSON: {{\"queries\": [\"...\", \"...\"]}}\n\n\
Query: {query}"
    )
}

/// Lenient parse: strip code fences, read `queries[]`; fall back to line-split.
fn parse_queries(content: &str) -> Vec<String> {
    let cleaned = strip_code_fences(content).trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(cleaned) {
        if let Some(arr) = v.get("queries").and_then(|q| q.as_array()) {
            return arr
                .iter()
                .filter_map(|s| s.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    // Fallback: model emitted a plain/bulleted list despite the JSON contract.
    cleaned
        .lines()
        .map(|l| l.trim().trim_start_matches(['-', '*', '•']).trim())
        .map(|l| {
            // strip a leading "1. " / "2) " enumerator
            match l.find(['.', ')']) {
                Some(i) if i <= 2 && l[..i].chars().all(|c| c.is_ascii_digit()) => {
                    l[i + 1..].trim()
                }
                _ => l,
            }
        })
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

fn strip_code_fences(s: &str) -> &str {
    let s = s.trim();
    let s = s
        .strip_prefix("```json")
        .or_else(|| s.strip_prefix("```"))
        .unwrap_or(s);
    s.strip_suffix("```").unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_queries() {
        let out = parse_queries("{\"queries\": [\"where did A go\", \"when did B leave\"]}");
        assert_eq!(out, vec!["where did A go", "when did B leave"]);
    }

    #[test]
    fn parses_fenced_json() {
        let out = parse_queries("```json\n{\"queries\": [\"q1\", \"q2\"]}\n```");
        assert_eq!(out, vec!["q1", "q2"]);
    }

    #[test]
    fn falls_back_to_line_list() {
        let out = parse_queries("1. first query\n2. second query\n- third");
        assert_eq!(out, vec!["first query", "second query", "third"]);
    }

    #[test]
    fn empty_on_garbage() {
        assert!(parse_queries("").is_empty());
    }
}
