"""Virtual model registry: map Science claude-* shell ids to real upstream models.

Science only displays model ids starting with ``claude-``. This module allocates
shell ids from a fixed pool and routes incoming requests to configured real ids.
"""
from __future__ import annotations

import json
from dataclasses import dataclass, field


SHELL_POOL = [
    ("claude-opus-4-8", "main"),
    ("claude-sonnet-5", "main"),
    ("claude-haiku-4-5", "main"),
    ("claude-opus-4-7", "overflow"),
    ("claude-sonnet-4-6", "overflow"),
    ("claude-haiku-4-4", "overflow"),
    ("claude-opus-4-6", "overflow"),
    ("claude-sonnet-4-5", "overflow"),
]

# Science background agents commonly send these shell ids.
FALLBACK_SHELLS = {
    "claude-opus-4-8": "default",
    "claude-haiku-4-5": "fast",
    "claude-sonnet-5": "default",
}

CREATED_AT = "2026-01-01T00:00:00Z"


@dataclass(frozen=True)
class RegistryEntry:
    shell_id: str
    real_id: str
    display_name: str
    tier: str


@dataclass
class ModelRegistry:
    entries: list[RegistryEntry] = field(default_factory=list)
    routes: dict[str, str] = field(default_factory=dict)
    display_names: dict[str, str] = field(default_factory=dict)
    default_model: str = ""
    fast_model: str = ""

    @classmethod
    def from_payload(cls, payload: dict) -> "ModelRegistry":
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
        default_model = str(payload.get("default_model") or "").strip()
        if not default_model and cleaned:
            default_model = cleaned[0]
        fast_model = str(payload.get("fast_model") or "").strip()
        if not fast_model:
            fast_model = cleaned[-1] if len(cleaned) > 1 else default_model
        return cls.from_models(cleaned, default_model=default_model, fast_model=fast_model)

    @classmethod
    def from_json(cls, raw: str) -> "ModelRegistry":
        return cls.from_payload(json.loads(raw))

    @classmethod
    def from_models(
        cls,
        models: list[str],
        *,
        default_model: str = "",
        fast_model: str = "",
    ) -> "ModelRegistry":
        reg = cls()
        if not models:
            return reg
        reg.default_model = default_model or models[0]
        reg.fast_model = fast_model or (models[-1] if len(models) > 1 else reg.default_model)
        used_shells = set()
        entries = []
        pool_iter = iter(SHELL_POOL)
        for real_id in models:
            shell_id, tier = _next_shell(pool_iter, used_shells)
            used_shells.add(shell_id)
            entries.append(RegistryEntry(shell_id, real_id, real_id, tier))
        reg.entries = entries
        reg.routes = {e.shell_id: e.real_id for e in entries}
        reg.display_names = {e.shell_id: e.display_name for e in entries}
        return reg

    def resolve(self, shell_id: str) -> str | None:
        if not shell_id:
            return self.default_model or None
        if shell_id in self.routes:
            return self.routes[shell_id]
        kind = FALLBACK_SHELLS.get(shell_id)
        if kind == "fast" and self.fast_model:
            return self.fast_model
        if kind == "default" and self.default_model:
            return self.default_model
        return None

    def models_response(self):
        data = [{
            "type": "model",
            "id": e.shell_id,
            "display_name": e.display_name,
            "supports_tools": None,
            "created_at": CREATED_AT,
        } for e in self.entries]
        return 200, {
            "data": data,
            "has_more": False,
            "first_id": data[0]["id"] if data else None,
            "last_id": data[-1]["id"] if data else None,
        }


def _next_shell(pool_iter, used_shells: set[str]) -> tuple[str, str]:
    for shell_id, tier in pool_iter:
        if shell_id not in used_shells:
            return shell_id, tier
    raise ValueError("shell pool exhausted")
