---
tags: [hardware, electronics, debouncing, capacitors, rc-filter, raspberry-pi]
status: active
---

# 06 Hardware Debouncing & Capacitors

This document explains the physics, electrical engineering principles, and mathematics behind using **100nF capacitors** and **10kΩ pull-up resistors** for hardware debouncing on the Raspberry Pi GPIO inputs.

---

## 1. The Problem: Mechanical Switch Contact Bounce

When you press or release a mechanical push button, the metal contacts do not make or break a clean connection instantly. Instead, at microscopic scales, the contacts physically collide, deform elastically, and **bounce** against each other for **1 to 5 milliseconds** before settling.

```
Ideal Transition:
3.3V ──────┐
           │
  0V       └───────────────────── (Clean Active-LOW transition)

Real Mechanical Bounce (Without Capacitor):
3.3V ──┐ ┌─┐ ┌──┐ ┌┐
       │ │ │ │  │ ││
  0V   └─┘ └─┘  └─┘└───────────── (Dozens of false edge transitions in 2-5ms!)
```

### Why This Causes Problems in Software & Linux:
1. **Interrupt Storms**: A single physical button press can trigger **10 to 50 GPIO edge interrupts** in the Linux kernel (`rppal` / `gpiod`) within a few milliseconds.
2. **CPU Waste**: The CPU spends cycles handling jittery interrupts or running complex debounce timers.
3. **Missed / Double Clicks**: Without filtering, a user pressing a button once may trigger duplicate state machines or unintended counter increments.

---

## 2. How a 100nF Capacitor Solves This (RC Low-Pass Filter)

A capacitor stores electrical charge ($Q = C \cdot V$). In conjunction with the **10kΩ pull-up resistor**, it forms an **RC Low-Pass Filter**:

```
                   +3.3V (RPi Pin 1)
                      |
                     [R] 10kΩ Pull-Up Resistor
                      |
   To RPi GPIO Pin ---+-----------------------+
   (GPIO 23 or 24)    |                       |
                      |                      ===  C (100nF Ceramic Capacitor)
                      o                       |
                    [SW] Push Button (NO)     |
                      o                       |
                      |                       |
                      +-----------------------+
                      |
                   GND (RPi Pin 14 / 20)
```

### What Happens Electrically:

1. **Button Released (Charging Phase / Rising Edge)**:
   * When you release the switch, the capacitor cannot jump to 3.3V instantly. It charges smoothly through the $10\text{k}\Omega$ resistor following the exponential charging curve:
     $$V(t) = V_{\text{max}} \cdot \left(1 - e^{-t / \tau}\right)$$
   * Any micro-bounces during contact separation are absorbed because the capacitor smooths out high-frequency fluctuations.

2. **Button Pressed (Discharge Phase / Falling Edge)**:
   * When you press the switch, the capacitor discharges to Ground (0V).
   * The transition is fast and clean, dropping the voltage below the logic LOW threshold without high-frequency ringing.

---

## 3. Mathematical Analysis & Timing ($\tau$)

### The RC Time Constant ($\tau$)
The fundamental time constant $\tau$ (tau) determines the rate of charging:

$$\tau = R \times C$$

With **$R = 10\text{ k}\Omega\ (10{,}000\ \Omega)$** and **$C = 100\text{ nF}\ (100 \times 10^{-9}\text{ F})$**:

$$\tau = 10{,}000 \times (100 \times 10^{-9}) = 0.001\text{ seconds} = \mathbf{1.0\text{ ms}}$$

### Voltage vs. Time on Release:

| Elapsed Time | Formula | Capacitor Voltage ($V_C$) | Logic Level (Raspberry Pi 3) |
|---|---|---|---|
| **$t = 0\text{ ms}$** | $V(0)$ | **$0.0\text{ V}$** | LOW ($< 0.8\text{V}$) |
| **$t = 1\tau\ (1.0\text{ ms})$** | $3.3\text{V} \times 63.2\%$ | **$2.08\text{ V}$** | Crossing HIGH threshold ($V_{IH} \approx 2.0\text{V}$) |
| **$t = 2\tau\ (2.0\text{ ms})$** | $3.3\text{V} \times 86.5\%$ | **$2.85\text{ V}$** | Solid HIGH |
| **$t = 3\tau\ (3.0\text{ ms})$** | $3.3\text{V} \times 95.0\%$ | **$3.14\text{ V}$** | Stable HIGH (Full 3.3V) |

