---
tags: [rust, grpc, server, rppal, embedded]
status: active
---

# 03 Pi Node Service (Rust)

This document describes the implementation of the `pi_node` binary — the daemon running on the Raspberry Pi 3.

---

## 1. Responsibilities

1. **GPIO Initialization**: Configures GPIO 23 and 24 as inputs with internal pull-up enabled via `rppal::gpio::Gpio`.
2. **Interrupt & Debounce Loop**: Monitors asynchronous falling/rising edges, applies a 20ms debounce filter, and calculates press hold durations.
3. **Internal Broadcast Channel**: Pushes switch state transitions and diagnostic log messages to `tokio::sync::broadcast` channels.
4. **gRPC Server (`Tonic`)**: Binds on `0.0.0.0:50051`, handling `StreamEvents`, `StreamLogs`, and `GetStatus` requests.
5. **Cross-Platform Mock**: When built on non-Linux platforms (macOS / Windows), seamlessly switches to simulated interactive switch inputs.

---

## 2. Concurrency Architecture

```
                      +-----------------------------+
                      |   Hardware GPIO Interrupts  |
                      |     (GPIO 23 & GPIO 24)     |
                      +--------------+--------------+
                                     |
                                     v
                      +-----------------------------+
                      | Debounce & State Machine    |
                      +--------------+--------------+
                                     |
             +-----------------------+-----------------------+
             |                                               |
             v                                               v
+--------------------------+                   +--------------------------+
|  broadcast::Sender       |                   |  broadcast::Sender       |
|  <SwitchEvent>           |                   |  <LogEntry>              |
+------------+-------------+                   +------------+-------------+
             |                                               |
             +-----------------------+-----------------------+
                                     |
                                     v
                      +-----------------------------+
                      |    Tonic gRPC Service       |
                      |   (telemetry.v1 Service)    |
                      +--------------+--------------+
                                     |
                                 TCP 50051
                                     |
                                     v
                              Remote Clients
```

---

## 3. Key Rust Code Patterns

### GPIO Setup with `rppal` (on Linux / RPi 3)
```rust
let gpio = Gpio::new()?;
let mut pin23 = gpio.get(23)?.into_input_pullup();
pin23.set_interrupt(Trigger::Both)?;
```

### Active-Low Logic Mapping
- When switch is closed (pressed): pin is `Level::Low` -> `SwitchState::Pressed`.
- When switch is open (released): pin is `Level::High` -> `SwitchState::Released`.

### Tokio Broadcast Streams for Tonic
```rust
let mut rx = self.event_tx.subscribe();
let output_stream = async_stream::try_stream! {
    while let Ok(event) = rx.recv().await {
        yield event;
    }
};
Ok(Response::new(Box::pin(output_stream)))
```
