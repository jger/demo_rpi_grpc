use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use colored::Colorize;
use tonic::transport::Server;

use minimum_hw_project::proto::telemetry_service_server::TelemetryServiceServer;
use minimum_hw_project::proto::{LogLevel, SwitchId, SwitchState};
use minimum_hw_project::server::{PIN_SWITCH_1, PIN_SWITCH_2};
use minimum_hw_project::{SharedState, TelemetryServiceImpl};

/// The switches this node owns, in reporting order.
const SWITCHES: [(SwitchId, u8); 2] = [
    (SwitchId::Switch1, PIN_SWITCH_1),
    (SwitchId::Switch2, PIN_SWITCH_2),
];

const LISTEN_ADDR: &str = "0.0.0.0:50051";

// -----------------------------------------------------------------------------
// Linux / Raspberry Pi 3 Hardware GPIO Implementation
// -----------------------------------------------------------------------------

/// Switches are wired active LOW against a pull-up: a closed contact reads Low.
#[cfg(target_os = "linux")]
fn read_state(pin: &rppal::gpio::InputPin) -> SwitchState {
    if pin.read() == rppal::gpio::Level::Low {
        SwitchState::Pressed
    } else {
        SwitchState::Released
    }
}

#[cfg(target_os = "linux")]
fn configure_input_pin(
    gpio: &rppal::gpio::Gpio,
    pin_number: u8,
    state: &SharedState,
) -> Result<rppal::gpio::InputPin, rppal::gpio::Error> {
    gpio.get(pin_number)
        .map(|pin| pin.into_input_pullup())
        .inspect_err(|e| {
            state.log(
                LogLevel::Error,
                "gpio",
                format!("Failed to configure GPIO {}: {}", pin_number, e),
            )
        })
}

