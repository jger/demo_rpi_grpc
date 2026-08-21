use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// Initializes the hardware GPIO lines and starts the switch monitoring thread.
fn spawn_gpio_tasks(state: Arc<SharedState>) -> Result<(), Box<dyn std::error::Error>> {
    state.log(
        LogLevel::Info,
        "gpio",
        "Claiming GPIO lines via rppal on RPi 3...",
    );

    let gpio = rppal::gpio::Gpio::new().inspect_err(|e| report_gpio_error(&state, e))?;
    let mut pin1 = configure_input_pin(&gpio, PIN_SWITCH_1, &state)?;
    let mut pin2 = configure_input_pin(&gpio, PIN_SWITCH_2, &state)?;

    // The idle level shows immediately whether the pull-ups are doing their job:
    // an unpressed Active-LOW switch must read High.
    report_idle_level(&state, SwitchId::Switch1, &pin1);
    report_idle_level(&state, SwitchId::Switch2, &pin2);

    // Arm both lines for press (falling) and release (rising) edges.
    //
    // No kernel debounce attribute is requested: the board already debounces in
    // hardware with its 10 kΩ / 100 nF RC network, and a debounce attribute the kernel
    // rejects is one more way for the line request to fail. Bounce that still slips
    // through is filtered by `SwitchTracker` below.
    pin1.set_interrupt(rppal::gpio::Trigger::Both, None)
        .inspect_err(|e| report_gpio_error(&state, e))?;
    pin2.set_interrupt(rppal::gpio::Trigger::Both, None)
        .inspect_err(|e| report_gpio_error(&state, e))?;

    state.log(
        LogLevel::Info,
        "gpio",
        format!(
            "GPIO ready: SW1 -> GPIO {}, SW2 -> GPIO {}",
            PIN_SWITCH_1, PIN_SWITCH_2
        ),
    );

    // A single dedicated OS thread watches both lines.
    //
    // - `std::thread::spawn` keeps this permanently blocking loop off Tokio's blocking
    //   pool, so it can never stall the runtime's shutdown.
    // - `move ||` transfers ownership of the pins, the Gpio handle and the shared state
    //   into the thread. The pins must stay alive: dropping an `InputPin` releases the
    //   line and the switch goes silent.
    std::thread::spawn(move || monitor_switches(gpio, pin1, pin2, state));

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
            );
            report_gpio_error(state, e);
        })
}

/// Monitoring loop for BOTH switches, running on one dedicated OS thread.
///
/// How it works:
/// 1. `poll_interrupts` blocks efficiently on both lines at once, consuming virtually 0%
///    CPU while waiting for either button.
/// 2. Whenever an edge arrives, the owning switch's `SwitchTracker` debounces it and
///    turns it into a Pressed/Released event with a hold duration.
///
/// Why one thread and not one per pin: rppal funnels every synchronous poll through a
/// single global lock and a single shared epoll instance. Polling each pin from its own
/// thread makes the two switches block each other, and a poll that resets the event
/// cache discards the edges the other pin has queued up — so each switch only reacts
/// when the other one hands over the lock. `poll_interrupts` is the supported way to
/// wait on several pins, and `reset: false` returns cached events instead of dropping
/// them, so two near-simultaneous presses are both delivered.
fn monitor_switches(
    gpio: rppal::gpio::Gpio,
    pin1: rppal::gpio::InputPin,
    pin2: rppal::gpio::InputPin,
    state: Arc<SharedState>,
) {
    let pins = [&pin1, &pin2];
    let mut switch_1 = SwitchTracker::new(SwitchId::Switch1, PIN_SWITCH_1);
    let mut switch_2 = SwitchTracker::new(SwitchId::Switch2, PIN_SWITCH_2);

    loop {
        match gpio.poll_interrupts(&pins, false, None) {
            Ok(Some((pin, event))) => {
                let tracker = if pin.pin() == PIN_SWITCH_1 {
                    &mut switch_1
                } else {
                    &mut switch_2
                };
                tracker.handle(event, &state);
            }
            // Unreachable with an indefinite timeout, but harmless to loop on.
            Ok(None) => {}
            Err(e) => {
                state.log(
                    LogLevel::Error,
                    "gpio",
                    format!("GPIO polling stopped: {}", e),
                );
                return;
            }
        }
    }
}

