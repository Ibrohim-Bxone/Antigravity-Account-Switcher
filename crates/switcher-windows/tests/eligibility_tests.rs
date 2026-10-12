
use switcher_windows::service::eligibility::{EligibilityProbe, ProbeResult, is_eligibility_error};
use std::sync::{Arc, Mutex};
use switcher_core::{EligibilityStatus, ProfileMetadata};
use switcher_windows::SwitcherPaths;
use switcher_windows::SwitcherService;
use uuid::Uuid;

// Probe state in smart_switch.rs is global (correct for a single switcher instance).
// Tests that touch it run one at a time.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct MockProbe {
    results: Arc<Mutex<Vec<ProbeResult>>>,
}

impl EligibilityProbe for MockProbe {
    async fn probe(&self) -> ProbeResult {
        let mut results = self.results.lock().unwrap();
        if results.is_empty() {
            ProbeResult::Ok
        } else {
            results.remove(0)
        }
    }
}

#[tokio::test]
async fn test_eligibility_error_parsing() {
    assert!(is_eligibility_error("eligibility check failed: your account..."));
    assert!(is_eligibility_error("not eligible for antigravity"));
    assert!(is_eligibility_error("Please verify your account to continue"));
    assert!(is_eligibility_error(" Authentication required. Please visit the URL to log in:   https://accounts.google.com/x"));
    assert!(!is_eligibility_error("network timeout"));
    assert!(!is_eligibility_error("quota exceeded"));
}

#[tokio::test]
async fn test_probe_eligibility_error_switches_even_if_agent_working() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "1");
    }
    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();
    
    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("active@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();
    
    // Good quota!
    let good_quota = switcher_core::ProfileQuotaView {
        subscription_tier: "Google AI Pro".to_string(),
        quota_groups: vec![
            switcher_core::QuotaGroupView {
                display_name: "Gemini".to_string(),
                description: "".to_string(),
                buckets: vec![
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-5h".to_string(),
                        window: "5h".to_string(),
                        remaining_fraction: 1.0,
                        reset_time: None,
                        display_name: "5h".to_string(),
                        description: None,
                    },
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-weekly".to_string(),
                        window: "1w".to_string(),
                        remaining_fraction: 1.0,
                        reset_time: None,
                        display_name: "1w".to_string(),
                        description: None,
                    }
                ]
            }
        ]
    };
    switcher_windows::service::profiles::inject_test_quota("active@test.com".to_string(), good_quota.clone());

    // One candidate
    let cand_id = Uuid::new_v4();
    let cand_meta = ProfileMetadata {
        profile_id: cand_id,
        account_email: Some("cand@test.com".to_string()),
        display_name: "Cand".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let cand_profile_dir = paths.profile_dir(cand_id);
    std::fs::create_dir_all(&cand_profile_dir).unwrap();
    std::fs::write(cand_profile_dir.join("metadata.json"), serde_json::to_string(&cand_meta).unwrap()).unwrap();
    std::fs::write(cand_profile_dir.join("credentials.enc"), dummy_dpapi.0).unwrap();
    switcher_windows::service::profiles::inject_test_quota("cand@test.com".to_string(), good_quota.clone());

    let service = SwitcherService::new(paths.clone()).unwrap();
    
    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![
            ProbeResult::NeedsVerification("Verify your account".to_string()), // for active
            ProbeResult::Ok, // for candidate
        ])),
    };
    
    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());
    
    // Check active profile became NeedsVerification
    let meta_str = std::fs::read_to_string(active_profile_dir.join("metadata.json")).unwrap();
    let updated_meta: ProfileMetadata = serde_json::from_str(&meta_str).unwrap();
    assert!(matches!(updated_meta.eligibility, EligibilityStatus::NeedsVerification { .. }));

    // Config should have changed active_profile_id to cand_id because of bypass testing logic in smart_switch
    let config_str = std::fs::read_to_string(&paths.config).unwrap();
    let updated_config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    assert_eq!(updated_config["activeProfileId"].as_str().unwrap(), cand_id.to_string());
}

