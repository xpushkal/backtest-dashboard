//! Leg and SL state machine.
//!
//! A `Leg` represents a single option position (CE/PE, buy/sell) with
//! mark-to-market tracking and stop-loss state management.

use crate::config::{ExitReason, LegConfig, OptionType, PositionSide, SlType};
use chrono::NaiveTime;

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
        high_water: f64, // peak PnL since activation
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
    fn update_trail_state(&mut self) {
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
                    self.sl_state = SlState::TrailActivated {
                        high_water: self.unrealized_pnl,
                    };
                }
            }
            SlState::TrailActivated { high_water } => {
                let hw = if self.unrealized_pnl > *high_water {
                    self.unrealized_pnl
                } else {
                    *high_water
                };

                // Check if price has retraced enough to trigger
                let drawdown_from_peak = hw - self.unrealized_pnl;
                let trail_threshold = match self.config.trail_sl_type.as_str() {
                    "points" => self.config.trail_sl_value * self.quantity(),
                    "percent" | _ => hw * (self.config.trail_sl_value / 100.0),
                };

                if drawdown_from_peak >= trail_threshold && trail_threshold > 0.0 {
                    self.sl_state = SlState::Triggered {
                        reason: ExitReason::StopLoss,
                    };
                } else {
                    self.sl_state = SlState::TrailActivated { high_water: hw };
                }
            }
            SlState::Triggered { .. } => {} // terminal state
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

        let triggered = match self.config.stop_loss_type {
            SlType::Points => {
                // For sell: loss when price rises by X points
                // For buy: loss when price drops by X points
                let price_move = (self.current_price - self.entry_price) * -self.direction();
                price_move >= self.config.stop_loss_value
            }
            SlType::PercentOfPremium => {
                // Loss as % of entry premium
                let loss_pct = if self.entry_price > 0.0 {
                    (-self.unrealized_pnl / (self.entry_price * self.quantity())) * 100.0
                } else {
                    0.0
                };
                loss_pct >= self.config.stop_loss_value
            }
            SlType::IndexPoints => {
                // Spot moved X points against position
                let spot_move = (self.current_spot - self.entry_spot) * -self.direction();
                spot_move >= self.config.stop_loss_value
            }
            // PercentOfMargin, DeltaBreach, CombinedPremium → Phase 3+
            _ => false,
        };

        if triggered {
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

        let triggered = match self.config.target_profit_type {
            SlType::PercentOfPremium => {
                let profit_pct = if self.entry_price > 0.0 {
                    (self.unrealized_pnl / (self.entry_price * self.quantity())) * 100.0
                } else {
                    0.0
                };
                profit_pct >= self.config.target_profit_value
            }
            SlType::Points => {
                let price_move = (self.current_price - self.entry_price) * self.direction();
                price_move >= self.config.target_profit_value
            }
            _ => false,
        };

        if triggered {
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
    use crate::config::StrikeMode;

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
            trail_sl_type: "percent".to_string(),
            trail_sl_value: 0.0,
            reentry_on_sl: false,
            reentry_on_target: false,
            reentry_mode: "after_n_bars".to_string(),
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
}
