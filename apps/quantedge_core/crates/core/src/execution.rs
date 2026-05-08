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

    // ─── NSE Indian Options Statutory & Exchange Charges ──────
    //
    // Rates as of 2024 (verify periodically — these change). Applied in addition
    // to brokerage and STT in `calculate_indian_charges`.
    //
    //   Exchange transaction fee : 0.053% of premium turnover (both legs)
    //   SEBI turnover fee        : 0.0001% of premium turnover (both legs)
    //   Stamp duty               : 0.003% on the BUY leg only
    //   GST                      : 18% on (brokerage + exchange + SEBI)
    //
    // Premium turnover = premium × quantity (lots × lot_size).
    const NSE_EXCHANGE_RATE: f64 = 0.00053;
    const SEBI_TURNOVER_RATE: f64 = 0.000001; // 0.0001% = 1 / 1_000_000
    const STAMP_DUTY_BUY_RATE: f64 = 0.00003;
    const GST_RATE: f64 = 0.18;

    /// Total Indian regulatory & exchange charges for a one-way fill.
    ///
    /// `is_buy` controls stamp duty (charged on buy side only).
    /// Brokerage is provided so GST can be applied on it.
    pub fn calculate_one_way_charges(
        premium: f64,
        quantity: f64,
        brokerage: f64,
        is_buy: bool,
    ) -> f64 {
        let turnover = premium * quantity;
        let exchange = turnover * Self::NSE_EXCHANGE_RATE;
        let sebi = turnover * Self::SEBI_TURNOVER_RATE;
        let stamp = if is_buy { turnover * Self::STAMP_DUTY_BUY_RATE } else { 0.0 };
        let gst = (brokerage + exchange + sebi) * Self::GST_RATE;
        exchange + sebi + stamp + gst
    }

    /// Round-trip Indian regulatory & exchange charges (entry + exit).
    ///
    /// Stamp duty fires on the buy leg only — for a SELL position, that's the EXIT;
    /// for a BUY position, that's the ENTRY. `brokerage_round_trip` is the full
    /// 2-side brokerage (already × 2 from `calculate_brokerage`); we split it for GST.
    pub fn calculate_indian_charges(
        entry_price: f64,
        exit_price: f64,
        quantity: f64,
        brokerage_round_trip: f64,
        position: PositionSide,
    ) -> f64 {
        let one_way_brokerage = brokerage_round_trip / 2.0;
        let entry_is_buy = matches!(position, PositionSide::Buy);
        let exit_is_buy = !entry_is_buy;
        let entry_charges = Self::calculate_one_way_charges(
            entry_price, quantity, one_way_brokerage, entry_is_buy,
        );
        let exit_charges = Self::calculate_one_way_charges(
            exit_price, quantity, one_way_brokerage, exit_is_buy,
        );
        entry_charges + exit_charges
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
    fn test_indian_charges_short_position() {
        // Short CE entry @ 200, exit @ 150, qty=15, brokerage 80 round-trip.
        // Entry (sell): turnover=3000, exchange=1.59, sebi=0.003, stamp=0, gst=0.18*(40+1.59+0.003)=7.487
        //   one-way = 1.59 + 0.003 + 0 + 7.487 = 9.08
        // Exit (buy):  turnover=2250, exchange=1.1925, sebi=0.00225, stamp=0.0675,
        //   gst=0.18*(40+1.1925+0.00225)=7.4137 → one-way = 1.1925+0.00225+0.0675+7.4137=8.6760
        // Total ≈ 17.76
        let charges = ExecutionEngine::calculate_indian_charges(
            200.0, 150.0, 15.0, 80.0, PositionSide::Sell,
        );
        assert!((charges - 17.76).abs() < 0.05, "got {}", charges);
    }

    #[test]
    fn test_indian_charges_long_position() {
        // Long CE entry @ 100 (buy), exit @ 130 (sell). qty=15, brokerage 80.
        // Stamp duty fires on entry (buy) only.
        let charges = ExecutionEngine::calculate_indian_charges(
            100.0, 130.0, 15.0, 80.0, PositionSide::Buy,
        );
        // Entry buy turnover=1500, stamp=0.045 (charged here)
        // Exit sell turnover=1950 (no stamp)
        // Total exchange+sebi+gst+stamp ≈ 16.37
        assert!(charges > 14.0 && charges < 20.0, "got {}", charges);
    }

    #[test]
    fn test_indian_charges_scale_with_premium() {
        let small = ExecutionEngine::calculate_indian_charges(
            50.0, 40.0, 15.0, 80.0, PositionSide::Sell,
        );
        let large = ExecutionEngine::calculate_indian_charges(
            500.0, 400.0, 15.0, 80.0, PositionSide::Sell,
        );
        assert!(large > small, "charges should scale with premium turnover");
    }

    #[test]
    fn test_slippage_cost_none() {
        let cost = ExecutionEngine::calculate_slippage_cost(200.0, 150.0, &SlippageModel::None, 1.0, 1, 15);
        assert_eq!(cost, 0.0);
    }
}
