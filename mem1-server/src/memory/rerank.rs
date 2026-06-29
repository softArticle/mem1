//! LLM listwise reranker (optional, env-gated).
//!
//! Implements a RankGPT-style listwise permutation reranker (arXiv:2304.09542):
//! the candidate memories are numbered [1..N], the LLM is asked to emit a strict
//! relevance-ordered permutation like "[3] > [1] > [5] ...", and we reorder by
//! that permutation. Used to re-sort an over-fetched candidate pool so the most
//! query-relevant facts rise into the top-k window the answering LLM actually
//! sees — the failure mode where a relevant fact is retrieved but ranked just
//! outside the answer context.
//!
//! Defensive (mem0 lesson: never trust an LLM to honor a contract): a malformed
//! or partial permutation degrades gracefully — any IDs the model omits keep
//! their original relative order appended after the ranked ones, and on any
//! transport/parse failure the original order is returned unchanged.

pub struct LlmReranker {
    provider: RerankProvider,
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Clone, Copy, PartialEq)]
enum RerankProvider {
    /// RankGPT-style listwise permutation via an OpenAI-compatible chat endpoint.
    OpenAiListwise,
}

impl LlmReranker {
    pub fn from_env() -> Option<Self> {
        // `crossencoder` is served by the embedded LocalCrossEncoder (tract,
        // in-process) — see local_rerank.rs. This HTTP/LLM reranker only handles
        // the listwise `openai` provider.
        let provider = match std::env::var("MEM1_RERANK_PROVIDER").unwrap_or_default().as_str() {
            "openai" => RerankProvider::OpenAiListwise,
            _ => return None,
        };
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(90))
            .build()
            .ok()?;
        let api_key = std::env::var("MEM1_RERANK_API_KEY")
            .or_else(|_| std::env::var("MEM1_EXTRACT_API_KEY"))
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .ok()?;
        let base_url = std::env::var("MEM1_RERANK_BASE_URL")
            .or_else(|_| std::env::var("MEM1_EXTRACT_BASE_URL"))
            .or_else(|_| std::env::var("OPENAI_BASE_URL"))
            .ok()?
            .trim_end_matches('/')
            .to_string();
        let model = std::env::var("MEM1_RERANK_MODEL")
            .or_else(|_| std::env::var("MEM1_EXTRACT_MODEL"))
            .unwrap_or_else(|_| "gpt-4o-mini".to_string());
        Some(Self {
            provider,
            api_key,
            base_url,
            model,
            client,
        })
    }

    /// Reorder `passages` (content strings) by relevance to `query`. Returns a
    /// permutation of indices into `passages` (0-based). On failure, returns the
    /// identity order so the caller keeps the fused ranking.
    pub async fn rerank(&self, query: &str, passages: &[String]) -> Vec<usize> {
        let n = passages.len();
        let identity: Vec<usize> = (0..n).collect();
        if n <= 1 {
            return identity;
        }
        match self.provider {
            RerankProvider::OpenAiListwise => self.rerank_listwise(query, passages).await,
        }
    }

    /// Reorder via RankGPT-style listwise permutation (OpenAI-compatible chat).
    async fn rerank_listwise(&self, query: &str, passages: &[String]) -> Vec<usize> {
        let n = passages.len();
        let identity: Vec<usize> = (0..n).collect();
        let prompt = build_rank_prompt(query, passages);
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": prompt}
            ]
        });
        let resp = match self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                tracing::warn!(status = %r.status(), "rerank: non-200, keeping fused order");
                return identity;
            }
            Err(e) => {
                tracing::warn!(error = %e, "rerank: request failed, keeping fused order");
                return identity;
            }
        };
        let data: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return identity,
        };
        let content = data
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("");
        let perm = parse_permutation(content, n);
        if perm.is_empty() {
            identity
        } else {
            perm
        }
    }
}

const SYSTEM_PROMPT: &str =
    "You are RankGPT, an intelligent assistant that ranks passages by relevance to a query.";

fn build_rank_prompt(query: &str, passages: &[String]) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "I will provide you with {} passages, each indicated by a number identifier []. \
Rank them by relevance to the query: {}\n\n",
        passages.len(),
        query
    ));
    for (i, p) in passages.iter().enumerate() {
        // 1-based identifiers; truncate aggressively to keep the prompt short so
        // listwise reranking stays fast — relevance is judgeable from a snippet.
        let trimmed: String = p.chars().take(120).collect();
        s.push_str(&format!("[{}] {}\n", i + 1, trimmed));
    }
    s.push_str(&format!(
        "\nQuery: {}\n\
Rank the {} passages above by relevance to the query. List ALL identifiers in \
descending relevance, most relevant first, using the format [] > [] > [], \
e.g. [2] > [1] > [3]. Only respond with the ranking, do not say any word or explain.",
        query,
        passages.len()
    ));
    s
}

/// Parse a "[2] > [1] > [3]" permutation into 0-based indices. Tolerates missing
/// brackets, extra prose, duplicates (first wins), and out-of-range numbers. Any
/// of the n indices not mentioned are appended in their original order, so the
/// result is always a full permutation of 0..n.
fn parse_permutation(text: &str, n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = Vec::with_capacity(n);
    let mut seen = vec![false; n];
    let mut num = String::new();
    let flush = |num: &mut String, order: &mut Vec<usize>, seen: &mut [bool]| {
        if num.is_empty() {
            return;
        }
        if let Ok(v) = num.parse::<usize>() {
            if v >= 1 && v <= seen.len() && !seen[v - 1] {
                seen[v - 1] = true;
                order.push(v - 1);
            }
        }
        num.clear();
    };
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            num.push(ch);
        } else {
            flush(&mut num, &mut order, &mut seen);
        }
    }
    flush(&mut num, &mut order, &mut seen);
    if order.is_empty() {
        return Vec::new();
    }
    // Append any unranked indices in original order (graceful for partial output).
    for (i, was_seen) in seen.iter().enumerate() {
        if !was_seen {
            order.push(i);
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::parse_permutation;

    #[test]
    fn parses_full_permutation() {
        assert_eq!(parse_permutation("[2] > [1] > [3]", 3), vec![1, 0, 2]);
    }

    #[test]
    fn parses_without_brackets() {
        assert_eq!(parse_permutation("2 > 1 > 3", 3), vec![1, 0, 2]);
    }

    #[test]
    fn appends_unranked_in_original_order() {
        // model only ranked 3 and 1; 2 and 4 keep original order, appended.
        assert_eq!(parse_permutation("[3] > [1]", 4), vec![2, 0, 1, 3]);
    }

    #[test]
    fn ignores_out_of_range_and_dupes() {
        assert_eq!(parse_permutation("[9] > [2] > [2] > [1]", 3), vec![1, 0, 2]);
    }

    #[test]
    fn empty_on_no_digits() {
        assert!(parse_permutation("no ranking here", 3).is_empty());
    }
}
