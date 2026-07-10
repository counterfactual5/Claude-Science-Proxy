"""Virtual model registry: map Science claude-* shell ids to real upstream models.

Science only displays model ids starting with ``claude-``. This module allocates
shell ids from a fixed pool and routes incoming requests to configured real ids.

Science UI rules (binary s0/ZjO/XjO/hB_):
  1) id must start with ``claude-``
  2) main list keeps one id per family (opus / sonnet / haiku)
  3) remaining ids go to "More models", sorted by shell version

Critical: never emit a second haiku shell. If both ``claude-haiku-4-5`` and
``claude-haiku-4-4`` appear, Science parks the lower haiku in the main
"Fastest, lightweight" slot — which is how ``glm-4.5`` displaced newer models.
"""
from __future__ import annotations

import json
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from functools import cmp_to_key

import model_sort


# Exactly one haiku shell in the whole pool (main). Overflow uses opus/sonnet only.
SHELL_POOL = [
    ("claude-opus-4-8", "main"),
    ("claude-sonnet-5", "main"),
    ("claude-haiku-4-5", "main"),
    ("claude-opus-4-7", "overflow"),
    ("claude-sonnet-4-6", "overflow"),
    ("claude-opus-4-6", "overflow"),
    ("claude-sonnet-4-5", "overflow"),
    ("claude-opus-4-5", "overflow"),
]

# Science background agents commonly send these shell ids.
FALLBACK_SHELLS = {
    "claude-opus-4-8": "default",
    "claude-haiku-4-5": "fast",
    "claude-sonnet-5": "default",
}

# Lowest overflow shell (opus/sonnet only — never a second haiku).
FAST_PIN_SHELL = "claude-opus-4-5"

_BASE_CREATED = datetime(2026, 1, 1, tzinfo=timezone.utc)


@dataclass(frozen=True)
class RegistryEntry:
    shell_id: str
    real_id: str
    display_name: str
    tier: str
    profile_id: str = ""
    created_at: str = "2026-01-01T00:00:00Z"


@dataclass
class ModelRegistry:
    entries: list[RegistryEntry] = field(default_factory=list)
    routes: dict[str, str] = field(default_factory=dict)
    display_names: dict[str, str] = field(default_factory=dict)
    profile_routes: dict[str, str] = field(default_factory=dict)
    default_model: str = ""
    fast_model: str = ""
    default_profile_id: str = ""

    @classmethod
    def from_payload(cls, payload: dict) -> "ModelRegistry":
        if payload.get("merge") and isinstance(payload.get("profiles"), list):
            return cls.merge_payloads(payload["profiles"])
        models = payload.get("models") or []
        if isinstance(models, str):
            models = [models]
        cleaned = []
        seen = set()
        for raw in models:
            mid = str(raw).strip()
            if not mid or mid in seen:
                continue
            seen.add(mid)
            cleaned.append(mid)
        cleaned = model_sort.sort_model_ids(cleaned)
        # Flagship default always follows version sort (ignore stale payload default).
        default_model = cleaned[0] if cleaned else ""
        fast_model = str(payload.get("fast_model") or "").strip()
        if not fast_model:
            fast_model = cleaned[-1] if len(cleaned) > 1 else default_model
        profile_id = str(payload.get("profile_id") or "").strip()
        display_prefix = str(payload.get("display_name") or "").strip()
        return cls.from_models(
            cleaned,
            default_model=default_model,
            fast_model=fast_model,
            profile_id=profile_id,
            display_prefix=display_prefix,
        )

    @classmethod
    def from_json(cls, raw: str) -> "ModelRegistry":
        return cls.from_payload(json.loads(raw))

    @classmethod
    def merge_payloads(cls, payloads: list[dict]) -> "ModelRegistry":
        reg = cls()
        if not payloads:
            return reg
        used_shells: set[str] = set()
        pool_iter = iter(SHELL_POOL)
        entries: list[RegistryEntry] = []
        first_default = ""
        first_fast = ""
        first_profile = ""
        for payload in payloads:
            if not isinstance(payload, dict):
                continue
            slice_reg = cls.from_payload(payload)
            if not slice_reg.entries:
                continue
            for entry in slice_reg.entries:
                shell_id, tier = _next_shell(pool_iter, used_shells)
                used_shells.add(shell_id)
                entries.append(RegistryEntry(
                    shell_id,
                    entry.real_id,
                    entry.display_name,
                    tier,
                    entry.profile_id,
                    entry.created_at,
                ))
            if not first_default and slice_reg.default_model:
                first_default = slice_reg.default_model
                first_fast = slice_reg.fast_model or slice_reg.default_model
                first_profile = slice_reg.default_profile_id
        reg.entries = entries
        reg.routes = {e.shell_id: e.real_id for e in entries}
        reg.display_names = {e.shell_id: e.display_name for e in entries}
        reg.profile_routes = {e.shell_id: e.profile_id for e in entries if e.profile_id}
        reg.default_model = first_default
        reg.fast_model = first_fast or first_default
        reg.default_profile_id = first_profile
        return reg

    @classmethod
    def from_models(
        cls,
        models: list[str],
        *,
        default_model: str = "",
        fast_model: str = "",
        profile_id: str = "",
        display_prefix: str = "",
    ) -> "ModelRegistry":
        reg = cls()
        if not models:
            return reg
        ordered = model_sort.sort_model_ids(list(models))
        reg.default_model = default_model or ordered[0]
        reg.fast_model = fast_model or (ordered[-1] if len(ordered) > 1 else reg.default_model)
        reg.default_profile_id = profile_id
        main_shells = _shells_for_tier("main")
        overflow_shells = _shells_for_tier("overflow")
        main_models = ordered[: len(main_shells)]
        overflow_models = ordered[len(main_shells) :]
        pairs: list[tuple[str, str, str]] = []
        for (shell_id, tier), real_id in zip(main_shells, main_models):
            pairs.append((shell_id, tier, real_id))
        pairs.extend(
            _pair_overflow_models(
                overflow_models,
                overflow_shells,
                fast_model=reg.fast_model,
                default_model=reg.default_model,
            )
        )
        # Newest model → newest created_at (helps clients that sort on this field).
        rank = {mid: i for i, mid in enumerate(ordered)}
        n = max(len(ordered) - 1, 1)
        entries: list[RegistryEntry] = []
        for shell_id, tier, real_id in pairs:
            display = f"{display_prefix}: {real_id}" if display_prefix else real_id
            created = _created_at_for_rank(rank.get(real_id, n), n)
            entries.append(
                RegistryEntry(shell_id, real_id, display, tier, profile_id, created)
            )
        reg.entries = entries
        reg.routes = {e.shell_id: e.real_id for e in entries}
        reg.display_names = {e.shell_id: e.display_name for e in entries}
        reg.profile_routes = {e.shell_id: e.profile_id for e in entries if e.profile_id}
        return reg

    def resolve(self, shell_id: str) -> str | None:
        routed = self.resolve_route(shell_id)
        return routed[0] if routed else None

    def resolve_route(self, shell_id: str) -> tuple[str, str] | None:
        """Return (real_id, profile_id) for a shell id."""
        if not shell_id:
            if self.default_model:
                return self.default_model, self.default_profile_id
            return None
        if shell_id in self.routes:
            return self.routes[shell_id], self.profile_routes.get(shell_id, self.default_profile_id)
        kind = FALLBACK_SHELLS.get(shell_id)
        if kind == "fast" and self.fast_model:
            return self.fast_model, self.default_profile_id
        if kind == "default" and self.default_model:
            return self.default_model, self.default_profile_id
        return None

    def models_response(self):
        data = [{
            "type": "model",
            "id": e.shell_id,
            "display_name": e.display_name,
            "supports_tools": None,
            "created_at": e.created_at,
        } for e in self.entries]
        return 200, {
            "data": data,
            "has_more": False,
            "first_id": data[0]["id"] if data else None,
            "last_id": data[-1]["id"] if data else None,
        }


