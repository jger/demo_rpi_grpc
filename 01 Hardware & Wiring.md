---
tags: [hardware, wiring, gpio, raspberry-pi, schematic]
status: active
---

# 01 Hardware & Wiring

This document specifies the physical electrical connections between the **Raspberry Pi 3** and the **2 input push switches**, explaining pull-up resistor theory, pin mapping, and debouncing.

---

## 1. Raspberry Pi 3 GPIO Header Pinout

The Raspberry Pi 3 features a 40-pin header. GPIO lines operate at **3.3V logic level** (NOT 5V tolerant).

```
          3.3V Power [ 1] [ 2] 5V Power
   GPIO 2 (SDA/I2C1) [ 3] [ 4] 5V Power
   GPIO 3 (SCL/I2C1) [ 5] [ 6] Ground (GND)
             GPIO 4  [ 7] [ 8] GPIO 14 (TXD)
             Ground  [ 9] [10] GPIO 15 (RXD)
            GPIO 17  [11] [12] GPIO 18 (PCM_CLK)
            GPIO 27  [13] [14] Ground (GND)
            GPIO 22  [15] [16] GPIO 23 <--- SWITCH 1 (IN)
          3.3V Power [17] [18] GPIO 24 <--- SWITCH 2 (IN)
   GPIO 10 (MOSI/SPI)[19] [20] Ground (GND) <--- SWITCH COMMON GND
                 ... [..] [..] ...
```

---

## 2. Push Switch Circuit & Pull-Up Resistors

### Why Pull-Up Resistors?
When a momentary switch is open (unpressed), an unconnected GPIO pin floats in an undefined electrical state, picking up electromagnetic noise and causing phantom trigger events. 

A **pull-up resistor** connects the pin to **3.3V (VCC)** so the resting state is reliably **HIGH (1)**. When the button is pressed, it shorts the pin to **GND**, pulling the voltage to **LOW (0)** (Active-Low logic).

```
         +3.3V (Pin 1 or 17)
           |
          [R]  10kΩ Pull-Up Resistor (Optional if using internal pull-up)
           |
GPIO Pin --+--------o  Switch (Normally Open)
                    |
                   [ ]  Press connects to GND
                    |
GND (Pin 6, 14, or 20)
```

> [!tip] Internal vs. External Pull-Ups
> - **Internal Pull-Ups**: The BCM2837 SoC has internal software-configurable ~50kΩ pull-up resistors. Our Rust `rppal` driver enables `.set_pull(Pull::Up)` on initialization.
> - **External Pull-Ups (10kΩ)**: Recommended for industrial or noisy environments for stronger noise immunity and lower impedance.

---

## 3. Physical Wiring Table

| Component | Pin on Switch | Raspberry Pi 3 Pin | Header Pin # | Description |
|---|---|---|---|---|
| **Switch 1 (SW1)** | Terminal A | **GPIO 23** | Pin 16 | Switch 1 Input (Active LOW) |
| **Switch 1 (SW1)** | Terminal B | **GND** | Pin 14 or 20 | Ground return |
| *(Optional Ext R1)* | Across VCC & Pin 16 | **3.3V (Pin 1) & GPIO 23** | Pin 1 & 16 | 10kΩ Pull-up resistor |
| **Switch 2 (SW2)** | Terminal A | **GPIO 24** | Pin 18 | Switch 2 Input (Active LOW) |
| **Switch 2 (SW2)** | Terminal B | **GND** | Pin 14 or 20 | Ground return |
| *(Optional Ext R2)* | Across VCC & Pin 18 | **3.3V (Pin 1) & GPIO 24** | Pin 1 & 18 | 10kΩ Pull-up resistor |

---

## 4. Switch Contact Bouncing & Software Debouncing

Mechanical contacts inside push switches bounce for 2–10 milliseconds when pressed or released, generating dozens of rapid HIGH/LOW transitions.

### Debounce Strategy:
1. **Hardware (Optional)**: 100nF ceramic capacitor in parallel with switch to GND (RC low-pass filter).
2. **Software (Used in Rust Node)**: 
   - When an interrupt / level transition is detected, ignore subsequent state changes for **15–20 ms**.
   - Poll / edge detection timestamp comparison ensures single, clean `Pressed` and `Released` events.

---

## 5. Breadboard Assembly Step-by-Step

1. Insert **Switch 1** across the breadboard center trough.
2. Connect one leg of Switch 1 to **RPi Pin 16 (GPIO 23)** using a jumper wire.
3. Connect the other leg of Switch 1 to the breadboard **GND rail** (which goes to **RPi Pin 14 GND**).
4. Insert **Switch 2** on the breadboard.
5. Connect one leg of Switch 2 to **RPi Pin 18 (GPIO 24)**.
6. Connect the other leg of Switch 2 to the breadboard **GND rail**.
7. *(Optional)* Place 10kΩ resistors from **RPi Pin 1 (3.3V)** to GPIO 23 and GPIO 24.
