//! Leg and SL state machine.
//!
//! A `Leg` represents a single option position (CE/PE, buy/sell) with
//! mark-to-market tracking and stop-loss state management.

use crate::config::{ExitReason, LegConfig, PositionSide};
use crate::sl_types::{is_sl_triggered, is_target_triggered, SlContext};

/// Stop-loss state machine.
///
/// Transitions: Active → TrailActivated → Triggered
/// Once Triggered, no further transitions occur.
#[derive(Debug, Clone)]
pub enum SlState {
    /// Monitoring — SL not yet hit, trailing not activated.
    Active,
    /// Trailing SL activated — profit crossed activate_at threshold.
    TrailActivated {
        high_water: f64,             // peak PnL since activation (monotonic)
        locked_floor: Option<f64>,   // for Lock mode: PnL floor (ratchets up)
    },
    /// SL has been triggered.
    Triggered { reason: ExitReason },
}

/// A single option leg within a position.
#[derive(Debug, Clone)]
pub struct Leg {
    pub config: LegConfig,
    pub entry_price: f64,
    pub entry_spot: f64,
    pub current_price: f64,
    pub current_spot: f64,
    pub lots: u32,
    pub lot_size: u32,
    pub unrealized_pnl: f64,
    pub peak_pnl: f64,
    pub sl_state: SlState,
    pub bars_held: u32,
}

impl Leg {
    /// Create a new leg from config and entry bar data.
    pub fn new(
        config: &LegConfig,
        entry_price: f64,
        entry_spot: f64,
        lot_size: u32,
    ) -> Self {
        Self {
            config: config.clone(),
            entry_price,
            entry_spot,
            current_price: entry_price,
            current_spot: entry_spot,
            lots: config.lots,
            lot_size,
            unrealized_pnl: 0.0,
            peak_pnl: 0.0,
            sl_state: SlState::Active,
            bars_held: 0,
        }
    }

    /// Total quantity (lots × lot_size).
    pub fn quantity(&self) -> f64 {
        (self.lots * self.lot_size) as f64
    }

    /// Direction multiplier: +1 for buy (profit when price rises),
    /// -1 for sell (profit when price falls).
    fn direction(&self) -> f64 {
        match self.config.position {
            PositionSide::Buy => 1.0,
            PositionSide::Sell => -1.0,
        }
    }

    /// Calculate PnL from entry to current price.
    fn calculate_pnl(&self) -> f64 {
        (self.current_price - self.entry_price) * self.direction() * self.quantity()
    }

    /// Mark-to-market update. Called every bar.
    pub fn update(&mut self, close_price: f64, spot: f64) {
        self.current_price = close_price;
        self.current_spot = spot;
        self.bars_held += 1;
        self.unrealized_pnl = self.calculate_pnl();

        // Track peak PnL for trailing SL
        if self.unrealized_pnl > self.peak_pnl {
            self.peak_pnl = self.unrealized_pnl;
        }

        // Update trailing SL state
        self.update_trail_state();
    }

    /// Update trailing SL state machine.
    ///
    /// Supports two modes:
    /// - **Lock**: lock_in_pct% of peak PnL becomes the floor. Floor ratchets up, never down.
    /// - **Trail**: maintain distance (points or percent) from high-water mark.
    fn update_trail_state(&mut self) {
        use crate::config::{TrailSlMode, TrailUnit};

        if !self.config.trail_sl_enabled {
            return;
        }

        match &self.sl_state {
            SlState::Active => {
                // Check if profit threshold reached to activate trailing
                let profit_pct = if self.entry_price > 0.0 {
                    (self.unrealized_pnl / (self.entry_price * self.quantity())) * 100.0
                } else {
                    0.0
                };
                if profit_pct >= self.config.trail_sl_activate_at {
                    let floor = match self.config.trail_sl_mode {
                        TrailSlMode::Lock => {
                            Some(self.unrealized_pnl * (self.config.trail_sl_lock_in / 100.0))
                        }
                        TrailSlMode::Trail => None,
                    };
                    self.sl_state = SlState::TrailActivated {
                        high_water: self.unrealized_pnl,
                        locked_floor: floor,
                    };
                }
            }
            SlState::TrailActivated {
                high_water,
                locked_floor,
            } => {
                // HWM never decreases (monotonic invariant)
                let hw = self.unrealized_pnl.max(*high_water);

                match self.config.trail_sl_mode {
                    TrailSlMode::Lock => {
                        // Floor = lock_in_pct% of peak. Ratchets UP only.
                        let new_floor = hw * (self.config.trail_sl_lock_in / 100.0);
                        let floor = new_floor.max(locked_floor.unwrap_or(0.0));

                        if self.unrealized_pnl < floor {
                            self.sl_state = SlState::Triggered {
                                reason: ExitReason::StopLoss,
                            };
                        } else {
                            self.sl_state = SlState::TrailActivated {
                                high_water: hw,
                                locked_floor: Some(floor),
                            };
                        }
                    }
                    TrailSlMode::Trail => {
                        // Trail distance from HWM
                        let threshold = match self.config.trail_sl_unit {
                            TrailUnit::Points => self.config.trail_sl_value * self.quantity(),
                            TrailUnit::Percent => hw * (self.config.trail_sl_value / 100.0),
                        };
                        let drawdown = hw - self.unrealized_pnl;
                        if drawdown >= threshold && threshold > 0.0 {
                            self.sl_state = SlState::Triggered {
                                reason: ExitReason::StopLoss,
                            };
                        } else {
                            self.sl_state = SlState::TrailActivated {
                                high_water: hw,
                                locked_floor: None,
                            };
                        }
                    }
                }
            }
            SlState::Triggered { .. } => {} // terminal state
        }
    }

