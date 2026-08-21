use std::time::{Duration, Instant};
use crate::proto::SwitchState;

/// Configuration parameters for switch debouncing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebounceConfig {
    /// Time delay between consecutive samples during stabilization verification
    pub sample_interval: Duration,
    /// Number of consecutive identical samples required to confirm a state change
    pub required_stable_samples: usize,
}

impl Default for DebounceConfig {
    fn default() -> Self {
        Self {
            sample_interval: Duration::from_millis(5),
            required_stable_samples: 5, // 5 * 5ms = 25ms stable window
        }
    }
}

/// Output action resulting from a confirmed state change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceOutput {
    /// No state transition confirmed
    NoChange,
    /// Transitioned to Pressed (Active LOW)
    Pressed,
    /// Transitioned to Released with total held duration in milliseconds
    Released { duration_millis: u32 },
}

/// State machine for debouncing hardware switches with hold-duration tracking
#[derive(Debug)]
pub struct DebounceFilter {
    pub confirmed_state: SwitchState,
    pub press_start_time: Option<Instant>,
    pub config: DebounceConfig,
}

impl DebounceFilter {
    pub fn new(initial_state: SwitchState, config: DebounceConfig) -> Self {
        let press_start_time = if initial_state == SwitchState::Pressed {
            Some(Instant::now())
        } else {
            None
        };

        Self {
            confirmed_state: initial_state,
            press_start_time,
            config,
        }
    }

    /// Confirm a verified state transition and produce the corresponding DebounceOutput
    pub fn confirm_transition(&mut self, new_state: SwitchState, now: Instant) -> DebounceOutput {
        if new_state == self.confirmed_state || new_state == SwitchState::Unspecified {
            return DebounceOutput::NoChange;
        }

        self.confirmed_state = new_state;

        match new_state {
            SwitchState::Pressed => {
                self.press_start_time = Some(now);
                DebounceOutput::Pressed
            }
            SwitchState::Released => {
                let duration = self
                    .press_start_time
                    .take()
                    .map(|start| now.saturating_duration_since(start).as_millis() as u32)
                    .unwrap_or(0);
                DebounceOutput::Released {
                    duration_millis: duration,
                }
            }
            _ => DebounceOutput::NoChange,
        }
    }
}

/// Evaluates a sequence of samples to determine if a candidate state has stabilized
pub fn verify_sample_stability(
    samples: impl IntoIterator<Item = SwitchState>,
    candidate: SwitchState,
    required_count: usize,
) -> bool {
    let mut matching = 0;
    for s in samples {
        if s == candidate {
            matching += 1;
            if matching >= required_count {
                return true;
            }
        } else {
            return false;
        }
    }
    matching >= required_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debounce_filter_initial_state() {
        let filter = DebounceFilter::new(SwitchState::Released, DebounceConfig::default());
        assert_eq!(filter.confirmed_state, SwitchState::Released);
        assert!(filter.press_start_time.is_none());

        let filter_pressed = DebounceFilter::new(SwitchState::Pressed, DebounceConfig::default());
        assert_eq!(filter_pressed.confirmed_state, SwitchState::Pressed);
        assert!(filter_pressed.press_start_time.is_some());
    }

    #[test]
    fn test_debounce_clean_press_and_release() {
        let mut filter = DebounceFilter::new(SwitchState::Released, DebounceConfig::default());
        let t0 = Instant::now();

        // 1. Confirm press
        let out1 = filter.confirm_transition(SwitchState::Pressed, t0);
        assert_eq!(out1, DebounceOutput::Pressed);
        assert_eq!(filter.confirmed_state, SwitchState::Pressed);

        // 2. Duplicate press should be ignored
        let out_dup = filter.confirm_transition(SwitchState::Pressed, t0 + Duration::from_millis(50));
        assert_eq!(out_dup, DebounceOutput::NoChange);

        // 3. Confirm release after 250ms
        let t1 = t0 + Duration::from_millis(250);
        let out2 = filter.confirm_transition(SwitchState::Released, t1);
        assert_eq!(
            out2,
            DebounceOutput::Released {
                duration_millis: 250
            }
        );
        assert_eq!(filter.confirmed_state, SwitchState::Released);

        // 4. Duplicate release should be ignored
        let out_dup2 = filter.confirm_transition(SwitchState::Released, t1 + Duration::from_millis(50));
        assert_eq!(out_dup2, DebounceOutput::NoChange);
    }

    #[test]
    fn test_simulated_bounce_stream() {
        let config = DebounceConfig {
            sample_interval: Duration::from_millis(5),
            required_stable_samples: 5,
        };
        let mut filter = DebounceFilter::new(SwitchState::Released, config);
        let start = Instant::now();

        // Simulate press with physical bounce:
        // [Pressed, Released, Pressed, Pressed, Released, Pressed, Pressed, Pressed, Pressed, Pressed]
        let bounce_series = [
            SwitchState::Pressed,
            SwitchState::Released, // bounce
            SwitchState::Pressed,
            SwitchState::Pressed,
            SwitchState::Released, // bounce
            SwitchState::Pressed,
            SwitchState::Pressed,
            SwitchState::Pressed,
            SwitchState::Pressed,
            SwitchState::Pressed,  // 5th consecutive -> stable!
        ];

        let mut confirmed_press = false;
        let mut consecutive = 0;
        let mut last_candidate = SwitchState::Released;

        for s in bounce_series {
            if s != filter.confirmed_state {
                if s == last_candidate {
                    consecutive += 1;
                } else {
                    last_candidate = s;
                    consecutive = 1;
                }

                if consecutive >= config.required_stable_samples {
                    let out = filter.confirm_transition(s, start + Duration::from_millis(50));
                    if out == DebounceOutput::Pressed {
                        confirmed_press = true;
                    }
                }
            }
        }

        assert!(confirmed_press);
        assert_eq!(filter.confirmed_state, SwitchState::Pressed);
    }
}