```
Voltage (V)
3.3V |                           . - - - - (Fully Charged)
     |                     . '
2.0V | - - - - - - - . ' <--- RPi High Threshold (at t = 1.0 ms)
     |           . '
     |       . '
0.0V |____.'____________________________
     0    1.0 ms    2.0 ms    3.0 ms
```

### Cutoff Frequency ($f_c$)
The RC filter behaves as a first-order low-pass filter with a cutoff frequency:

$$f_c = \frac{1}{2 \pi R C} = \frac{1}{2 \pi \times 10{,}000 \times 100 \times 10^{-9}} \approx \mathbf{159.15\text{ Hz}}$$

* **Bounce noise** typically occurs at frequencies between **1 kHz and 100 kHz** $\rightarrow$ **Blocked by the filter**.
* **Human button presses** occur at **1 Hz to 10 Hz** $\rightarrow$ **Passes cleanly through the filter**.

---

## 4. Why 100nF is the Sweet Spot

Selecting capacitor values requires balancing noise suppression against tactile latency and contact wear:

| Capacitor Value | Time Constant ($\tau$) | Charge Time ($3\tau$) | Behavior / Assessment |
|---|---|---|---|
| **100 pF** (Too Small) | $0.001\text{ ms}$ | $0.003\text{ ms}$ | ❌ Too fast — fails to filter out 1–5 ms mechanical contact bounce. |
| **10 nF** (Small) | $0.1\text{ ms}$ | $0.3\text{ ms}$ | ⚠️ Filters light bounce, but long bounces may still slip through. |
| **100 nF** (**Gold Standard**) | **$1.0\text{ ms}$** | **$3.0\text{ ms}$** | ✅ **Ideal balance**: Absorbs 100% of contact bounce with **zero perceived latency** (human reaction threshold is 20–50 ms). |
| **10 µF** (Too Large) | $100\text{ ms}$ | $300\text{ ms}$ | ❌ **Severe input lag**: The button feels unresponsive and mushy (300 ms delay). |

---

## 5. Contact Safety & Energy Stored

When pressing the button, the capacitor discharges stored electrostatic energy ($E$):

$$E = \frac{1}{2} C V^2 = \frac{1}{2} \times (100 \times 10^{-9}\text{ F}) \times (3.3\text{ V})^2 \approx \mathbf{0.54\ \mu\text{J}}\ (0.00054\text{ mJ})$$

* **0.54 microjoules** is so small that it produces no switch contact arcing, heating, or degradation over millions of cycles.

---

## 6. Comparison: Hardware vs. Software Debouncing

| Metric | Software-Only Debounce | Hardware 100nF RC Filter | Combined (Best Practice) |
|---|---|---|---|
| **Kernel Interrupt Count** | High (10–50 interrupts per press) | Clean (1 interrupt per press) | Clean (1 interrupt per press) |
| **CPU Utilization** | High during rapid clicking | Minimal | Minimal |
| **Latency** | 15–50 ms software delay | ~2–3 ms analog rise | Instant response |
| **Electromagnetic Noise Immunity** | Vulnerable to stray RF antenna pickup | **High (Capacitor shunts RF noise to GND)** | **Maximum robustness** |

---

## 7. Identification & Breadboard Tips

* **Capacitor Label**: Look for **`104`** stamped on small ceramic discs or yellow multi-layer ceramic capacitors (MLCC):
  $$\text{Code } 104 = 10 \times 10^4\text{ pF} = 100{,}000\text{ pF} = \mathbf{100\text{ nF}} = \mathbf{0.1\ \mu\text{F}}$$
* **Polarity**: Ceramic capacitors are **non-polarized** — they can be plugged into the breadboard in either direction without risk.
