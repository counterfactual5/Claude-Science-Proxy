"""Model id sort: newest / flagship first (descending parseable version segments)."""

from functools import cmp_to_key


def _version_tokens(model_id: str) -> list[int]:
    tokens: list[int] = []
    cur = ""
    for ch in model_id:
        if ch.isdigit():
            cur += ch
        elif cur:
            tokens.append(int(cur))
            cur = ""
    if cur:
        tokens.append(int(cur))
    return tokens


def compare_models_desc(a: str, b: str) -> int:
    """Return <0 if a should sort before b (newer/better first)."""
    ta = _version_tokens(a)
    tb = _version_tokens(b)
    max_len = max(len(ta), len(tb))
    for i in range(max_len):
        va = ta[i] if i < len(ta) else 0
        vb = tb[i] if i < len(tb) else 0
        if vb != va:
            return vb - va  # descending: larger version first
    if a == b:
        return 0
    return -1 if a > b else 1  # lexicographic desc tie-break


def sort_model_ids(models: list[str]) -> list[str]:
    """Return a new list sorted newest-first."""
    return sorted(models, key=cmp_to_key(compare_models_desc))
