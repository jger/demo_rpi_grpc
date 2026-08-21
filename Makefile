# ==============================================================================
# Makefile for minimum-hw-project (RPi 3 gRPC Hardware & Telemetry)
# ==============================================================================

.DEFAULT_GOAL := help
.PHONY: help build build-release build-node build-receiver run-node run-receiver \
        check test fmt clippy clean check-buttons \
        build-macos build-macos-arm64 build-macos-x86 build-macos-universal \
        build-rpi build-rpi32 build-rpi64 cross-rpi32 cross-rpi64 deploy-rpi deploy-rpi64

# Configuration Variables (override via CLI, e.g., make deploy-rpi RPI_HOST=pi@192.168.1.50)
RPI_HOST   ?= pi@raspberrypi.local
RPI_DIR    ?= /home/pi
SERVER_URL ?= http://127.0.0.1:50051

# ANSI Color Codes
CYAN   := \033[36m
GREEN  := \033[32m
YELLOW := \033[33m
BOLD   := \033[1m
RESET  := \033[0m

## -----------------------------------------------------------------------------
## Help
## -----------------------------------------------------------------------------

help: ## Show this help menu with descriptions
	@echo ""
	@echo "$(BOLD)$(CYAN)minimum-hw-project$(RESET) - Available Make Commands"
	@echo "================================================================"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z0-9_-]+:.*?## / {printf "  $(GREEN)%-22s$(RESET) %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@echo ""
	@echo "$(BOLD)Variables (can be overridden):$(RESET)"
	@echo "  $(YELLOW)RPI_HOST$(RESET)   = $(RPI_HOST) (e.g. make deploy-rpi RPI_HOST=pi@192.168.1.10)"
	@echo "  $(YELLOW)SERVER_URL$(RESET) = $(SERVER_URL) (e.g. make run-receiver SERVER_URL=http://192.168.1.10:50051)"
	@echo ""

## -----------------------------------------------------------------------------
## Local Build & Development
## -----------------------------------------------------------------------------

build: ## Build all binaries in debug mode (dev)
	cargo build

build-release: ## Build all binaries in release mode (current host)
	cargo build --release

build-node: ## Build only the pi_node binary (debug)
	cargo build --bin pi_node

build-receiver: ## Build only the log_receiver binary (debug)
	cargo build --bin log_receiver

run-node: ## Run pi_node locally (interactive mock mode on macOS/PC)
	cargo run --bin pi_node

run-receiver: ## Run log_receiver client (connects to SERVER_URL)
	cargo run --bin log_receiver -- --server-url $(SERVER_URL)

check-buttons: ## Run the standalone Python button & timing checker
	python3 check_buttons.py

## -----------------------------------------------------------------------------
## macOS Builds
## -----------------------------------------------------------------------------

build-macos: ## Build release binaries for current macOS architecture
	cargo build --release --bin pi_node --bin log_receiver

build-macos-arm64: ## Build release binaries for macOS Apple Silicon (M1/M2/M3/M4)
	rustup target add aarch64-apple-darwin
	cargo build --release --target aarch64-apple-darwin --bin pi_node --bin log_receiver

build-macos-x86: ## Build release binaries for macOS Intel (x86_64)
	rustup target add x86_64-apple-darwin
	cargo build --release --target x86_64-apple-darwin --bin pi_node --bin log_receiver

build-macos-universal: build-macos-arm64 build-macos-x86 ## Create macOS Universal (fat) binaries (Intel + Apple Silicon)
	@mkdir -p target/universal/release
	lipo -create -output target/universal/release/pi_node target/aarch64-apple-darwin/release/pi_node target/x86_64-apple-darwin/release/pi_node
	lipo -create -output target/universal/release/log_receiver target/aarch64-apple-darwin/release/log_receiver target/x86_64-apple-darwin/release/log_receiver
	@echo "$(GREEN)Universal binaries created in target/universal/release/$(RESET)"

## -----------------------------------------------------------------------------
## Raspberry Pi Builds & Deployment (Cross-Compilation via `cross`)
## -----------------------------------------------------------------------------

build-rpi: build-rpi32 ## Alias for build-rpi32 (RPi 3 standard 32-bit build)

build-rpi32: ## Build release binaries for Raspberry Pi OS 32-bit (ARMv7 / RPi 3, static musl)
	cross build --target armv7-unknown-linux-musleabihf --release --bin pi_node --bin log_receiver

build-rpi64: ## Build release binaries for Raspberry Pi OS 64-bit (AArch64 / RPi 3, 4, 5, static musl)
	cross build --target aarch64-unknown-linux-musl --release --bin pi_node --bin log_receiver

cross-rpi32: build-rpi32 ## (Alias) Cross-compile for RPi 32-bit

cross-rpi64: build-rpi64 ## (Alias) Cross-compile for RPi 64-bit

deploy-rpi: build-rpi32 ## Build 32-bit binary and SCP to Raspberry Pi
	@echo "$(CYAN)Deploying pi_node to $(RPI_HOST):$(RPI_DIR)...$(RESET)"
	scp target/armv7-unknown-linux-musleabihf/release/pi_node $(RPI_HOST):$(RPI_DIR)/
	@echo "$(GREEN)Deployment complete!$(RESET)"

deploy-rpi64: build-rpi64 ## Build 64-bit binary and SCP to Raspberry Pi
	@echo "$(CYAN)Deploying 64-bit pi_node to $(RPI_HOST):$(RPI_DIR)...$(RESET)"
	scp target/aarch64-unknown-linux-musl/release/pi_node $(RPI_HOST):$(RPI_DIR)/
	@echo "$(GREEN)Deployment complete!$(RESET)"

## -----------------------------------------------------------------------------
## Testing & Code Quality
## -----------------------------------------------------------------------------

check: ## Fast syntax and type checking across all targets
	cargo check --all-targets

test: ## Run unit and integration tests
	cargo test

fmt: ## Format Rust codebase with rustfmt
	cargo fmt

clippy: ## Run Clippy linter with compiler warnings
	cargo clippy --all-targets -- -D warnings

## -----------------------------------------------------------------------------
## Cleanup
## -----------------------------------------------------------------------------

clean: ## Remove build artifacts and target directory
	cargo clean
