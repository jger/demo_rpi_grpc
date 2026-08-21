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

## 2. Push Switch Circuit: 10kΩ Pull-Up & 100nF Hardware Debounce

### Why Pull-Up Resistors & Capacitors?
* **Pull-Up Resistor (10kΩ)**: Connects the GPIO pin to **3.3V (VCC)** so the resting state is reliably **HIGH (1)** without floating in an undefined electrical state. Pressing the button pulls the voltage to **LOW (0)** (Active-Low logic).
* **Hardware Debounce Capacitor (100nF / 0.1µF)**: Placed across the switch to GND to form an **RC Low-Pass Filter** ($\tau = R \times C = 10\text{k}\Omega \times 100\text{nF} = 1.0\text{ ms}$). This smoothly eliminates contact bounce without CPU overhead.

```
                   +3.3V (Pin 1 or 17)
                      |
                     [R]  10kΩ Pull-Up Resistor
                      |
GPIO Pin (23 or 24) --+---------------------+
                      |                     |
                      |                    ===  C (100nF / 0.1µF Ceramic)
                      o                     |
                    [SW] Push Button (NO)   |
                      o                     |
                      |                     |
                      +---------------------+
                      |
            GND (Pin 6, 14, or 20)
```

---

## 3. Physical Wiring Table

| Component | Designator | From / Terminal | Raspberry Pi 3 Pin | Header Pin # | Description |
|---|---|---|---|---|---|
| **Switch 1 (SW1)** | SW1 | Terminal A <br> Terminal B | **GPIO 23** <br> **GND** | Pin 16 <br> Pin 14 | Switch 1 Input (Active LOW) <br> Ground return |
| **Pull-Up Resistor 1** | R1 (10kΩ) | Between 3.3V & GPIO 23 | **3.3V & GPIO 23** | Pin 1 & Pin 16 | 10kΩ Pull-Up to 3.3V |
| **Debounce Capacitor 1** | C1 (100nF) | Between GPIO 23 & GND | **GPIO 23 & GND** | Pin 16 & Pin 14 | 100nF (0.1µF) Ceramic Filter |
| **Switch 2 (SW2)** | SW2 | Terminal A <br> Terminal B | **GPIO 24** <br> **GND** | Pin 18 <br> Pin 20 | Switch 2 Input (Active LOW) <br> Ground return |
| **Pull-Up Resistor 2** | R2 (10kΩ) | Between 3.3V & GPIO 24 | **3.3V & GPIO 24** | Pin 1 & Pin 18 | 10kΩ Pull-Up to 3.3V |
| **Debounce Capacitor 2** | C2 (100nF) | Between GPIO 24 & GND | **GPIO 24 & GND** | Pin 18 & Pin 20 | 100nF (0.1µF) Ceramic Filter |

---

## 4. Complete Dual-Button Hardware Schematic

```
          +3.3V (Pin 1)
            ├─────────────────────────┐
            │                         │
           [R1] 10kΩ                 [R2] 10kΩ
            │                         │
 GPIO 23 ───┼──────────┐    GPIO 24 ───┼──────────┐
 (Pin 16)   │          │    (Pin 18)   │          │
            o         === C1           o         === C2
          [SW1]       │ (100nF)      [SW2]       │ (100nF)
            o          │               o          │
            │          │               │          │
 GND ───────┴──────────┴───────────────┴──────────┴──── GND (Pin 14 / 20)
```

---

## 5. Contact Bouncing & Filtering Theory

Mechanical contacts inside push switches bounce for 2–5 ms when pressed or released.

### Dual-Layer Debounce Strategy:
1. **Hardware (100nF Capacitor + 10kΩ Resistor)**:
   * Provides an analog low-pass cutoff at $f_c \approx 159\text{ Hz}$.
   * $\tau_{\text{charge}} = 10\text{k}\Omega \times 100\text{nF} = 1.0\text{ ms}$, smoothing the release transition into a clean, monotonic slope.
   * Eliminates Linux kernel interrupt storms.
   * See detailed analysis in [[06 Hardware Debouncing & Capacitors]].
2. **Software (In Rust Node & Python script)**:
   * Additional lightweight edge validation ensures rock-solid state transitions.

---

## 6. Breadboard Assembly Step-by-Step

1. **Power Rails**:
   * Connect **RPi Pin 1 (3.3V)** to the breadboard **(+) rail**.
   * Connect **RPi Pin 14 (GND)** to the breadboard **(-) rail**.
2. **Switch 1 (SW1 / GPIO 23)**:
   * Insert **Switch 1** across the breadboard center trough.
   * Connect one side of Switch 1 to the **(-) GND rail**.
   * On the other side of Switch 1 (shared terminal row):
     * Insert one leg of **$R_1$ (10kΩ)** (other leg to **(+) 3.3V rail**).
     * Insert one leg of **$C_1$ (100nF)** (other leg to **(-) GND rail**).
     * Connect a jumper wire to **RPi Pin 16 (GPIO 23)**.
3. **Switch 2 (SW2 / GPIO 24)**:
   * Insert **Switch 2** across the breadboard center trough.
   * Connect one side of Switch 2 to the **(-) GND rail**.
   * On the other side of Switch 2 (shared terminal row):
     * Insert one leg of **$R_2$ (10kΩ)** (other leg to **(+) 3.3V rail**).
     * Insert one leg of **$C_2$ (100nF)** (other leg to **(-) GND rail**).
     * Connect a jumper wire to **RPi Pin 18 (GPIO 24)**.
4. **Verification**:
   * Run `python3 check_buttons.py` on the Raspberry Pi to verify instant, bounce-free button detections.