    /// Build SL evaluation context from leg state.
    fn make_sl_context(&self) -> SlContext {
        SlContext {
            entry_price: self.entry_price,
            current_price: self.current_price,
            entry_spot: self.entry_spot,
            current_spot: self.current_spot,
            quantity: self.quantity(),
            lots: self.lots,
            lot_size: self.lot_size,
            direction: self.direction(),
            unrealized_pnl: self.unrealized_pnl,
        }
    }

    /// Check if the fixed SL has been hit.
    pub fn check_sl(&self) -> Option<ExitReason> {
        // If trailing SL already triggered, return that
        if let SlState::Triggered { reason } = &self.sl_state {
            return Some(*reason);
        }

        if !self.config.stop_loss_enabled {
            return None;
        }

        let ctx = self.make_sl_context();
        if is_sl_triggered(&self.config.stop_loss_type, self.config.stop_loss_value, &ctx) {
            Some(ExitReason::StopLoss)
        } else {
            None
        }
    }

    /// Check if target profit has been reached.
    pub fn check_target(&self) -> Option<ExitReason> {
        if !self.config.target_profit_enabled {
            return None;
        }

        let ctx = self.make_sl_context();
        if is_target_triggered(&self.config.target_profit_type, self.config.target_profit_value, &ctx) {
            Some(ExitReason::Target)
        } else {
            None
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OptionType, SlType, StrikeMode};

    fn make_leg_config(
        option_type: OptionType,
        position: PositionSide,
        sl_enabled: bool,
        sl_type: SlType,
        sl_value: f64,
        target_enabled: bool,
        target_type: SlType,
        target_value: f64,
    ) -> LegConfig {
        LegConfig {
            option_type,
            position,
            lots: 1,
            expiry: "weekly".to_string(),
            strike_mode: StrikeMode::AtmOffset,
            strike_offset: 0,
            delta_target: None,
            premium_target: None,
            stop_loss_enabled: sl_enabled,
            stop_loss_type: sl_type,
            stop_loss_value: sl_value,
            target_profit_enabled: target_enabled,
            target_profit_type: target_type,
            target_profit_value: target_value,
            trail_sl_enabled: false,
            trail_sl_activate_at: 0.0,
            trail_sl_lock_in: 0.0,
            trail_sl_mode: crate::config::TrailSlMode::Trail,
            trail_sl_unit: crate::config::TrailUnit::Percent,
            trail_sl_value: 0.0,
            reentry_on_sl: false,
            reentry_on_target: false,
            reentry_mode: crate::config::ReEntryMode::AfterNBars,
            reentry_cooldown_bars: 5,
            reentry_max_attempts: 2,
            momentum_filter_enabled: false,
        }
    }

    #[test]
    fn test_sell_leg_profit() {
        // Sell CE at 200, price drops to 100 → profit
        let config = make_leg_config(
            OptionType::CE, PositionSide::Sell,
            false, SlType::None, 0.0, false, SlType::None, 0.0,
        );
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        leg.update(100.0, 47900.0);
        // PnL = (100 - 200) * (-1) * 15 = 1500
        assert_eq!(leg.unrealized_pnl, 1500.0);
    }

    #[test]
    fn test_sell_leg_loss() {
        // Sell CE at 200, price rises to 400 → loss
        let config = make_leg_config(
            OptionType::CE, PositionSide::Sell,
            false, SlType::None, 0.0, false, SlType::None, 0.0,
        );
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        leg.update(400.0, 48200.0);
        // PnL = (400 - 200) * (-1) * 15 = -3000
        assert_eq!(leg.unrealized_pnl, -3000.0);
    }

    #[test]
    fn test_buy_leg_profit() {
        // Buy CE at 100, price rises to 200 → profit
        let config = make_leg_config(
            OptionType::CE, PositionSide::Buy,
            false, SlType::None, 0.0, false, SlType::None, 0.0,
        );
        let mut leg = Leg::new(&config, 100.0, 48000.0, 15);
        leg.update(200.0, 48100.0);
        // PnL = (200 - 100) * 1 * 15 = 1500
        assert_eq!(leg.unrealized_pnl, 1500.0);
    }

    #[test]
    fn test_sl_percent_of_premium_triggered() {
        // Sell CE at 200, SL=100%, price rises to 400 → 100% loss → SL hit
        let config = make_leg_config(
            OptionType::CE, PositionSide::Sell,
            true, SlType::PercentOfPremium, 100.0,
            false, SlType::None, 0.0,
        );
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        leg.update(400.0, 48200.0);
        assert_eq!(leg.check_sl(), Some(ExitReason::StopLoss));
    }

    #[test]
    fn test_sl_percent_of_premium_not_triggered() {
        // Sell CE at 200, SL=100%, price at 350 → 75% loss → not yet
        let config = make_leg_config(
            OptionType::CE, PositionSide::Sell,
            true, SlType::PercentOfPremium, 100.0,
            false, SlType::None, 0.0,
        );
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        leg.update(350.0, 48150.0);
        assert_eq!(leg.check_sl(), None);
    }

    #[test]
    fn test_sl_points_triggered() {
        // Sell CE at 200, SL=50 points, price rises to 250 → 50pt move → SL hit
        let config = make_leg_config(
            OptionType::CE, PositionSide::Sell,
            true, SlType::Points, 50.0,
            false, SlType::None, 0.0,
        );
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        leg.update(250.0, 48050.0);
        assert_eq!(leg.check_sl(), Some(ExitReason::StopLoss));
    }

    #[test]
    fn test_target_profit_hit() {
        // Sell CE at 200, target=50%, price drops to 100 → 50% of premium captured → target
        let config = make_leg_config(
            OptionType::CE, PositionSide::Sell,
            false, SlType::None, 0.0,
            true, SlType::PercentOfPremium, 50.0,
        );
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        leg.update(100.0, 47900.0);
        assert_eq!(leg.check_target(), Some(ExitReason::Target));
    }

    #[test]
    fn test_target_not_hit() {
        // Sell CE at 200, target=50%, price at 150 → only 25% captured → not yet
        let config = make_leg_config(
            OptionType::CE, PositionSide::Sell,
            false, SlType::None, 0.0,
            true, SlType::PercentOfPremium, 50.0,
        );
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        leg.update(150.0, 47950.0);
        assert_eq!(leg.check_target(), None);
    }

    #[test]
    fn test_bars_held_counter() {
        let config = make_leg_config(
            OptionType::CE, PositionSide::Sell,
            false, SlType::None, 0.0, false, SlType::None, 0.0,
        );
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        leg.update(198.0, 48000.0);
        leg.update(195.0, 48000.0);
        leg.update(190.0, 48000.0);
        assert_eq!(leg.bars_held, 3);
    }

    // ─── Trailing SL Tests ─────────────────────────────────

    fn make_trail_config(
        mode: crate::config::TrailSlMode,
        unit: crate::config::TrailUnit,
        activate_at: f64,
        lock_in: f64,
        trail_value: f64,
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
            trail_sl_enabled: true,
            trail_sl_activate_at: activate_at,
            trail_sl_lock_in: lock_in,
            trail_sl_mode: mode,
            trail_sl_unit: unit,
            trail_sl_value: trail_value,
            reentry_on_sl: false,
            reentry_on_target: false,
            reentry_mode: crate::config::ReEntryMode::AfterNBars,
            reentry_cooldown_bars: 5,
            reentry_max_attempts: 2,
            momentum_filter_enabled: false,
        }
    }