#[tokio::test]
async fn test_probe_network_error_keeps_unknown_no_switch() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "0");
    }
    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();
    
    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("active@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), dummy_dpapi.0).unwrap();
    
    let good_quota = switcher_core::ProfileQuotaView {
        subscription_tier: "Google AI Pro".to_string(),
        quota_groups: vec![
            switcher_core::QuotaGroupView {
                display_name: "Gemini".to_string(),
                description: "".to_string(),
                buckets: vec![
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-5h".to_string(),
                        window: "5h".to_string(),
                        remaining_fraction: 1.0,
                        reset_time: None,
                        display_name: "5h".to_string(),
                        description: None,
                    },
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-weekly".to_string(),
                        window: "1w".to_string(),
                        remaining_fraction: 1.0,
                        reset_time: None,
                        display_name: "1w".to_string(),
                        description: None,
                    }
                ]
            }
        ]
    };
    switcher_windows::service::profiles::inject_test_quota("active@test.com".to_string(), good_quota.clone());

    let service = SwitcherService::new(paths.clone()).unwrap();
    
    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![
            ProbeResult::Error("Network error".to_string()),
        ])),
    };
    
    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());
    
    // Check active profile remained Unknown
    let meta_str = std::fs::read_to_string(active_profile_dir.join("metadata.json")).unwrap();
    let updated_meta: ProfileMetadata = serde_json::from_str(&meta_str).unwrap();
    assert_eq!(updated_meta.eligibility, EligibilityStatus::Unknown);

    // Config should not have changed
    let config_str = std::fs::read_to_string(&paths.config).unwrap();
    let updated_config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    assert_eq!(updated_config["activeProfileId"].as_str().unwrap(), active_id.to_string());

    // No re-probe for 2 minutes after an error: the very next tick must not call the probe.
    let second_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![ProbeResult::NeedsVerification("must not be called".to_string())])),
    };
    service.check_and_perform_smart_switch_with_probe(&second_probe).await.unwrap();
    let meta_str = std::fs::read_to_string(active_profile_dir.join("metadata.json")).unwrap();
    let updated_meta: ProfileMetadata = serde_json::from_str(&meta_str).unwrap();
    assert_eq!(updated_meta.eligibility, EligibilityStatus::Unknown, "re-probed right after an error");
}

#[tokio::test]
async fn test_eligible_account_30m_throttle() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "0");
    }
    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();
    
    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("active@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Eligible,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), dummy_dpapi.0).unwrap();
    
    let good_quota = switcher_core::ProfileQuotaView {
        subscription_tier: "Google AI Pro".to_string(),
        quota_groups: vec![
            switcher_core::QuotaGroupView {
                display_name: "Gemini".to_string(),
                description: "".to_string(),
                buckets: vec![
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-5h".to_string(),
                        window: "5h".to_string(),
                        remaining_fraction: 1.0,
                        reset_time: None,
                        display_name: "5h".to_string(),
                        description: None,
                    },
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-weekly".to_string(),
                        window: "1w".to_string(),
                        remaining_fraction: 1.0,
                        reset_time: None,
                        display_name: "1w".to_string(),
                        description: None,
                    }
                ]
            }
        ]
    };
    switcher_windows::service::profiles::inject_test_quota("active@test.com".to_string(), good_quota.clone());

    let service = SwitcherService::new(paths.clone()).unwrap();
    
    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![
            ProbeResult::Ok,
        ])),
    };
    
    // 1st tick: Since it's not probed yet, it should probe and consume the Ok.
    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());
    assert_eq!(mock_probe.results.lock().unwrap().len(), 0);

    // Provide another Ok to mock probe.
    mock_probe.results.lock().unwrap().push(ProbeResult::Ok);

    // 2nd tick right after: it should NOT probe because it's eligible and 30m hasn't passed.
    let result2 = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result2.is_ok());
    assert_eq!(mock_probe.results.lock().unwrap().len(), 1, "Probe was unnecessarily called within 30m");
}

