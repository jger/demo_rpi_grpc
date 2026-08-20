use std::pin::Pin;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use colored::Colorize;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tonic::{Request, Response, Status};

use crate::proto::telemetry_service_server::TelemetryService;
use crate::proto::{
    GetStatusRequest, LogEntry, LogLevel, StatusResponse, StreamEventsRequest, StreamLogsRequest,
    SwitchEvent, SwitchId, SwitchState,
};

pub const PIN_SWITCH_1: u32 = 23;
pub const PIN_SWITCH_2: u32 = 24;
#[allow(dead_code)]
pub const DEBOUNCE_MS: u64 = 20;

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Debug)]
pub struct SharedState {
    pub event_tx: broadcast::Sender<SwitchEvent>,
    pub log_tx: broadcast::Sender<LogEntry>,
    pub seq_counter: AtomicI64,
    pub switch_1_state: AtomicU8,
    pub switch_2_state: AtomicU8,
    pub switch_1_presses: AtomicU64,
    pub switch_2_presses: AtomicU64,
    pub start_time: Instant,
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedState {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(128);
        let (log_tx, _) = broadcast::channel(256);

        Self {
            event_tx,
            log_tx,
            seq_counter: AtomicI64::new(1),
            switch_1_state: AtomicU8::new(SwitchState::Released as u8),
            switch_2_state: AtomicU8::new(SwitchState::Released as u8),
            switch_1_presses: AtomicU64::new(0),
            switch_2_presses: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    pub fn log(&self, level: LogLevel, component: &str, message: impl Into<String>) {
        let entry = LogEntry {
            timestamp_unix_millis: now_millis(),
            level: level.into(),
            component: component.to_string(),
            message: message.into(),
        };

        // Local server stdout print
        let level_str = match level {
            LogLevel::Debug => "[DEBUG]".cyan(),
            LogLevel::Info => "[INFO ]".green(),
            LogLevel::Warn => "[WARN ]".yellow(),
            LogLevel::Error => "[ERROR]".red().bold(),
        };
        println!("{} [{}] {}", level_str, entry.component.magenta(), entry.message);

        let _ = self.log_tx.send(entry);
    }

    pub fn record_event(
        &self,
        switch_id: SwitchId,
        state: SwitchState,
        raw_pin: u32,
        duration_pressed_millis: u32,
    ) -> SwitchEvent {
        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
        let event = SwitchEvent {
            sequence_number: seq,
            timestamp_unix_millis: now_millis(),
            switch_id: switch_id.into(),
            state: state.into(),
            raw_gpio_pin: raw_pin,
            duration_pressed_millis,
        };

        match switch_id {
            SwitchId::Switch1 => {
                self.switch_1_state.store(state as u8, Ordering::SeqCst);
                if state == SwitchState::Pressed {
                    self.switch_1_presses.fetch_add(1, Ordering::SeqCst);
                }
            }
            SwitchId::Switch2 => {
                self.switch_2_state.store(state as u8, Ordering::SeqCst);
                if state == SwitchState::Pressed {
                    self.switch_2_presses.fetch_add(1, Ordering::SeqCst);
                }
            }
            _ => {}
        }

        let state_name = match state {
            SwitchState::Pressed => "PRESSED".bright_green().bold(),
            SwitchState::Released => "RELEASED".bright_yellow(),
            SwitchState::Unspecified => "UNSPECIFIED".dimmed(),
        };

        let duration_info = if duration_pressed_millis > 0 {
            format!(" (held for {}ms)", duration_pressed_millis)
        } else {
            String::new()
        };

        println!(
            "{} Switch {:?} (GPIO {}) -> {}{}",
            "[EVENT]".bright_cyan().bold(),
            switch_id,
            raw_pin,
            state_name,
            duration_info
        );

        let _ = self.event_tx.send(event.clone());
        event
    }
}

#[derive(Clone)]
pub struct TelemetryServiceImpl {
    pub state: Arc<SharedState>,
}

impl TelemetryServiceImpl {
    pub fn new(state: Arc<SharedState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl TelemetryService for TelemetryServiceImpl {
    type StreamEventsStream = Pin<Box<dyn Stream<Item = Result<SwitchEvent, Status>> + Send + 'static>>;
    type StreamLogsStream = Pin<Box<dyn Stream<Item = Result<LogEntry, Status>> + Send + 'static>>;

    async fn stream_events(
        &self,
        request: Request<StreamEventsRequest>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        let req = request.into_inner();
        let client_id = if req.client_id.is_empty() {
            "anonymous".to_string()
        } else {
            req.client_id
        };

        self.state.log(
            LogLevel::Info,
            "grpc",
            format!("Client '{}' subscribed to switch events stream", client_id),
        );

        let rx = self.state.event_tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|res| match res {
            Ok(event) => Some(Ok(event)),
            Err(_) => None,
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn stream_logs(
        &self,
        request: Request<StreamLogsRequest>,
    ) -> Result<Response<Self::StreamLogsStream>, Status> {
        let req = request.into_inner();
        let client_id = if req.client_id.is_empty() {
            "anonymous".to_string()
        } else {
            req.client_id
        };
        let min_level = req.min_level;

        self.state.log(
            LogLevel::Info,
            "grpc",
            format!(
                "Client '{}' subscribed to log stream (min_level: {:?})",
                client_id, min_level
            ),
        );

        let rx = self.state.log_tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(move |res| match res {
            Ok(entry) if entry.level >= min_level => Some(Ok(entry)),
            Ok(_) => None,
            Err(_) => None,
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let uptime = self.state.start_time.elapsed().as_secs();
        let s1 = self.state.switch_1_state.load(Ordering::SeqCst) as i32;
        let s2 = self.state.switch_2_state.load(Ordering::SeqCst) as i32;

        let res = StatusResponse {
            timestamp_unix_millis: now_millis(),
            switch_1_state: s1,
            switch_2_state: s2,
            total_switch_1_presses: self.state.switch_1_presses.load(Ordering::SeqCst),
            total_switch_2_presses: self.state.switch_2_presses.load(Ordering::SeqCst),
            uptime_seconds: uptime,
        };

        Ok(Response::new(res))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_state_initial_values() {
        let state = SharedState::new();
        assert_eq!(state.switch_1_state.load(Ordering::SeqCst), SwitchState::Released as u8);
        assert_eq!(state.switch_2_state.load(Ordering::SeqCst), SwitchState::Released as u8);
        assert_eq!(state.switch_1_presses.load(Ordering::SeqCst), 0);
        assert_eq!(state.switch_2_presses.load(Ordering::SeqCst), 0);
        assert_eq!(state.seq_counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_record_event_state_and_counter_updates() {
        let state = SharedState::new();
        let mut rx = state.event_tx.subscribe();

        // 1. Press Switch 1
        let ev1 = state.record_event(SwitchId::Switch1, SwitchState::Pressed, PIN_SWITCH_1, 0);
        assert_eq!(ev1.sequence_number, 1);
        assert_eq!(ev1.switch_id, SwitchId::Switch1 as i32);
        assert_eq!(ev1.state, SwitchState::Pressed as i32);
        assert_eq!(state.switch_1_state.load(Ordering::SeqCst), SwitchState::Pressed as u8);
        assert_eq!(state.switch_1_presses.load(Ordering::SeqCst), 1);

        // 2. Release Switch 1 with 150ms hold
        let ev2 = state.record_event(SwitchId::Switch1, SwitchState::Released, PIN_SWITCH_1, 150);
        assert_eq!(ev2.sequence_number, 2);
        assert_eq!(ev2.duration_pressed_millis, 150);
        assert_eq!(state.switch_1_state.load(Ordering::SeqCst), SwitchState::Released as u8);
        assert_eq!(state.switch_1_presses.load(Ordering::SeqCst), 1); // Not incremented on release

        // 3. Press Switch 2
        let ev3 = state.record_event(SwitchId::Switch2, SwitchState::Pressed, PIN_SWITCH_2, 0);
        assert_eq!(ev3.sequence_number, 3);
        assert_eq!(state.switch_2_state.load(Ordering::SeqCst), SwitchState::Pressed as u8);
        assert_eq!(state.switch_2_presses.load(Ordering::SeqCst), 1);

        // Verify broadcast receiver received all 3 events
        let r1 = rx.try_recv().unwrap();
        assert_eq!(r1.sequence_number, 1);
        let r2 = rx.try_recv().unwrap();
        assert_eq!(r2.sequence_number, 2);
        let r3 = rx.try_recv().unwrap();
        assert_eq!(r3.sequence_number, 3);
    }

    #[test]
    fn test_log_broadcasting() {
        let state = SharedState::new();
        let mut rx = state.log_tx.subscribe();

        state.log(LogLevel::Warn, "test_comp", "Warning alert");
        let entry = rx.try_recv().unwrap();
        assert_eq!(entry.level, LogLevel::Warn as i32);
        assert_eq!(entry.component, "test_comp");
        assert_eq!(entry.message, "Warning alert");
    }
}
