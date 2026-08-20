---
tags: [rust, build, cross-compile, deploy, raspberry-pi]
status: active
---

# 05 Build & Deploy Guide

This guide covers building locally on your workstation (with simulated GPIO) and cross-compiling or compiling directly on the **Raspberry Pi 3**.

---

## 1. Local Development (macOS / Linux / Windows)

The project includes an automatic mock hardware mode for local development.

### Run the Pi Node (Server) in Mock Mode:
```bash
cd minimum-hw-project
cargo run --bin pi_node
```
*In mock mode on macOS/non-RPi, pressing keys `1` or `2` in the terminal triggers Switch 1 or Switch 2 events.*

### Run the Debug Receiver Client:
In a second terminal:
```bash
cd minimum-hw-project
cargo run --bin log_receiver
```

---

## 2. Compiling on the Raspberry Pi 3 Directly

If Rust is installed on the Pi (Raspberry Pi OS 32-bit `armv7` or 64-bit `aarch64`):

```bash
# On the Raspberry Pi:
git clone <your-repo>
cd minimum-hw-project
cargo build --release --bin pi_node
sudo ./target/release/pi_node
```

> [!note] Permissions
> On modern Raspberry Pi OS, standard users in the `gpio` group can access `/dev/gpiomem` without `sudo`.

---

## 3. Cross-Compiling with `cross`

From your development machine:

```bash
# Install cross tool
cargo install cross --git https://github.com/cross-rs/cross

# For 32-bit Raspberry Pi OS (ARMv7):
cross build --target armv7-unknown-linux-gnueabihf --release --bin pi_node

# For 64-bit Raspberry Pi OS (aarch64):
cross build --target aarch64-unknown-linux-gnu --release --bin pi_node

# Copy binary to Raspberry Pi:
scp target/armv7-unknown-linux-gnueabihf/release/pi_node pi@raspberrypi.local:~/
```

---

## 4. Running as a `systemd` Service on RPi 3

Create `/etc/systemd/system/pi-node.service`:

```ini
[Unit]
Description=RPi 3 gRPC Hardware Node
After=network.target

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi
ExecStart=/home/pi/pi_node
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now pi-node.service
sudo journalctl -u pi-node.service -f
```
