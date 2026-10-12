# ADR-0011: Active-Account Quota Without Refresh and Limit Freshness

- Status: Accepted
- Date: 2026-10-12
- Extends: [ADR-0010](0010-probe-based-smart-switch.md)

## Context

After ADR-0010 the switcher worked in probe mode, but the limits shown in the window were wrong or
missing:

1. **Gemini.** With `invalid_client` no profile had quota, so nothing was shown and Smart Switch
   could not switch before a limit was hit. Yet the active account's credential in Windows Credential
   Manager (`gemini:antigravity`) holds an `access_token` that Antigravity/agy refreshes on its own
   (about hourly, `token.expiry` in RFC 3339 with 7 fractional digits).
2. **Codex.** The last `rate_limits` event from the rollout log was shown as is. When its window had
   already reset (e.g. reset 09:30, now 10:53) the old 70% stayed on screen, and the target time was
   sent as `HH:MM` without a date, so the UI moved it to the next day. The "updated" label showed the
   current time, making old data look live. A missing reset time became a hard-coded `19:41`.
3. **Claude.** The switcher never refreshes the Claude Code token (that would break the user's
   Claude Code login). Once the token expired, the in-memory cache was the only source, so after a
   restart nothing was shown.

## Decision

1. **Active Gemini quota from the stored access token.** For the active profile, if the credential's
   `access_token` has at least 60 seconds left, quota is read with it directly
   (`QuotaDecryptor::fetch_quota_summary`), without a refresh. Otherwise the refresh path of
   ADR-0010 applies (paused after `invalid_client`). Inactive profiles are unchanged.
2. **Quota mode tolerates candidates without quota.** When the active profile has quota, candidates
   with quota are ranked by remaining 5-hour quota as before; if none qualifies, candidates without
   quota are chosen by the probe-mode rule (oldest activation, not `NeedsVerification`, not
   `QuotaExhausted` within 5 hours). A recent `QuotaExhausted` probe on the active profile is an
   emergency in quota mode too.
3. **One view of quota.** Non-live views (`list_profiles`, the HTTP API) reuse quotas fetched by the
   live path within the last 5 minutes, so `/api/v1/eligibility` reports the real `mode`.
4. **Reset-aware limits.** `effective_used(used, resets_at, now)` returns 0 once a window has reset;
   the reset label becomes "Window reset". The data time comes from the source (Codex event
   timestamp, Claude fetch time) and is shown in the UI source label.
5. **Claude disk cache.** A successful Claude usage response is written to
   `%LOCALAPPDATA%\AntigravitySwitcher\claude_usage_cache.json` (percentages, reset times and fetch
   time only, no token). When the token is expired or the request fails, the cache is returned with
   reset windows applied and `source: "cached_oauth"`.
6. **Tests stay offline.** With `BYPASS_SYNC_ACTIVE_PROFILE` (debug builds only) the background quota
   fetch is skipped, so tests never read the real credential store or call Google.

## Consequences

- The active account shows real Gemini quota and Smart Switch can switch before the limit, as long as
  Antigravity or agy has refreshed the token within the hour; otherwise it falls back to probe mode.
- Other accounts still show no Gemini quota until a working OAuth client secret is configured.
- Codex data is only as fresh as the last Codex session; the label says when that was.
- Claude data refreshes whenever Claude Code refreshes its own token; in between it is marked
  cached.
