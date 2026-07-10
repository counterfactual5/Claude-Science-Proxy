## Summary

<!-- What does this PR change and why? -->

## Test plan

- [ ] `bash test/run_all.sh`
- [ ] `(cd desktop/src-tauri && cargo test)` (if Rust touched)
- [ ] `node --check desktop/src/main.js` (if frontend touched)
- [ ] Real-machine / Science E2E (only if applicable; isolated HOME + ports; see `test/docs/REAL_MACHINE_TEST.md`)

## Iron rules

- [ ] Does **not** read, copy, modify, or delete real `~/.claude-science` credentials or port **8765**
- [ ] Does **not** commit secrets (keys, tokens, `.env`)

## Related issues

<!-- Fixes #123 or links discussion -->
