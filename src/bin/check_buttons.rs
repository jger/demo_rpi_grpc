//! Hardware Button Verification Utility for Raspberry Pi 3 (Rust port of `check_buttons.py`)
//! -------------------------------------------------------------------------------------
//! Checks the 2 momentary push switches wired to GPIO 23 (SW1) and GPIO 24 (SW2),
//! each with a 10 kΩ pull-up to 3.3 V and a 100 nF debounce cap (Active-LOW logic).
//!
//! Raspberry Pi / Linux only — there is no simulation mode. On any other platform,
//! or when the GPIO lines cannot be claimed, it exits with an explanatory message.
//!
//! Usage (on the Raspberry Pi):
//!   ./check_buttons

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("check_buttons requires Raspberry Pi GPIO hardware and only runs on Linux.");
    eprintln!("Build and run it on the Pi instead: make deploy-rpi && ssh <pi> ./check_buttons");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn main() {
    pi::main();
}

#[cfg(target_os = "linux")]
mod pi {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use chrono::Local;
    use colored::Colorize;
    use rppal::gpio::{Event, Gpio, InputPin, Level, Trigger};

    /// GPIO 23 (physical header pin 16)
    const PIN_SW1: u8 = 23;
    /// GPIO 24 (physical header pin 18)
    const PIN_SW2: u8 = 24;
    /// Software debounce guard. The board already filters bounce with its 10 kΩ/100 nF
    /// RC network (τ ≈ 1 ms); this only rejects anything that slips through.
    const DEBOUNCE: Duration = Duration::from_millis(20);

    // -------------------------------------------------------------------------
    // Entry point
    // -------------------------------------------------------------------------

    #[tokio::main]
    pub async fn main() {
        println!("{}", "=".repeat(55));
        println!(
            "{}",
            "  Raspberry Pi 3 — Push Button Verification Utility"
                .bright_cyan()
                .bold()
        );
        println!(
            "{}",
            "  SW1 -> GPIO 23 (Pin 16) | SW2 -> GPIO 24 (Pin 18) [Pull-Up]".dimmed()
        );
        println!("{}", "=".repeat(55));

        let stats = Arc::new(Stats::default());
        let start = Instant::now();

        if let Err(e) = start_watching(stats.clone()) {
            report_gpio_error(&e);
            std::process::exit(1);
        }

        // Block until Ctrl+C, then report the session totals and leave immediately:
        // the poll loop is parked in a blocking syscall and would stall a clean shutdown.
        let _ = tokio::signal::ctrl_c().await;
        println!("\n{}", "Stopping button checker...".bright_yellow());
        stats.print_summary(start);
        std::process::exit(0);
    }

    // -------------------------------------------------------------------------
    // GPIO monitoring
    // -------------------------------------------------------------------------

    /// Claims both lines, arms them for both-edge interrupts, and starts the poll loop.
    ///
    /// Every fallible step happens here, on the calling thread, so a failure is reported
    /// instead of leaving a silent process. (`set_async_interrupt` cannot do this: it
    /// requests the line on its own thread and returns `Ok(())` before that can fail.)
    fn start_watching(stats: Arc<Stats>) -> Result<(), rppal::gpio::Error> {
        println!("{}", "Claiming GPIO lines via rppal...".bright_cyan());

        let gpio = Gpio::new()?;
        let mut pin1 = gpio.get(PIN_SW1)?.into_input_pullup();
        let mut pin2 = gpio.get(PIN_SW2)?.into_input_pullup();

        // Idle level tells you instantly whether the pull-ups are doing their job:
        // an unpressed Active-LOW button must read High.
        report_idle_level(1, &pin1);
        report_idle_level(2, &pin2);

        // No kernel debounce attribute: the board debounces in hardware, and a rejected
        // debounce attribute is one more way for the line request to fail.
        pin1.set_interrupt(Trigger::Both, None)?;
        pin2.set_interrupt(Trigger::Both, None)?;

        println!(
            "{}\n",
            "✓ Ready! Press the physical buttons on the breadboard.".bright_green()
        );

        std::thread::spawn(move || poll_loop(gpio, pin1, pin2, stats));
        Ok(())
    }

    /// Watches both switches from a single thread.
    ///
    /// `poll_interrupts` is the supported way to wait on several pins: rppal funnels all
    /// synchronous polling through one global lock and one shared epoll, so polling each
    /// pin from its own thread makes the two switches block and cannibalise each other's
    /// events. `reset: false` returns events cached during a previous wait rather than
    /// discarding them, so simultaneous presses are both delivered.
    fn poll_loop(gpio: Gpio, pin1: InputPin, pin2: InputPin, stats: Arc<Stats>) {
        let pins = [&pin1, &pin2];
        let mut sw1 = SwitchTracker::new(1, PIN_SW1);
        let mut sw2 = SwitchTracker::new(2, PIN_SW2);

        loop {
            match gpio.poll_interrupts(&pins, false, None) {
                Ok(Some((pin, event))) => {
                    let tracker = if pin.pin() == PIN_SW1 {
                        &mut sw1
                    } else {
                        &mut sw2
                    };
                    tracker.handle(event, &stats);
                }
                Ok(None) => {} // timeout; cannot happen with an indefinite wait
                Err(e) => {
                    eprintln!("{}", format!("GPIO polling stopped: {}", e).bright_red());
                    return;
                }
            }
        }
    }

