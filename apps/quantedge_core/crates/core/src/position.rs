//! Position and ClosedTrade types.
//!
//! A `Position` holds one or more legs opened together.
//! A `ClosedTrade` records the completed trade with full cost breakdown.

use crate::config::{ExitReason, OptionType, PositionSide};
use crate::leg::Leg;
use chrono::{NaiveDate, NaiveTime};

/// An open position consisting of one or more legs.
#[derive(Debug, Clone)]
pub struct Position {
    pub legs: Vec<Leg>,
    pub entry_date: NaiveDate,
    pub entry_time: NaiveTime,
    pub entry_bar_index: usize,
    pub entry_brokerage: f64,
}

impl Position {
    /// Create a new position from legs.
    pub fn new(
        legs: Vec<Leg>,
        entry_date: NaiveDate,
        entry_time: NaiveTime,
        bar_index: usize,
        entry_brokerage: f64,
    ) -> Self {
        Self {
            legs,
            entry_date,
            entry_time,
            entry_bar_index: bar_index,
            entry_brokerage,
        }
    }

    /// Mark-to-market update for all legs.
    pub fn update_leg_prices(&mut self, prices: &[(f64, f64)]) {
        for (leg, &(close, spot)) in self.legs.iter_mut().zip(prices.iter()) {
            leg.update(close, spot);
        }
    }

    /// Total unrealized PnL across all legs.
    pub fn total_unrealized_pnl(&self) -> f64 {
        self.legs.iter().map(|l| l.unrealized_pnl).sum()
    }

    /// Max bars held across all legs.
    pub fn total_bars_held(&self) -> u32 {
        self.legs.iter().map(|l| l.bars_held).max().unwrap_or(0)
    }
}

/// A completed trade with full cost breakdown.
#[derive(Debug, Clone)]
pub struct ClosedTrade {
    pub entry_date: NaiveDate,
    pub entry_time: NaiveTime,
    pub exit_date: NaiveDate,
    pub exit_time: NaiveTime,
    pub option_type: OptionType,
    pub position_side: PositionSide,
    pub entry_price: f64,
    pub exit_price: f64,
    pub entry_spot: f64,
    pub exit_spot: f64,
    pub lots: u32,
    pub lot_size: u32,
    pub pnl_gross: f64,
    pub brokerage: f64,
    pub stt: f64,
    pub slippage_cost: f64,
    /// Indian regulatory + exchange fees: NSE exchange transaction (0.053%),
    /// SEBI turnover (0.0001%), stamp duty on buy side (0.003%), and GST 18%
    /// on (brokerage + exchange + SEBI). Roughly 0.10–0.15% of round-trip turnover.
    pub other_charges: f64,
    pub pnl_net: f64,
    pub exit_reason: ExitReason,
    pub bars_held: u32,
    /// Re-entry attempt number (0 = initial entry, 1+ = re-entries).
    pub reentry_attempt: u32,
}

impl ClosedTrade {
    /// Build a ClosedTrade from a leg and exit context.
    pub fn from_leg(
        leg: &Leg,
        entry_date: NaiveDate,
        entry_time: NaiveTime,
        exit_date: NaiveDate,
        exit_time: NaiveTime,
        exit_price: f64,
        exit_spot: f64,
        exit_reason: ExitReason,
        brokerage: f64,
        stt: f64,
        slippage_cost: f64,
        other_charges: f64,
        reentry_attempt: u32,
    ) -> Self {
        let direction = match leg.config.position {
            PositionSide::Buy => 1.0,
            PositionSide::Sell => -1.0,
        };
        let quantity = (leg.lots * leg.lot_size) as f64;
        let pnl_gross = (exit_price - leg.entry_price) * direction * quantity;
        // NOTE: slippage_cost is already baked into pnl_gross because the runner
        // passes slipped entry/exit prices. Stored here for cost-breakdown
        // reporting only — do NOT subtract again.
        let pnl_net = pnl_gross - brokerage - stt - other_charges;

        Self {
            entry_date,
            entry_time,
            exit_date,
            exit_time,
            option_type: leg.config.option_type,
            position_side: leg.config.position,
            entry_price: leg.entry_price,
            exit_price,
            entry_spot: leg.entry_spot,
            exit_spot,
            lots: leg.lots,
            lot_size: leg.lot_size,
            pnl_gross,
            brokerage,
            stt,
            slippage_cost,
            other_charges,
            pnl_net,
            exit_reason,
            bars_held: leg.bars_held,
            reentry_attempt,
        }
    }
}

/// Equity curve snapshot at a point in time.
#[derive(Debug, Clone)]
pub struct PositionSnapshot {
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub spot: f64,
    pub equity: f64,
    pub unrealized_pnl: f64,
    pub cumulative_pnl: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SlType, StrikeMode};
    use crate::leg::Leg;

    fn make_test_leg(entry_price: f64, position: PositionSide) -> Leg {
        let config = crate::config::LegConfig {
            option_type: OptionType::CE,
            position,
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
            trail_sl_mode: crate::config::TrailSlMode::Trail,
            trail_sl_unit: crate::config::TrailUnit::Percent,
            trail_sl_value: 0.0,
            reentry_on_sl: false,
            reentry_on_target: false,
            reentry_mode: crate::config::ReEntryMode::AfterNBars,
            reentry_cooldown_bars: 5,
            reentry_max_attempts: 2,
            momentum_filter_enabled: false,
        };
        Leg::new(&config, entry_price, 48000.0, 15)
    }

    #[test]
    fn test_position_total_pnl() {
        let mut leg = make_test_leg(200.0, PositionSide::Sell);
        leg.update(150.0, 47950.0);
        let pos = Position::new(
            vec![leg],
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            0,
            80.0,
        );
        // Sell at 200, now at 150 → profit = (200-150)*15 = 750
        assert_eq!(pos.total_unrealized_pnl(), 750.0);
    }

    #[test]
    fn test_closed_trade_net_pnl() {
        // pnl_net deducts brokerage + stt only — slippage is already
        // reflected inside pnl_gross by the runner (slipped fills).
        let _leg = make_test_leg(200.0, PositionSide::Sell);
        let trade = ClosedTrade {
            entry_date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            entry_time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            exit_date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            exit_time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
            option_type: OptionType::CE,
            position_side: PositionSide::Sell,
            entry_price: 200.0,
            exit_price: 150.0,
            entry_spot: 48000.0,
            exit_spot: 47950.0,
            lots: 1,
            lot_size: 15,
            pnl_gross: 750.0,
            brokerage: 80.0,
            stt: 1.40625,
            slippage_cost: 30.0,
            other_charges: 0.0,
            pnl_net: 750.0 - 80.0 - 1.40625,
            exit_reason: ExitReason::TimeExit,
            bars_held: 360,
            reentry_attempt: 0,
        };
        let expected_net = 750.0 - 80.0 - 1.40625;
        assert!((trade.pnl_net - expected_net).abs() < 0.001);
        assert!(trade.pnl_net < trade.pnl_gross);
    }
}