def _created_at_for_rank(rank: int, last_rank: int) -> str:
    """Newer models get later timestamps (rank 0 = newest)."""
    days = last_rank - rank
    return (_BASE_CREATED + timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")


def _shells_for_tier(tier: str) -> list[tuple[str, str]]:
    return [(shell_id, t) for shell_id, t in SHELL_POOL if t == tier]


def _sort_shells_desc(shells: list[tuple[str, str]]) -> list[tuple[str, str]]:
    return sorted(
        shells,
        key=lambda item: cmp_to_key(model_sort.compare_models_desc)(item[0]),
    )


def _pair_overflow_models(
    overflow_models: list[str],
    overflow_shells: list[tuple[str, str]],
    *,
    fast_model: str,
    default_model: str,
) -> list[tuple[str, str, str]]:
    """Pair overflow shells to models by descending version rank.

    Science sorts "More models" by shell id version. Matching ranks keeps
    glm-4.7 before glm-4.6 before glm-4.5-air in that submenu.
    """
    if not overflow_models:
        return []
    shells = list(overflow_shells)
    models = list(overflow_models)
    pin_fast = (
        fast_model in models
        and fast_model != default_model
        and any(shell_id == FAST_PIN_SHELL for shell_id, _ in shells)
    )
    if pin_fast:
        shells = [(sid, tier) for sid, tier in shells if sid != FAST_PIN_SHELL]
        models = [m for m in models if m != fast_model]
    shells = _sort_shells_desc(shells)
    models = model_sort.sort_model_ids(models)
    if len(models) > len(shells):
        raise ValueError("shell pool exhausted")
    pairs: list[tuple[str, str, str]] = [
        (shell_id, tier, real_id)
        for (shell_id, tier), real_id in zip(shells, models)
    ]
    if pin_fast:
        pairs.append((FAST_PIN_SHELL, "overflow", fast_model))
        pairs.sort(key=lambda item: cmp_to_key(model_sort.compare_models_desc)(item[0]))
    return pairs


def _next_shell(pool_iter, used_shells: set[str]) -> tuple[str, str]:
    for shell_id, tier in pool_iter:
        if shell_id not in used_shells:
            return shell_id, tier
    raise ValueError("shell pool exhausted")


def science_main_display_names(reg: ModelRegistry) -> list[str]:
    """Approximate Science main-list picks: highest opus/sonnet, sole haiku."""
    by_family: dict[str, list[RegistryEntry]] = {"opus": [], "sonnet": [], "haiku": []}
    for e in reg.entries:
        for fam in by_family:
            if e.shell_id.startswith(f"claude-{fam}-"):
                by_family[fam].append(e)
                break
    out: list[str] = []
    for fam, prefer_highest in (("opus", True), ("sonnet", True), ("haiku", True)):
        entries = by_family[fam]
        if not entries:
            continue
        entries = sorted(
            entries,
            key=lambda e: cmp_to_key(model_sort.compare_models_desc)(e.shell_id),
        )
        out.append(entries[0].display_name if prefer_highest else entries[-1].display_name)
    return out
