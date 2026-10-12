/**
 * Local HTTP server for plugin communication.
 * Sets up a lightweight TcpListener routing plugin commands to switcher operations.
 * Main exports: start_http_server
 */
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use switcher_windows::SwitcherService;

#[derive(Debug, PartialEq)]
pub enum HttpRoute<'a> {
    Status,
    State,
    SmartSwitch,
    AppShow,
    Unlock,
    EligibilityList,
    EligibilityReset(&'a str),
    EligibilityProbeActive,
    ProfileActivate(&'a str),
    NotFound,
    BadRequest(String),
}

pub fn parse_route<'a>(method: &str, path: &'a str) -> HttpRoute<'a> {
    if method == "GET" && path == "/api/v1/status" {
        HttpRoute::Status
    } else if method == "GET" && path == "/api/v1/state" {
        HttpRoute::State
    } else if method == "POST" && path == "/api/v1/smart-switch" {
        HttpRoute::SmartSwitch
    } else if method == "POST" && path == "/api/v1/app/show" {
        HttpRoute::AppShow
    } else if method == "POST" && path == "/api/v1/unlock" {
        HttpRoute::Unlock
    } else if method == "GET" && path == "/api/v1/eligibility" {
        HttpRoute::EligibilityList
    } else if method == "POST" && path.starts_with("/api/v1/eligibility/") && path.ends_with("/reset") {
        let prefix = "/api/v1/eligibility/";
        let suffix = "/reset";
        if path.len() > prefix.len() + suffix.len() {
            let id = &path[prefix.len()..path.len() - suffix.len()];
            HttpRoute::EligibilityReset(id)
        } else {
            HttpRoute::BadRequest("Invalid path".to_string())
        }
    } else if method == "POST" && path == "/api/v1/eligibility/probe-active" {
        HttpRoute::EligibilityProbeActive
    } else if method == "POST" && path.starts_with("/api/v1/profiles/") && path.ends_with("/activate") {
        let prefix = "/api/v1/profiles/";
        let suffix = "/activate";
        if path.len() > prefix.len() + suffix.len() {
            let id = &path[prefix.len()..path.len() - suffix.len()];
            HttpRoute::ProfileActivate(id)
        } else {
            HttpRoute::BadRequest("Invalid path".to_string())
        }
    } else {
        HttpRoute::NotFound
    }
}

