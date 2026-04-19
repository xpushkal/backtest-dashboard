//! Execution engine — slippage, brokerage, and STT calculations.
//!
//! All transaction costs are computed here. The simulation runner
//! delegates cost calculation to this module.

use crate::config::{PositionSide, SlippageModel};

/// Transaction cost calculator.
pub struct ExecutionEngine;

impl ExecutionEngine {
    /// Apply slippage to a fill price.
    ///
    /// For sells: entry price reduced (worse fill = lower premium received).
    /// For buys: entry price increased (worse fill = higher premium paid).
    pub fn apply_slippage(
        price: f64,
        side: PositionSide,
        model: &SlippageModel,
        value: f64,
    ) -> f64 {
        match model {
            SlippageModel::None => price,
            SlippageModel::FixedPts => match side {
                PositionSide::Sell => (price - value).max(0.01),
                PositionSide::Buy => price + value,
            },
            SlippageModel::Percent => match side {
                PositionSide::Sell => price * (1.0 - value / 100.0),
                PositionSide::Buy => price * (1.0 + value / 100.0),
            },
            SlippageModel::VolumeBased => price, // stub
        }
    }

    /// Calculate brokerage for a trade (entry + exit, both sides).
    pub fn calculate_brokerage(brokerage_per_lot: f64, lots: u32) -> f64 {
        brokerage_per_lot * lots as f64 * 2.0
    }

    /// Calculate STT (Securities Transaction Tax).
    ///
    /// FNO sell side: 0.0625% of (premium × quantity).
    /// Applied on the sell leg of each transaction.
    pub fn calculate_stt(
        sell_price: f64,
        quantity: u32,
        stt_on_sell: bool,
    ) -> f64 {
        if !stt_on_sell {
            return 0.0;
        }
        let sell_value = sell_price * quantity as f64;
        sell_value * 0.000625
    }

    /// Calculate total slippage cost for a round-trip (entry + exit).
    pub fn calculate_slippage_cost(
        entry_price: f64,
        exit_price: f64,
        model: &SlippageModel,
        value: f64,
        lots: u32,
        lot_size: u32,
    ) -> f64 {
        let quantity = (lots * lot_size) as f64;
        match model {
            SlippageModel::None => 0.0,
            SlippageModel::FixedPts => value * quantity * 2.0,
            SlippageModel::Percent => {
                let entry_slip = entry_price * (value / 100.0) * quantity;
                let exit_slip = exit_price * (value / 100.0) * quantity;
                entry_slip + exit_slip
            }
            SlippageModel::VolumeBased => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slippage_fixed_pts_sell() {
        let result = ExecutionEngine::apply_slippage(200.0, PositionSide::Sell, &SlippageModel::FixedPts, 1.0);
        assert_eq!(result, 199.0);
    }

    #[test]
    fn test_slippage_fixed_pts_buy() {
        let result = ExecutionEngine::apply_slippage(200.0, PositionSide::Buy, &SlippageModel::FixedPts, 1.0);
        assert_eq!(result, 201.0);
    }

    #[test]
    fn test_slippage_none() {
        let result = ExecutionEngine::apply_slippage(200.0, PositionSide::Sell, &SlippageModel::None, 5.0);
        assert_eq!(result, 200.0);
    }

    #[test]
    fn test_slippage_percent() {
        let result = ExecutionEngine::apply_slippage(200.0, PositionSide::Sell, &SlippageModel::Percent, 1.0);
        assert_eq!(result, 198.0); // 200 * 0.99
    }

    #[test]
    fn test_slippage_floor_at_001() {
        let result = ExecutionEngine::apply_slippage(0.5, PositionSide::Sell, &SlippageModel::FixedPts, 5.0);
        assert_eq!(result, 0.01); // floored
    }

    #[test]
    fn test_brokerage_calculation() {
        assert_eq!(ExecutionEngine::calculate_brokerage(40.0, 1), 80.0);
        assert_eq!(ExecutionEngine::calculate_brokerage(40.0, 2), 160.0);
        assert_eq!(ExecutionEngine::calculate_brokerage(20.0, 3), 120.0);
    }

    #[test]
    fn test_stt_calculation() {
        // 200 * 15 * 0.000625 = 1.875
        let stt = ExecutionEngine::calculate_stt(200.0, 15, true);
        assert!((stt - 1.875).abs() < 0.0001);
    }

    #[test]
    fn test_stt_disabled() {
        let stt = ExecutionEngine::calculate_stt(200.0, 15, false);
        assert_eq!(stt, 0.0);
    }

    #[test]
    fn test_slippage_cost_fixed() {
        // 1pt * 15 quantity * 2 sides = 30
        let cost = ExecutionEngine::calculate_slippage_cost(200.0, 150.0, &SlippageModel::FixedPts, 1.0, 1, 15);
        assert_eq!(cost, 30.0);
    }

    #[test]
    fn test_slippage_cost_none() {
        let cost = ExecutionEngine::calculate_slippage_cost(200.0, 150.0, &SlippageModel::None, 1.0, 1, 15);
        assert_eq!(cost, 0.0);
    }
}
