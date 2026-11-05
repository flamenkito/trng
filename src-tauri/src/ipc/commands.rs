use serde::Serialize;

/// Static status payload returned to Angular on startup.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackendStatus {
    pub app_name: String,
    pub app_version: String,
    pub tauri_version: String,
    pub status: String,
    pub timestamp_ms: u64,
}

/// Returns basic backend identity/health information.
///
/// Angular displays this value to prove the invoke path works.
#[tauri::command]
pub fn get_backend_status() -> BackendStatus {
    BackendStatus {
        app_name: "Infinite Noise TRNG Visualizer".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        tauri_version: tauri::VERSION.to_string(),
        status: "online".to_string(),
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    }
}

/// Frontend-callable trigger for the test event.
///
/// The frontend subscribes first, then invokes this command, so emission can
/// never race ahead of the subscription.
#[tauri::command]
pub fn request_test_event(app: tauri::AppHandle) {
    super::events::emit_test_event_now(&app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_status_reports_online_identity() {
        let status = get_backend_status();
        assert_eq!(status.status, "online");
        assert_eq!(status.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(status.tauri_version, tauri::VERSION);
        assert!(status.timestamp_ms > 0);
    }

    #[test]
    fn backend_status_serializes_camel_case() {
        let json = serde_json::to_value(get_backend_status()).unwrap();
        assert!(json.get("appName").is_some(), "expected camelCase keys");
        assert!(json.get("timestampMs").is_some());
        assert!(json.get("app_name").is_none(), "unexpected snake_case keys");
    }
}
