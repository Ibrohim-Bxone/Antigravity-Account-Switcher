/**
 * Live Quota Provider for External AI (Claude Code & OpenAI Codex).
 * Fully isolated, read-only, non-blocking telemetry integration.
 * Strictly adheres to QA audit requirements: zero panics, zero token refresh side-effects,
 * defensive file reads (tail read 64KB), strict 3s timeouts, graceful degradation.
 */
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub fn effective_used(used: f64, resets_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> f64 {
    match resets_at {
        Some(reset) if now >= reset => 0.0,
        _ => used,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveQuotaItem {
    pub provider: String,          // "claude" | "chatgpt"
    pub name: String,              // e.g. "<Plan> (Claude Code)" | "<Plan> (Codex)"
    pub plan: String,              // e.g. "Claude Pro" | "ChatGPT Plus"
    pub model: String,             // Model name if available, otherwise empty
    pub five_hour_used_percent: f64,
    pub five_hour_remaining_percent: f64,
    pub five_hour_reset_time: String,
    pub five_hour_target_time: String,
    pub weekly_used_percent: f64,
    pub weekly_remaining_percent: f64,
    pub weekly_reset_time: String,
    pub cloud_credits_remaining: Option<f64>,
    pub cloud_credits_total: Option<f64>,
    pub cloud_credits_expiry: Option<String>,
    pub status: String,            // "active" | "low" | "exhausted"
    pub source: String,            // "live_oauth" | "live_rollout_log"
    pub last_updated_str: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAiQuotasResponse {
    pub claude: Option<LiveQuotaItem>,
    pub codex: Option<LiveQuotaItem>,
}

pub fn format_claude_plan(subscription_type: Option<&str>) -> String {
    match subscription_type {
        Some("pro") => "Claude Pro".to_string(),
        Some("max") => "Claude Max".to_string(),
        Some("team") => "Claude Team".to_string(),
        Some("enterprise") => "Claude Enterprise".to_string(),
        Some("free") => "Claude Free".to_string(),
        Some(s) if !s.trim().is_empty() => {
            let trimmed = s.trim();
            let mut chars = trimmed.chars();
            let capitalized = match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            };
            format!("Claude {}", capitalized)
        }
        _ => "Claude".to_string(),
    }
}

pub fn format_codex_plan(plan_type: Option<&str>) -> String {
    match plan_type {
        Some("plus") => "ChatGPT Plus".to_string(),
        Some("pro") => "ChatGPT Pro".to_string(),
        Some("team") => "ChatGPT Team".to_string(),
        Some("business") => "ChatGPT Business".to_string(),
        Some("enterprise") => "ChatGPT Enterprise".to_string(),
        Some("edu") => "ChatGPT Edu".to_string(),
        Some("free") => "ChatGPT Free".to_string(),
        Some(s) if !s.trim().is_empty() => format!("ChatGPT {}", s.trim()),
        _ => "ChatGPT".to_string(),
    }
}

// ---------------------------------------------------------------------------
// 1. CLAUDE CODE LIVE QUOTA FETCHER
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
struct ClaudeCredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<ClaudeOauthDetails>,
}

#[derive(Deserialize)]
struct ClaudeOauthDetails {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsageResponse {
    five_hour: Option<AnthropicWindowDetails>,
    seven_day: Option<AnthropicWindowDetails>,
    iguana_necktie: Option<AnthropicCreditDetails>,
}

#[derive(Deserialize)]
struct AnthropicWindowDetails {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicCreditDetails {
    limit_dollars: Option<f64>,
    remaining_dollars: Option<f64>,
    resets_at: Option<String>,
}

static CLAUDE_CACHE: Mutex<Option<(Instant, LiveQuotaItem)>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeUsageDiskCache {
    item: LiveQuotaItem,
    five_hour_raw_used: f64,
    seven_day_raw_used: f64,
    five_hour_resets_at: Option<DateTime<Utc>>,
    seven_day_resets_at: Option<DateTime<Utc>>,
    fetched_at: DateTime<Utc>,
}

fn claude_disk_cache_path() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(Path::new(&local).join("AntigravitySwitcher").join("claude_usage_cache.json"))
}

fn save_claude_disk_cache(cache: &ClaudeUsageDiskCache) {
    if let Some(path) = claude_disk_cache_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(cache) {
            let _ = std::fs::write(&path, json);
        }
    }
}

fn load_claude_disk_cache(now_utc: DateTime<Utc>) -> Option<LiveQuotaItem> {
    let path = claude_disk_cache_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    let cache: ClaudeUsageDiskCache = serde_json::from_str(&content).ok()?;

    let five_used = effective_used(cache.five_hour_raw_used, cache.five_hour_resets_at, now_utc);
    let five_rem = (100.0 - five_used).max(0.0).min(100.0);

    let (five_hour_target_time, five_hour_reset_time) = match cache.five_hour_resets_at {
        Some(dt) if now_utc >= dt => (String::new(), "Window reset".to_string()),
        Some(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            let target = local.format("%H:%M").to_string();
            let reset = format!("Resets at {}", target);
            (target, reset)
        }
        None => (String::new(), "Active window".to_string()),
    };

    let weekly_used = effective_used(cache.seven_day_raw_used, cache.seven_day_resets_at, now_utc);
    let weekly_rem = (100.0 - weekly_used).max(0.0).min(100.0);

    let weekly_reset_time = match cache.seven_day_resets_at {
        Some(dt) if now_utc >= dt => "Window reset".to_string(),
        Some(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            local.format("Resets %a %I:%M %p").to_string()
        }
        None => "Active cycle".to_string(),
    };

    let status = if five_rem <= 0.0 || weekly_rem <= 0.0 {
        "exhausted".to_string()
    } else if five_rem < 20.0 || weekly_rem < 20.0 {
        "low".to_string()
    } else {
        "active".to_string()
    };

    let mut item = cache.item;
    item.five_hour_used_percent = five_used.round();
    item.five_hour_remaining_percent = five_rem.round();
    item.five_hour_target_time = five_hour_target_time;
    item.five_hour_reset_time = five_hour_reset_time;
    item.weekly_used_percent = weekly_used.round();
    item.weekly_remaining_percent = weekly_rem.round();
    item.weekly_reset_time = weekly_reset_time;
    item.status = status;
    item.source = "cached_oauth".to_string();
    item.last_updated_str = cache.fetched_at.with_timezone(&chrono::Local).format("%H:%M").to_string();

    Some(item)
}

async fn fetch_claude_live_quota() -> Option<LiveQuotaItem> {
    // 1. Check in-memory cache first (fresh within 45 seconds to avoid Anthropic 429 Rate Limits)
    if let Ok(guard) = CLAUDE_CACHE.lock() {
        if let Some((cached_at, ref item)) = *guard {
            if cached_at.elapsed() < Duration::from_secs(45) {
                return Some(item.clone());
            }
        }
    }

    let user_profile = match std::env::var_os("USERPROFILE") {
        Some(p) => p,
        None => return load_claude_disk_cache(Utc::now()),
    };
    let creds_path = Path::new(&user_profile)
        .join(".claude")
        .join(".credentials.json");

    if !creds_path.exists() {
        return load_claude_disk_cache(Utc::now());
    }

    let file_content = match std::fs::read_to_string(&creds_path) {
        Ok(c) => c,
        Err(_) => return load_claude_disk_cache(Utc::now()),
    };

    let parsed: ClaudeCredentialsFile = match serde_json::from_str(&file_content) {
        Ok(p) => p,
        Err(_) => return load_claude_disk_cache(Utc::now()),
    };

    let oauth = match parsed.claude_ai_oauth {
        Some(o) => o,
        None => return load_claude_disk_cache(Utc::now()),
    };

    let token = match oauth.access_token {
        Some(t) if !t.trim().is_empty() => t,
        _ => return load_claude_disk_cache(Utc::now()),
    };

    // QA Check: Check token expiry. NEVER refresh token automatically to avoid invalidating user's Claude Code session.
    if let Some(exp) = oauth.expires_at {
        let now_ms = chrono::Utc::now().timestamp_millis();
        if now_ms >= exp {
            log::warn!("Claude Code accessToken expired at {}, skipping remote poll", exp);
            return load_claude_disk_cache(Utc::now());
        }
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .connect_timeout(Duration::from_secs(2))
        .build() {
        Ok(c) => c,
        Err(_) => return load_claude_disk_cache(Utc::now()),
    };

    let resp = match client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "Claude-Code/2.1.284")
        .send()
        .await {
        Ok(r) => r,
        Err(_) => return load_claude_disk_cache(Utc::now()),
    };

    if !resp.status().is_success() {
        return load_claude_disk_cache(Utc::now());
    }

    let usage: AnthropicUsageResponse = match resp.json().await {
        Ok(u) => u,
        Err(_) => return load_claude_disk_cache(Utc::now()),
    };

    let now_utc = Utc::now();

    let five_resets_at = usage
        .five_hour
        .as_ref()
        .and_then(|w| w.resets_at.as_deref())
        .and_then(|raw_iso| DateTime::parse_from_rfc3339(raw_iso).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let raw_five_used = usage.five_hour.as_ref().and_then(|w| w.utilization).unwrap_or(0.0);
    let five_used = effective_used(raw_five_used, five_resets_at, now_utc);
    let five_rem = (100.0 - five_used).max(0.0).min(100.0);

    let (five_hour_target_time, five_hour_reset_time) = match five_resets_at {
        Some(dt) if now_utc >= dt => (String::new(), "Window reset".to_string()),
        Some(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            let target = local.format("%H:%M").to_string();
            let reset = format!("Resets at {}", target);
            (target, reset)
        }
        None => (String::new(), "Active window".to_string()),
    };

    let weekly_resets_at = usage
        .seven_day
        .as_ref()
        .and_then(|w| w.resets_at.as_deref())
        .and_then(|raw_iso| DateTime::parse_from_rfc3339(raw_iso).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let raw_weekly_used = usage.seven_day.as_ref().and_then(|w| w.utilization).unwrap_or(0.0);
    let weekly_used = effective_used(raw_weekly_used, weekly_resets_at, now_utc);
    let weekly_rem = (100.0 - weekly_used).max(0.0).min(100.0);

    let weekly_reset_time = match weekly_resets_at {
        Some(dt) if now_utc >= dt => "Window reset".to_string(),
        Some(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            local.format("Resets %a %I:%M %p").to_string()
        }
        None => "Active cycle".to_string(),
    };

    let (credit_rem, credit_total, credit_exp) = if let Some(credits) = usage.iguana_necktie {
        let exp_formatted = credits
            .resets_at
            .as_deref()
            .and_then(|iso| DateTime::parse_from_rfc3339(iso).ok())
            .map(|dt| dt.with_timezone(&chrono::Local).format("Expires %I:%M %p, %b %d").to_string());
        (credits.remaining_dollars, credits.limit_dollars, exp_formatted)
    } else {
        (None, None, None)
    };

    let status = if five_rem <= 0.0 || weekly_rem <= 0.0 {
        "exhausted".to_string()
    } else if five_rem < 20.0 || weekly_rem < 20.0 {
        "low".to_string()
    } else {
        "active".to_string()
    };

    let plan = format_claude_plan(oauth.subscription_type.as_deref());
    let name = format!("{} (Claude Code)", plan);
    let last_updated_str = chrono::Local::now().format("%H:%M").to_string();

    let item = LiveQuotaItem {
        provider: "claude".to_string(),
        name,
        plan,
        model: String::new(),
        five_hour_used_percent: five_used.round(),
        five_hour_remaining_percent: five_rem.round(),
        five_hour_reset_time,
        five_hour_target_time,
        weekly_used_percent: weekly_used.round(),
        weekly_remaining_percent: weekly_rem.round(),
        weekly_reset_time,
        cloud_credits_remaining: credit_rem.map(|v| (v * 100.0).round() / 100.0),
        cloud_credits_total: credit_total.map(|v| (v * 100.0).round() / 100.0),
        cloud_credits_expiry: credit_exp,
        status,
        source: "live_oauth".to_string(),
        last_updated_str,
    };

    save_claude_disk_cache(&ClaudeUsageDiskCache {
        item: item.clone(),
        five_hour_raw_used: raw_five_used,
        seven_day_raw_used: raw_weekly_used,
        five_hour_resets_at: five_resets_at,
        seven_day_resets_at: weekly_resets_at,
        fetched_at: now_utc,
    });

    if let Ok(mut guard) = CLAUDE_CACHE.lock() {
        *guard = Some((Instant::now(), item.clone()));
    }

    Some(item)
}

// ---------------------------------------------------------------------------
// 2. OPENAI CODEX LIVE QUOTA FETCHER (Defensive Tail-Read of Rollout Logs)
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
#[allow(dead_code)]
struct CodexEventPayload {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    event_type: Option<String>,
    payload: Option<CodexEventInnerPayload>,
}

#[derive(Deserialize)]
struct CodexEventInnerPayload {
    #[serde(rename = "type")]
    inner_type: Option<String>,
    rate_limits: Option<CodexRateLimitsOuter>,
}

#[derive(Deserialize)]
struct CodexRateLimitsOuter {
    plan_type: Option<String>,
    primary: Option<CodexLimitWindow>,
    secondary: Option<CodexLimitWindow>,
}

#[derive(Deserialize)]
struct CodexLimitWindow {
    used_percent: Option<f64>,
    resets_at: Option<i64>,
}

fn find_recent_codex_rollout_files() -> Vec<PathBuf> {
    let user_profile = match std::env::var_os("USERPROFILE") {
        Some(p) => p,
        None => return Vec::new(),
    };
    let sessions_dir = Path::new(&user_profile)
        .join(".codex")
        .join("sessions");

    if !sessions_dir.exists() {
        return Vec::new();
    }

    // Defensive scan: Traverse recent year/month/day without exhaustive deep listing
    let mut files = Vec::new();
    if let Ok(years) = std::fs::read_dir(&sessions_dir) {
        for year_entry in years.flatten() {
            if !year_entry.path().is_dir() { continue; }
            if let Ok(months) = std::fs::read_dir(year_entry.path()) {
                for month_entry in months.flatten() {
                    if !month_entry.path().is_dir() { continue; }
                    if let Ok(days) = std::fs::read_dir(month_entry.path()) {
                        for day_entry in days.flatten() {
                            if !day_entry.path().is_dir() { continue; }
                            if let Ok(session_files) = std::fs::read_dir(day_entry.path()) {
                                for f in session_files.flatten() {
                                    let path = f.path();
                                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                                        if let Ok(meta) = f.metadata() {
                                            if let Ok(mtime) = meta.modified() {
                                                files.push((mtime, path));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    files.sort_by(|a, b| b.0.cmp(&a.0));
    // Inspect up to 8 most recent rollout session files to find the absolute latest telemetry event
    files.into_iter().take(8).map(|(_, p)| p).collect()
}

fn fetch_codex_live_quota() -> Option<LiveQuotaItem> {
    let recent_files = find_recent_codex_rollout_files();
    if recent_files.is_empty() {
        return None;
    }

    let mut best_timestamp = String::new();
    let mut best_rate_limits: Option<CodexRateLimitsOuter> = None;

    for rollout_path in recent_files {
        let Ok(mut file) = File::open(&rollout_path) else { continue };
        let Ok(meta) = file.metadata() else { continue };
        let file_len = meta.len();
        if file_len == 0 { continue; }

        // QA Check: Tail read only the last 64KB to avoid memory / IO spikes on large multi-megabyte session files
        let chunk_size = 65536u64.min(file_len);
        let seek_offset = file_len.saturating_sub(chunk_size);
        if file.seek(SeekFrom::Start(seek_offset)).is_err() { continue; }

        let mut buffer = Vec::with_capacity(chunk_size as usize);
        if file.read_to_end(&mut buffer).is_err() { continue; }

        let content_lossy = String::from_utf8_lossy(&buffer);

        // Read lines in reverse to grab the most recent token_count rate_limits event
        for line in content_lossy.lines().rev() {
            if !line.contains("\"rate_limits\"") {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<CodexEventPayload>(line) {
                if let Some(inner) = event.payload {
                    if inner.inner_type.as_deref() == Some("token_count") {
                        if let Some(rate_limits) = inner.rate_limits {
                            let ts = event.timestamp.unwrap_or_default();
                            if ts > best_timestamp {
                                best_timestamp = ts;
                                best_rate_limits = Some(rate_limits);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    let rate_limits = best_rate_limits?;
    let primary = rate_limits.primary.as_ref();
    let secondary = rate_limits.secondary.as_ref();

    let raw_five_used = primary.and_then(|p| p.used_percent).unwrap_or(0.0);

    let now_utc = Utc::now();

    let primary_resets_at = primary
        .and_then(|p| p.resets_at)
        .and_then(|ts| DateTime::from_timestamp(ts, 0));

    let five_used = effective_used(raw_five_used, primary_resets_at, now_utc);
    let five_rem = (100.0 - five_used).max(0.0).min(100.0);

    let (five_target_time, five_reset_time) = match primary_resets_at {
        Some(dt) if now_utc >= dt => (String::new(), "Window reset".to_string()),
        Some(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            let target = local.format("%H:%M").to_string();
            let reset = format!("Resets at {}", target);
            (target, reset)
        }
        None => (String::new(), "Active window".to_string()),
    };

    let secondary_resets_at = secondary
        .and_then(|s| s.resets_at)
        .and_then(|ts| DateTime::from_timestamp(ts, 0));

    let raw_weekly_used = secondary.and_then(|s| s.used_percent).unwrap_or(0.0);
    let weekly_used = effective_used(raw_weekly_used, secondary_resets_at, now_utc);
    let weekly_rem = (100.0 - weekly_used).max(0.0).min(100.0);

    let weekly_reset_time = match secondary_resets_at {
        Some(dt) if now_utc >= dt => "Window reset".to_string(),
        Some(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            local.format("Resets %a %b %d").to_string()
        }
        None => "Active cycle".to_string(),
    };

    let plan = format_codex_plan(rate_limits.plan_type.as_deref());
    let name = format!("{} (Codex)", plan);

    let status = if five_rem <= 0.0 || weekly_rem <= 0.0 {
        "exhausted".to_string()
    } else if five_rem < 20.0 || weekly_rem < 20.0 {
        "low".to_string()
    } else {
        "active".to_string()
    };

    Some(LiveQuotaItem {
        provider: "chatgpt".to_string(),
        name,
        plan,
        model: String::new(),
        five_hour_used_percent: five_used.round(),
        five_hour_remaining_percent: five_rem.round(),
        five_hour_reset_time: five_reset_time,
        five_hour_target_time: five_target_time,
        weekly_used_percent: weekly_used.round(),
        weekly_remaining_percent: weekly_rem.round(),
        weekly_reset_time,
        cloud_credits_remaining: None,
        cloud_credits_total: None,
        cloud_credits_expiry: None,
        status,
        source: "live_rollout_log".to_string(),
        last_updated_str: if !best_timestamp.is_empty() {
            DateTime::parse_from_rfc3339(&best_timestamp)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
                .unwrap_or_else(|_| chrono::Local::now().format("%H:%M").to_string())
        } else {
            chrono::Local::now().format("%H:%M").to_string()
        },
    })
}

// ---------------------------------------------------------------------------
// 3. COMBINED TAURI COMMAND HANDLER
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_external_ai_quotas() -> Result<ExternalAiQuotasResponse, String> {
    // Run Claude fetch in async runtime with safety
    let claude_res = fetch_claude_live_quota().await;

    // Run Codex disk tail-read in blocking thread so UI remains 100% fluid
    let codex_res = tokio::task::spawn_blocking(fetch_codex_live_quota)
        .await
        .unwrap_or(None);

    Ok(ExternalAiQuotasResponse {
        claude: claude_res,
        codex: codex_res,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_used() {
        let now = Utc::now();
        let past = now - chrono::Duration::minutes(30);
        let future = now + chrono::Duration::minutes(30);

        // Reset time passed -> window starts over
        assert_eq!(effective_used(70.0, Some(past), now), 0.0);
        // Reset still ahead -> unchanged
        assert_eq!(effective_used(70.0, Some(future), now), 70.0);
        // Unknown reset -> unchanged
        assert_eq!(effective_used(70.0, None, now), 70.0);
    }

    #[test]
    fn test_format_claude_plan() {
        assert_eq!(format_claude_plan(Some("pro")), "Claude Pro");
        assert_eq!(format_claude_plan(Some("max")), "Claude Max");
        assert_eq!(format_claude_plan(Some("team")), "Claude Team");
        assert_eq!(format_claude_plan(Some("enterprise")), "Claude Enterprise");
        assert_eq!(format_claude_plan(Some("free")), "Claude Free");
        assert_eq!(format_claude_plan(Some("custom")), "Claude Custom");
        assert_eq!(format_claude_plan(Some("CustomTier")), "Claude CustomTier");
        assert_eq!(format_claude_plan(None), "Claude");
        assert_eq!(format_claude_plan(Some("")), "Claude");
        assert_eq!(format_claude_plan(Some("   ")), "Claude");
    }

    #[test]
    fn test_format_codex_plan() {
        assert_eq!(format_codex_plan(Some("plus")), "ChatGPT Plus");
        assert_eq!(format_codex_plan(Some("pro")), "ChatGPT Pro");
        assert_eq!(format_codex_plan(Some("team")), "ChatGPT Team");
        assert_eq!(format_codex_plan(Some("business")), "ChatGPT Business");
        assert_eq!(format_codex_plan(Some("enterprise")), "ChatGPT Enterprise");
        assert_eq!(format_codex_plan(Some("edu")), "ChatGPT Edu");
        assert_eq!(format_codex_plan(Some("free")), "ChatGPT Free");
        assert_eq!(format_codex_plan(Some("custom")), "ChatGPT custom");
        assert_eq!(format_codex_plan(None), "ChatGPT");
        assert_eq!(format_codex_plan(Some("")), "ChatGPT");
        assert_eq!(format_codex_plan(Some("   ")), "ChatGPT");
    }
}

