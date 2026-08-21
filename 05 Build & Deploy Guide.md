---
tags: [rust, build, cross-compile, deploy, raspberry-pi]
status: active
---

# 05 Build & Deploy Guide

This guide covers building the **`pi_node`** service for **Raspberry Pi / Linux** and the **`log_receiver`** client for **macOS / Linux / Windows**.

---

## 1. Local Development & Testing

- **`pi_node`**: Hardware GPIO service compiled for Linux / Raspberry Pi (RPi 3, 4, 5).
- **`log_receiver`**: Universal telemetry receiver client that runs on macOS, Linux, and Windows to monitor the Pi over the network.

### Run the Debug Receiver Client on Workstation (macOS / Linux):
```bash
cd minimum-hw-project
cargo run --bin log_receiver -- --server-url http://<PI_IP>:50051
```

---

## 2. Compiling on the Raspberry Pi 3 Directly

If Rust is installed on the Pi (Raspberry Pi OS 32-bit `armv7` or 64-bit `aarch64`):

```bash
# On the Raspberry Pi:
git clone <your-repo>
cd minimum-hw-project
cargo build --release --bin pi_node
./target/release/pi_node
```

> [!note] Permissions
> On modern Raspberry Pi OS, standard users in the `gpio` group can access `/dev/gpiomem` without `sudo`.

---

## 3. Cross-Compiling with `cross` (Static musl Binaries)

From your development machine (macOS / Linux):

```bash
# Install cross tool
cargo install cross --git https://github.com/cross-rs/cross

# For 32-bit Raspberry Pi OS (ARMv7 - static musl, zero GLIBC dependency):
cross build --target armv7-unknown-linux-musleabihf --release --bin pi_node

# For 64-bit Raspberry Pi OS (AArch64 - static musl, zero GLIBC dependency):
cross build --target aarch64-unknown-linux-musl --release --bin pi_node

# Copy binary to Raspberry Pi:
scp target/armv7-unknown-linux-musleabihf/release/pi_node pi@raspberrypi.local:~/
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

---

## 5. Automated CI/CD & Semantic Release

This repository uses **Semantic Release** and **GitHub Actions** (`.github/workflows/release.yml`) to automatically version, changelog, and cross-compile binary releases on push to `main`.

### Conventional Commits Format:
- `feat: ...` ➔ Triggers a **Minor** version bump (e.g. `v0.1.0` ➔ `v0.2.0`).
- `fix: ...` or `perf: ...` ➔ Triggers a **Patch** version bump (e.g. `v0.1.0` ➔ `v0.1.1`).
- `feat!: ...` or `BREAKING CHANGE:` ➔ Triggers a **Major** version bump (e.g. `v0.1.0` ➔ `v1.0.0`).
- `docs:`, `chore:`, `ci:`, `test:` ➔ Does not trigger a release.

### Generated Multi-Platform Release Assets:
Every release publishes archives, standalone binaries, and SHA-256 checksums to GitHub Releases:
- **Raspberry Pi 3 (ARMv7 32-bit)**: `minimum-hw-rpi-armv7-linux-vX.Y.Z.tar.gz` (`pi_node` + `log_receiver`)
- **Raspberry Pi (AArch64 64-bit)**: `minimum-hw-rpi-aarch64-linux-vX.Y.Z.tar.gz` (`pi_node` + `log_receiver`)
- **Linux (x86_64)**: `minimum-hw-x86_64-linux-vX.Y.Z.tar.gz` (`pi_node` + `log_receiver`)
- **macOS Apple Silicon (M1/M2/M3/M4)**: `minimum-hw-macos-aarch64-apple-silicon-vX.Y.Z.tar.gz` (`log_receiver` client)
- **macOS Intel (x86_64)**: `minimum-hw-macos-x86_64-intel-vX.Y.Z.tar.gz` (`log_receiver` client)
