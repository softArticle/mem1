"""
Lightweight metrics for mem1 evaluation: BLEU and token F1.
Avoids heavy deps (bert_score, sentence_transformers). Optional: install nltk for BLEU.
"""

from typing import Dict


def simple_tokenize(text: str) -> list[str]:
    text = str(text).lower()
    for c in ".,!?":
        text = text.replace(c, " ")
    return text.split()


def _bleu1_fallback(pred_tokens: list, ref_tokens: list) -> float:
    """Unigram precision when nltk is not available; ref_tokens is a list (one reference)."""
    if not ref_tokens or not pred_tokens:
        return 0.0
    ref_bag = dict((t, ref_tokens.count(t)) for t in set(ref_tokens))
    match = 0
    for t in pred_tokens:
        if ref_bag.get(t, 0) > 0:
            ref_bag[t] -= 1
            match += 1
    return match / len(pred_tokens) if pred_tokens else 0.0


def calculate_bleu_scores(prediction: str, reference: str) -> Dict[str, float]:
    """BLEU 1–4; uses nltk if available, else fallback to simple unigram (bleu1 only)."""
    pred_tokens = simple_tokenize(prediction)
    ref_tokens = simple_tokenize(reference)
    if not ref_tokens:
        return {"bleu1": 0.0, "bleu2": 0.0, "bleu3": 0.0, "bleu4": 0.0}
    try:
        import nltk
        from nltk.translate.bleu_score import sentence_bleu, SmoothingFunction
        nltk.download("punkt", quiet=True)
        pred_nltk = nltk.word_tokenize(prediction.lower())
        ref_nltk = [nltk.word_tokenize(reference.lower())]
        smooth = SmoothingFunction().method1
        weights_list = [
            (1, 0, 0, 0),
            (0.5, 0.5, 0, 0),
            (0.33, 0.33, 0.33, 0),
            (0.25, 0.25, 0.25, 0.25),
        ]
        return {
            f"bleu{n}": sentence_bleu(ref_nltk, pred_nltk, weights=w, smoothing_function=smooth)
            for n, w in enumerate(weights_list, start=1)
        }
    except Exception:
        bleu1 = _bleu1_fallback(pred_tokens, ref_tokens)
        return {"bleu1": bleu1, "bleu2": 0.0, "bleu3": 0.0, "bleu4": 0.0}


def calculate_metrics(prediction: str, reference: str) -> Dict[str, float]:
    """Token F1 and exact match."""
    pred = str(prediction).strip() if prediction else ""
    ref = str(reference).strip() if reference else ""
    if not pred or not ref:
        return {"exact_match": 0, "f1": 0.0}

    exact_match = 1 if pred.lower() == ref.lower() else 0
    pred_tokens = set(simple_tokenize(pred))
    ref_tokens = set(simple_tokenize(ref))
    common = pred_tokens & ref_tokens
    if not pred_tokens or not ref_tokens:
        f1 = 0.0
    else:
        prec = len(common) / len(pred_tokens)
        rec = len(common) / len(ref_tokens)
        f1 = 2 * prec * rec / (prec + rec) if (prec + rec) > 0 else 0.0

    return {"exact_match": exact_match, "f1": f1}
