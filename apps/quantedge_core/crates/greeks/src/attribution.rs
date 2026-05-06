//! Greeks PnL attribution.
//!
//! Decomposes each trade's PnL into delta, gamma, theta, and vega
//! components using first-order Greeks at entry. The residual is
//! captured as "unexplained" to account for higher-order effects
//! and discrete time approximation.

use crate::black_scholes::{BsInputs, RISK_FREE_RATE_INDIA};
use serde::{Deserialize, Serialize};

/// Per-trade PnL attribution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnlAttribution {
    pub delta_pnl: f64,
    pub gamma_pnl: f64,
    pub theta_pnl: f64,
    pub vega_pnl: f64,
    pub unexplained: f64,
    pub total_attributed: f64,
    pub actual_pnl: f64,
    /// |unexplained| / |actual| × 100
    pub attribution_error_pct: f64,
}

/// Inputs for PnL attribution computation.
#[derive(Debug, Clone)]
pub struct AttributionInputs {
    pub entry_spot: f64,
    pub exit_spot: f64,
    pub entry_iv: f64,       // decimal (e.g. 0.15)
    pub exit_iv: f64,        // decimal
    pub entry_strike: f64,
    pub dte_at_entry: f64,   // days
    pub days_held: f64,
    pub option_type: String, // "CE" or "PE"
    pub lot_size: u32,
    pub lots: u32,
    pub actual_pnl: f64,
    /// "buy" or "sell"
    pub position_side: String,
}

impl PnlAttribution {
    /// Compute PnL attribution from entry Greeks and market moves.
    ///
    /// Formulas (from PRD):
    /// - delta_pnl = delta × (exit_spot - entry_spot) × quantity
    /// - gamma_pnl = 0.5 × gamma × (spot_move)² × quantity
    /// - theta_pnl = theta × days_held (theta is per-day)
    /// - vega_pnl  = vega × (exit_iv - entry_iv) × 100 × quantity
    /// - unexplained = actual_pnl - (delta + gamma + theta + vega)
    pub fn compute(inputs: &AttributionInputs) -> Self {
        let quantity = inputs.lot_size as f64 * inputs.lots as f64;
        let spot_move = inputs.exit_spot - inputs.entry_spot;
        let iv_change = inputs.exit_iv - inputs.entry_iv;

        // Compute Greeks at entry
        let bs = BsInputs {
            spot: inputs.entry_spot,
            strike: inputs.entry_strike,
            tte: inputs.dte_at_entry / 365.25,
            iv: inputs.entry_iv,
            risk_free_rate: RISK_FREE_RATE_INDIA,
        };

        let out = match inputs.option_type.as_str() {
            "CE" => bs.price_ce(),
            "PE" => bs.price_pe(),
            _ => return Self::zero(inputs.actual_pnl),
        };

        // Position sign: +1 for buy, -1 for sell
        let sign = if inputs.position_side == "buy" { 1.0 } else { -1.0 };

        let delta_pnl = sign * out.delta * spot_move * quantity;
        let gamma_pnl = sign * 0.5 * out.gamma * spot_move * spot_move * quantity;
        // theta is per-day, already divided by 365.25 in BsOutput
        let theta_pnl = sign * out.theta * inputs.days_held * quantity;
        // vega is per 1% IV move, iv_change is decimal so multiply by 100
        let vega_pnl = sign * out.vega * iv_change * 100.0 * quantity;

        let total_attributed = delta_pnl + gamma_pnl + theta_pnl + vega_pnl;
        let unexplained = inputs.actual_pnl - total_attributed;
        let error_pct = if inputs.actual_pnl.abs() > 0.001 {
            (unexplained.abs() / inputs.actual_pnl.abs()) * 100.0
        } else {
            0.0
        };

        Self {
            delta_pnl,
            gamma_pnl,
            theta_pnl,
            vega_pnl,
            unexplained,
            total_attributed,
            actual_pnl: inputs.actual_pnl,
            attribution_error_pct: error_pct,
        }
    }

    fn zero(actual_pnl: f64) -> Self {
        Self {
            delta_pnl: 0.0, gamma_pnl: 0.0, theta_pnl: 0.0, vega_pnl: 0.0,
            unexplained: actual_pnl, total_attributed: 0.0,
            actual_pnl, attribution_error_pct: 100.0,
        }
    }
}

/// Batch attribution for a vector of trades.
pub fn attribute_all(inputs: &[AttributionInputs]) -> Vec<PnlAttribution> {
    inputs.iter().map(PnlAttribution::compute).collect()
}

/// Aggregate attribution summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttributionSummary {
    pub total_delta_pnl: f64,
    pub total_gamma_pnl: f64,
    pub total_theta_pnl: f64,
    pub total_vega_pnl: f64,
    pub total_unexplained: f64,
    pub avg_attribution_error_pct: f64,
}

