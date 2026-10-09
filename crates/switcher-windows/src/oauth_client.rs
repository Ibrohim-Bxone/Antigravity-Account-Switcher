pub const DEFAULT_CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";

pub fn resolve_client_id(env_val: Option<String>) -> String {
    env_val
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string())
}

pub fn resolve_secret(env_val: Option<String>) -> Result<String, String> {
    env_val
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            "ANTIGRAVITY_OAUTH_CLIENT_SECRET is not set. See README > OAuth configuration."
                .to_string()
        })
}

pub fn oauth_client_credentials() -> Result<(String, String), String> {
    let client_id = resolve_client_id(std::env::var("ANTIGRAVITY_OAUTH_CLIENT_ID").ok());
    let client_secret = resolve_secret(std::env::var("ANTIGRAVITY_OAUTH_CLIENT_SECRET").ok())?;
    Ok((client_id, client_secret))
}
