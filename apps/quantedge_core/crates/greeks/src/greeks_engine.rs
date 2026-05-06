//! Batch Greeks computation engine.
//!
//! Wraps the Black-Scholes pricer to compute Greeks for trade records
//! at entry and exit points. Used by PnL attribution (Phase 5.2).

use crate::black_scholes::{BsInputs, RISK_FREE_RATE_INDIA};

/// Greeks values for a single computation point.
#[derive(Debug, Clone, Default)]
pub struct TradeGreeks {
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
}

/// Batch Greeks computation engine.
pub struct GreeksEngine;

impl GreeksEngine {
    /// Compute Greeks at a point (entry or exit).
    ///
    /// # Arguments
    /// * `spot` - Underlying spot price
    /// * `strike` - Option strike price
    /// * `iv` - Implied volatility as decimal (e.g. 0.15)
    /// * `dte_days` - Days to expiry
    /// * `option_type` - "CE" or "PE"
    pub fn compute(
        spot: f64,
        strike: f64,
        iv: f64,
        dte_days: f64,
        option_type: &str,
    ) -> TradeGreeks {
        if iv <= 0.0 || dte_days <= 0.0 || spot <= 0.0 || strike <= 0.0 {
            return TradeGreeks::default();
        }

        let inputs = BsInputs {
            spot,
            strike,
            tte: dte_days / 365.25,
            iv,
            risk_free_rate: RISK_FREE_RATE_INDIA,
        };

        let out = match option_type {
            "CE" => inputs.price_ce(),
            "PE" => inputs.price_pe(),
            _ => return TradeGreeks::default(),
        };

        TradeGreeks {
            delta: out.delta,
            gamma: out.gamma,
            theta: out.theta,
            vega: out.vega,
        }
    }

    /// Compute Greeks at entry for a trade.
    pub fn compute_at_entry(
        spot: f64,
        strike: f64,
        iv: f64,
        dte_days: f64,
        option_type: &str,
    ) -> TradeGreeks {
        Self::compute(spot, strike, iv, dte_days, option_type)
    }

    /// Compute Greeks at exit for a trade.
    pub fn compute_at_exit(
        spot: f64,
        strike: f64,
        iv: f64,
        dte_days: f64,
        option_type: &str,
    ) -> TradeGreeks {
        Self::compute(spot, strike, iv, dte_days, option_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greeks_engine_ce() {
        let g = GreeksEngine::compute(48000.0, 48000.0, 0.15, 7.0, "CE");
        assert!(g.delta > 0.0 && g.delta < 1.0);
        assert!(g.gamma > 0.0);
        assert!(g.theta < 0.0);
        assert!(g.vega > 0.0);
    }

    #[test]
    fn test_greeks_engine_pe() {
        let g = GreeksEngine::compute(48000.0, 48000.0, 0.15, 7.0, "PE");
        assert!(g.delta < 0.0 && g.delta > -1.0);
        assert!(g.gamma > 0.0);
        assert!(g.vega > 0.0);
    }

    #[test]
    fn test_greeks_engine_invalid() {
        let g = GreeksEngine::compute(48000.0, 48000.0, 0.0, 7.0, "CE");
        assert_eq!(g.delta, 0.0);
        assert_eq!(g.gamma, 0.0);
    }

    #[test]
    fn test_greeks_engine_unknown_type() {
        let g = GreeksEngine::compute(48000.0, 48000.0, 0.15, 7.0, "XX");
        assert_eq!(g.delta, 0.0);
    }
}