#[tokio::test]
async fn test_needs_verification_skipped_and_retry_limit() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "0");
    }
    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();
    
    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("active@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), dummy_dpapi.0).unwrap();
    
    let exhausted_quota = switcher_core::ProfileQuotaView {
        subscription_tier: "Google AI Pro".to_string(),
        quota_groups: vec![
            switcher_core::QuotaGroupView {
                display_name: "Gemini".to_string(),
                description: "".to_string(),
                buckets: vec![
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-5h".to_string(),
                        window: "5h".to_string(),
                        remaining_fraction: 0.0,
                        reset_time: None,
                        display_name: "5h".to_string(),
                        description: None,
                    },
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-weekly".to_string(),
                        window: "1w".to_string(),
                        remaining_fraction: 0.0,
                        reset_time: None,
                        display_name: "1w".to_string(),
                        description: None,
                    }
                ]
            }
        ]
    };
    switcher_windows::service::profiles::inject_test_quota("active@test.com".to_string(), exhausted_quota);

    for i in 0..6 {
        let cand_id = Uuid::new_v4();
        let cand_email = format!("cand{}@test.com", i);
        let cand_meta = ProfileMetadata {
            profile_id: cand_id,
            account_email: Some(cand_email.clone()),
            display_name: format!("Cand{}", i),
            created_at: chrono::Utc::now(),
            last_activated_at: chrono::Utc::now(),
            token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
            snapshot_initialized: true,
            eligibility: EligibilityStatus::Unknown,
        };
        let cand_profile_dir = paths.profile_dir(cand_id);
        std::fs::create_dir_all(&cand_profile_dir).unwrap();
        std::fs::write(cand_profile_dir.join("metadata.json"), serde_json::to_string(&cand_meta).unwrap()).unwrap();
        let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
        std::fs::write(cand_profile_dir.join("credentials.enc"), dummy_dpapi.0).unwrap();
        
        let good_quota = switcher_core::ProfileQuotaView {
            subscription_tier: "Google AI Pro".to_string(),
            quota_groups: vec![
                switcher_core::QuotaGroupView {
                    display_name: "Gemini".to_string(),
                    description: "".to_string(),
                    buckets: vec![
                        switcher_core::QuotaBucketView {
                            bucket_id: "gemini-5h".to_string(),
                            window: "5h".to_string(),
                            remaining_fraction: 1.0,
                            reset_time: None,
                            display_name: "5h".to_string(),
                            description: None,
                        },
                        switcher_core::QuotaBucketView {
                            bucket_id: "gemini-weekly".to_string(),
                            window: "1w".to_string(),
                            remaining_fraction: 1.0,
                            reset_time: None,
                            display_name: "1w".to_string(),
                            description: None,
                        }
                    ]
                }
            ]
        };
        switcher_windows::service::profiles::inject_test_quota(cand_email, good_quota);
    }

    let service = SwitcherService::new(paths).unwrap();
    
    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![
            ProbeResult::Ok, // Active probe ok (because active is Unknown initially)
            ProbeResult::NeedsVerification("Verify your account".to_string()),
            ProbeResult::NeedsVerification("Verify your account".to_string()),
            ProbeResult::NeedsVerification("Verify your account".to_string()),
            ProbeResult::NeedsVerification("Verify your account".to_string()),
            ProbeResult::NeedsVerification("Verify your account".to_string()),
            ProbeResult::Ok,
        ])),
    };
    
    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());
    
    let remaining_results = mock_probe.results.lock().unwrap().len();
    assert_eq!(remaining_results, 1, "The loop should have stopped after exactly 5 attempts");
}

#[tokio::test]
async fn test_legacy_format_read_without_error() {
    let json = r#"{
        "profileId": "00000000-0000-0000-0000-000000000000",
        "displayName": "Test",
        "createdAt": "2026-10-09T00:00:00Z",
        "lastActivatedAt": "2026-10-09T00:00:00Z"
    }"#;
    let meta: ProfileMetadata = serde_json::from_str(json).unwrap();
    assert_eq!(meta.eligibility, EligibilityStatus::Unknown);
}
#[tokio::test]
async fn test_probe_mode_active_ok_no_switch() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "0");
    }
    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();
    
    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("active@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), dummy_dpapi.0).unwrap();
    
    let service = SwitcherService::new(paths.clone()).unwrap();
    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![ProbeResult::Ok])),
    };
    
    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());
    
    let config_str = std::fs::read_to_string(&paths.config).unwrap();
    let updated_config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    assert_eq!(updated_config["activeProfileId"].as_str().unwrap(), active_id.to_string());
}

