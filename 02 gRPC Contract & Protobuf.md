---
tags: [grpc, protobuf, api, contract]
status: active
---

# 02 gRPC Contract & Protobuf

This document outlines the Protocol Buffers interface definition (`telemetry.proto`) between the Raspberry Pi hardware node and client applications.

---

## 1. Proto Definition: `telemetry.proto`

Located in `minimum-hw-project/proto/telemetry.proto`:

```protobuf
syntax = "proto3";

package telemetry.v1;

// Service exposed by the Raspberry Pi hardware node
service TelemetryService {
  // Server-streaming RPC: Emits switch state transition events in real time
  rpc StreamEvents (StreamEventsRequest) returns (stream SwitchEvent);

  // Server-streaming RPC: Emits system and debug log messages
  rpc StreamLogs (StreamLogsRequest) returns (stream LogEntry);

  // Unary RPC: Query the current instantaneous state of both switches
  rpc GetStatus (GetStatusRequest) returns (StatusResponse);
}

// Request to stream switch events
message StreamEventsRequest {
  string client_id = 1;
}

// Request to stream logs
message StreamLogsRequest {
  string client_id = 1;
  LogLevel min_level = 2;
}

// Switch state enum
enum SwitchState {
  SWITCH_STATE_UNSPECIFIED = 0;
  RELEASED = 1;
  PRESSED = 2;
}

// Switch ID enum
enum SwitchId {
  SWITCH_ID_UNSPECIFIED = 0;
  SWITCH_1 = 1; // GPIO 23
  SWITCH_2 = 2; // GPIO 24
}

// Real-time switch event
message SwitchEvent {
  int64 sequence_number = 1;
  int64 timestamp_unix_millis = 2;
  SwitchId switch_id = 3;
  SwitchState state = 4;
  uint32 raw_gpio_pin = 5;
  uint32 duration_pressed_millis = 6; // Provided when state is RELEASED
}

// Log level
enum LogLevel {
  DEBUG = 0;
  INFO = 1;
  WARN = 2;
  ERROR = 3;
}

// Diagnostic and operational log entry
message LogEntry {
  int64 timestamp_unix_millis = 1;
  LogLevel level = 2;
  string component = 3; // e.g. "gpio", "grpc", "system"
  string message = 4;
}

// Instantaneous status query
message GetStatusRequest {}

message StatusResponse {
  int64 timestamp_unix_millis = 1;
  SwitchState switch_1_state = 2;
  SwitchState switch_2_state = 3;
  uint64 total_switch_1_presses = 4;
  uint64 total_switch_2_presses = 5;
  uint64 uptime_seconds = 6;
}
```

---

## 2. API Design Principles

1. **Server-Streaming over Polling**: Button events happen asynchronously. Using `stream SwitchEvent` provides sub-millisecond event delivery without CPU-wasting client polling.
2. **Deterministic Enums**: Enums have explicit `_UNSPECIFIED = 0` default values to adhere to Protobuf best practices.
3. **Structured Logging**: `LogEntry` provides level, component tags, and timestamps for clean filtering and colored terminal output on the receiver.
4. **Resilient Reconnection**: The receiver client handles connection drops and auto-reconnects with exponential backoff.
