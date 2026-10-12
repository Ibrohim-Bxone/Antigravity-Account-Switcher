# Antigravity Account Switcher — notes for AI coding agents

Windows desktop app (Rust + Tauri 2, React UI) that swaps Google Antigravity accounts and
switches automatically when the active one is out of quota or needs verification.

## Layout

| Path | What |
| --- | --- |
| `crates/switcher-core` | Shared models (`ProfileMetadata`, `EligibilityStatus`, views) |
| `crates/switcher-windows/src/service/` | Service: profiles, switching, `smart_switch.rs`, `eligibility.rs` |
| `crates/switcher-windows/src/quota.rs` | Live quota fetch (OAuth refresh + quota API), invalid_client pause |
| `crates/switcher-windows/tests/` | Integration tests (`eligibility_tests.rs`) |
| `src-tauri/src/http.rs` | Local Bearer-protected HTTP API (`parse_route` + handlers) |
| `src/` | React UI |
| `docs/decisions/` | ADRs — change an accepted decision with a new ADR, never by editing it |

## Commands

```bash
cargo test --workspace --quiet        # must be green before every commit
npx tauri build --no-bundle           # release binary: target/release/app.exe
```

## Rules

- Never commit the OAuth client secret. It comes from `ANTIGRAVITY_OAUTH_CLIENT_SECRET`
  (see README). Do not try to recover or replace it in code.
- Code, comments, logs and API fields are in English (public repo).
- Smart Switch keeps global state (`active_probe_state`, `quota_exhausted_state`, quota pause).
  Tests touching it take the `SERIAL` lock and use `MockProbe`; no real `agy`, Google API or
  account switch in tests. `BYPASS_SYNC_ACTIVE_PROFILE` works only in debug builds.
- Probe classification (`eligibility.rs`) is text based: when Google changes wording, add the
  phrase and a test case in `test_eligibility_error_parsing`.

## Deploying a local build

The running app locks its exe: stop it, back up the installed exe, copy `target/release/app.exe`
over it, start it. The app rotates `switcher.log` into `logs/archive/` on start, so check new log
lines by timestamp, not by line count.
