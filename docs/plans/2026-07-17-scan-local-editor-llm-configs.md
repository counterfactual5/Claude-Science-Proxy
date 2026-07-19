---
title: Scan local editor LLM configs
date: 2026-07-17
type: unified-plan
artifact_readiness: requirements-only
status: implemented
target_release: v2.0.0
---

# Scan local editor LLM configs

## Goal Capsule

- **Objective:** Let users import provider connections (base URL + model catalog, optionally API key) from local editor configs — starting with Zed — so filling the multi-provider platter does not require hand-copying endpoints.
- **Authority:** Product direction confirmed 2026-07-17 after reviewing `~/.config/zed/settings.json`.
- **Execution profile:** Major version (v2.0); sits on top of v1.9 multi-provider platter.
- **Stop condition:** User can scan Zed OpenAI-compatible providers, preview candidates, import missing connections into CSP (with key from Keychain when available, else prompt), skip duplicates of existing base URLs.
- **Out of scope for first v2.0 slice:** Full Keychain write-back; non-macOS secret stores; auto-rewriting OpenAI `/v1` URLs into Anthropic relay templates without user choice.

---

## Product Contract

### Summary

Add a **Scan local configs** path (similar to Skills/MCP discover) that reads known HOME-level editor settings, lists provider candidates, and imports selected ones as CSP profiles for use in the multi-provider platter.

### Requirements

- R1. Scan Zed `~/.config/zed/settings.json` → `language_models.openai_compatible` entries (name, `api_url`, `available_models`).
- R2. UI lists candidates with URL + model count; user multi-selects what to import.
- R3. Import creates `custom-openai` (or mapped template when URL matches a known Anthropic preset) profiles with discovered models as `active_models` (cap at Science 8 when enabling into platter separately).
- R4. Duplicate detection: same `base_url` (normalized) already in CSP → skip or offer merge, never silent duplicate.
- R5. API keys: prefer macOS Keychain lookup by URL when present; otherwise leave key empty and mark “needs key”.
- R6. Never log or display full keys; mask in UI.
- R7. Deferred expansions (same release or follow-up): Cursor / Continue / OpenCode home configs with documented roots.

### Scope Boundaries

**In v2.0**

- Zed OpenAI-compatible scan + import + Keychain read (macOS).

**Deferred / later**

- Auto-convert OpenAI URLs to Anthropic relay templates.
- Windows/Linux secret backends.
- One-click “fill platter from scanned providers”.

---

## Definition of Done (product)

- [ ] Scan finds Zed providers present on a machine with `settings.json`.
- [ ] Import creates usable CSP profiles; platter can pick models across them.
- [ ] Existing GLM / other profiles are not duplicated without confirmation.
- [ ] Documented as v2.0.0.
