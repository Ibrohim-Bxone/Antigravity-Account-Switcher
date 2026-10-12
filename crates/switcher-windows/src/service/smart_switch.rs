/**
 * Smart Switch automation with Process Guardian & Token Health Shield.
 * Periodically checks Gemini usage levels and automatically swaps active accounts when limits are exhausted.
 * CRITICAL SAFETY: Never interrupts active tasks, compiler jobs, or subagents.
 * Never switches to accounts requiring re-verification.
 * Main exports: impl SwitcherService smart switch methods
 */
use rusqlite::Connection;
use uuid::Uuid;

use crate::SwitcherService;
use switcher_core::{ProfileQuotaView, Result, TokenStatus};

fn get_bucket_remaining_fraction(quota: &ProfileQuotaView, bucket_id: &str) -> Option<f64> {
    for group in &quota.quota_groups {
        if let Some(bucket) = group.buckets.iter().find(|b| b.bucket_id == bucket_id) {
            return Some(bucket.remaining_fraction);
        }
    }
    None
}

use std::sync::{Mutex as StdMutex, OnceLock};
use std::collections::HashMap;

pub fn active_probe_state() -> &'static StdMutex<Option<(Uuid, std::time::Instant)>> {
    static STATE: OnceLock<StdMutex<Option<(Uuid, std::time::Instant)>>> = OnceLock::new();
    STATE.get_or_init(|| StdMutex::new(None))
}

pub fn quota_exhausted_state() -> &'static StdMutex<HashMap<Uuid, chrono::DateTime<chrono::Utc>>> {
    static STATE: OnceLock<StdMutex<HashMap<Uuid, chrono::DateTime<chrono::Utc>>>> = OnceLock::new();
    STATE.get_or_init(|| StdMutex::new(HashMap::new()))
}

fn probe_in_progress() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

static PROBE_MODE_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LOCK_LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);


