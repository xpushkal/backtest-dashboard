//! Stop-loss type evaluation module.
//!
//! Centralizes all 7 SL type checks. The Leg delegates to this
//! module for SL and target evaluation.

use crate::config::SlType;

/// Context for SL/target evaluation — all the market state needed.
#[derive(Debug, Clone)]
pub struct SlContext {
    pub entry_price: f64,
    pub current_price: f64,
    pub entry_spot: f64,
    pub current_spot: f64,
    pub quantity: f64,       // lots × lot_size
    pub lots: u32,
    pub lot_size: u32,
    pub direction: f64,      // +1 buy, -1 sell
    pub unrealized_pnl: f64,
}

/// Check if a stop-loss is triggered given the SL type, value, and context.
pub fn is_sl_triggered(sl_type: &SlType, sl_value: f64, ctx: &SlContext) -> bool {
    match sl_type {
        SlType::None => false,

        SlType::Points => {
            let price_move = (ctx.current_price - ctx.entry_price) * -ctx.direction;
            price_move >= sl_value
        }

        SlType::PercentOfPremium => {
            let loss_pct = if ctx.entry_price > 0.0 {
                (-ctx.unrealized_pnl / (ctx.entry_price * ctx.quantity)) * 100.0
            } else {
                0.0
            };
            loss_pct >= sl_value
        }

        SlType::PercentOfMargin => {
            // Simplified SPAN: max(3 × premium × quantity, factor × spot × quantity × 0.12)
            // factor = 0.20 default (average across BankNifty/Nifty/Sensex)
            let premium_margin = 3.0 * ctx.entry_price * ctx.quantity;
            let span_margin = 0.20 * ctx.entry_spot * ctx.quantity * 0.12;
            let margin = premium_margin.max(span_margin);
            let loss_pct = if margin > 0.0 {
                (-ctx.unrealized_pnl / margin) * 100.0
            } else {
                0.0
            };
            loss_pct >= sl_value
        }

        SlType::IndexPoints => {
            let spot_move = (ctx.current_spot - ctx.entry_spot) * -ctx.direction;
            spot_move >= sl_value
        }

        SlType::DeltaBreach => {
            // Stub: requires Greeks engine (Phase 5)
            false
        }

        SlType::CombinedPremium => {
            // Handled at strategy/position level by CombinedSlMonitor
            false
        }
    }
}

/// Check if a target is triggered.
pub fn is_target_triggered(target_type: &SlType, target_value: f64, ctx: &SlContext) -> bool {
    match target_type {
        SlType::PercentOfPremium => {
            let profit_pct = if ctx.entry_price > 0.0 {
                (ctx.unrealized_pnl / (ctx.entry_price * ctx.quantity)) * 100.0
            } else {
                0.0
            };
            profit_pct >= target_value
        }
        SlType::Points => {
            let price_move = (ctx.current_price - ctx.entry_price) * ctx.direction;
            price_move >= target_value
        }
        _ => false,
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(entry: f64, current: f64, dir: f64, spot_entry: f64, spot_current: f64) -> SlContext {
        let lots = 1_u32;
        let lot_size = 15_u32;
        let qty = (lots * lot_size) as f64;
        SlContext {
            entry_price: entry,
            current_price: current,
            entry_spot: spot_entry,
            current_spot: spot_current,
            quantity: qty,
            lots,
            lot_size,
            direction: dir,
            unrealized_pnl: (current - entry) * dir * qty,
        }
    }

    #[test]
    fn test_points_sl_triggered() {
        // Sell at 200, price rises to 260 → 60pt move, SL=50 → triggered
        let ctx = make_ctx(200.0, 260.0, -1.0, 48000.0, 48060.0);
        assert!(is_sl_triggered(&SlType::Points, 50.0, &ctx));
    }

    #[test]
    fn test_points_sl_not_triggered() {
        let ctx = make_ctx(200.0, 240.0, -1.0, 48000.0, 48040.0);
        assert!(!is_sl_triggered(&SlType::Points, 50.0, &ctx));
    }

    #[test]
    fn test_percent_premium_triggered() {
        // Sell at 200, rises to 400 → 100% premium loss
        let ctx = make_ctx(200.0, 400.0, -1.0, 48000.0, 48200.0);
        assert!(is_sl_triggered(&SlType::PercentOfPremium, 100.0, &ctx));
    }

    #[test]
    fn test_percent_premium_not_triggered() {
        let ctx = make_ctx(200.0, 350.0, -1.0, 48000.0, 48150.0);
        assert!(!is_sl_triggered(&SlType::PercentOfPremium, 100.0, &ctx));
    }

    #[test]
    fn test_percent_margin_triggered() {
        // Sell at 200, lots=1, lot_size=15, qty=15
        // Premium margin = 3 × 200 × 15 = 9000
        // SPAN margin = 0.20 × 48000 × 15 × 0.12 = 17280
        // Margin = max(9000, 17280) = 17280
        // PnL = (400-200) * (-1) * 15 = -3000
        // Loss% = 3000/17280 * 100 = 17.36%
        let ctx = make_ctx(200.0, 400.0, -1.0, 48000.0, 48200.0);
        assert!(is_sl_triggered(&SlType::PercentOfMargin, 15.0, &ctx)); // 17.36% > 15%
        assert!(!is_sl_triggered(&SlType::PercentOfMargin, 20.0, &ctx)); // 17.36% < 20%
    }

    #[test]
    fn test_index_points_triggered() {
        // Sell, spot 48000→48500 → 500pt move against sell, SL=400 → triggered
        let ctx = make_ctx(200.0, 250.0, -1.0, 48000.0, 48500.0);
        assert!(is_sl_triggered(&SlType::IndexPoints, 400.0, &ctx));
    }

    #[test]
    fn test_index_points_not_triggered() {
        let ctx = make_ctx(200.0, 220.0, -1.0, 48000.0, 48300.0);
        assert!(!is_sl_triggered(&SlType::IndexPoints, 400.0, &ctx));
    }

    #[test]
    fn test_delta_breach_stub() {
        let ctx = make_ctx(200.0, 400.0, -1.0, 48000.0, 48500.0);
        assert!(!is_sl_triggered(&SlType::DeltaBreach, 0.5, &ctx));
    }

    #[test]
    fn test_combined_premium_stub_per_leg() {
        let ctx = make_ctx(200.0, 400.0, -1.0, 48000.0, 48500.0);
        assert!(!is_sl_triggered(&SlType::CombinedPremium, 5000.0, &ctx));
    }

    #[test]
    fn test_target_percent_premium() {
        // Sell at 200, drops to 100 → 50% of premium captured
        let ctx = make_ctx(200.0, 100.0, -1.0, 48000.0, 47900.0);
        assert!(is_target_triggered(&SlType::PercentOfPremium, 50.0, &ctx));
    }

    #[test]
    fn test_target_points() {
        // Sell at 200, drops to 150 → 50pt profit move
        let ctx = make_ctx(200.0, 150.0, -1.0, 48000.0, 47950.0);
        assert!(is_target_triggered(&SlType::Points, 50.0, &ctx));
    }
}
