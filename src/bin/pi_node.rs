use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use colored::Colorize;
use tonic::transport::Server;

use minimum_hw_project::proto::telemetry_service_server::TelemetryServiceServer;
use minimum_hw_project::proto::{LogLevel, SwitchId, SwitchState};
use minimum_hw_project::server::{PIN_SWITCH_1, PIN_SWITCH_2};
use minimum_hw_project::{SharedState, TelemetryServiceImpl};

#[cfg(target_os = "linux")]
use minimum_hw_project::server::DEBOUNCE_MS;

// -----------------------------------------------------------------------------
// Linux / Raspberry Pi 3 Hardware GPIO Implementation
// -----------------------------------------------------------------------------
#[cfg(target_os = "linux")]
async fn spawn_gpio_tasks(state: Arc<SharedState>) -> Result<(), Box<dyn std::error::Error>> {
    use rppal::gpio::{Gpio, Level, Trigger};
    use std::time::Instant;

    state.log(LogLevel::Info, "gpio", "Initializing rppal hardware GPIO on RPi 3...");

    let gpio = match Gpio::new() {
        Ok(g) => g,
        Err(e) => {
            state.log(LogLevel::Error, "gpio", format!("Failed to access GPIO: {}. Falling back to simulation mode.", e));
            spawn_simulated_gpio(state);
            return Ok(());
        }
    };

    // Switch 1 on GPIO 23 with pull-up resistor
    let mut pin23 = match gpio.get(PIN_SWITCH_1) {
        Ok(p) => p.into_input_pullup(),
        Err(e) => {
            state.log(LogLevel::Error, "gpio", format!("Failed to configure GPIO {}: {}", PIN_SWITCH_1, e));
            return Err(e.into());
        }
    };

    // Switch 2 on GPIO 24 with pull-up resistor
    let mut pin24 = match gpio.get(PIN_SWITCH_2) {
        Ok(p) => p.into_input_pullup(),
        Err(e) => {
            state.log(LogLevel::Error, "gpio", format!("Failed to configure GPIO {}: {}", PIN_SWITCH_2, e));
            return Err(e.into());
        }
    };

    pin23.set_interrupt(Trigger::Both, Some(Duration::from_millis(DEBOUNCE_MS)))?;
    pin24.set_interrupt(Trigger::Both, Some(Duration::from_millis(DEBOUNCE_MS)))?;

    state.log(
        LogLevel::Info,
        "gpio",
        format!(
            "Hardware GPIO ready: SW1 -> GPIO {} (Pull-Up), SW2 -> GPIO {} (Pull-Up)",
            PIN_SWITCH_1, PIN_SWITCH_2
        ),
    );

    // Monitoring task for Pin 23
    let state_23 = state.clone();
    tokio::task::spawn_blocking(move || {
        let mut last_press_time: Option<Instant> = None;
        let mut last_transition = Instant::now();

        loop {
            if let Ok(Some(_event)) = pin23.poll_interrupt(true, Some(Duration::from_millis(500))) {
                let now = Instant::now();
                if now.duration_since(last_transition) < Duration::from_millis(DEBOUNCE_MS) {
                    continue;
                }
                last_transition = now;

                let level = pin23.read();
                match level {
                    Level::Low => {
                        last_press_time = Some(now);
                        state_23.record_event(SwitchId::Switch1, SwitchState::Pressed, PIN_SWITCH_1, 0);
                        state_23.log(LogLevel::Debug, "gpio", "Switch 1 contact closed (Active LOW)");
                    }
                    Level::High => {
                        let duration = last_press_time
                            .take()
                            .map(|t| t.elapsed().as_millis() as u32)
                            .unwrap_or(0);
                        state_23.record_event(
                            SwitchId::Switch1,
                            SwitchState::Released,
                            PIN_SWITCH_1,
                            duration,
                        );
                        state_23.log(LogLevel::Debug, "gpio", format!("Switch 1 released after {}ms", duration));
                    }
                }
            }
        }
    });

    // Monitoring task for Pin 24
    let state_24 = state.clone();
    tokio::task::spawn_blocking(move || {
        let mut last_press_time: Option<Instant> = None;
        let mut last_transition = Instant::now();

        loop {
            if let Ok(Some(_event)) = pin24.poll_interrupt(true, Some(Duration::from_millis(500))) {
                let now = Instant::now();
                if now.duration_since(last_transition) < Duration::from_millis(DEBOUNCE_MS) {
                    continue;
                }
                last_transition = now;

                let level = pin24.read();
                match level {
                    Level::Low => {
                        last_press_time = Some(now);
                        state_24.record_event(SwitchId::Switch2, SwitchState::Pressed, PIN_SWITCH_2, 0);
                        state_24.log(LogLevel::Debug, "gpio", "Switch 2 contact closed (Active LOW)");
                    }
                    Level::High => {
                        let duration = last_press_time
                            .take()
                            .map(|t| t.elapsed().as_millis() as u32)
                            .unwrap_or(0);
                        state_24.record_event(
                            SwitchId::Switch2,
                            SwitchState::Released,
                            PIN_SWITCH_2,
                            duration,
                        );
                        state_24.log(LogLevel::Debug, "gpio", format!("Switch 2 released after {}ms", duration));
                    }
                }
            }
        }
    });

    Ok(())
}

