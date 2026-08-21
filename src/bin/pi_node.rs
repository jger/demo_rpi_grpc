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

// -----------------------------------------------------------------------------
// Constants & Configuration
// -----------------------------------------------------------------------------

/// The local IP address and port where the gRPC server listens for incoming client connections.
/// "0.0.0.0" means it accepts connections from any network interface on port 50051.
const LISTEN_ADDR: &str = "0.0.0.0:50051";

// -----------------------------------------------------------------------------
// Application Entry Point
// -----------------------------------------------------------------------------

/// The entry point of the Raspberry Pi hardware node application.
///
/// - `#[tokio::main]`: Automatically sets up the Tokio asynchronous multithreaded runtime.
/// - `async fn main()`: Allows asynchronous operations like `.await` inside main.
/// - `-> Result<(), Box<dyn std::error::Error>>`: Returns `Ok(())` on clean exit or propagates any error.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        "==================================================".cyan()
    );
    println!(
        "{}",
        "  Raspberry Pi 3 — gRPC Hardware Node Service"
            .bright_cyan()
            .bold()
    );
    println!(
        "{}",
        format!(
            "  Pins: SW1 -> GPIO {} (Pull-up) | SW2 -> GPIO {} (Pull-up)",
            PIN_SWITCH_1, PIN_SWITCH_2
        )
        .dimmed()
    );
    println!(
        "{}",
        "==================================================".cyan()
    );

    // Step 1: Create shared application state wrapped in an Arc (thread-safe reference counter)
    // to allow safe concurrent access across all background tasks and gRPC handlers.
    let state = Arc::new(SharedState::new());

    // Step 2: Initialize hardware GPIO pins and spawn hardware monitoring tasks
    spawn_gpio_tasks(state.clone())?;

    // Step 3: Spawn the 10-second periodic heartbeat logger
    spawn_heartbeat_logger(state.clone());

    // Step 4: Parse server listening address (0.0.0.0:50051)
    let addr: SocketAddr = LISTEN_ADDR.parse()?;
    state.log(
        LogLevel::Info,
        "grpc",
        format!("Starting gRPC server listening on {}", addr),
    );

    // Step 5: Build and start the Tonic gRPC server to serve telemetry to connecting clients
    Server::builder()
        .add_service(TelemetryServiceServer::new(TelemetryServiceImpl::new(
            state.clone(),
        )))
        .serve(addr)
        .await?;

    Ok(())
}

// -----------------------------------------------------------------------------
// Raspberry Pi 3 Hardware GPIO Implementation
// -----------------------------------------------------------------------------

/// Initializes hardware GPIO pins and spawns background worker threads for SW1 and SW2.
fn spawn_gpio_tasks(state: Arc<SharedState>) -> Result<(), Box<dyn std::error::Error>> {
    state.log(
        LogLevel::Info,
        "gpio",
        "Initializing rppal hardware GPIO on RPi 3...",
    );

    let gpio = rppal::gpio::Gpio::new()?;
    let pin1 = configure_input_pin(&gpio, PIN_SWITCH_1, &state)?;
    let pin2 = configure_input_pin(&gpio, PIN_SWITCH_2, &state)?;

    state.log(
        LogLevel::Info,
        "gpio",
        format!(
            "GPIO ready: SW1 -> GPIO {}, SW2 -> GPIO {}",
            PIN_SWITCH_1, PIN_SWITCH_2
        ),
    );

    // Spawn a dedicated background OS thread to monitor Switch 1 (SW1 / GPIO 23).

    // - `state.clone()` creates a new thread-safe reference (Arc) to the shared state.
    let state1 = state.clone();

    // - `tokio::task::spawn_blocking` offloads the blocking interrupt loop to Tokio's
    //   blocking thread pool so it never freezes the main async event loop.
    // - `move ||` transfers ownership of `pin1` and `state1` into the thread closure.
    tokio::task::spawn_blocking(move || {
        monitor_hardware_switch(pin1, SwitchId::Switch1, PIN_SWITCH_1, state1);
    });

    // Spawn a dedicated background OS thread to monitor Switch 2 (SW2 / GPIO 24)
    let state2 = state.clone();
    tokio::task::spawn_blocking(move || {
        monitor_hardware_switch(pin2, SwitchId::Switch2, PIN_SWITCH_2, state2);
    });

    // Return `Ok(())` containing the unit type `()` to signal that all GPIO initialization
    // and thread spawning completed successfully without errors.
    Ok(())
}

/// Configures a single Raspberry Pi GPIO pin as an input with an internal pull-up resistor enabled.
/// If configuration fails (e.g. invalid permissions or busy pin), an error is logged.
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

/// Dedicated monitoring loop running on a separate OS thread for each switch.
///
/// How it works:
/// 1. Configures hardware interrupts to detect both voltage transitions (High->Low and Low->High).
/// 2. Blocks efficiently using `poll_interrupt`, consuming virtually 0% CPU while waiting for button presses.
/// 3. Whenever state changes:
///    - On Press: Records timestamp and publishes a Pressed event.
///    - On Release: Calculates how many milliseconds the button was held down, and publishes a Released event.
fn monitor_hardware_switch(
    mut pin: rppal::gpio::InputPin,
    switch_id: SwitchId,
    pin_number: u8,
    state: Arc<SharedState>,
) {
    use std::time::Instant;

    // Enable hardware edge triggers for both press (falling edge) and release (rising edge)
    let _ = pin.set_interrupt(rppal::gpio::Trigger::Both, None);

    let mut last_state = read_state(&pin);
    let mut press_start: Option<Instant> = if last_state == SwitchState::Pressed {
        Some(Instant::now())
    } else {
        None
    };

    loop {
        // Sleep efficiently until the hardware detects a voltage change, or timeout after 250ms
        let _ = pin.poll_interrupt(true, Some(Duration::from_millis(250)));
        let current_state = read_state(&pin);

        // If the electrical state hasn't changed, continue waiting
        if current_state == last_state {
            continue;
        }

        last_state = current_state;

        match current_state {
            SwitchState::Pressed => {
                // User pressed the button: start the timer and record the event
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
                // User released the button: compute duration held in milliseconds
                let duration_millis = press_start
                    .take()
                    .map(|start| start.elapsed().as_millis() as u32)
                    .unwrap_or(0);
                state.record_event(
                    switch_id,
                    SwitchState::Released,
                    pin_number,
                    duration_millis,
                );
                state.log(
                    LogLevel::Debug,
                    "gpio",
                    format!(
                        "{:?} (GPIO {}) contact opened [Released] (held for {}ms)",
                        switch_id, pin_number, duration_millis
                    ),
                );
            }
            SwitchState::Unspecified => {}
        }
    }
}

/// Reads the electrical voltage level on a GPIO pin and translates it into a `SwitchState`.
///
/// Hardware Wiring Explanation (Active-LOW with Internal Pull-Up):
/// - When the switch is open (unpressed), the internal pull-up resistor keeps the pin at High (+3.3V) -> `Released`.
/// - When the button is pressed, the contact closes to Ground (0V), pulling the pin Low -> `Pressed`.
fn read_state(pin: &rppal::gpio::InputPin) -> SwitchState {
    if pin.read() == rppal::gpio::Level::Low {
        SwitchState::Pressed
    } else {
        SwitchState::Released
    }
}

/// Periodically logs system diagnostics (uptime and total switch press counts) every 10 seconds.
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