#[cfg(target_os = "linux")]
fn monitor_hardware_switch(
    mut pin: rppal::gpio::InputPin,
    switch_id: SwitchId,
    pin_number: u8,
    state: Arc<SharedState>,
) {
    use std::time::Instant;

    let mut last_state = read_state(&pin);
    let mut press_start: Option<Instant> = if last_state == SwitchState::Pressed {
        Some(Instant::now())
    } else {
        None
    };

    // Trigger on both rising and falling edges (hardware capacitors handle filtering)
    if let Err(e) = pin.set_interrupt(rppal::gpio::Trigger::Both, None) {
        state.log(
            LogLevel::Warn,
            "gpio",
            format!("Warning: Failed to set interrupt on GPIO {}: {}", pin_number, e),
        );
    }

    loop {
        // Wait for an interrupt edge, or fall back to a 100ms poll timeout
        let _ = pin.poll_interrupt(true, Some(Duration::from_millis(100)));

        let current_state = read_state(&pin);
        if current_state == last_state {
            continue;
        }

        last_state = current_state;

        match current_state {
            SwitchState::Pressed => {
                press_start = Some(Instant::now());
                state.record_event(switch_id, SwitchState::Pressed, pin_number, 0);
                state.log(
                    LogLevel::Debug,
                    "gpio",
                    format!(
                        "{:?} (GPIO {}) contact closed [Active LOW Pressed]",
                        switch_id, pin_number
                    ),
                );
            }
            SwitchState::Released => {
                let duration_millis = press_start
                    .take()
                    .map(|start| start.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                state.record_event(switch_id, SwitchState::Released, pin_number, duration_millis);
                state.log(
                    LogLevel::Debug,
                    "gpio",
                    format!(
                        "{:?} (GPIO {}) contact opened [Released] (held for {}ms)",
                        switch_id, pin_number, duration_millis
                    ),
                );
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn spawn_gpio_tasks(state: Arc<SharedState>) -> Result<(), Box<dyn std::error::Error>> {
    state.log(LogLevel::Info, "gpio", "Initializing rppal hardware GPIO on RPi 3...");

    let gpio = match rppal::gpio::Gpio::new() {
        Ok(gpio) => gpio,
        Err(e) => {
            state.log(
                LogLevel::Error,
                "gpio",
                format!("Failed to access GPIO: {}. Falling back to simulation mode.", e),
            );
            spawn_simulated_gpio(state);
            return Ok(());
        }
    };

    let pins = SWITCHES
        .iter()
        .map(|&(switch_id, pin_number)| {
            configure_input_pin(&gpio, pin_number, &state).map(|pin| (switch_id, pin_number, pin))
        })
        .collect::<Result<Vec<_>, _>>()?;

    state.log(
        LogLevel::Info,
        "gpio",
        format!(
            "Hardware GPIO ready: SW1 -> GPIO {} (Pull-Up), SW2 -> GPIO {} (Pull-Up) | Direct HW Filtered (Capacitors)",
            PIN_SWITCH_1, PIN_SWITCH_2
        ),
    );

    for (switch_id, pin_number, pin) in pins {
        let state = state.clone();
        tokio::task::spawn_blocking(move || {
            monitor_hardware_switch(pin, switch_id, pin_number, state);
        });
    }

    Ok(())
}

// -----------------------------------------------------------------------------
// Non-Linux / Simulation Mock for local testing on macOS / Windows
// -----------------------------------------------------------------------------
#[cfg(not(target_os = "linux"))]
fn spawn_gpio_tasks(state: Arc<SharedState>) -> Result<(), Box<dyn std::error::Error>> {
    spawn_simulated_gpio(state);
    Ok(())
}

fn spawn_simulated_gpio(state: Arc<SharedState>) {
    state.log(
        LogLevel::Info,
        "sim",
        "Running in SIMULATION mode. (Simulating periodic switch pulses + keyboard trigger)",
    );

    tokio::spawn(async move {
        for (switch_id, pin) in SWITCHES.iter().copied().cycle() {
            tokio::time::sleep(Duration::from_secs(4)).await;
            state.log(LogLevel::Debug, "sim", format!("Simulating {:?} press", switch_id));
            state.record_event(switch_id, SwitchState::Pressed, pin, 0);

            tokio::time::sleep(Duration::from_millis(350)).await;
            state.record_event(switch_id, SwitchState::Released, pin, 350);
            state.log(LogLevel::Debug, "sim", format!("Simulating {:?} release", switch_id));
        }
    });
}

// Periodic system health diagnostic logger
fn spawn_heartbeat_logger(state: Arc<SharedState>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(10));
        loop {
            ticker.tick().await;
            state.log(
                LogLevel::Info,
                "health",
                format!(
                    "System heartbeat: Uptime={}s | Press Counts: SW1={} SW2={}",
                    state.start_time.elapsed().as_secs(),
                    state.switch_1_presses.load(Ordering::Relaxed),
                    state.switch_2_presses.load(Ordering::Relaxed),
                ),
            );
        }
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "==================================================".cyan());
    println!("{}", "  Raspberry Pi 3 — gRPC Hardware Node Service".bright_cyan().bold());
    println!(
        "{}",
        format!(
            "  Pins: SW1 -> GPIO {} (Pull-up) | SW2 -> GPIO {} (Pull-up)",
            PIN_SWITCH_1, PIN_SWITCH_2
        )
        .dimmed()
    );
    println!("{}", "==================================================".cyan());

    let state = Arc::new(SharedState::new());

    spawn_gpio_tasks(state.clone())?;
    spawn_heartbeat_logger(state.clone());

    let addr: SocketAddr = LISTEN_ADDR.parse()?;
    state.log(
        LogLevel::Info,
        "grpc",
        format!("Starting gRPC server listening on {}", addr),
    );

    Server::builder()
        .add_service(TelemetryServiceServer::new(TelemetryServiceImpl::new(
            state.clone(),
        )))
        .serve(addr)
        .await?;

    Ok(())
}