    /// Per-switch edge bookkeeping: debounce guard, press/release state, hold timing.
    struct SwitchTracker {
        sw_id: u8,
        pin_number: u8,
        pressed: bool,
        press_start: Option<Instant>,
        last_edge: Option<Instant>,
    }

    impl SwitchTracker {
        fn new(sw_id: u8, pin_number: u8) -> Self {
            Self {
                sw_id,
                pin_number,
                pressed: false,
                press_start: None,
                last_edge: None,
            }
        }

        /// Active-LOW wiring: a falling edge is a press, a rising edge is a release.
        fn handle(&mut self, event: Event, stats: &Stats) {
            let now = Instant::now();

            // Reject contact chatter, and any repeat of an edge we already acted on.
            if self.last_edge.is_some_and(|t| now - t < DEBOUNCE) {
                return;
            }
            let pressed = match event.trigger {
                Trigger::FallingEdge => true,
                Trigger::RisingEdge => false,
                _ => return,
            };
            if pressed == self.pressed {
                return;
            }

            self.last_edge = Some(now);
            self.pressed = pressed;

            if pressed {
                self.press_start = Some(now);
                stats.on_press(self.sw_id, self.pin_number);
            } else {
                let held = self.press_start.take().map_or(Duration::ZERO, |t| now - t);
                stats.on_release(self.sw_id, self.pin_number, held);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Diagnostics
    // -------------------------------------------------------------------------

    fn report_idle_level(sw_id: u8, pin: &InputPin) {
        let level = pin.read();
        let note = if level == Level::High {
            "idle High — pull-up OK".bright_green()
        } else {
            "idle LOW — button held down, or the pull-up to 3.3 V is missing".bright_red()
        };
        println!("  SW{} (GPIO {}): {}", sw_id, pin.pin(), note);
    }

    fn report_gpio_error(e: &rppal::gpio::Error) {
        eprintln!("{}", format!("Cannot claim Raspberry Pi GPIO: {}", e).bright_red());
        eprintln!(
            "{}",
            "  • Is pi_node (or another GPIO program) already running and holding GPIO 23/24? \
             Stop it first — a line can only be claimed by one process."
                .dimmed()
        );
        eprintln!(
            "{}",
            "  • Otherwise check that /dev/gpiochip0 is accessible (run as root, or add the \
             user to the 'gpio' group)."
                .dimmed()
        );
    }

    // -------------------------------------------------------------------------
    // Shared press counters & output formatting
    // -------------------------------------------------------------------------

    #[derive(Default)]
    struct Stats {
        sw1_presses: AtomicU32,
        sw2_presses: AtomicU32,
    }

    impl Stats {
        fn on_press(&self, sw_id: u8, pin: u8) {
            let counter = if sw_id == 1 {
                &self.sw1_presses
            } else {
                &self.sw2_presses
            };
            let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
            println!(
                "{} {} {} (Total: {})",
                timestamp().dimmed(),
                switch_label(sw_id, pin),
                "▼ PRESSED ".bright_green().bold(),
                count
            );
        }

        fn on_release(&self, sw_id: u8, pin: u8, held: Duration) {
            println!(
                "{} {} {} {}",
                timestamp().dimmed(),
                switch_label(sw_id, pin),
                "▲ RELEASED".bright_yellow().bold(),
                format!("(held for {} ms)", held.as_millis()).bright_cyan()
            );
        }

        fn print_summary(&self, start: Instant) {
            println!("\n{}", "=".repeat(55));
            println!("{}", "Session Summary:".bold());
            println!("  • Total Uptime : {}s", start.elapsed().as_secs());
            println!(
                "  • SW1 Presses  : {}",
                self.sw1_presses.load(Ordering::Relaxed)
            );
            println!(
                "  • SW2 Presses  : {}",
                self.sw2_presses.load(Ordering::Relaxed)
            );
            println!("{}", "=".repeat(55));
        }
    }

    fn timestamp() -> String {
        format!("[{}]", Local::now().format("%Y-%m-%d %H:%M:%S%.3f"))
    }

    fn switch_label(sw_id: u8, pin: u8) -> String {
        let text = format!("SW{} (GPIO {})", sw_id, pin);
        if sw_id == 1 {
            text.bright_magenta().bold().to_string()
        } else {
            text.bright_cyan().bold().to_string()
        }
    }
}
