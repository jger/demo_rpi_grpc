use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

use minimum_hw_project::proto::telemetry_service_client::TelemetryServiceClient;
use minimum_hw_project::proto::telemetry_service_server::TelemetryServiceServer;
use minimum_hw_project::proto::{
    GetStatusRequest, LogLevel, StreamEventsRequest, StreamLogsRequest, SwitchId, SwitchState,
};
use minimum_hw_project::server::{PIN_SWITCH_1, PIN_SWITCH_2};
use minimum_hw_project::{SharedState, TelemetryServiceImpl};

async fn spawn_test_server() -> (SocketAddr, Arc<SharedState>) {
    let state = Arc::new(SharedState::new());
    let service = TelemetryServiceImpl::new(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        Server::builder()
            .add_service(TelemetryServiceServer::new(service))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    (addr, state)
}

#[tokio::test]
async fn test_grpc_get_status() {
    let (addr, state) = spawn_test_server().await;
    let server_url = format!("http://{}", addr);

    let mut client = TelemetryServiceClient::connect(server_url).await.unwrap();

    // 1. Check initial status
    let res = client.get_status(GetStatusRequest {}).await.unwrap().into_inner();
    assert_eq!(res.switch_1_state, SwitchState::Released as i32);
    assert_eq!(res.switch_2_state, SwitchState::Released as i32);
    assert_eq!(res.total_switch_1_presses, 0);
    assert_eq!(res.total_switch_2_presses, 0);

    // 2. Trigger events on the server
    state.record_event(SwitchId::Switch1, SwitchState::Pressed, PIN_SWITCH_1, 0);
    state.record_event(SwitchId::Switch1, SwitchState::Released, PIN_SWITCH_1, 200);
    state.record_event(SwitchId::Switch1, SwitchState::Pressed, PIN_SWITCH_1, 0);
    state.record_event(SwitchId::Switch2, SwitchState::Pressed, PIN_SWITCH_2, 0);

    // 3. Query status again
    let updated = client.get_status(GetStatusRequest {}).await.unwrap().into_inner();
    assert_eq!(updated.switch_1_state, SwitchState::Pressed as i32);
    assert_eq!(updated.switch_2_state, SwitchState::Pressed as i32);
    assert_eq!(updated.total_switch_1_presses, 2);
    assert_eq!(updated.total_switch_2_presses, 1);
}

#[tokio::test]
async fn test_grpc_stream_events() {
    let (addr, state) = spawn_test_server().await;
    let server_url = format!("http://{}", addr);

    let mut client = TelemetryServiceClient::connect(server_url).await.unwrap();

    let mut stream = client
        .stream_events(StreamEventsRequest {
            client_id: "integration-test-runner".to_string(),
        })
        .await
        .unwrap()
        .into_inner();

    // Small delay to ensure subscription is established
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Emit Switch 1 sequence
    state.record_event(SwitchId::Switch1, SwitchState::Pressed, PIN_SWITCH_1, 0);
    state.record_event(SwitchId::Switch1, SwitchState::Released, PIN_SWITCH_1, 320);

    // Emit Switch 2 sequence
    state.record_event(SwitchId::Switch2, SwitchState::Pressed, PIN_SWITCH_2, 0);
    state.record_event(SwitchId::Switch2, SwitchState::Released, PIN_SWITCH_2, 180);

    // Verify received events in exact order
    let ev1 = stream.message().await.unwrap().expect("ev1");
    assert_eq!(ev1.sequence_number, 1);
    assert_eq!(ev1.switch_id, SwitchId::Switch1 as i32);
    assert_eq!(ev1.state, SwitchState::Pressed as i32);
    assert_eq!(ev1.raw_gpio_pin, PIN_SWITCH_1);
    assert_eq!(ev1.duration_pressed_millis, 0);

    let ev2 = stream.message().await.unwrap().expect("ev2");
    assert_eq!(ev2.sequence_number, 2);
    assert_eq!(ev2.switch_id, SwitchId::Switch1 as i32);
    assert_eq!(ev2.state, SwitchState::Released as i32);
    assert_eq!(ev2.duration_pressed_millis, 320);

    let ev3 = stream.message().await.unwrap().expect("ev3");
    assert_eq!(ev3.sequence_number, 3);
    assert_eq!(ev3.switch_id, SwitchId::Switch2 as i32);
    assert_eq!(ev3.state, SwitchState::Pressed as i32);
    assert_eq!(ev3.raw_gpio_pin, PIN_SWITCH_2);

    let ev4 = stream.message().await.unwrap().expect("ev4");
    assert_eq!(ev4.sequence_number, 4);
    assert_eq!(ev4.switch_id, SwitchId::Switch2 as i32);
    assert_eq!(ev4.state, SwitchState::Released as i32);
    assert_eq!(ev4.duration_pressed_millis, 180);
}

#[tokio::test]
async fn test_grpc_stream_logs_filtering() {
    let (addr, state) = spawn_test_server().await;
    let server_url = format!("http://{}", addr);

    let mut client_all = TelemetryServiceClient::connect(server_url.clone()).await.unwrap();
    let mut client_warn = TelemetryServiceClient::connect(server_url).await.unwrap();

    let mut stream_all = client_all
        .stream_logs(StreamLogsRequest {
            client_id: "client-all".to_string(),
            min_level: LogLevel::Debug as i32,
        })
        .await
        .unwrap()
        .into_inner();

    let mut stream_warn = client_warn
        .stream_logs(StreamLogsRequest {
            client_id: "client-warn-only".to_string(),
            min_level: LogLevel::Warn as i32,
        })
        .await
        .unwrap()
        .into_inner();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Emit 4 log entries with increasing severity
    state.log(LogLevel::Debug, "gpio", "Debug trace log");
    state.log(LogLevel::Info, "network", "Connected to broker");
    state.log(LogLevel::Warn, "power", "Low voltage warning");
    state.log(LogLevel::Error, "sensor", "Sensor fault detected");

    // Client ALL should receive our 4 test log entries
    // (Skipping any initial internal grpc connection logs)
    let mut all_test_logs = Vec::new();
    while all_test_logs.len() < 4 {
        if let Some(msg) = stream_all.message().await.unwrap() {
            if msg.component != "grpc" {
                all_test_logs.push(msg);
            }
        }
    }

    assert_eq!(all_test_logs[0].level, LogLevel::Debug as i32);
    assert_eq!(all_test_logs[0].component, "gpio");

    assert_eq!(all_test_logs[1].level, LogLevel::Info as i32);
    assert_eq!(all_test_logs[1].component, "network");

    assert_eq!(all_test_logs[2].level, LogLevel::Warn as i32);
    assert_eq!(all_test_logs[2].component, "power");

    assert_eq!(all_test_logs[3].level, LogLevel::Error as i32);
    assert_eq!(all_test_logs[3].component, "sensor");

    // Client WARN should ONLY receive: Warn, Error
    let mut warn_test_logs = Vec::new();
    while warn_test_logs.len() < 2 {
        if let Some(msg) = stream_warn.message().await.unwrap() {
            if msg.component != "grpc" {
                warn_test_logs.push(msg);
            }
        }
    }

    assert_eq!(warn_test_logs[0].level, LogLevel::Warn as i32);
    assert_eq!(warn_test_logs[0].component, "power");

    assert_eq!(warn_test_logs[1].level, LogLevel::Error as i32);
    assert_eq!(warn_test_logs[1].component, "sensor");
}
