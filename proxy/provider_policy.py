"""Provider policy pure functions + state (extracted from csp_proxy).

Depends on nothing above this layer (no import of csp_proxy / anthropic_compat).
Compat handlers take an explicit ProviderState assembled once at proxy startup.
"""
import random
import re
from dataclasses import dataclass
from typing import Callable

_DATE_SUFFIX = re.compile(r"-\d{8}$")

RULE_PROVIDER_VIRTUAL_MODEL_REGISTRY = "provider.virtual-model-registry"
RULE_PROVIDER_RELAY_FORCE_MODEL_SHELL = "provider.relay.force-model-shell"
RULE_PROVIDER_KIMI_RELAY_THINKING_ENABLED = "provider.kimi.relay-thinking-enabled"
RULE_PROVIDER_DASHSCOPE_RESPONSES_TOOLS_CAP = "provider.dashscope.responses-tools-cap"
RULE_TOOL_KIMI_WEB_SEARCH_SERVER_TOOL_FILTER = "tool.kimi.web_search.server-tool-filter"
RULE_TOOL_RELAY_INPUT_SCHEMA_NORMALIZE = "tool.relay.input-schema-normalize"
RULE_TOOL_DEEPSEEK_FORCED_TOOL_CHOICE_DISABLE_THINKING = (
    "tool.deepseek.forced-tool-choice-disable-thinking"
)
RULE_TOOL_DASHSCOPE_RESPONSES_WEB_SEARCH_DROP = "tool.dashscope.responses.web_search-drop"


@dataclass(frozen=True)
class Policy:
    """Provider model/context policy fields only. Skeleton/startup fields (url, auth_style,
    key_env, models_url, mode, dsml_capable) never belong here — keeps the S1b cross-language
    seam clean."""
    passthrough: bool
    force_model_override: bool
    default_model: str
    model_map: dict
    models: list
    model_caps: dict
    default_cap: object          # int or None


def policy_from_prov(prov):
    """Extract policy fields from the runtime PROV dict into a Policy."""
    return Policy(
        passthrough=bool(prov.get("passthrough")),
        force_model_override=bool(prov.get("force_model_override")),
        default_model=prov.get("default_model"),
        model_map=prov.get("model_map") or {},
        models=prov.get("models") or [],
        model_caps=prov.get("model_caps") or {},
        default_cap=prov.get("default_cap"),
    )


def _default_nonce():
    """Default nonce factory: equivalent to legacy id(areq) randomness (rewrite tool_use ids
    are already random; golden tests do not record rewrite bodies; tests inject a fixed factory
    for exact assertions)."""
    return format(random.getrandbits(24), "x")


@dataclass
class ProviderState:
    """The three compat entry points consume only this — no global reads. The skeleton assembles
    it once from module globals and passes it in."""
    policy: Policy
    prov_name: str
    relay_force_model: object     # str or None
    relay_models: list
    relay_thinking: object         # str or None
    shim_mode: str
    nonce_factory: Callable[[], str] = _default_nonce
    model_registry: object = None


def _snap_relay_model(name, relay_models):
    """Relay passthrough: align the requested model to the relay's real upstream id. Exact match
    first; else pick an upstream id prefixed with the request name; otherwise return unchanged."""
    if not relay_models or name in relay_models:
        return name
    for mid in relay_models:
        if mid.startswith(name + "-") or mid == name:
            return mid
    return name


def resolve_model(name, state, registry=None):
    """Resolve a Science model name to the provider's target model.
    Priority: virtual registry > force-model override > selector name > explicit map >
    strip date suffix > prefix match > default."""
    registry = registry or getattr(state, "model_registry", None)
    if registry is not None:
        routed = registry.resolve(name)
        if routed:
            return routed
    p = state.policy
    if p.force_model_override and state.relay_force_model:
        return state.relay_force_model
    if not name:
        return p.default_model
    if p.passthrough:
        return _snap_relay_model(name, state.relay_models)
    mm = p.model_map
    if name in mm:
        return mm[name]
    ids = {m[0] for m in p.models}
    if name in ids:
        return name
    stripped = _DATE_SUFFIX.sub("", name)
    if stripped in mm:
        return mm[stripped]
    for k, v in mm.items():
        if name.startswith(k) or stripped.startswith(k):
            return v
    return p.default_model


def _enabled_budget(max_tokens):
    """thinking:enabled needs budget_tokens, and it must be < max_tokens (reserve tokens for output)."""
    default = 1024
    if isinstance(max_tokens, int) and max_tokens > 0:
        return max(1, min(default, max_tokens - 1))
    return default


def _append_rule_id(rule_ids, rule_id):
    if rule_ids is not None and rule_id not in rule_ids:
        rule_ids.append(rule_id)


def normalize_thinking(body, prov_name, relay_thinking=None, rule_ids=None):
    """Normalize thinking (pure function; signature and semantics match legacy csp_proxy).
      (A) Forced tool_choice (any/tool) → disabled: deepseek only.
      (B) Relay thinking policy: enabled (e.g. Kimi) / adaptive|None (default, e.g. MiniMax).
          Kimi thinking models reject "thinking enabled + explicit tool_choice"; keep tools,
          drop the explicit choice so the model triggers tool_use on its own (official multi-step
          tool-call pattern + device validation).
      (C) deepseek non-forced + auto → adaptive. Mutates body in place and returns it."""
    tc = body.get("tool_choice")
    forcing = isinstance(tc, dict) and tc.get("type") in ("any", "tool")
    if forcing and prov_name == "deepseek":
        _append_rule_id(rule_ids, RULE_TOOL_DEEPSEEK_FORCED_TOOL_CHOICE_DISABLE_THINKING)
        body["thinking"] = {"type": "disabled"}
        return body
    if prov_name == "relay" and relay_thinking == "enabled":
        if forcing:
            body.pop("tool_choice", None)
        th = body.get("thinking")
        if not (isinstance(th, dict) and th.get("type") == "enabled"):
            body["thinking"] = {"type": "enabled",
                                "budget_tokens": _enabled_budget(body.get("max_tokens"))}
        return body
    th = body.get("thinking")
    if isinstance(th, dict) and th.get("type") == "auto":
        th = dict(th)
        th["type"] = "adaptive"
        body["thinking"] = th
    return body


def clamp_max_tokens(v, model, state):
    if not v:
        return v
    caps = state.policy.model_caps or {}
    cap = caps.get(model, state.policy.default_cap)
    if cap:
        return min(int(v), cap)
    return v
