use chrono::{DateTime, Utc};
use humansize::{BINARY, format_size};
use serde_json::Value;
use std::collections::HashMap;
use tera::{Result as TeraResult, to_value};

/// Convert timestamp to "time ago" format (e.g., "2 hours ago")
pub fn timeago(value: &Value, _args: &HashMap<String, Value>) -> TeraResult<Value> {
    let timestamp_str = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("timeago filter expects a string timestamp"))?;

    // Parse ISO 8601 timestamp
    let parsed = DateTime::parse_from_rfc3339(timestamp_str)
        .or_else(|_| {
            // Try parsing without timezone
            timestamp_str.parse::<DateTime<Utc>>().map(|dt| dt.into())
        })
        .map_err(|e| tera::Error::msg(format!("Failed to parse timestamp: {}", e)))?;

    let now = Utc::now();
    let duration = now.signed_duration_since(parsed.with_timezone(&Utc));

    let result = if duration.num_seconds() < 60 {
        format!("{}s", duration.num_seconds())
    } else if duration.num_minutes() < 60 {
        format!("{}m", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h", duration.num_hours())
    } else {
        format!("{}d", duration.num_days())
    };

    to_value(result).map_err(|e| tera::Error::msg(format!("Failed to convert to value: {}", e)))
}

/// Format bytes as human-readable file size (e.g., "1.5 GB")
pub fn filesizeformat(value: &Value, _args: &HashMap<String, Value>) -> TeraResult<Value> {
    let bytes = if let Some(n) = value.as_u64() {
        n
    } else if let Some(n) = value.as_i64() {
        n as u64
    } else if let Some(s) = value.as_str() {
        s.parse::<u64>()
            .map_err(|e| tera::Error::msg(format!("Failed to parse bytes: {}", e)))?
    } else {
        return Err(tera::Error::msg(
            "filesizeformat filter expects a number or string",
        ));
    };

    let result = format_size(bytes, BINARY);

    to_value(result).map_err(|e| tera::Error::msg(format!("Failed to convert to value: {}", e)))
}

/// Map status values to color names
pub fn status_color(value: &Value, _args: &HashMap<String, Value>) -> TeraResult<Value> {
    let status = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("status_color filter expects a string"))?
        .to_lowercase();

    let color = match status.as_str() {
        "running" | "active" | "ready" | "true" | "succeeded" | "healthy" | "ok" => "green",
        "pending" | "starting" | "waiting" | "unknown" => "yellow",
        "failed" | "error" | "unhealthy" | "false" | "terminated" | "crashloopbackoff" => "red",
        "completed" => "blue",
        _ => "white",
    };

    to_value(color).map_err(|e| tera::Error::msg(format!("Failed to convert to value: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_timeago() {
        let timestamp = json!("2024-01-01T12:00:00Z");
        let result = timeago(&timestamp, &HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_filesizeformat() {
        let bytes = json!(1536);
        let result = filesizeformat(&bytes, &HashMap::new()).unwrap();
        assert_eq!(result.as_str().unwrap(), "1.50 KiB");
    }

    #[test]
    fn test_status_color() {
        assert_eq!(
            status_color(&json!("running"), &HashMap::new())
                .unwrap()
                .as_str()
                .unwrap(),
            "green"
        );
        assert_eq!(
            status_color(&json!("pending"), &HashMap::new())
                .unwrap()
                .as_str()
                .unwrap(),
            "yellow"
        );
        assert_eq!(
            status_color(&json!("failed"), &HashMap::new())
                .unwrap()
                .as_str()
                .unwrap(),
            "red"
        );
    }
}
