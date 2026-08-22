---
tags: [moc, project, hardware, raspberry-pi, grpc, rust]
status: active
---

# Minimum Hardware Project (RPi 3 + gRPC + Rust)

A minimal, robust end-to-end hardware-to-cloud/client demonstration: a **Raspberry Pi 3** connected to **2 push switches** (with pull-up resistors), publishing real-time button events and diagnostic logs over **gRPC** to a **Rust debug receiver / client** and a companion **[Flutter Mobile Client (`demo_flutter_grpc`)](https://github.com/jger/demo_flutter_grpc)**.

> [!tip] Goal & Purpose
> Demonstrate bare-metal hardware interfacing on an embedded Linux device (RPi 3), reliable signal conditioning (pull-ups & debounce), and modern type-safe async streaming using **Rust** (`tonic` + `tokio`) over **gRPC/Protobuf**.

```
  +-------------------------------------+
  |   Raspberry Pi 3 (Linux ARMv7/aarch64)
  |                                     |
  |  [ Switch 1 ] --> GPIO 23 (Pull-up) |
  |  [ Switch 2 ] --> GPIO 24 (Pull-up) |
  |                                     |
  |        pi_node (Rust Server)        |
  |    * rppal GPIO polling/interrupts  |
  |    * Debounce filter                |
  |    * gRPC Telemetry Service (50051) |
  +------------------+------------------+
                     |
         gRPC Stream (HTTP/2 / Proto)
                     |
                     v
  +-------------------------------------+
  |  log_receiver (Rust Debug Client)   |
  |                                     |
  |  * Connects to Pi at :50051         |
  |  * Subscribes to StreamEvents / Logs|
  |  * Live colored terminal log viewer |
  +-------------------------------------+
```

---

## Documentation Map

1. [[01 Hardware & Wiring]] — Raspberry Pi 3 pinout, 10kΩ pull-up resistor & 100nF capacitor circuits, physical wiring table.
2. [[02 gRPC Contract & Protobuf]] — `telemetry.proto` definition with `StreamEvents` and `StreamLogs` RPCs.
3. [[03 Pi Node Service (Rust)]] — Server daemon implementation: GPIO handling, event channels, and gRPC broadcast.
4. [[04 Debug Receiver Client (Rust)]] — Debug client implementation: streaming receiver and diagnostic logger.
5. [[05 Build & Deploy Guide]] — How to build locally (with mock mode), cross-compile for Raspberry Pi 3, and run.
6. [[06 Hardware Debouncing & Capacitors]] — Electrical engineering principles, RC time constant ($\tau=1\text{ms}$), frequency response, and why 100nF capacitors are used.

---

## Quick Reference Summary

| Parameter | Specification |
|---|---|
| **Target SBC** | Raspberry Pi 3 Model B / B+ (Broadcom BCM2837) |
| **Inputs** | 2x Momentary Push Buttons (Normally Open) |
| **GPIO Pins** | GPIO 23 (Pin 16) & GPIO 24 (Pin 18) |
| **Pull-Up & Filter** | 10kΩ pull-up to 3.3V + 100nF ceramic capacitor to GND ($\tau = 1.0\text{ ms}$) |
| **Protocol** | gRPC (HTTP/2 with Protocol Buffers v3) |
| **Language** | Rust 2021 edition (`tonic`, `prost`, `tokio`, `rppal`) |
| **Default Port** | `50051` |
