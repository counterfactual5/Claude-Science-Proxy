---
title: Multi-provider model platter
date: 2026-07-17
type: unified-plan
artifact_readiness: requirements-only
status: draft
target_release: v1.9.0
---

# Multi-provider model platter

## Goal Capsule

- **Objective:** Let users run Claude Science with up to 8 models chosen from any saved providers, without hand-editing CSP.json — while keeping single-provider “当前生效” as a first-class path.
- **Authority:** Product decisions locked in brainstorm (2026-07-17). Implementation design deferred to planning.
- **Execution profile:** Minor version (v1.9); schema + runtime routing + UI.
- **Stop condition:** Single-provider mode still works; multi card can be set current-effective with ≤8 cross-provider models; Science selector shows those shells; each request routes to the owning provider’s key/base/adapter.
- **Out of scope for v1.9 product:** Redesigning wizard presets; removing single-provider cards; unlimited Science models beyond 8; scanning local editor LLM configs (→ v2.0).

---

## Product Contract

### Summary

Add a fixed special card **「多提供商 · 自选模型」** under the existing provider list on「我的配置」. Users either activate one single-provider connection (today’s behavior) or activate this multi card and pick models from any already-imported providers. At most **8** models enter Science. The **first selected** model is the default. Upstream model discovery per provider is **not** capped at 8 — only the Science platter is.

### Problem Frame

Today CSP supports many profiles, but only one can be active, and Science’s visible models come from that profile’s `active_models`. Cross-provider selection requires editing CSP.json. Separately, the product language around “8 models” conflates (a) how many upstream candidates to show and (b) how many Science can display — users reasonably want a full catalog per provider and a hard 8 only at the Science boundary.

### Requirements

**List & activation**

- R1. 「我的配置」keeps per-provider cards as today (name, base URL, key mask, enabled-model count,「当前生效」).
- R2. Below provider cards, always show a special card titled **多提供商 · 自选模型** (even if only one provider exists).
- R3. Exactly one card may be「当前生效」at a time: either one single-provider card **or** the multi card — mutually exclusive.
- R4. The multi card cannot become「当前生效」until at least one model is selected in its platter.
- R5. When the multi card is empty / not configured, secondary text communicates that (e.g.「未选用 · 点击配置」); when configured, show selected count (e.g.「N 个模型 · 跨 M 个提供商」).

**Multi card editor**

- R6. Opening the multi card lets the user pick models from **all saved providers’ catalogs** (providers that already have key/base configured).
- R7. Selection is capped at **8**; UI hard-blocks a 9th selection with a clear reason (Science limit).
- R8. Selection order matters: the **first selected** model is the **default** model for Science; users may reorder (first slot = default) without a separate “pick default” control.
- R9. Fast / small-model slot is **auto**: prefer a catalog entry that looks like a small/fast model among the selected set; else second selected; else same as default. No dedicated fast picker in v1.9.
- R10. Optional convenience: first-time open can offer「从当前生效连接导入已启用模型」to seed the platter from the previously active single-provider `active_models`.

**Catalog vs Science slots**

- R11. Per-provider「刷新 / 发现模型」shows the **full** upstream (or builtin) candidate list — **no artificial 8-item truncation** for browsing/checking.
- R12. The number **8** applies only to models that are **enabled into Science** (single-provider `active_models` when that card is effective, or the multi platter when the multi card is effective).
- R13. Single-provider cards may still show「N 个模型已启用」for their own enable list; that list is used only when that card is「当前生效」.

**Runtime behavior**

- R14. When a single-provider card is「当前生效」, behavior matches today’s single-profile virtual registry (or DeepSeek static path) — no regression for existing users.
- R15. When the multi card is「当前生效」, Science’s `/v1/models` exposes up to 8 virtual `claude-*` shells mapped to the platter entries, including models owned by different providers.
- R16. Each inference request resolves shell → owning provider credentials (key, base URL, adapter) and upstream model id; failures name which provider/model failed.
- R17. Starting Claude Science with the multi card effective must not require a separate “active profile” beyond the multi platter’s membership.

**Migration & versioning**

- R18. Existing installs migrate without data loss: profiles and keys preserved; current exclusive active profile remains「当前生效」after upgrade; multi card starts empty (or optionally seedable via R10).
- R19. Ship as **v1.9.0** (new card + activation semantics + cross-provider routing; schema v5).

### Key Flows

- F1. Single-provider path (unchanged intent)
  - **Trigger:** User sets a provider card「当前生效」and starts Science.
  - **Outcome:** Science sees that profile’s enabled models (≤8); multi card is not effective.

- F2. Configure multi platter
  - **Trigger:** User opens「多提供商 · 自选模型」, selects models across providers (order = priority), saves.
  - **Outcome:** Card shows count; still not Science-visible until set「当前生效」.

- F3. Activate multi platter
  - **Trigger:** User sets multi card「当前生效」(≥1 model selected).
  - **Outcome:** Other provider cards lose「当前生效」; next Start uses merged registry ≤8 shells; first selected is default.

- F4. Full catalog, capped Science
  - **Trigger:** User refreshes models on a provider that returns >8 ids.
  - **Outcome:** Edit UI lists all; only platter / single-provider enable list enforces 8.

### Acceptance Examples

- A1. Two providers saved (e.g. GLM + Kimi). User selects 3 GLM + 2 Kimi in multi card, first pick GLM-X. Sets multi「当前生效」, starts Science. Selector shows 5 models; default is GLM-X; requests to Kimi shells use Kimi key.
- A2. User with only GLM, single card「当前生效」with 8 enabled models — upgrade to v1.9 — behavior unchanged; multi card visible but unused.
- A3. Multi card with 0 selections —「设为当前生效」disabled or errors clearly.
- A4. Provider discover returns 20 models — all visible in picker; user cannot put more than 8 into the multi platter.

### Scope Boundaries

**In v1.9**

- Special list card + editor; mutual exclusion with single-provider active; cross-provider routing; lift catalog 8-cap; first-selected = default; auto fast.

**Deferred**

- Unlimited Science models beyond 8 (platform hard limit).
- Per-model custom display names beyond existing science-safe naming.
- Simultaneous “partially active” multiple single-provider cards.
- Dedicated UI for fast-model override.
- **Scan local editor LLM configs** (Zed / Cursor / …) — tracked as v2.0 in `docs/plans/2026-07-17-scan-local-editor-llm-configs.md`.

**Outside product identity**

- Changing Claude Science’s own 8-shell UI.
- Hosted Anthropic model directory / real claude.ai login.

---

## Definition of Done (product)

- [ ] Multi card「多提供商 · 自选模型」appears under provider cards and can be configured and set「当前生效」.
- [ ] Single-provider「当前生效」path still works for existing configs after migration.
- [ ] Science shows ≤8 models from the effective source; first platter selection is default.
- [ ] Cross-provider requests use the correct provider credentials (relay / DeepSeek / OpenAI-custom).
- [ ] Provider model discovery is not truncated to 8 for browsing.
- [ ] Documented as v1.9.0 in changelog / README when shipped.
