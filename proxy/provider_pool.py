"""Provider pool runtime: route requests to the correct upstream profile."""
from __future__ import annotations

import json
import re
from dataclasses import dataclass

import model_discovery


def normalize_relay_base(base: str) -> str:
    return (base or "").strip().rstrip("/")


def normalize_openai_base(base: str) -> str:
    return (base or "").strip().rstrip("/")


def openai_endpoint(base: str, suffix: str) -> str:
    return base + suffix


@dataclass(frozen=True)
class PoolProfile:
    profile_id: str
    adapter: str
    api_format: str
    base_url: str
    key: str
    thinking_policy: str
    default_model: str


@dataclass
class ProviderPool:
    profiles: dict[str, PoolProfile]

    @classmethod
    def from_json(cls, raw: str) -> "ProviderPool":
        data = json.loads(raw)
        entries = data.get("profiles") or []
        profiles: dict[str, PoolProfile] = {}
        for item in entries:
            if not isinstance(item, dict):
                continue
            pid = str(item.get("profile_id") or "").strip()
            if not pid:
                continue
            profiles[pid] = PoolProfile(
                profile_id=pid,
                adapter=str(item.get("adapter") or "relay"),
                api_format=str(item.get("api_format") or "anthropic"),
                base_url=str(item.get("base_url") or ""),
                key=str(item.get("key") or ""),
                thinking_policy=str(item.get("thinking_policy") or ""),
                default_model=str(item.get("default_model") or ""),
            )
        return cls(profiles=profiles)

    def get(self, profile_id: str) -> PoolProfile | None:
        if not profile_id:
            return next(iter(self.profiles.values()), None)
        return self.profiles.get(profile_id)


def build_prov_dict(entry: PoolProfile, providers: dict) -> dict:
    """Assemble a runtime PROV dict for one pool member."""
    adapter = entry.adapter
    if adapter in ("deepseek", "qwen"):
        return dict(providers[adapter])
    if adapter not in providers:
        adapter = "relay"
    base = dict(providers[adapter])
    if adapter == "relay":
        relay_base = normalize_relay_base(entry.base_url)
        base["url"] = relay_base + "/v1/messages"
        base["models_url"] = relay_base + "/v1/models"
        base["passthrough"] = True
    elif adapter in ("openai-custom", "openai-responses"):
        obase = normalize_openai_base(entry.base_url)
        suffix = "/responses" if entry.api_format == "openai_responses" else "/chat/completions"
        base["url"] = openai_endpoint(obase, suffix)
        base["models_url"] = openai_endpoint(obase, "/models")
        if entry.default_model:
            base["default_model"] = entry.default_model
    return base


def upstream_auth_headers(prov: dict, key: str) -> dict:
    style = prov.get("auth_style", "x-api-key")
    if style == "bearer":
        return {"authorization": f"Bearer {key}"}
    if style == "both":
        return {"x-api-key": key, "authorization": f"Bearer {key}"}
    return {"x-api-key": key}


def resolve_pool_target(shell_id: str, registry, pool: ProviderPool):
    """Map shell id → (pool entry, real model id)."""
    if registry is None:
        return None, None
    routed = registry.resolve_route(shell_id)
    if not routed:
        return None, None
    real_id, profile_id = routed
    entry = pool.get(profile_id)
    if entry is None:
        return None, None
    return entry, real_id


def models_response_for_pool(registry) -> tuple[int, dict]:
    if registry is not None:
        return registry.models_response()
    return model_discovery.static_models_response([])
