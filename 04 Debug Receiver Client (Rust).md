---
tags: [rust, grpc, client, logging, debugging]
status: active
---

# 04 Debug Receiver Client (Rust)

This document describes the implementation of `log_receiver` — a lightweight CLI debugging tool that connects over gRPC to display live switch events and server logs.

---

## 1. Features

- **Concurrent Streaming**: Listens simultaneously to both `StreamEvents` (switch transitions) and `StreamLogs` (server diagnostic logs).
- **Color-Coded Terminal Output**:
  - `DEBUG` -> Cyan / Dim
  - `INFO` -> Green
  - `WARN` -> Yellow
  - `ERROR` -> Red
  - `Switch 1 Pressed/Released` -> Bright Magenta
  - `Switch 2 Pressed/Released` -> Bright Blue
- **Duration Tracking**: Highlights how long switches were held down when released.
- **Connection Resilience**: Automatically attempts reconnection if the Raspberry Pi server restarts or network drops.

---

## 2. Terminal Output Example

```text
[2026-08-20 07:59:12.104] [INFO] [grpc] Client 'debug-console-8291' connected
[2026-08-20 07:59:13.450] [EVENT] SW1 (GPIO 23) -> PRESSED  [Seq: 1]
[2026-08-20 07:59:13.720] [EVENT] SW1 (GPIO 23) -> RELEASED [Seq: 2] (held for 270ms)
[2026-08-20 07:59:15.010] [EVENT] SW2 (GPIO 24) -> PRESSED  [Seq: 3]
[2026-08-20 07:59:15.110] [DEBUG] [gpio] Debounce filter active for GPIO 24
[2026-08-20 07:59:15.420] [EVENT] SW2 (GPIO 24) -> RELEASED [Seq: 4] (held for 410ms)
```

---

## 3. CLI Options

```bash
cargo run --bin log_receiver -- [OPTIONS]

Options:
  -s, --server-url <URL>    gRPC server endpoint [default: http://127.0.0.1:50051]
  -l, --min-level <LEVEL>   Minimum log level: debug, info, warn, error [default: debug]
  -h, --help                Print help
```