fn handle_client(
    mut stream: TcpStream,
    service: Arc<SwitcherService>,
    api_secret: &str,
    app_handle: &AppHandle,
) -> anyhow::Result<()> {
    let mut buffer = [0; 8192];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }
    let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = request_str.lines();
    let first_line = match lines.next() {
        Some(line) => line,
        None => return Ok(()),
    };
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let method = parts[0];
    let path = parts[1];

    let mut auth_header = None;
    for line in lines {
        if line.to_lowercase().starts_with("authorization:") {
            auth_header = Some(line["authorization:".len()..].trim());
            break;
        }
    }

    // /api/v1/app/show does not require auth so single-instance reactivation works instantly
    if path != "/api/v1/app/show" {
        let expected_auth = format!("Bearer {}", api_secret);
        if auth_header != Some(expected_auth.as_str()) {
            send_response(
                &mut stream,
                401,
                "Unauthorized",
                r#"{"error":"Unauthorized"}"#,
            )?;
            return Ok(());
        }
    }

    let route = parse_route(method, path);

    match route {
        HttpRoute::Status => {
            match service.http_status() {
                Ok(status) => {
                    let json = serde_json::to_string(&status)?;
                    send_response(&mut stream, 200, "OK", &json)?;
                }
                Err(e) => {
                    let err_msg = format!(r#"{{"error":{:?}}}"#, e.to_string());
                    send_response(&mut stream, 500, "Internal Server Error", &err_msg)?;
                }
            }
        }
        HttpRoute::State => {
            match tauri::async_runtime::block_on(async {
                service.app_state_live(env!("CARGO_PKG_VERSION")).await
            }) {
                Ok(state) => {
                    let json = serde_json::to_string(&state)?;
                    send_response(&mut stream, 200, "OK", &json)?;
                }
                Err(e) => {
                    let err_msg = format!(r#"{{"error":{:?}}}"#, e.to_string());
                    send_response(&mut stream, 500, "Internal Server Error", &err_msg)?;
                }
            }
        }
        HttpRoute::SmartSwitch => {
            match tauri::async_runtime::block_on(async {
                service.check_and_perform_smart_switch().await
            }) {
                Ok(_) => {
                    send_response(&mut stream, 200, "OK", r#"{"success":true,"message":"Smart switch checked"}"#)?;
                }
                Err(e) => {
                    let err_msg = format!(r#"{{"success":false,"error":{:?}}}"#, e.to_string());
                    send_response(&mut stream, 500, "Internal Server Error", &err_msg)?;
                }
            }
        }
        HttpRoute::AppShow => {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_size(tauri::LogicalSize::new(800.0, 600.0));
                let _ = window.center();
                let _ = window.set_always_on_top(true);
                let _ = window.set_focus();
                let _ = window.set_always_on_top(false);
                #[cfg(target_os = "windows")]
                {
                    if let Ok(hwnd) = window.hwnd() {
                        switcher_windows::force_window_foreground(hwnd.0 as isize);
                    }
                }
            }
            send_response(&mut stream, 200, "OK", r#"{"success":true}"#)?;
        }
        HttpRoute::Unlock => {
            let body = if let Some(idx) = request_str.find("\r\n\r\n") {
                &request_str[idx + 4..]
            } else if let Some(idx) = request_str.find("\n\n") {
                &request_str[idx + 2..]
            } else {
                ""
            };
            #[derive(serde::Deserialize)]
            struct UnlockReq {
                password: String,
            }
            match serde_json::from_str::<UnlockReq>(body.trim()) {
                Ok(req) => match service.unlock_app(&req.password) {
                    Ok(_) => {
                        send_response(
                            &mut stream,
                            200,
                            "OK",
                            r#"{"success":true,"message":"Application unlocked"}"#,
                        )?;
                    }
                    Err(e) => {
                        let err_msg = format!(r#"{{"success":false,"error":{:?}}}"#, e.to_string());
                        send_response(&mut stream, 400, "Bad Request", &err_msg)?;
                    }
                },
                Err(e) => {
                    let err_msg = format!(r#"{{"success":false,"error":{:?}}}"#, e.to_string());
                    send_response(&mut stream, 400, "Bad Request", &err_msg)?;
                }
            }
        }
        HttpRoute::EligibilityList => {
            match service.list_profiles(None) {
                Ok(profiles) => {
                    let mut is_probe_mode = false;
                    for p in &profiles {
                        if p.is_active && p.quota.is_none() {
                            is_probe_mode = true;
                            break;
                        }
                    }
                    
                    let map = switcher_windows::service::smart_switch::quota_exhausted_state().lock().unwrap();
                    let mut data = Vec::new();
                    
                    for p in profiles {
                        let id = p.metadata.profile_id;
                        let exhausted_at = map.get(&id).map(|t| t.to_rfc3339());
                        let mut entry = serde_json::json!({
                            "id": id,
                            "email": p.metadata.account_email,
                            "eligibility": "unknown",
                            "quota_exhausted_at": exhausted_at
                        });
                        if let switcher_core::EligibilityStatus::NeedsVerification { since, .. } = p.metadata.eligibility {
                            entry["eligibility"] = serde_json::json!("needs_verification");
                            entry["since"] = serde_json::json!(since);
                        } else if p.metadata.eligibility == switcher_core::EligibilityStatus::Eligible {
                            entry["eligibility"] = serde_json::json!("eligible");
                        }
                        data.push(entry);
                    }
                    
                    let resp = serde_json::json!({
                        "mode": if is_probe_mode { "probe" } else { "quota" },
                        "profiles": data
                    });
                    
                    let json = serde_json::to_string(&resp)?;
                    send_response(&mut stream, 200, "OK", &json)?;
                }
                Err(e) => {
                    let err_msg = format!(r#"{{"error":{:?}}}"#, e.to_string());
                    send_response(&mut stream, 500, "Internal Server Error", &err_msg)?;
                }
            }
        }
        HttpRoute::EligibilityReset(profile_id_str) => {
            match Uuid::parse_str(profile_id_str) {
                Ok(id) => {
                    if let Ok(mut metadata) = service.require_profile(id) {
                        metadata.eligibility = switcher_core::EligibilityStatus::Unknown;
                        if service.save_profile_metadata(id, &metadata).is_ok() {
                            send_response(&mut stream, 200, "OK", r#"{"success":true}"#)?;
                        } else {
                            send_response(&mut stream, 500, "Internal Server Error", r#"{"error":"Failed to save"}"#)?;
                        }
                    } else {
                        send_response(&mut stream, 404, "Not Found", r#"{"error":"Profile not found"}"#)?;
                    }
                }
                Err(_) => {
                    send_response(&mut stream, 400, "Bad Request", r#"{"error":"Invalid profile ID"}"#)?;
                }
            }
        }
        HttpRoute::EligibilityProbeActive => {
            use switcher_windows::service::eligibility::{EligibilityProbe, RealEligibilityProbe, ProbeResult};
            match tauri::async_runtime::block_on(async {
                RealEligibilityProbe.probe().await
            }) {
                ProbeResult::Ok => {
                    send_response(&mut stream, 200, "OK", r#"{"status":"ok"}"#)?;
                }
                ProbeResult::NeedsVerification(msg) => {
                    let json = serde_json::json!({
                        "status": "needs_verification",
                        "message": msg
                    });
                    send_response(&mut stream, 200, "OK", &json.to_string())?;
                }
                ProbeResult::QuotaExhausted(msg) => {
                    let json = serde_json::json!({
                        "status": "quota_exhausted",
                        "message": msg
                    });
                    send_response(&mut stream, 200, "OK", &json.to_string())?;
                }
                ProbeResult::Error(err) => {
                    let json = serde_json::json!({
                        "status": "error",
                        "error": err
                    });
                    send_response(&mut stream, 500, "Internal Server Error", &json.to_string())?;
                }
            }
        }
        HttpRoute::ProfileActivate(profile_id_str) => {
            match Uuid::parse_str(profile_id_str) {
                Ok(target_profile_id) => match service.request_switch(target_profile_id, None) {
                    Ok(res) => {
                        let service_clone = service.clone();
                        let operation_id = res.operation_id;
                        std::thread::spawn(move || {
                            if let Err(e) = service_clone.confirm_switch(operation_id) {
                                service_clone.logger().error(
                                    Some(operation_id),
                                    "http",
                                    format!("Switch failed: {}", e),
                                );
                            }
                        });
                        let resp = format!(
                            r#"{{"accepted":true,"operationId":"{}","message":"Profile switching started"}}"#,
                            operation_id.to_string()
                        );
                        send_response(&mut stream, 200, "OK", &resp)?;
                    }
                    Err(e) => {
                        let err_msg =
                            format!(r#"{{"accepted":false,"message":{:?}}}"#, e.to_string());
                        send_response(&mut stream, 400, "Bad Request", &err_msg)?;
                    }
                },
                Err(_) => {
                    send_response(
                        &mut stream,
                        400,
                        "Bad Request",
                        r#"{"error":"Invalid profile ID"}"#,
                    )?;
                }
            }
        }
        HttpRoute::NotFound => {
            send_response(&mut stream, 404, "Not Found", r#"{"error":"Not Found"}"#)?;
        }
        HttpRoute::BadRequest(err) => {
            let json = serde_json::json!({"error": err});
            send_response(&mut stream, 400, "Bad Request", &json.to_string())?;
        }
    }
    Ok(())
}

fn send_response(
    stream: &mut TcpStream,
    status_code: u16,
    status_text: &str,
    body: &str,
) -> anyhow::Result<()> {
    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: application/json; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status_code,
        status_text,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

pub fn start_http_server(service: Arc<SwitcherService>, app_handle: AppHandle) {
    let port = service.http_port();
    let api_secret = service.api_secret();
    std::thread::spawn(move || {
        let listener = match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(l) => l,
            Err(e) => {
                service.logger().error(
                    None,
                    "http",
                    format!("Failed to bind to port {}: {}", port, e),
                );
                return;
            }
        };
        service.logger().info(
            None,
            "http",
            format!("Local HTTP server running on port {}", port),
        );
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let service = service.clone();
                    let api_secret = api_secret.clone();
                    let app_handle = app_handle.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = handle_client(stream, service, &api_secret, &app_handle) {
                            eprintln!("HTTP client handler error: {:?}", e);
                        }
                    });
                }
                Err(e) => {
                    service.logger().error(
                        None,
                        "http",
                        format!("Failed to accept connection: {}", e),
                    );
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_route_eligibility_reset() {
        let route = parse_route("POST", "/api/v1/eligibility/reset");
        assert_eq!(route, HttpRoute::BadRequest("Invalid path".to_string()));

        let route = parse_route("POST", "/api/v1/eligibility//reset");
        assert_eq!(route, HttpRoute::BadRequest("Invalid path".to_string()));

        let route = parse_route("POST", "/api/v1/eligibility/123/reset");
        assert_eq!(route, HttpRoute::EligibilityReset("123"));
    }
}
