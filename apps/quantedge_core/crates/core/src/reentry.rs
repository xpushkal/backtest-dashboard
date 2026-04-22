//! Re-entry state machine for option legs.
//!
//! After a leg exits via SL or target, the re-entry tracker manages
//! the lifecycle: cooldown → readiness → re-entry → exhaustion.

use crate::config::{ExitReason, LegConfig, ReEntryMode};

/// Re-entry state machine per leg.
#[derive(Debug, Clone)]
pub enum ReEntryState {
    /// No re-entry pending — waiting for next exit event.
    Idle,
    /// Cooling down — waiting for cooldown_bars to elapse.
    Cooling { bars_remaining: u32, attempt: u32 },
    /// Waiting for momentum filter confirmation (MomentumConfirm mode only).
    WaitingForMomentum { attempt: u32 },
    /// Cleared to re-enter on next entry opportunity.
    Ready { attempt: u32 },
    /// All re-entry attempts exhausted — terminal state.
    Exhausted,
}

/// Tracks re-entry state for a single leg.
#[derive(Debug, Clone)]
pub struct ReEntryTracker {
    pub state: ReEntryState,
    /// Whether to re-enter after SL hit.
    reentry_on_sl: bool,
    /// Whether to re-enter after target hit.
    reentry_on_target: bool,
    /// Which mode to use for re-entry timing.
    mode: ReEntryMode,
    /// How many bars to wait (AfterNBars mode).
    cooldown_bars: u32,
    /// Maximum re-entry attempts before Exhausted.
    max_attempts: u32,
    /// How many re-entries have been completed.
    pub completed_attempts: u32,
}

impl ReEntryTracker {
    /// Create a new tracker from leg configuration.
    pub fn new(config: &LegConfig) -> Self {
        let enabled = config.reentry_on_sl || config.reentry_on_target;
        Self {
            state: if enabled { ReEntryState::Idle } else { ReEntryState::Exhausted },
            reentry_on_sl: config.reentry_on_sl,
            reentry_on_target: config.reentry_on_target,
            mode: config.reentry_mode,
            cooldown_bars: config.reentry_cooldown_bars,
            max_attempts: config.reentry_max_attempts,
            completed_attempts: 0,
        }
    }

    /// Called when a leg exits. Determines whether to arm re-entry.
    pub fn on_exit(&mut self, exit_reason: &ExitReason) {
        // Only transition from Idle
        if !matches!(self.state, ReEntryState::Idle) {
            return;
        }

        // Check if this exit reason triggers re-entry
        let should_arm = match exit_reason {
            ExitReason::StopLoss | ExitReason::CombinedSl => self.reentry_on_sl,
            ExitReason::Target | ExitReason::CombinedTarget => self.reentry_on_target,
            // TimeExit, EndOfData → no re-entry
            _ => false,
        };

        if !should_arm {
            return;
        }

        let attempt = self.completed_attempts + 1;

        // Check if we've exceeded max attempts
        if attempt > self.max_attempts {
            self.state = ReEntryState::Exhausted;
            return;
        }

        self.state = match self.mode {
            ReEntryMode::Asap => ReEntryState::Ready { attempt },
            ReEntryMode::SameTime => ReEntryState::Ready { attempt },
            ReEntryMode::AfterNBars => {
                if self.cooldown_bars == 0 {
                    ReEntryState::Ready { attempt }
                } else {
                    ReEntryState::Cooling {
                        bars_remaining: self.cooldown_bars,
                        attempt,
                    }
                }
            }
            ReEntryMode::MomentumConfirm => ReEntryState::WaitingForMomentum { attempt },
        };
    }

    /// Called every bar when no position is open. Decrements cooldown counter.
    pub fn tick(&mut self) {
        if let ReEntryState::Cooling { bars_remaining, attempt } = &mut self.state {
            if *bars_remaining > 0 {
                *bars_remaining -= 1;
            }
            if *bars_remaining == 0 {
                self.state = ReEntryState::Ready { attempt: *attempt };
            }
        }
    }

    /// Returns true when the tracker is in Ready state (cleared to re-enter).
    pub fn should_reenter(&self) -> bool {
        matches!(self.state, ReEntryState::Ready { .. })
    }

