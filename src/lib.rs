pub mod proto {
    tonic::include_proto!("telemetry.v1");
}

pub mod server;

pub use proto::telemetry_service_client::TelemetryServiceClient;
pub use proto::telemetry_service_server::{TelemetryService, TelemetryServiceServer};
pub use proto::*;
pub use server::{SharedState, TelemetryServiceImpl, PIN_SWITCH_1, PIN_SWITCH_2};

use chrono::{Local, TimeZone};

/// Helper to convert unix timestamp in millis to formatted local string
pub fn format_timestamp(millis: i64) -> String {
    let secs = millis / 1000;
    let nsecs = ((millis % 1000) * 1_000_000) as u32;
    if let Some(dt) = Local.timestamp_opt(secs, nsecs).single() {
        dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
    } else {
        format!("{}ms", millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp_non_empty() {
        let ts = 1771488000000; // valid millis
        let formatted = format_timestamp(ts);
        assert!(!formatted.is_empty());
        assert!(formatted.contains("2026") || formatted.contains("ms"));
    }

    #[test]
    fn test_protobuf_enum_conversions() {
        assert_eq!(SwitchState::try_from(1), Ok(SwitchState::Released));
        assert_eq!(SwitchState::try_from(2), Ok(SwitchState::Pressed));
        assert_eq!(SwitchState::try_from(0), Ok(SwitchState::Unspecified));
        assert!(SwitchState::try_from(99).is_err());

        assert_eq!(SwitchId::try_from(1), Ok(SwitchId::Switch1));
        assert_eq!(SwitchId::try_from(2), Ok(SwitchId::Switch2));
        assert_eq!(SwitchId::try_from(0), Ok(SwitchId::Unspecified));
        assert!(SwitchId::try_from(99).is_err());

        assert_eq!(LogLevel::try_from(0), Ok(LogLevel::Debug));
        assert_eq!(LogLevel::try_from(1), Ok(LogLevel::Info));
        assert_eq!(LogLevel::try_from(2), Ok(LogLevel::Warn));
        assert_eq!(LogLevel::try_from(3), Ok(LogLevel::Error));
    }
}