    #[test]
    fn test_trail_lock_mode_activates() {
        use crate::config::{TrailSlMode, TrailUnit};
        // Sell at 200, lot_size=15, qty=15
        // activate_at=30%, lock_in=50%
        // Profit at price=100: PnL=(200-100)*15=1500, pct=1500/(200*15)*100=50% > 30%
        let config = make_trail_config(TrailSlMode::Lock, TrailUnit::Percent, 30.0, 50.0, 0.0);
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        // Price drops to 100 → 50% profit → activate
        leg.update(100.0, 47900.0);
        match &leg.sl_state {
            SlState::TrailActivated { locked_floor, .. } => {
                // Floor = 50% of PnL 1500 = 750
                assert!(locked_floor.is_some());
                assert!((locked_floor.unwrap() - 750.0).abs() < 0.01);
            }
            _ => panic!("Expected TrailActivated, got {:?}", leg.sl_state),
        }
    }

    #[test]
    fn test_trail_lock_floor_ratchets_up() {
        use crate::config::{TrailSlMode, TrailUnit};
        let config = make_trail_config(TrailSlMode::Lock, TrailUnit::Percent, 30.0, 50.0, 0.0);
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        // Activate at 100 → PnL=1500, floor=750
        leg.update(100.0, 47900.0);
        let floor1 = match &leg.sl_state {
            SlState::TrailActivated { locked_floor, .. } => locked_floor.unwrap(),
            _ => panic!("Expected TrailActivated"),
        };
        // Push higher profit: price=50 → PnL=2250, new floor=50%*2250=1125
        leg.update(50.0, 47850.0);
        let floor2 = match &leg.sl_state {
            SlState::TrailActivated { locked_floor, .. } => locked_floor.unwrap(),
            _ => panic!("Expected TrailActivated"),
        };
        assert!(floor2 > floor1, "Floor must ratchet up: {} > {}", floor2, floor1);
    }