// -----------------------------------------------------------------------------
// Non-Linux / Simulation Mock for local testing on macOS / Windows
// -----------------------------------------------------------------------------
#[cfg(not(target_os = "linux"))]
async fn spawn_gpio_tasks(state: Arc<SharedState>) -> Result<(), Box<dyn std::error::Error>> {
    spawn_simulated_gpio(state);
    Ok(())
}

fn spawn_simulated_gpio(state: Arc<SharedState>) {
    state.log(
        LogLevel::Info,
        "sim",
        "Running in SIMULATION mode. (Simulating periodic switch pulses + keyboard trigger)",
    );

    let state_sim = state.clone();
    tokio::spawn(async move {
        let mut toggle = false;
        loop {
            tokio::time::sleep(Duration::from_secs(4)).await;
            toggle = !toggle;
            let (sw_id, pin) = if toggle {
                (SwitchId::Switch1, PIN_SWITCH_1)
            } else {
                (SwitchId::Switch2, PIN_SWITCH_2)
            };

            state_sim.log(LogLevel::Debug, "sim", format!("Simulating {:?} press", sw_id));
            state_sim.record_event(sw_id, SwitchState::Pressed, pin, 0);

            tokio::time::sleep(Duration::from_millis(350)).await;
            state_sim.record_event(sw_id, SwitchState::Released, pin, 350);
            state_sim.log(LogLevel::Debug, "sim", format!("Simulating {:?} release", sw_id));
        }
    });
}

// Periodic system health diagnostic logger
fn spawn_heartbeat_logger(state: Arc<SharedState>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(10));
        loop {
            ticker.tick().await;
            let uptime = state.start_time.elapsed().as_secs();
            let p1 = state.switch_1_presses.load(std::sync::atomic::Ordering::Relaxed);
            let p2 = state.switch_2_presses.load(std::sync::atomic::Ordering::Relaxed);
            state.log(
                LogLevel::Info,
                "health",
                format!(
                    "System heartbeat: Uptime={}s | Press Counts: SW1={} SW2={}",
                    uptime, p1, p2
                ),
            );
        }
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "==================================================".cyan());
    println!("{}", "  Raspberry Pi 3 — gRPC Hardware Node Service".bright_cyan().bold());
    println!("{}", "  Pins: SW1 -> GPIO 23 (Pull-up) | SW2 -> GPIO 24 (Pull-up)".dimmed());
    println!("{}", "==================================================".cyan());

    let state = Arc::new(SharedState::new());

    // Spawn GPIO hardware / simulation worker
    spawn_gpio_tasks(state.clone()).await?;

    // Spawn diagnostic heartbeat
    spawn_heartbeat_logger(state.clone());

    let addr: SocketAddr = "0.0.0.0:50051".parse()?;
    state.log(
        LogLevel::Info,
        "grpc",
        format!("Starting gRPC server listening on {}", addr),
    );

    let service = TelemetryServiceImpl::new(state.clone());

    Server::builder()
        .add_service(TelemetryServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