impl SwitcherService {
    /**
     * Multi-layered activity check (Process Guardian):
     * 1. Detects active sub-processes (PowerShell, Cargo, Git, Node, Python, agy) running under Antigravity.
     * 2. Detects real-time transcript/brain file write activity in ~/.gemini/antigravity/brain.
     * 3. Queries SQLite state database item table if available.
     * Returns true if ANY background work is active, blocking any switch operation.
     */
    pub fn is_agent_working(&self) -> bool {
        if cfg!(debug_assertions) && std::env::var("MOCK_AGENT_WORKING").is_ok() {
            return std::env::var("MOCK_AGENT_WORKING").unwrap() == "1";
        }
        
        // 1. Process Tree Check: Check if Antigravity or its child processes are running task workers
        if let Ok(process_mgr) = self.process_manager() {
            if let Ok(procs) = process_mgr.enumerate() {
                for p in &procs {
                    let name = p.name.to_lowercase();
                    // Active compilation, build, or heavy task runner processes
                    if matches!(
                        name.as_str(),
                        "cargo.exe"
                            | "rustc.exe"
                            | "agy.exe"
                            | "docker.exe"
                            | "cl.exe"
                            | "link.exe"
                            | "make.exe"
                            | "tsc.exe"
                    ) {
                        return true;
                    }
                }
            }
        }

        // 2. Fast Brain Activity Check: Check active session transcript files without 25,000 recursive walks
        if let Some(user_profile) = std::env::var_os("USERPROFILE") {
            let brain_dir = std::path::Path::new(&user_profile)
                .join(".gemini")
                .join("antigravity")
                .join("brain");
            if brain_dir.is_dir() {
                let now = std::time::SystemTime::now();
                if let Ok(entries) = std::fs::read_dir(&brain_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            // Check top-level folder modified time
                            if let Ok(meta) = entry.metadata() {
                                if let Ok(modified) = meta.modified() {
                                    // mtime after `now` (written during the scan) means active, not unknown
                                    let elapsed = now.duration_since(modified).unwrap_or_default();
                                    if elapsed.as_secs() < 120 {
                                        return true;
                                    }
                                }
                            }
                            // Also check transcript.jsonl inside active session
                            let transcript_path = path
                                .join(".system_generated")
                                .join("logs")
                                .join("transcript.jsonl");
                            if transcript_path.is_file() {
                                if let Ok(meta) = std::fs::metadata(&transcript_path) {
                                    if let Ok(modified) = meta.modified() {
                                        let elapsed = now.duration_since(modified).unwrap_or_default();
                                        if elapsed.as_secs() < 120 {
                                            return true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. SQLite ItemTable check (read-only with busy timeout to avoid lock conflicts)
        let path = &self.paths.state_db;
        if path.is_file() {
            if let Ok(conn) = Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
            ) {
                let _ = conn.busy_timeout(std::time::Duration::from_millis(300));
                let query = "SELECT value FROM ItemTable WHERE key = 'antigravity.agent.working'";
                if let Ok(working_str) = conn.query_row(query, [], |row| row.get::<_, String>(0)) {
                    if working_str.trim().to_lowercase() == "true" {
                        return true;
                    }
                }
            }
        }

        false
    }

    pub async fn check_and_perform_smart_switch(&self) -> Result<()> {
        let probe = crate::service::eligibility::RealEligibilityProbe;
        self.check_and_perform_smart_switch_with_probe(&probe).await
    }

    pub async fn check_and_perform_smart_switch_with_probe<P: crate::service::eligibility::EligibilityProbe>(
        &self,
        probe: &P,
    ) -> Result<()> {
        if !self.config.read().smart_switch_enabled {
            return Ok(());
        }

        // While the master password lock is on, list_profiles returns nothing and Smart Switch
        // would idle silently. Say so once in the log.
        let app_locked = self.paths.root.join("master_lock.json").is_file() && self.master_key.read().is_none();
        if app_locked {
            if !LOCK_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                self.logger.warn(None, "smart_switch", "App is locked (master password) - Smart Switch paused until unlocked".to_string());
            }
            return Ok(());
        }
        LOCK_LOGGED.store(false, std::sync::atomic::Ordering::Relaxed);

        if self.journal().exists() {
            return Ok(());
        }
        if self.progress.read().is_some() {
            return Ok(());
        }

        let mut attempts = 0;
        let max_attempts = 5;

        while attempts < max_attempts {
            attempts += 1;

            let active_profile_id = match self.config.read().active_profile_id {
                Some(id) => id,
                None => return Ok(()),
            };

            let profiles = self.list_profiles_live(Some(active_profile_id)).await?;
            let mut active_profile = match profiles.iter().find(|p| p.is_active) {
                Some(p) => p.clone(),
                None => return Ok(()),
            };

            // Probe logic for active profile
            let mut should_probe = false;
            let is_probe_mode = active_profile.quota.is_none();
            
            if is_probe_mode {
                if !PROBE_MODE_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    self.logger.info(None, "smart_switch", "Entering probe mode (quota unavailable)".to_string());
                }
            } else {
                PROBE_MODE_LOGGED.store(false, std::sync::atomic::Ordering::Relaxed);
            }

            {
                let last = active_probe_state().lock().unwrap();
                match *last {
                    Some((id, time)) => {
                        if id != active_profile_id {
                            should_probe = true;
                        } else {
                            if matches!(active_profile.metadata.eligibility, switcher_core::EligibilityStatus::Eligible) {
                                let interval = if is_probe_mode { 10 * 60 } else { 30 * 60 };
                                if time.elapsed() >= std::time::Duration::from_secs(interval) {
                                    should_probe = true;
                                }
                            } else if matches!(active_profile.metadata.eligibility, switcher_core::EligibilityStatus::Unknown) {
                                if time.elapsed() >= std::time::Duration::from_secs(120) {
                                    should_probe = true;
                                }
                            }
                        }
                    }
                    None => {
                        should_probe = true;
                    }
                }
            }

            if should_probe {
                if let Ok(_guard) = probe_in_progress().try_lock() {
                    let probe_res = tokio::time::timeout(std::time::Duration::from_secs(90), probe.probe()).await;
                    let mut new_eligibility = active_profile.metadata.eligibility.clone();
                    let mut is_error = false;
                    
                    match &probe_res {
                        Ok(crate::service::eligibility::ProbeResult::NeedsVerification(msg)) => {
                            new_eligibility = switcher_core::EligibilityStatus::NeedsVerification {
                                since: chrono::Utc::now(),
                                message: msg.clone(),
                            };
                        }
                        Ok(crate::service::eligibility::ProbeResult::QuotaExhausted(_msg)) => {
                            quota_exhausted_state().lock().unwrap().insert(active_profile_id, chrono::Utc::now());
                        }
                        Ok(crate::service::eligibility::ProbeResult::Ok) => {
                            new_eligibility = switcher_core::EligibilityStatus::Eligible;
                        }
                        Ok(crate::service::eligibility::ProbeResult::Error(e)) => {
                            is_error = true;
                            self.logger.warn(None, "smart_switch", format!("Probe error for active account: {}", e));
                        }
                        Err(_) => {
                            is_error = true;
                            self.logger.warn(None, "smart_switch", "Probe timed out for active account".to_string());
                        }
                    }

                    if new_eligibility != active_profile.metadata.eligibility {
                        if let Ok(mut metadata) = self.require_profile(active_profile_id) {
                            metadata.eligibility = new_eligibility.clone();
                            let _ = self.save_profile_metadata(active_profile_id, &metadata);
                            active_profile.metadata.eligibility = new_eligibility;
                        }
                    }

                    if is_error {
                        // Retry 2 minutes after an error. Unknown uses a 120s threshold, so backdating
                        // by `interval - 120` would re-probe it on every 10s tick.
                        let interval = if is_probe_mode { 10 * 60 } else { 30 * 60 };
                        let threshold = if matches!(active_profile.metadata.eligibility, switcher_core::EligibilityStatus::Eligible) { interval } else { 120 };
                        let retry_time = std::time::Instant::now()
                            .checked_sub(std::time::Duration::from_secs(threshold - 120))
                            .unwrap_or_else(|| std::time::Instant::now());
                        *active_probe_state().lock().unwrap() = Some((active_profile_id, retry_time));
                    } else {
                        *active_probe_state().lock().unwrap() = Some((active_profile_id, std::time::Instant::now()));
                    }
                }
            }

            let mut is_emergency = matches!(active_profile.metadata.eligibility, switcher_core::EligibilityStatus::NeedsVerification { .. });

            if is_probe_mode {
                let exhausted = {
                    let map = quota_exhausted_state().lock().unwrap();
                    if let Some(time) = map.get(&active_profile_id) {
                        chrono::Utc::now().signed_duration_since(*time).num_hours() < 5
                    } else {
                        false
                    }
                };

                if exhausted {
                    is_emergency = true;
                }

                if !exhausted && !matches!(active_profile.metadata.eligibility, switcher_core::EligibilityStatus::NeedsVerification { .. }) {
                    return Ok(());
                }

                if !is_emergency && self.is_agent_working() {
                    return Ok(());
                }

                self.logger.warn(
                    None,
                    "smart_switch",
                    "Active profile probe returned QuotaExhausted or NeedsVerification. Finding alternative profile in probe mode...",
                );
            } else {
                let active_quota = active_profile.quota.as_ref().unwrap();

                let rem_5h = get_bucket_remaining_fraction(active_quota, "gemini-5h").unwrap_or(1.0);
                let rem_weekly =
                    get_bucket_remaining_fraction(active_quota, "gemini-weekly").unwrap_or(1.0);
                let rem_3p_5h = get_bucket_remaining_fraction(active_quota, "3p-5h").unwrap_or(1.0);
                let rem_3p_weekly =
                    get_bucket_remaining_fraction(active_quota, "3p-weekly").unwrap_or(1.0);

                let gemini_exhausted = rem_5h < 0.10 || rem_weekly < 0.05;
                let p3_exhausted = rem_3p_5h < 0.10 || rem_3p_weekly < 0.05;

                // If active profile still has ample quota, do nothing
                if !gemini_exhausted && !p3_exhausted && !matches!(active_profile.metadata.eligibility, switcher_core::EligibilityStatus::NeedsVerification { .. }) {
                    return Ok(());
                }

                if rem_5h < 0.03 || rem_weekly < 0.02 {
                    is_emergency = true;
                }

                if !is_emergency && self.is_agent_working() {
                    self.logger.warn(
                        None,
                        "smart_switch",
                        "Automated switch DEFERRED: Heavy compiler/task running while buffer quota remains.",
                    );
                    return Ok(());
                }

                self.logger.warn(
                    None,
                    "smart_switch",
                    format!(
                        "Active profile limits exhausted (Gemini: 5h={:.1}%, weekly={:.1}%; Claude/Codex: 5h={:.1}%, weekly={:.1}%). Finding verified alternative profile...",
                        rem_5h * 100.0,
                        rem_weekly * 100.0,
                        rem_3p_5h * 100.0,
                        rem_3p_weekly * 100.0,
                    ),
                );
            }

            let mut candidate: Option<(Uuid, f64)> = None;
            let mut candidate_no_quota: Option<Uuid> = None;

            for profile in &profiles {
                if profile.is_active || profile.token_status != TokenStatus::Valid {
                    continue;
                }

                if let switcher_core::EligibilityStatus::NeedsVerification { .. } = profile.metadata.eligibility {
                    continue;
                }

                if is_probe_mode {
                    let exhausted = {
                        let map = quota_exhausted_state().lock().unwrap();
                        if let Some(time) = map.get(&profile.metadata.profile_id) {
                            chrono::Utc::now().signed_duration_since(*time).num_hours() < 5
                        } else {
                            false
                        }
                    };
                    if !exhausted {
                        if let Some(existing) = candidate_no_quota {
                            let existing_prof = profiles.iter().find(|p| p.metadata.profile_id == existing).unwrap();
                            if profile.metadata.last_activated_at < existing_prof.metadata.last_activated_at {
                                candidate_no_quota = Some(profile.metadata.profile_id);
                            }
                        } else {
                            candidate_no_quota = Some(profile.metadata.profile_id);
                        }
                    }
                } else {
                    if let Some(ref q) = profile.quota {
                        let cand_5h = get_bucket_remaining_fraction(q, "gemini-5h").unwrap_or(0.0);
                        let cand_weekly = get_bucket_remaining_fraction(q, "gemini-weekly").unwrap_or(0.0);

                        if cand_5h >= 0.15 && cand_weekly >= 0.05 {
                            if let Some((_, best_5h)) = candidate {
                                if cand_5h > best_5h {
                                    candidate = Some((profile.metadata.profile_id, cand_5h));
                                }
                            } else {
                                candidate = Some((profile.metadata.profile_id, cand_5h));
                            }
                        }
                    }
                }
            }

            let target_id = if is_probe_mode { candidate_no_quota } else { candidate.map(|(id, _)| id) };

            if let Some(target_id) = target_id {
                self.logger.warn(
                    None,
                    "smart_switch",
                    format!("Triggering safe verified smart switch to profile {}", target_id),
                );
                
                let is_test_bypass = cfg!(debug_assertions) && std::env::var("BYPASS_SYNC_ACTIVE_PROFILE").is_ok();
                let mut switch_ok = true;
                
                if is_test_bypass {
                    self.logger.info(None, "smart_switch", format!("Bypassing actual switch for test to {}", target_id));
                    let mut config = self.config.write();
                    config.active_profile_id = Some(target_id);
                    let _ = switcher_core::save_json(&self.paths.config, &*config);
                } else {
                    match self.request_switch(target_id, None) {
                        Ok(req) => {
                            if let Err(e) = self.confirm_switch(req.operation_id) {
                                self.logger.error(None, "smart_switch", format!("Smart switch confirm failed: {}", e));
                                switch_ok = false;
                            }
                        }
                        Err(e) => {
                            self.logger.error(None, "smart_switch", format!("Smart switch request failed: {}", e));
                            switch_ok = false;
                        }
                    }
                }

                if !switch_ok {
                    break;
                }

                // language_server restarts after a switch; probing immediately makes agy return
                // "context canceled". Wait, and retry once on error.
                if !is_test_bypass {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                }
                let mut probe_result = probe.probe().await;
                if !is_test_bypass && matches!(probe_result, crate::service::eligibility::ProbeResult::Error(_)) {
                    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    probe_result = probe.probe().await;
                }
                // The new active account was just probed; do not probe it again on the next tick.
                *active_probe_state().lock().unwrap() = Some((target_id, std::time::Instant::now()));
                match probe_result {
                    crate::service::eligibility::ProbeResult::Ok => {
                        self.logger.info(None, "smart_switch", "Profile verified successfully after switch");
                        // Otherwise the profile stays Unknown and is needlessly re-probed in 2 minutes.
                        if let Ok(mut metadata) = self.require_profile(target_id) {
                            metadata.eligibility = switcher_core::EligibilityStatus::Eligible;
                            let _ = self.save_profile_metadata(target_id, &metadata);
                        }
                        break;
                    }
                    crate::service::eligibility::ProbeResult::NeedsVerification(msg) => {
                        self.logger.error(
                            None,
                            "smart_switch",
                            format!("Profile {} requires verification. Marking as NeedsVerification.", target_id),
                        );
                        if let Ok(mut metadata) = self.require_profile(target_id) {
                            metadata.eligibility = switcher_core::EligibilityStatus::NeedsVerification {
                                since: chrono::Utc::now(),
                                message: msg,
                            };
                            let _ = self.save_profile_metadata(target_id, &metadata);
                        }
                        // Continue to next attempt
                    }
                    crate::service::eligibility::ProbeResult::QuotaExhausted(_msg) => {
                        self.logger.warn(
                            None,
                            "smart_switch",
                            format!("Profile {} hit quota limit immediately. Marking as exhausted.", target_id),
                        );
                        quota_exhausted_state().lock().unwrap().insert(target_id, chrono::Utc::now());
                        // Continue to next attempt
                    }
                    crate::service::eligibility::ProbeResult::Error(err) => {
                        self.logger.error(
                            None,
                            "smart_switch",
                            format!("Eligibility probe failed with error: {}", err),
                        );
                        break;
                    }
                }
            } else {
                self.logger.warn(
                    None,
                    "smart_switch",
                    "No candidate left: all profiles need verification or hit their limit",
                );
                break;
            }
        }

        Ok(())
    }

    #[cfg(debug_assertions)]
    pub async fn force_smart_switch_bypass_quota(&self) -> Result<()> {
        let active_profile_id = match self.config.read().active_profile_id {
            Some(id) => id,
            None => return Err(switcher_core::SwitcherError::NoActiveProfile),
        };

        let profiles = self.list_profiles_live(Some(active_profile_id)).await?;

        let mut candidate: Option<(Uuid, f64, f64)> = None;
        let fallback_candidate: Option<Uuid> = None;

        for profile in &profiles {
            if profile.is_active || profile.token_status != TokenStatus::Valid {
                continue;
            }
            if let Some(ref q) = profile.quota {
                let cand_5h = get_bucket_remaining_fraction(q, "gemini-5h").unwrap_or(0.0);
                let cand_weekly = get_bucket_remaining_fraction(q, "gemini-weekly").unwrap_or(0.0);
                if cand_5h >= 0.15 && cand_weekly >= 0.08 {
                    if let Some((_, best_5h, _)) = candidate {
                        if cand_5h > best_5h {
                            candidate = Some((profile.metadata.profile_id, cand_5h, cand_weekly));
                        }
                    } else {
                        candidate = Some((profile.metadata.profile_id, cand_5h, cand_weekly));
                    }
                }
            }
        }

        let target_id = if let Some((id, _, _)) = candidate {
            id
        } else if let Some(id) = fallback_candidate {
            id
        } else {
            return Err(switcher_core::SwitcherError::Message(
                "No alternative valid profiles found to switch to".to_string(),
            ));
        };

        self.logger.warn(
            None,
            "smart_switch",
            format!(
                "Forcing smart switch to profile {} (bypassing quota checks)",
                target_id
            ),
        );

        match self.request_switch(target_id, None) {
            Ok(req) => {
                let op_id = req.operation_id;
                if let Err(e) = self.confirm_switch(op_id) {
                    self.logger.error(
                        None,
                        "smart_switch",
                        format!("Smart switch confirm failed: {}", e),
                    );
                    return Err(e);
                }
            }
            Err(e) => {
                self.logger.error(
                    None,
                    "smart_switch",
                    format!("Smart switch request failed: {}", e),
                );
                return Err(e);
            }
        }

        Ok(())
    }
}