    /// Called after a re-entry position is successfully opened.
    /// Transitions Ready → Idle (awaiting next exit) or → Exhausted.
    pub fn confirm_reentry(&mut self) {
        if let ReEntryState::Ready { attempt } = self.state {
            self.completed_attempts = attempt;
            if attempt >= self.max_attempts {
                // This was the last allowed attempt
                self.state = ReEntryState::Idle;
                // After next exit, on_exit will see attempt > max and go Exhausted
            } else {
                self.state = ReEntryState::Idle;
            }
        }
    }

    /// Returns true if all re-entry attempts have been exhausted.
    pub fn is_exhausted(&self) -> bool {
        matches!(self.state, ReEntryState::Exhausted)
    }

    /// Feed a momentum signal (used in MomentumConfirm mode).
    /// For sell positions: Bearish confirms re-entry.
    /// For buy positions: Bullish confirms re-entry.
    pub fn feed_momentum(&mut self, bullish: bool, is_sell: bool) {
        if let ReEntryState::WaitingForMomentum { attempt } = self.state {
            let confirmed = if is_sell { !bullish } else { bullish };
            if confirmed {
                self.state = ReEntryState::Ready { attempt };
            }
        }
    }

    /// Get current attempt number (0 = initial entry, 1+ = re-entries).
    pub fn current_attempt(&self) -> u32 {
        match &self.state {
            ReEntryState::Ready { attempt } => *attempt,
            ReEntryState::Cooling { attempt, .. } => *attempt,
            ReEntryState::WaitingForMomentum { attempt } => *attempt,
            _ => self.completed_attempts,
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    fn make_config(
        on_sl: bool,
        on_target: bool,
        mode: ReEntryMode,
        cooldown: u32,
        max_attempts: u32,
    ) -> LegConfig {
        LegConfig {
            option_type: OptionType::CE,
            position: PositionSide::Sell,
            lots: 1,
            expiry: "weekly".to_string(),
            strike_mode: StrikeMode::AtmOffset,
            strike_offset: 0,
            delta_target: None,
            premium_target: None,
            stop_loss_enabled: false,
            stop_loss_type: SlType::None,
            stop_loss_value: 0.0,
            target_profit_enabled: false,
            target_profit_type: SlType::None,
            target_profit_value: 0.0,
            trail_sl_enabled: false,
            trail_sl_activate_at: 0.0,
            trail_sl_lock_in: 0.0,
            trail_sl_mode: TrailSlMode::Trail,
            trail_sl_unit: TrailUnit::Percent,
            trail_sl_value: 0.0,
            reentry_on_sl: on_sl,
            reentry_on_target: on_target,
            reentry_mode: mode,
            reentry_cooldown_bars: cooldown,
            reentry_max_attempts: max_attempts,
            momentum_filter_enabled: false,
        }
    }

    #[test]
    fn test_idle_initial_state() {
        let cfg = make_config(true, false, ReEntryMode::Asap, 0, 2);
        let tracker = ReEntryTracker::new(&cfg);
        assert!(matches!(tracker.state, ReEntryState::Idle));
    }

    #[test]
    fn test_disabled_starts_exhausted() {
        let cfg = make_config(false, false, ReEntryMode::Asap, 0, 2);
        let tracker = ReEntryTracker::new(&cfg);
        assert!(tracker.is_exhausted());
    }

    #[test]
    fn test_no_reentry_on_wrong_trigger() {
        // reentry_on_sl=true but exit is Target → stays Idle
        let cfg = make_config(true, false, ReEntryMode::Asap, 0, 2);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::Target);
        assert!(matches!(tracker.state, ReEntryState::Idle));
    }

    #[test]
    fn test_asap_immediate_ready() {
        let cfg = make_config(true, false, ReEntryMode::Asap, 0, 2);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::StopLoss);
        assert!(tracker.should_reenter());
        assert!(matches!(tracker.state, ReEntryState::Ready { attempt: 1 }));
    }

