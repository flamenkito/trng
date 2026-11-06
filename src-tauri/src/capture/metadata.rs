//! `metadata.json` for a capture session.
//!
//! Metadata lives beside `stream.bin`, never inside it.

use std::path::Path;
use std::time::SystemTime;

use serde::Serialize;

/// Capture metadata. Serialized camelCase to match the frontend model.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CaptureMetadata {
    pub device: String,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    pub bytes_received: u64,
    pub application_version: String,
    /// Conceptual data mode; `stream.bin` always holds whatever the active
    /// source produced (whitened output for the initial milestone).
    pub source_mode: String,
    /// Set when the capture ended without an error; absent/omitted on error
    /// so a partially written capture never looks complete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complete: Option<bool>,
}

impl CaptureMetadata {
    pub fn started(now: SystemTime) -> Self {
        Self {
            device: "Infinite Noise TRNG".to_string(),
            started_at: format_timestamp_rfc3339(now),
            ended_at: None,
            bytes_received: 0,
            application_version: env!("CARGO_PKG_VERSION").to_string(),
            source_mode: "whitened".to_string(),
            complete: None,
        }
    }

    /// Atomically replaces `metadata.json` in the capture directory.
    pub fn write_to(&self, dir: &Path) -> std::io::Result<()> {
        let path = dir.join("metadata.json");
        let tmp = dir.join("metadata.json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap())?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

/// Formats a `SystemTime` as RFC 3339 (`2026-11-06T08-30-00+01:00`-free,
/// UTC `Z` form) using only `std`. Uses the well-known civil-from-days
/// algorithm so no date dependency is needed for one timestamp format.
pub fn format_timestamp_rfc3339(ts: SystemTime) -> String {
    let secs = ts
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;

    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;

    // Howard Hinnant's civil_from_days (shifted epoch).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{year:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// `captures/<YYYY-MM-DD_HH-MM-SS>` directory name in local wall-clock terms.
/// For directory naming a UTC-based name is deterministic and unambiguous.
pub fn capture_dir_name(now: SystemTime) -> String {
    let formatted = format_timestamp_rfc3339(now);
    // "2026-11-06T08:30:00Z" -> "2026-11-06_08-30-00"
    formatted
        .replace('T', "_")
        .trim_end_matches('Z')
        .replace(':', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_formats_as_unix_epoch() {
        assert_eq!(
            format_timestamp_rfc3339(std::time::UNIX_EPOCH),
            "1970-01-01T00:00:00Z"
        );
    }

    #[test]
    fn known_date_formats_correctly() {
        // 2026-11-06T00:00:00Z = 1_793_923_200 (2026-01-01 = 1_767_225_600,
        // + 309 days * 86_400).
        let ts = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_793_923_200);
        assert_eq!(format_timestamp_rfc3339(ts), "2026-11-06T00:00:00Z");
        let ts = ts + std::time::Duration::from_secs(36_000 + 754);
        assert_eq!(format_timestamp_rfc3339(ts), "2026-11-06T10:12:34Z");
    }

    #[test]
    fn dir_name_is_filesystem_safe() {
        let ts = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_793_923_200);
        assert_eq!(capture_dir_name(ts), "2026-11-06_00-00-00");
    }

    #[test]
    fn metadata_serializes_expected_shape() {
        let meta = CaptureMetadata::started(std::time::UNIX_EPOCH);
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["device"], "Infinite Noise TRNG");
        assert_eq!(json["sourceMode"], "whitened");
        assert_eq!(json["startedAt"], "1970-01-01T00:00:00Z");
        assert!(json.get("endedAt").is_none(), "endedAt omitted before stop");
        assert!(json.get("complete").is_none(), "complete omitted before stop");
    }
}