impl AttributionSummary {
    pub fn from_attributions(attrs: &[PnlAttribution]) -> Self {
        if attrs.is_empty() {
            return Self::default();
        }
        let n = attrs.len() as f64;
        Self {
            total_delta_pnl: attrs.iter().map(|a| a.delta_pnl).sum(),
            total_gamma_pnl: attrs.iter().map(|a| a.gamma_pnl).sum(),
            total_theta_pnl: attrs.iter().map(|a| a.theta_pnl).sum(),
            total_vega_pnl: attrs.iter().map(|a| a.vega_pnl).sum(),
            total_unexplained: attrs.iter().map(|a| a.unexplained).sum(),
            avg_attribution_error_pct: attrs.iter().map(|a| a.attribution_error_pct).sum::<f64>() / n,
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ce_buy_inputs(spot_move: f64, iv_change: f64, pnl: f64) -> AttributionInputs {
        AttributionInputs {
            entry_spot: 48000.0,
            exit_spot: 48000.0 + spot_move,
            entry_iv: 0.15,
            exit_iv: 0.15 + iv_change,
            entry_strike: 48000.0,
            dte_at_entry: 7.0,
            days_held: 1.0,
            option_type: "CE".to_string(),
            lot_size: 15,
            lots: 1,
            actual_pnl: pnl,
            position_side: "buy".to_string(),
        }
    }

    #[test]
    fn test_delta_pnl_ce_buy() {
        // Long CE, spot up 200 → positive delta PnL
        let attr = PnlAttribution::compute(&ce_buy_inputs(200.0, 0.0, 1500.0));
        assert!(attr.delta_pnl > 0.0, "Delta PnL should be positive for long CE + spot up, got {}", attr.delta_pnl);
    }

    #[test]
    fn test_delta_pnl_pe_sell() {
        // Short PE, spot UP → positive for sell PE (PE loses value)
        // PE delta is negative, sell sign flips it, spot up is positive move
        // sell(-1) × delta(negative) × spot_move(positive) = positive
        let inputs = AttributionInputs {
            entry_spot: 48000.0,
            exit_spot: 48200.0,  // spot UP
            entry_iv: 0.15,
            exit_iv: 0.15,
            entry_strike: 48000.0,
            dte_at_entry: 7.0,
            days_held: 1.0,
            option_type: "PE".to_string(),
            lot_size: 15,
            lots: 1,
            actual_pnl: 1000.0,
            position_side: "sell".to_string(),
        };
        let attr = PnlAttribution::compute(&inputs);
        assert!(attr.delta_pnl > 0.0, "Delta PnL should be positive, got {}", attr.delta_pnl);
    }

    #[test]
    fn test_gamma_pnl_atm() {
        let attr = PnlAttribution::compute(&ce_buy_inputs(500.0, 0.0, 5000.0));
        // Gamma PnL = 0.5 × gamma × ΔS² × qty — should be positive for long
        assert!(attr.gamma_pnl > 0.0, "Gamma PnL should be positive for long, got {}", attr.gamma_pnl);
    }

    #[test]
    fn test_theta_pnl_sell() {
        // Short CE: theta decay collected (positive PnL from time)
        let inputs = AttributionInputs {
            entry_spot: 48000.0,
            exit_spot: 48000.0,
            entry_iv: 0.15,
            exit_iv: 0.15,
            entry_strike: 48000.0,
            dte_at_entry: 7.0,
            days_held: 1.0,
            option_type: "CE".to_string(),
            lot_size: 15,
            lots: 1,
            actual_pnl: 50.0,
            position_side: "sell".to_string(),
        };
        let attr = PnlAttribution::compute(&inputs);
        // sell × negative theta = positive theta_pnl
        assert!(attr.theta_pnl > 0.0, "Theta PnL should be positive for sell, got {}", attr.theta_pnl);
    }

    #[test]
    fn test_vega_pnl_iv_crush() {
        // Short CE, IV drops → positive vega PnL for sell
        let inputs = AttributionInputs {
            entry_spot: 48000.0,
            exit_spot: 48000.0,
            entry_iv: 0.20,
            exit_iv: 0.15,
            entry_strike: 48000.0,
            dte_at_entry: 7.0,
            days_held: 1.0,
            option_type: "CE".to_string(),
            lot_size: 15,
            lots: 1,
            actual_pnl: 200.0,
            position_side: "sell".to_string(),
        };
        let attr = PnlAttribution::compute(&inputs);
        // sell × positive vega × negative IV change = positive
        assert!(attr.vega_pnl > 0.0, "Vega PnL should be positive for sell+IV crush, got {}", attr.vega_pnl);
    }

    #[test]
    fn test_attribution_components_sum() {
        let attr = PnlAttribution::compute(&ce_buy_inputs(100.0, 0.0, 800.0));
        let sum = attr.delta_pnl + attr.gamma_pnl + attr.theta_pnl + attr.vega_pnl + attr.unexplained;
        assert!((sum - attr.actual_pnl).abs() < 0.01,
            "Components ({:.2}) + unexplained ({:.2}) should equal actual ({:.2})",
            attr.total_attributed, attr.unexplained, attr.actual_pnl);
    }

    #[test]
    fn test_batch_attribution() {
        let inputs = vec![
            ce_buy_inputs(100.0, 0.0, 800.0),
            ce_buy_inputs(-200.0, 0.01, -1500.0),
            ce_buy_inputs(50.0, -0.02, 300.0),
        ];
        let attrs = attribute_all(&inputs);
        assert_eq!(attrs.len(), 3);
        let summary = AttributionSummary::from_attributions(&attrs);
        assert!(summary.avg_attribution_error_pct.is_finite());
    }

    #[test]
    fn test_unknown_option_type() {
        let mut inputs = ce_buy_inputs(100.0, 0.0, 500.0);
        inputs.option_type = "XX".to_string();
        let attr = PnlAttribution::compute(&inputs);
        assert!((attr.attribution_error_pct - 100.0).abs() < 0.01);
    }
}