#[tokio::test]
async fn test_probe_mode_active_quota_exhausted_switches_agent_working() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "1"); // Agent is working, but should switch anyway!
    }
    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();
    
    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("probe_active@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    let cand1_id = Uuid::new_v4();
    let cand1_meta = ProfileMetadata {
        profile_id: cand1_id,
        account_email: Some("probe_cand1@test.com".to_string()),
        display_name: "Cand1".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let cand1_dir = paths.profile_dir(cand1_id);
    std::fs::create_dir_all(&cand1_dir).unwrap();
    std::fs::write(cand1_dir.join("metadata.json"), serde_json::to_string(&cand1_meta).unwrap()).unwrap();
    std::fs::write(cand1_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    let cand2_id = Uuid::new_v4();
    let cand2_meta = ProfileMetadata {
        profile_id: cand2_id,
        account_email: Some("probe_cand2@test.com".to_string()),
        display_name: "Cand2".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now() - chrono::Duration::hours(1), // Older!
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let cand2_dir = paths.profile_dir(cand2_id);
    std::fs::create_dir_all(&cand2_dir).unwrap();
    std::fs::write(cand2_dir.join("metadata.json"), serde_json::to_string(&cand2_meta).unwrap()).unwrap();
    std::fs::write(cand2_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    let service = SwitcherService::new(paths.clone()).unwrap();
    
    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![
            ProbeResult::QuotaExhausted("Rate limit".to_string()),
            ProbeResult::Ok, // for candidate
        ])),
    };
    
    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());

    let config_str = std::fs::read_to_string(&paths.config).unwrap();
    let updated_config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    // Should switch to cand2 because it's older
    assert_eq!(updated_config["activeProfileId"].as_str().unwrap(), cand2_id.to_string());
}

#[tokio::test]
async fn test_probe_mode_exhausted_within_5h_skipped() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "0");
    }
    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();
    
    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("probe_ex_active@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    let cand_id = Uuid::new_v4();
    let cand_meta = ProfileMetadata {
        profile_id: cand_id,
        account_email: Some("probe_ex_cand@test.com".to_string()),
        display_name: "Cand".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let cand_dir = paths.profile_dir(cand_id);
    std::fs::create_dir_all(&cand_dir).unwrap();
    std::fs::write(cand_dir.join("metadata.json"), serde_json::to_string(&cand_meta).unwrap()).unwrap();
    std::fs::write(cand_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    // Mark cand as exhausted 1 hour ago
    switcher_windows::service::smart_switch::quota_exhausted_state().lock().unwrap().insert(cand_id, chrono::Utc::now() - chrono::Duration::hours(1));

    let service = SwitcherService::new(paths.clone()).unwrap();
    
    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![
            ProbeResult::QuotaExhausted("Rate limit".to_string()),
            // No more probes because it shouldn't find any candidate
        ])),
    };
    
    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());

    let config_str = std::fs::read_to_string(&paths.config).unwrap();
    let updated_config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    // Should NOT switch to cand
    assert_eq!(updated_config["activeProfileId"].as_str().unwrap(), active_id.to_string());
}

#[tokio::test]
async fn test_probe_mode_all_candidates_exhausted_stops_after_5_attempts() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "0");
    }
    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();
    
    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("probe_all_active@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    for i in 0..6 {
        let cand_id = Uuid::new_v4();
        let cand_meta = ProfileMetadata {
            profile_id: cand_id,
            account_email: Some(format!("probe_all_cand{}@test.com", i)),
            display_name: format!("Cand{}", i),
            created_at: chrono::Utc::now(),
            last_activated_at: chrono::Utc::now(),
            token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
            snapshot_initialized: true,
            eligibility: EligibilityStatus::Unknown,
        };
        let cand_dir = paths.profile_dir(cand_id);
        std::fs::create_dir_all(&cand_dir).unwrap();
        std::fs::write(cand_dir.join("metadata.json"), serde_json::to_string(&cand_meta).unwrap()).unwrap();
        std::fs::write(cand_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();
    }

    let service = SwitcherService::new(paths.clone()).unwrap();
    
    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![
            ProbeResult::QuotaExhausted("Rate limit".to_string()),
            ProbeResult::QuotaExhausted("Rate limit".to_string()),
            ProbeResult::QuotaExhausted("Rate limit".to_string()),
            ProbeResult::QuotaExhausted("Rate limit".to_string()),
            ProbeResult::QuotaExhausted("Rate limit".to_string()),
            ProbeResult::QuotaExhausted("Rate limit".to_string()),
            ProbeResult::Ok,
        ])),
    };
    
    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());

    let remaining_results = mock_probe.results.lock().unwrap().len();
    assert_eq!(remaining_results, 1, "Should have stopped after exactly 5 switch attempts (6 probes total)");
}

