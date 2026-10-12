# ADR-0010: Probe-Based Smart Switch and Account Eligibility

- Status: Accepted
- Date: 2026-10-11
- Extends: [ADR-0009](0009-smart-switch-limits-thresholds.md)

## Context

ADR-0009 decides every switch from quota numbers. In practice two failure modes stopped the
whole workflow:

1. **Accounts that ask for verification.** A profile can hold a valid refresh token while Google
   refuses it with `Eligibility check failed` / `Verify your account`, or the stored login is gone
   (`Authentication required`). Quota looks fine, so the engine switched to such accounts and stayed
   there; every agent call failed until a human noticed.
2. **Quota cannot be read.** When Google rejects the OAuth client itself (`401 invalid_client`), no
   profile gets a quota. The engine returned early on `quota == None` and never switched. The UI
   kept polling every 5 seconds, so 8 profiles produced about 90,000 failed token requests and log
   lines in 15 hours.

## Decision

1. **Eligibility status per profile** (`Eligible`, `NeedsVerification { since, message }`, `Unknown`)
   is stored in profile metadata. It is set by a *probe*: a real one-line `agy` request
   (`Reply with exactly: OK`, `gemini-3.8-flash-low`, 90s timeout) on the active account.
   - `NeedsVerification`: output matches verification or missing-login phrases.
   - `QuotaExhausted`: output matches quota / rate-limit phrases (kept in memory for 5 hours).
   - `Ok`: a clean `OK`, or any non-empty successful reply. An empty reply is **not** Ok.
   - `Error`: anything else (network, timeout). Never triggers a switch.
2. **When the active profile is probed:** immediately after it changes, then every 30 minutes
   (quota mode) or every 10 minutes (probe mode). After an `Error`, the next probe is in 2 minutes.
3. **Emergency switch:** `NeedsVerification` on the active profile (both modes), or `QuotaExhausted`
   in probe mode, switches even while the agent is busy, because the agent cannot make progress on
   that account anyway. In quota mode the quota numbers of ADR-0009 still decide.
4. **Verified switch:** after a switch the target is probed (after 10s, retried once on `Error`,
   because `language_server` restarts and returns `context canceled`). A failing target is marked
   and the next candidate is tried, at most 5 attempts per cycle.
5. **Probe mode:** if the active profile has no quota, candidates are all other profiles with a valid
   token that are neither `NeedsVerification` nor `QuotaExhausted` in the last 5 hours, ordered by
   oldest `last_activated_at` (round robin). Profiles with quota keep the ADR-0009 behaviour.
6. **Quota pause:** after `invalid_client`, token refresh for quota is skipped for 6 hours and the
   paused error is not logged. The pause resets on restart.
7. **Locked app:** while the master password lock is on, profiles cannot be read; Smart Switch logs
   this once instead of idling silently.

## HTTP API (Bearer-protected, localhost)

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/v1/eligibility` | `{ mode: "quota" \| "probe", profiles: [{ id, email, eligibility, quota_exhausted_at }] }` |
| POST | `/api/v1/eligibility/{id}/reset` | Clear `NeedsVerification` after the account was fixed |
| POST | `/api/v1/eligibility/probe-active` | Probe the active account now |

## Consequences

- An account that needs a human no longer stops the workflow. It is skipped for 6 hours, then
  returns to `Unknown` and is probed again when selected; `/reset` clears it at once.
- Each probe is one tiny request on the active account: about 6 per hour in probe mode while the
  account is healthy, one every 2 minutes while probes keep failing with `Error`.
- The probe depends on the `agy` CLI at `%LOCALAPPDATA%\agy\bin\agy.exe`. `agy` follows the account
  the switcher activates (verified live: its log shows the new email after a switch).
- Probe classification is text based. New Google error wording may need new phrases in
  `eligibility.rs`.
