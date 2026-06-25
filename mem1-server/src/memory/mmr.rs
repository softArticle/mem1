//! MMR (Maximal Marginal Relevance) diversity reranking — pure vector math, no LLM.
//!
//! Reorders an over-fetched candidate pool to balance query relevance against
//! diversity: score(d) = λ·sim(query, d) − (1−λ)·max_{s∈selected} sim(d, s).
//! Greedily picks the highest-MMR candidate each step. This targets the multi-hop
//! failure mode where the answer is a SET of distinct facts (beach / mountains /
//! forest): pure relevance ranking surfaces near-duplicates of the single best
//! match, while MMR spreads selection across distinct facts about the entity.
//!
//! Enabled via MEM1_MMR_LAMBDA in [0,1]: 1.0 = pure relevance (no diversity),
//! lower = more diversity. Unset disables MMR entirely.

/// MMR mixing weight. Default 0.85 (light diversity) is enabled out of the box:
/// on LOCOMO medium it lifts multi-hop 0.558->0.651 and open-domain +0.077 by
/// spreading selection across distinct facts, at a small temporal cost, net +.
/// Override with MEM1_MMR_LAMBDA in [0,1] (1.0 = pure relevance / disable MMR).
const DEFAULT_MMR_LAMBDA: f32 = 0.85;

pub fn mmr_lambda_from_env() -> Option<f32> {
    let lambda = std::env::var("MEM1_MMR_LAMBDA")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|l| (0.0..=1.0).contains(l))
        .unwrap_or(DEFAULT_MMR_LAMBDA);
    // λ == 1.0 means pure relevance — equivalent to no MMR, so skip the rerank.
    if lambda >= 0.999 {
        None
    } else {
        Some(lambda)
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < 1e-12 || nb < 1e-12 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Return MMR-ordered indices into `embeddings` (0-based), selecting `k` items.
/// `query` is the query embedding. The first `protect` candidates (the strongest
/// by the upstream fused ranking) are kept in their original order at the front —
/// this preserves precise top ranking for single-fact / temporal queries where the
/// best answer is the most-relevant item — and MMR diversity selection only fills
/// the remaining slots, surfacing distinct facts for multi-hop / set queries.
/// Candidates without an embedding fall back to original order, appended last.
pub fn mmr_order(
    query: &[f32],
    embeddings: &[Option<Vec<f32>>],
    lambda: f32,
    k: usize,
    protect: usize,
) -> Vec<usize> {
    let n = embeddings.len();
    let mut selected: Vec<usize> = Vec::with_capacity(n.min(k));
    let mut chosen = vec![false; n];

    // Protect the top `protect` fused-ranking candidates: keep them up front in
    // their original order, so MMR never demotes the most-relevant answers.
    for i in 0..protect.min(n) {
        if embeddings[i].is_some() {
            selected.push(i);
            chosen[i] = true;
        }
    }

    let mut remaining: Vec<usize> = (0..n)
        .filter(|&i| embeddings[i].is_some() && !chosen[i])
        .collect();
    // Precompute query relevance for each candidate.
    let rel: Vec<f32> = embeddings
        .iter()
        .map(|e| e.as_ref().map(|v| cosine(query, v)).unwrap_or(0.0))
        .collect();

    let target = k.min(selected.len() + remaining.len());
    while selected.len() < target && !remaining.is_empty() {
        let mut best_pos = 0usize;
        let mut best_score = f32::NEG_INFINITY;
        for (pos, &cand) in remaining.iter().enumerate() {
            let max_sim_to_selected = selected
                .iter()
                .map(|&s| {
                    match (&embeddings[cand], &embeddings[s]) {
                        (Some(a), Some(b)) => cosine(a, b),
                        _ => 0.0,
                    }
                })
                .fold(0.0_f32, f32::max);
            let score = lambda * rel[cand] - (1.0 - lambda) * max_sim_to_selected;
            if score > best_score {
                best_score = score;
                best_pos = pos;
            }
        }
        selected.push(remaining.remove(best_pos));
    }
    // Append any candidates without embeddings (and any leftover) in original order.
    for i in 0..n {
        if !selected.contains(&i) {
            selected.push(i);
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::mmr_order;

    #[test]
    fn pure_relevance_when_lambda_1() {
        // query=[1,0]; docs: most-aligned first by relevance.
        let q = vec![1.0, 0.0];
        let embs = vec![
            Some(vec![0.5, 0.5]), // rel ~0.707
            Some(vec![1.0, 0.0]), // rel 1.0
            Some(vec![0.0, 1.0]), // rel 0.0
        ];
        let order = mmr_order(&q, &embs, 1.0, 3, 0);
        assert_eq!(order[0], 1); // highest relevance first
    }

    #[test]
    fn diversity_picks_spread_when_lambda_low() {
        // Two near-duplicate high-relevance docs + one diverse doc.
        let q = vec![1.0, 0.0];
        let embs = vec![
            Some(vec![1.0, 0.0]),   // rel 1.0
            Some(vec![0.99, 0.01]), // rel ~1.0, near-dup of [0]
            Some(vec![0.3, 0.95]),  // diverse, lower rel
        ];
        // low lambda → after picking [0], the diverse [2] should beat the near-dup [1].
        let order = mmr_order(&q, &embs, 0.3, 2, 0);
        assert_eq!(order[0], 0);
        assert_eq!(order[1], 2);
    }

    #[test]
    fn missing_embeddings_appended_last() {
        let q = vec![1.0, 0.0];
        let embs = vec![Some(vec![1.0, 0.0]), None, Some(vec![0.0, 1.0])];
        let order = mmr_order(&q, &embs, 0.5, 3, 0);
        assert_eq!(order.len(), 3);
        assert_eq!(*order.last().unwrap(), 1); // the None one is appended last
    }
}