#[tokio::test]
async fn test_quota_mode_switches_to_oldest_candidate_without_quota() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "0");
    }
    switcher_windows::service::smart_switch::quota_exhausted_state().lock().unwrap().clear();
    *switcher_windows::service::smart_switch::active_probe_state().lock().unwrap() = None;

    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();

    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("active_c@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    // Active profile has quota: 5h at 2% (< 10% threshold)
    let low_quota = switcher_core::ProfileQuotaView {
        subscription_tier: "Google AI Pro".to_string(),
        quota_groups: vec![
            switcher_core::QuotaGroupView {
                display_name: "Gemini".to_string(),
                description: "".to_string(),
                buckets: vec![
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-5h".to_string(),
                        window: "5h".to_string(),
                        remaining_fraction: 0.02,
                        reset_time: None,
                        display_name: "5h".to_string(),
                        description: None,
                    },
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-weekly".to_string(),
                        window: "1w".to_string(),
                        remaining_fraction: 1.0,
                        reset_time: None,
                        display_name: "1w".to_string(),
                        description: None,
                    }
                ]
            }
        ]
    };
    switcher_windows::service::profiles::inject_test_quota("active_c@test.com".to_string(), low_quota);

    // Candidate 1: No quota, activated 30 minutes ago
    let cand1_id = Uuid::new_v4();
    let cand1_meta = ProfileMetadata {
        profile_id: cand1_id,
        account_email: Some("cand1_no_quota@test.com".to_string()),
        display_name: "Cand1".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now() - chrono::Duration::minutes(30),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let cand1_dir = paths.profile_dir(cand1_id);
    std::fs::create_dir_all(&cand1_dir).unwrap();
    std::fs::write(cand1_dir.join("metadata.json"), serde_json::to_string(&cand1_meta).unwrap()).unwrap();
    std::fs::write(cand1_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    // Candidate 2: No quota, activated 2 hours ago (older last_activated_at!)
    let cand2_id = Uuid::new_v4();
    let cand2_meta = ProfileMetadata {
        profile_id: cand2_id,
        account_email: Some("cand2_no_quota@test.com".to_string()),
        display_name: "Cand2".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now() - chrono::Duration::hours(2),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let cand2_dir = paths.profile_dir(cand2_id);
    std::fs::create_dir_all(&cand2_dir).unwrap();
    std::fs::write(cand2_dir.join("metadata.json"), serde_json::to_string(&cand2_meta).unwrap()).unwrap();
    std::fs::write(cand2_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    let service = SwitcherService::new(paths.clone()).unwrap();

    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![
            ProbeResult::Ok, // active probe
            ProbeResult::Ok, // cand2 probe after switch
        ])),
    };

    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());

    let config_str = std::fs::read_to_string(&paths.config).unwrap();
    let updated_config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    // In quota mode without quota on candidates, should switch to cand2 (oldest last_activated_at)
    assert_eq!(updated_config["activeProfileId"].as_str().unwrap(), cand2_id.to_string());
}