    #[test]
    fn test_trail_lock_triggers_below_floor() {
        use crate::config::{TrailSlMode, TrailUnit};
        let config = make_trail_config(TrailSlMode::Lock, TrailUnit::Percent, 30.0, 50.0, 0.0);
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        // Activate: PnL=1500, floor=750
        leg.update(100.0, 47900.0);
        // Push peak higher: PnL=2250, floor=1125
        leg.update(50.0, 47850.0);
        // Drop PnL below floor: price=160 → PnL=(200-160)*15=600 < 1125 → trigger
        leg.update(160.0, 47960.0);
        assert!(matches!(leg.sl_state, SlState::Triggered { reason: ExitReason::StopLoss }));
    }

    #[test]
    fn test_trail_mode_activates_and_triggers() {
        use crate::config::{TrailSlMode, TrailUnit};
        // activate_at=20%, trail_by=30% of peak
        let config = make_trail_config(TrailSlMode::Trail, TrailUnit::Percent, 20.0, 0.0, 30.0);
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        // PnL at price=140: (200-140)*15=900, pct=900/(200*15)*100=30% > 20% → activate
        leg.update(140.0, 47940.0);
        assert!(matches!(leg.sl_state, SlState::TrailActivated { .. }));
        // Push higher: price=80 → PnL=1800, HWM=1800
        leg.update(80.0, 47880.0);
        // Retrace: price=160 → PnL=600, drawdown=1200, 30% of 1800=540 → 1200>540 → trigger
        leg.update(160.0, 47960.0);
        assert!(matches!(leg.sl_state, SlState::Triggered { reason: ExitReason::StopLoss }));
    }

    #[test]
    fn test_hwm_never_decreases() {
        use crate::config::{TrailSlMode, TrailUnit};
        let config = make_trail_config(TrailSlMode::Trail, TrailUnit::Percent, 20.0, 0.0, 90.0);
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        // Activate
        leg.update(140.0, 47940.0); // PnL=900
        let hw1 = match &leg.sl_state {
            SlState::TrailActivated { high_water, .. } => *high_water,
            _ => panic!("Expected TrailActivated"),
        };
        // Drop PnL
        leg.update(180.0, 47980.0); // PnL=300
        let hw2 = match &leg.sl_state {
            SlState::TrailActivated { high_water, .. } => *high_water,
            _ => panic!("Expected TrailActivated"),
        };
        // Rise again
        leg.update(100.0, 47900.0); // PnL=1500
        let hw3 = match &leg.sl_state {
            SlState::TrailActivated { high_water, .. } => *high_water,
            _ => panic!("Expected TrailActivated"),
        };
        assert!(hw2 >= hw1, "HWM must not decrease: {} >= {}", hw2, hw1);
        assert!(hw3 >= hw2, "HWM must not decrease: {} >= {}", hw3, hw2);
    }

    #[test]
    fn test_trail_not_activated_below_threshold() {
        use crate::config::{TrailSlMode, TrailUnit};
        let config = make_trail_config(TrailSlMode::Trail, TrailUnit::Percent, 50.0, 0.0, 30.0);
        let mut leg = Leg::new(&config, 200.0, 48000.0, 15);
        // PnL at price=170: (200-170)*15=450, pct=450/(200*15)*100=15% < 50% → stays Active
        leg.update(170.0, 47970.0);
        assert!(matches!(leg.sl_state, SlState::Active));
    }
}
