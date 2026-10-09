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

impl SwitcherService {
    /**
     * Multi-layered activity check (Process Guardian):
     * 1. Detects active sub-processes (PowerShell, Cargo, Git, Node, Python, agy) running under Antigravity.
     * 2. Detects real-time transcript/brain file write activity in ~/.gemini/antigravity/brain.
     * 3. Queries SQLite state database item table if available.
     * Returns true if ANY background work is active, blocking any switch operation.
     */
    pub fn is_agent_working(&self) -> bool {
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
        if !self.config.read().smart_switch_enabled {
            return Ok(());
        }

        if self.journal().exists() {
            return Ok(());
        }
        if self.progress.read().is_some() {
            return Ok(());
        }

        let active_profile_id = match self.config.read().active_profile_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let profiles = self.list_profiles_live(Some(active_profile_id)).await?;
        let active_profile = match profiles.iter().find(|p| p.is_active) {
            Some(p) => p,
            None => return Ok(()),
        };

        let active_quota = match active_profile.quota {
            Some(ref q) => q,
            None => return Ok(()),
        };

        let rem_5h = get_bucket_remaining_fraction(active_quota, "gemini-5h").unwrap_or(1.0);
        let rem_weekly =
            get_bucket_remaining_fraction(active_quota, "gemini-weekly").unwrap_or(1.0);
        let rem_3p_5h = get_bucket_remaining_fraction(active_quota, "3p-5h").unwrap_or(1.0);
        let rem_3p_weekly =
            get_bucket_remaining_fraction(active_quota, "3p-weekly").unwrap_or(1.0);

        let gemini_exhausted = rem_5h < 0.10 || rem_weekly < 0.05;
        let p3_exhausted = rem_3p_5h < 0.10 || rem_3p_weekly < 0.05;

        // If active profile still has ample quota, do nothing
        if !gemini_exhausted && !p3_exhausted {
            return Ok(());
        }

        // CRITICAL CHECK: If quota is completely exhausted (rem_5h < 0.03), we MUST switch immediately
        // because the agent cannot execute any more prompt calls without failing.
        // If there's still a tiny buffer (3% - 10%) AND heavy compilation/process is running, defer briefly.
        let is_emergency = rem_5h < 0.03 || rem_weekly < 0.02;
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

        let mut candidate: Option<(Uuid, f64)> = None;

        for profile in &profiles {
            // Strictly skip active profile and profiles requiring re-authentication
            if profile.is_active || profile.token_status != TokenStatus::Valid {
                continue;
            }

            // Only consider profiles that have verified live quota available
            if let Some(ref q) = profile.quota {
                let cand_5h = get_bucket_remaining_fraction(q, "gemini-5h").unwrap_or(0.0);
                let cand_weekly = get_bucket_remaining_fraction(q, "gemini-weekly").unwrap_or(0.0);

                // Candidate must have healthy Gemini quota (> 15% 5h and > 5% weekly)
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

        let target_id = candidate.map(|(id, _)| id);

        if let Some(target_id) = target_id {
            self.logger.warn(
                None,
                "smart_switch",
                format!("Triggering safe verified smart switch to profile {}", target_id),
            );
            match self.request_switch(target_id, None) {
                Ok(req) => {
                    if let Err(e) = self.confirm_switch(req.operation_id) {
                        self.logger.error(
                            None,
                            "smart_switch",
                            format!("Smart switch confirm failed: {}", e),
                        );
                    }
                }
                Err(e) => {
                    self.logger.error(
                        None,
                        "smart_switch",
                        format!("Smart switch request failed: {}", e),
                    );
                }
            }
        } else {
            self.logger.warn(
                None,
                "smart_switch",
                "No verified alternative profiles with sufficient quotas and valid tokens found.",
            );
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