    #[test]
    fn test_after_n_bars_cooling() {
        let cfg = make_config(true, false, ReEntryMode::AfterNBars, 5, 3);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::StopLoss);
        assert!(matches!(tracker.state, ReEntryState::Cooling { bars_remaining: 5, attempt: 1 }));
        assert!(!tracker.should_reenter());
    }

    #[test]
    fn test_cooling_decrement() {
        let cfg = make_config(true, false, ReEntryMode::AfterNBars, 3, 2);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::StopLoss);
        tracker.tick(); // 3 → 2
        assert!(matches!(tracker.state, ReEntryState::Cooling { bars_remaining: 2, .. }));
        tracker.tick(); // 2 → 1
        assert!(matches!(tracker.state, ReEntryState::Cooling { bars_remaining: 1, .. }));
        tracker.tick(); // 1 → 0 → Ready
        assert!(tracker.should_reenter());
    }

    #[test]
    fn test_ready_to_idle_on_confirm() {
        let cfg = make_config(true, false, ReEntryMode::Asap, 0, 3);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::StopLoss);
        assert!(tracker.should_reenter());
        tracker.confirm_reentry();
        assert!(matches!(tracker.state, ReEntryState::Idle));
        assert_eq!(tracker.completed_attempts, 1);
    }

    #[test]
    fn test_max_attempts_exhausted() {
        let cfg = make_config(true, false, ReEntryMode::Asap, 0, 2);
        let mut tracker = ReEntryTracker::new(&cfg);

        // Attempt 1
        tracker.on_exit(&ExitReason::StopLoss);
        tracker.confirm_reentry();
        assert_eq!(tracker.completed_attempts, 1);

        // Attempt 2
        tracker.on_exit(&ExitReason::StopLoss);
        tracker.confirm_reentry();
        assert_eq!(tracker.completed_attempts, 2);

        // Attempt 3 — should be exhausted
        tracker.on_exit(&ExitReason::StopLoss);
        assert!(tracker.is_exhausted());
    }

    #[test]
    fn test_exhausted_is_terminal() {
        let cfg = make_config(true, false, ReEntryMode::Asap, 0, 1);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::StopLoss);
        tracker.confirm_reentry();
        tracker.on_exit(&ExitReason::StopLoss);
        assert!(tracker.is_exhausted());
        // Further exits don't change state
        tracker.on_exit(&ExitReason::StopLoss);
        assert!(tracker.is_exhausted());
    }

    #[test]
    fn test_momentum_waiting() {
        let cfg = make_config(true, false, ReEntryMode::MomentumConfirm, 0, 2);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::StopLoss);
        assert!(matches!(tracker.state, ReEntryState::WaitingForMomentum { attempt: 1 }));
        assert!(!tracker.should_reenter());
    }

    #[test]
    fn test_momentum_confirm_bearish_for_sell() {
        let cfg = make_config(true, false, ReEntryMode::MomentumConfirm, 0, 2);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::StopLoss);
        // Bullish signal for sell → no confirm
        tracker.feed_momentum(true, true);
        assert!(!tracker.should_reenter());
        // Bearish signal for sell → confirm
        tracker.feed_momentum(false, true);
        assert!(tracker.should_reenter());
    }

    #[test]
    fn test_momentum_confirm_bullish_for_buy() {
        let cfg = make_config(true, false, ReEntryMode::MomentumConfirm, 0, 2);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::StopLoss);
        // Bearish for buy → no confirm
        tracker.feed_momentum(false, false);
        assert!(!tracker.should_reenter());
        // Bullish for buy → confirm
        tracker.feed_momentum(true, false);
        assert!(tracker.should_reenter());
    }

    #[test]
    fn test_no_reentry_on_time_exit() {
        let cfg = make_config(true, true, ReEntryMode::Asap, 0, 5);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::TimeExit);
        assert!(matches!(tracker.state, ReEntryState::Idle));
    }

    #[test]
    fn test_reentry_on_target() {
        let cfg = make_config(false, true, ReEntryMode::Asap, 0, 2);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::Target);
        assert!(tracker.should_reenter());
    }

    #[test]
    fn test_combined_sl_triggers_reentry() {
        let cfg = make_config(true, false, ReEntryMode::Asap, 0, 2);
        let mut tracker = ReEntryTracker::new(&cfg);
        tracker.on_exit(&ExitReason::CombinedSl);
        assert!(tracker.should_reenter());
    }

    #[test]
    fn test_current_attempt_tracking() {
        let cfg = make_config(true, false, ReEntryMode::Asap, 0, 5);
        let mut tracker = ReEntryTracker::new(&cfg);
        assert_eq!(tracker.current_attempt(), 0);
        tracker.on_exit(&ExitReason::StopLoss);
        assert_eq!(tracker.current_attempt(), 1);
        tracker.confirm_reentry();
        tracker.on_exit(&ExitReason::StopLoss);
        assert_eq!(tracker.current_attempt(), 2);
    }
}