/// Software debounce window applied on top of the board's RC filter.
const DEBOUNCE: Duration = Duration::from_millis(20);

/// Per-switch edge bookkeeping: debounce guard, press/release state and hold timing.
struct SwitchTracker {
    switch_id: SwitchId,
    pin_number: u8,
    pressed: bool,
    press_start: Option<Instant>,
    last_edge: Option<Instant>,
}

impl SwitchTracker {
    fn new(switch_id: SwitchId, pin_number: u8) -> Self {
        Self {
            switch_id,
            pin_number,
            pressed: false,
            press_start: None,
            last_edge: None,
        }
    }

    /// Turns one edge into a telemetry event.
    ///
    /// Hardware Wiring Explanation (Active-LOW with Pull-Up):
    /// - When the switch is open (unpressed), the pull-up keeps the line at High (+3.3V).
    /// - When the button is pressed, the contact closes to Ground (0V), pulling it Low.
    ///
    /// So a falling edge is a press and a rising edge is a release.
    fn handle(&mut self, event: rppal::gpio::Event, state: &SharedState) {
        let now = Instant::now();

        // Reject contact chatter, and any repeat of an edge already acted on.
        if self.last_edge.is_some_and(|last| now - last < DEBOUNCE) {
            return;
        }
        let pressed = match event.trigger {
            rppal::gpio::Trigger::FallingEdge => true,
            rppal::gpio::Trigger::RisingEdge => false,
            _ => return,
        };
        if pressed == self.pressed {
            return;
        }

        self.last_edge = Some(now);
        self.pressed = pressed;

        if pressed {
            // User pressed the button: start the timer and record the event
            self.press_start = Some(now);
            state.record_event(self.switch_id, SwitchState::Pressed, self.pin_number, 0);
            state.log(
                LogLevel::Debug,
                "gpio",
                format!(
                    "{:?} (GPIO {}) contact closed [Active LOW Pressed]",
                    self.switch_id, self.pin_number
                ),
            );
        } else {
            // User released the button: compute duration held in milliseconds
            let duration_millis = self
                .press_start
                .take()
                .map_or(0, |start| (now - start).as_millis() as u32);
            state.record_event(
                self.switch_id,
                SwitchState::Released,
                self.pin_number,
                duration_millis,
            );
            state.log(
                LogLevel::Debug,
                "gpio",
                format!(
                    "{:?} (GPIO {}) contact opened [Released] (held for {}ms)",
                    self.switch_id, self.pin_number, duration_millis
                ),
            );
        }
    }
}

/// Logs the resting voltage level of a line, so a wiring fault is obvious at startup.
fn report_idle_level(state: &SharedState, switch_id: SwitchId, pin: &rppal::gpio::InputPin) {
    if pin.read() == rppal::gpio::Level::High {
        state.log(
            LogLevel::Info,
            "gpio",
            format!(
                "{:?} (GPIO {}) idle High - pull-up OK",
                switch_id,
                pin.pin()
            ),
        );
    } else {
        state.log(
            LogLevel::Warn,
            "gpio",
            format!(
                "{:?} (GPIO {}) idle LOW - button held down, or the pull-up to 3.3 V is missing",
                switch_id,
                pin.pin()
            ),
        );
    }
}

/// Explains the most common reasons a GPIO line cannot be claimed.
fn report_gpio_error(state: &SharedState, error: &rppal::gpio::Error) {
    state.log(
        LogLevel::Error,
        "gpio",
        format!("Cannot claim Raspberry Pi GPIO: {}", error),
    );
    state.log(
        LogLevel::Error,
        "gpio",
        "Is check_buttons (or another GPIO program) already running and holding GPIO 23/24? \
         A line can only be claimed by one process. Otherwise check that /dev/gpiochip0 is \
         accessible (run as root, or add the user to the 'gpio' group).",
    );
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
