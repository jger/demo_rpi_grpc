//! Hardware Button Verification Utility for Raspberry Pi 3 (Rust port of `check_buttons.py`)
//! -------------------------------------------------------------------------------------
//! Checks the 2 momentary push switches wired to GPIO 23 (SW1) and GPIO 24 (SW2)
//! with internal pull-up resistors enabled (Active-LOW logic).
//!
//! Raspberry Pi / Linux only — there is no simulation mode. On any other platform,
//! or when the GPIO peripheral cannot be opened, it exits with an explanatory message.
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
    use rppal::gpio::{Gpio, InputPin, Level, Trigger};

    /// GPIO 23 (physical header pin 16)
    const PIN_SW1: u8 = 23;
    /// GPIO 24 (physical header pin 18)
    const PIN_SW2: u8 = 24;
    /// Ignore contact bounce for this long after each detected edge
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

        // No fallback: if the hardware is not there, say so and stop.
        if let Err(e) = spawn_gpio_monitors(stats.clone()) {
            eprintln!(
                "{}",
                format!("Cannot access Raspberry Pi GPIO: {}", e).bright_red()
            );
            eprintln!(
                "{}",
                "Check that this is a Raspberry Pi and that /dev/gpiochip0 is accessible \
                 (run as root, or add the user to the 'gpio' group)."
                    .dimmed()
            );
            std::process::exit(1);
        }

        // Block until Ctrl+C, then report the session totals.
        let _ = tokio::signal::ctrl_c().await;
        println!("\n{}", "Stopping button checker...".bright_yellow());
        stats.print_summary(start);
    }

    // -------------------------------------------------------------------------
    // GPIO monitoring
    // -------------------------------------------------------------------------

    /// Configures both pins as pull-up inputs and spawns one blocking monitor thread each.
    fn spawn_gpio_monitors(stats: Arc<Stats>) -> Result<(), rppal::gpio::Error> {
        println!(
            "{}",
            "Initializing GPIO with rppal (pull-up, 20 ms debounce)...".bright_cyan()
        );
        let gpio = Gpio::new()?;
        let pin1 = gpio.get(PIN_SW1)?.into_input_pullup();
        let pin2 = gpio.get(PIN_SW2)?.into_input_pullup();

        let stats1 = stats.clone();
        tokio::task::spawn_blocking(move || monitor_switch(pin1, 1, PIN_SW1, stats1));
        tokio::task::spawn_blocking(move || monitor_switch(pin2, 2, PIN_SW2, stats));

        println!(
            "{}\n",
            "✓ Ready! Press the physical buttons on the breadboard.".bright_green()
        );
        Ok(())
    }

    /// Blocks on hardware edge interrupts (≈0% CPU) and prints every press/release.
    ///
    /// Wiring is Active-LOW: the internal pull-up holds the pin HIGH while the switch
    /// is open, and pressing it closes the contact to ground, pulling the pin LOW.
    fn monitor_switch(mut pin: InputPin, sw_id: u8, pin_number: u8, stats: Arc<Stats>) {
        let is_pressed = |pin: &InputPin| pin.read() == Level::Low;

        if let Err(e) = pin.set_interrupt(Trigger::Both, Some(DEBOUNCE)) {
            eprintln!(
                "{}",
                format!("GPIO {} interrupt error: {}", pin_number, e).bright_red()
            );
            return;
        }

        let mut pressed = is_pressed(&pin);
        let mut press_start = pressed.then(Instant::now);

        loop {
            // Wake up on the next voltage change; the timeout keeps the loop responsive
            // to state changes that were missed while an edge was being debounced.
            if pin
                .poll_interrupt(true, Some(Duration::from_millis(250)))
                .is_err()
            {
                return;
            }

            let now_pressed = is_pressed(&pin);
            if now_pressed == pressed {
                continue;
            }
            pressed = now_pressed;

            if pressed {
                press_start = Some(Instant::now());
                stats.on_press(sw_id, pin_number);
            } else {
                let held = press_start.take().map_or(Duration::ZERO, |t| t.elapsed());
                stats.on_release(sw_id, pin_number, held);
            }
        }
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
