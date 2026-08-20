#!/usr/bin/env python3
"""
Hardware Button Verification Script for Raspberry Pi 3
-------------------------------------------------------
Checks the 2 momentary push switches wired to GPIO 23 (SW1) and GPIO 24 (SW2)
with pull-up resistors (Active-LOW logic).

Usage:
  On Raspberry Pi:
    python3 check_buttons.py

  Local simulation test (macOS / Linux / Windows):
    python3 check_buttons.py --sim
"""

import sys
import time
import argparse
from datetime import datetime

# ANSI Color formatting
GREEN = "\033[92m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
MAGENTA = "\033[95m"
RED = "\033[91m"
BOLD = "\033[1m"
DIM = "\033[2m"
RESET = "\033[0m"

PIN_SW1 = 23  # GPIO 23 (Pin 16)
PIN_SW2 = 24  # GPIO 24 (Pin 18)


class ButtonChecker:
    def __init__(self, mode="auto"):
        self.mode = mode
        self.sw1_presses = 0
        self.sw2_presses = 0
        self.sw1_press_time = None
        self.sw2_press_time = None
        self.start_time = time.time()

    def get_timestamp(self):
        return datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]

    def on_press(self, sw_id, pin):
        now = time.time()
        ts = self.get_timestamp()
        if sw_id == 1:
            self.sw1_presses += 1
            self.sw1_press_time = now
            name = f"{MAGENTA}{BOLD}SW1 (GPIO {pin}){RESET}"
        else:
            self.sw2_presses += 1
            self.sw2_press_time = now
            name = f"{CYAN}{BOLD}SW2 (GPIO {pin}){RESET}"

        count = self.sw1_presses if sw_id == 1 else self.sw2_presses
        print(f"{DIM}[{ts}]{RESET} {name} {GREEN}{BOLD}▼ PRESSED {RESET} (Total: {count})")

    def on_release(self, sw_id, pin):
        now = time.time()
        ts = self.get_timestamp()
        if sw_id == 1:
            start = self.sw1_press_time
            name = f"{MAGENTA}{BOLD}SW1 (GPIO {pin}){RESET}"
        else:
            start = self.sw2_press_time
            name = f"{CYAN}{BOLD}SW2 (GPIO {pin}){RESET}"

        duration_str = ""
        if start:
            duration_ms = int((now - start) * 1000)
            duration_str = f" {CYAN}(held for {duration_ms} ms){RESET}"

        print(f"{DIM}[{ts}]{RESET} {name} {YELLOW}{BOLD}▲ RELEASED{RESET}{duration_str}")

    def run_gpiozero(self):
        """Hardware mode using modern gpiozero library."""
        from gpiozero import Button

        print(f"{CYAN}Initializing GPIO with gpiozero (pull_up=True, bounce_time=0.02s)...{RESET}")
        btn1 = Button(PIN_SW1, pull_up=True, bounce_time=0.02)
        btn2 = Button(PIN_SW2, pull_up=True, bounce_time=0.02)

        btn1.when_pressed = lambda: self.on_press(1, PIN_SW1)
        btn1.when_released = lambda: self.on_release(1, PIN_SW1)

        btn2.when_pressed = lambda: self.on_press(2, PIN_SW2)
        btn2.when_released = lambda: self.on_release(2, PIN_SW2)

        print(f"{GREEN}✓ Ready! Press the physical buttons on the breadboard.{RESET}\n")
        while True:
            time.sleep(1)

    def run_rpi_gpio(self):
        """Hardware mode fallback using RPi.GPIO library."""
        import RPi.GPIO as GPIO

        print(f"{CYAN}Initializing GPIO with RPi.GPIO (PUD_UP, bouncetime=20ms)...{RESET}")
        GPIO.setmode(GPIO.BCM)
        GPIO.setup(PIN_SW1, GPIO.IN, pull_up_down=GPIO.PUD_UP)
        GPIO.setup(PIN_SW2, GPIO.IN, pull_up_down=GPIO.PUD_UP)

        def cb_sw1(channel):
            if GPIO.input(channel) == GPIO.LOW:
                self.on_press(1, PIN_SW1)
            else:
                self.on_release(1, PIN_SW1)

        def cb_sw2(channel):
            if GPIO.input(channel) == GPIO.LOW:
                self.on_press(2, PIN_SW2)
            else:
                self.on_release(2, PIN_SW2)

        GPIO.add_event_detect(PIN_SW1, GPIO.BOTH, callback=cb_sw1, bouncetime=20)
        GPIO.add_event_detect(PIN_SW2, GPIO.BOTH, callback=cb_sw2, bouncetime=20)

        print(f"{GREEN}✓ Ready! Press the physical buttons on the breadboard.{RESET}\n")
        try:
            while True:
                time.sleep(1)
        finally:
            GPIO.cleanup()

    def run_simulated(self):
        """Local simulated mode when running on macOS/Windows/PC."""
        print(f"{YELLOW}[SIMULATION MODE]{RESET} Running simulated button pulses...")
        print(f"{DIM}Simulating button events every 3 seconds. Press Ctrl+C to stop.{RESET}\n")

        toggle = False
        while True:
            time.sleep(2.5)
            toggle = not toggle
            sw_id = 1 if toggle else 2
            pin = PIN_SW1 if sw_id == 1 else PIN_SW2

            self.on_press(sw_id, pin)
            time.sleep(0.3)
            self.on_release(sw_id, pin)

    def print_summary(self):
        uptime = int(time.time() - self.start_time)
        print("\n" + "=" * 55)
        print(f"{BOLD}Session Summary:{RESET}")
        print(f"  • Total Uptime : {uptime}s")
        print(f"  • SW1 Presses  : {self.sw1_presses}")
        print(f"  • SW2 Presses  : {self.sw2_presses}")
        print("=" * 55)


def main():
    parser = argparse.ArgumentParser(description="Check Raspberry Pi 3 push buttons (GPIO 23 & 24)")
    parser.add_argument("--sim", action="store_true", help="Force simulation mode (useful for testing on PC/Mac)")
    args = parser.parse_args()

    print("=" * 55)
    print(f"{BOLD}{CYAN}  Raspberry Pi 3 — Push Button Verification Utility{RESET}")
    print(f"{DIM}  SW1 -> GPIO 23 (Pin 16) | SW2 -> GPIO 24 (Pin 18) [Pull-Up]{RESET}")
    print("=" * 55)

    checker = ButtonChecker()

    try:
        if args.sim:
            checker.run_simulated()
        else:
            # Try gpiozero first, then RPi.GPIO, then fallback to sim
            try:
                checker.run_gpiozero()
            except ImportError:
                try:
                    checker.run_rpi_gpio()
                except (ImportError, RuntimeError) as e:
                    print(f"{YELLOW}GPIO libraries unavailable on this system ({e}).{RESET}")
                    checker.run_simulated()
    except KeyboardInterrupt:
        print(f"\n{YELLOW}Stopping button checker...{RESET}")
    finally:
        checker.print_summary()


if __name__ == "__main__":
    main()