#[tokio::test]
async fn test_quota_mode_switches_when_active_exhausted_in_state_even_if_quota_100_percent() {
    let _serial = SERIAL.lock().await;
    unsafe {
        std::env::set_var("BYPASS_SYNC_ACTIVE_PROFILE", "1");
        std::env::set_var("MOCK_AGENT_WORKING", "0");
    }
    switcher_windows::service::smart_switch::quota_exhausted_state().lock().unwrap().clear();
    *switcher_windows::service::smart_switch::active_probe_state().lock().unwrap() = None;

    let temp = tempfile::tempdir().unwrap();
    let mut paths = SwitcherPaths::from_root(temp.path().to_path_buf()).unwrap();
    paths.state_db = temp.path().join("state.vscdb");
    paths.ensure().unwrap();

    let active_id = Uuid::new_v4();
    let config_json = serde_json::json!({
        "schemaVersion": 1,
        "httpPort": 48731,
        "apiSecret": "test",
        "activeProfileId": active_id,
        "smartSwitchEnabled": true,
        "switchLevel": 1,
        "patchCooldownMs": 100,
        "minimizeToTray": true
    });
    std::fs::write(&paths.config, config_json.to_string()).unwrap();

    let active_meta = ProfileMetadata {
        profile_id: active_id,
        account_email: Some("active_d@test.com".to_string()),
        display_name: "Active".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let active_profile_dir = paths.profile_dir(active_id);
    std::fs::create_dir_all(&active_profile_dir).unwrap();
    std::fs::write(active_profile_dir.join("metadata.json"), serde_json::to_string(&active_meta).unwrap()).unwrap();
    let dummy_dpapi = switcher_windows::CredentialStore::default().protect(b"dummy session").unwrap();
    std::fs::write(active_profile_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();

    // Active profile has 100% quota!
    let full_quota = switcher_core::ProfileQuotaView {
        subscription_tier: "Google AI Pro".to_string(),
        quota_groups: vec![
            switcher_core::QuotaGroupView {
                display_name: "Gemini".to_string(),
                description: "".to_string(),
                buckets: vec![
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-5h".to_string(),
                        window: "5h".to_string(),
                        remaining_fraction: 1.0,
                        reset_time: None,
                        display_name: "5h".to_string(),
                        description: None,
                    },
                    switcher_core::QuotaBucketView {
                        bucket_id: "gemini-weekly".to_string(),
                        window: "1w".to_string(),
                        remaining_fraction: 1.0,
                        reset_time: None,
                        display_name: "1w".to_string(),
                        description: None,
                    }
                ]
            }
        ]
    };
    switcher_windows::service::profiles::inject_test_quota("active_d@test.com".to_string(), full_quota.clone());

    // Mark active profile as QuotaExhausted in state within 5h (emergency!)
    switcher_windows::service::smart_switch::quota_exhausted_state()
        .lock()
        .unwrap()
        .insert(active_id, chrono::Utc::now() - chrono::Duration::minutes(15));

    let cand_id = Uuid::new_v4();
    let cand_meta = ProfileMetadata {
        profile_id: cand_id,
        account_email: Some("cand_d@test.com".to_string()),
        display_name: "Cand".to_string(),
        created_at: chrono::Utc::now(),
        last_activated_at: chrono::Utc::now(),
        token_expiry: Some(chrono::Utc::now() + chrono::Duration::days(1)),
        snapshot_initialized: true,
        eligibility: EligibilityStatus::Unknown,
    };
    let cand_dir = paths.profile_dir(cand_id);
    std::fs::create_dir_all(&cand_dir).unwrap();
    std::fs::write(cand_dir.join("metadata.json"), serde_json::to_string(&cand_meta).unwrap()).unwrap();
    std::fs::write(cand_dir.join("credentials.enc"), &dummy_dpapi.0).unwrap();
    switcher_windows::service::profiles::inject_test_quota("cand_d@test.com".to_string(), full_quota);

    let service = SwitcherService::new(paths.clone()).unwrap();

    let mock_probe = MockProbe {
        results: Arc::new(Mutex::new(vec![
            ProbeResult::Ok, // active probe
            ProbeResult::Ok, // candidate probe after switch
        ])),
    };

    let result = service.check_and_perform_smart_switch_with_probe(&mock_probe).await;
    assert!(result.is_ok());

    let config_str = std::fs::read_to_string(&paths.config).unwrap();
    let updated_config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    // Must switch even though quota numbers were 100%
    assert_eq!(updated_config["activeProfileId"].as_str().unwrap(), cand_id.to_string());
}
