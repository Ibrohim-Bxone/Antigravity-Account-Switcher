use std::collections::HashMap;
use switcher_core::{ProfileQuotaView, QuotaBucketView, QuotaGroupView, Result};

pub struct QuotaDecryptor;

/// Prefix of the error returned while quota checks are paused; callers skip logging it.
pub const QUOTA_PAUSED_PREFIX: &str = "Quota checks paused";

// Google rejects the OAuth client itself (invalid_client) for every account until the secret
// changes, yet the UI polls every 5s: 8 profiles produced ~90k failed token requests and log
// lines in 15h (2026-10-11). After a rejection the network is skipped for a while.
const CLIENT_REJECTED_PAUSE: std::time::Duration = std::time::Duration::from_secs(6 * 3600);

fn client_rejected_at() -> &'static std::sync::Mutex<Option<std::time::Instant>> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<Option<std::time::Instant>>> =
        std::sync::OnceLock::new();
    STATE.get_or_init(|| std::sync::Mutex::new(None))
}

pub fn quota_checks_paused() -> bool {
    client_rejected_at()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .is_some_and(|t| t.elapsed() < CLIENT_REJECTED_PAUSE)
}

pub fn note_token_refresh_error(err_body: &str) {
    if err_body.contains("invalid_client") {
        *client_rejected_at().lock().unwrap_or_else(|p| p.into_inner()) =
            Some(std::time::Instant::now());
    }
}

pub fn is_quota_paused_error(err: &str) -> bool {
    err.starts_with(QUOTA_PAUSED_PREFIX)
}

impl QuotaDecryptor {
    pub fn decrypt_all_quotas() -> Result<HashMap<String, ProfileQuotaView>> {
        Ok(HashMap::new())
    }

    pub async fn fetch_quota_summary(
        access_token: &str,
    ) -> std::result::Result<ProfileQuotaView, String> {
        let client = reqwest::Client::new();
        let quota_resp = client
            .post("https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "antigravity/0.19.0 windows/amd64")
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch quota: {e}"))?;

        if !quota_resp.status().is_success() {
            let status = quota_resp.status();
            let err_body = quota_resp.text().await.unwrap_or_default();
            return Err(format!("Quota API returned status {status}: {err_body}"));
        }

        let quota_body = quota_resp
            .text()
            .await
            .map_err(|e| format!("Failed to read quota body: {e}"))?;

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct GoogleQuotaBucket {
            bucket_id: String,
            display_name: String,
            window: String,
            reset_time: Option<String>,
            description: Option<String>,
            remaining_fraction: f64,
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct GoogleQuotaGroup {
            display_name: String,
            description: Option<String>,
            buckets: Vec<GoogleQuotaBucket>,
        }

        #[derive(serde::Deserialize)]
        struct GoogleQuotaResponse {
            groups: Vec<GoogleQuotaGroup>,
        }

        let api_res: GoogleQuotaResponse = serde_json::from_str(&quota_body)
            .map_err(|e| format!("Failed to parse quota JSON: {e}"))?;

        let quota_groups = api_res
            .groups
            .into_iter()
            .map(|g| {
                let buckets = g
                    .buckets
                    .into_iter()
                    .map(|b| QuotaBucketView {
                        bucket_id: b.bucket_id,
                        window: b.window,
                        remaining_fraction: b.remaining_fraction,
                        reset_time: b.reset_time,
                        display_name: b.display_name,
                        description: b.description,
                    })
                    .collect();
                QuotaGroupView {
                    display_name: g.display_name,
                    description: g.description.unwrap_or_default(),
                    buckets,
                }
            })
            .collect();

        Ok(ProfileQuotaView {
            subscription_tier: "Google AI Pro".to_owned(),
            quota_groups,
        })
    }

    pub async fn fetch_live_quota(
        refresh_token: &str,
    ) -> std::result::Result<ProfileQuotaView, String> {
        if quota_checks_paused() {
            return Err(format!(
                "{QUOTA_PAUSED_PREFIX}: OAuth client was rejected (invalid_client)"
            ));
        }
        let (client_id, client_secret) = crate::oauth_client::oauth_client_credentials()?;
        let client = reqwest::Client::new();

        let params = [
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];

        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Failed to refresh token: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            note_token_refresh_error(&err_body);
            return Err(format!(
                "Token refresh returned status {status}: {err_body}"
            ));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read token response: {e}"))?;
        let val: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse token response: {e}"))?;
        let access_token = val
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "No access_token in refresh response".to_owned())?;

        Self::fetch_quota_summary(access_token).await
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_live_quota_skips_network_after_invalid_client() {
        note_token_refresh_error(r#"{"error": "invalid_client", "error_description": "x"}"#);
        assert!(quota_checks_paused());
        let err = QuotaDecryptor::fetch_live_quota("dummy").await.unwrap_err();
        assert!(is_quota_paused_error(&err), "{err}");
    }

    #[test]
    fn test_is_quota_paused_error_ignores_other_errors() {
        assert!(!is_quota_paused_error("Token refresh returned status 400: invalid_grant"));
    }
}
