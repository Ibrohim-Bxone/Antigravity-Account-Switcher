use std::time::Duration;
use tokio::time::timeout;
use std::process::Command;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ProbeResult {
    Ok,
    NeedsVerification(String),
    QuotaExhausted(String),
    Error(String),
}

pub fn is_eligibility_error(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("eligibility check failed") || 
    lower.contains("not eligible for antigravity") || 
    lower.contains("verify your account") ||
    // A stored login that is no longer valid needs a human, just like verification. Treated as
    // a network error, two live accounts were re-probed forever instead of being skipped.
    lower.contains("authentication required") ||
    lower.contains("not logged into antigravity")
}

pub fn is_quota_error(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("quota") ||
    lower.contains("rate limit") ||
    lower.contains("resource_exhausted") ||
    lower.contains("too many requests") ||
    lower.contains("usage limit") ||
    lower.contains("limit reached") ||
    lower.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "429")
}

// Used only with concrete probe types inside this crate and its tests; no Send bound needed.
#[allow(async_fn_in_trait)]
pub trait EligibilityProbe: Send + Sync {
    async fn probe(&self) -> ProbeResult;
}

pub struct RealEligibilityProbe;

impl EligibilityProbe for RealEligibilityProbe {
    async fn probe(&self) -> ProbeResult {
        // Default install location; fall back to PATH when LOCALAPPDATA is not set.
        let exe_path = std::env::var("LOCALAPPDATA")
            .map(|d| format!(r"{}\agy\bin\agy.exe", d))
            .unwrap_or_else(|_| "agy.exe".to_string());

        let future = tokio::task::spawn_blocking(move || {
            let mut cmd = Command::new(exe_path);
            cmd.arg("-p")
               .arg("Reply with exactly: OK")
               .arg("--model")
               .arg("gemini-3.8-flash-low");
            cmd.output()
        });
           
        match timeout(Duration::from_secs(90), future).await {
            Ok(Ok(Ok(output))) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}\n{}", stdout, stderr);
                
                // A clean "OK" wins even if stderr mentions quota; an empty reply (exit 0) is not Ok.
                let clean_ok = output.status.success() && stdout.trim().eq_ignore_ascii_case("ok");
                if is_eligibility_error(&combined) {
                    ProbeResult::NeedsVerification(combined)
                } else if clean_ok {
                    ProbeResult::Ok
                } else if is_quota_error(&combined) {
                    ProbeResult::QuotaExhausted(combined)
                } else if output.status.success() && !stdout.trim().is_empty() {
                    ProbeResult::Ok
                } else {
                    ProbeResult::Error(format!("Command failed with status: {}, output: {}", output.status, combined))
                }
            },
            Ok(Ok(Err(e))) => ProbeResult::Error(format!("Failed to execute agy.exe: {}", e)),
            Ok(Err(e)) => ProbeResult::Error(format!("Task panic: {}", e)),
            Err(_) => ProbeResult::Error("Command timed out after 90 seconds".to_string()),
        }
    }
}
