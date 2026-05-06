//! Black-Scholes option pricing engine.
//!
//! Computes European option prices and Greeks (delta, gamma, theta, vega)
//! using the standard Black-Scholes model. Uses `statrs` for normal
//! distribution CDF/PDF.

use statrs::distribution::{ContinuousCDF, Normal};

/// Risk-free rate for Indian markets (6.5%).
pub const RISK_FREE_RATE_INDIA: f64 = 0.065;

/// Inputs for Black-Scholes pricing.
#[derive(Debug, Clone)]
pub struct BsInputs {
    /// Current underlying spot price.
    pub spot: f64,
    /// Option strike price.
    pub strike: f64,
    /// Time to expiry in years (e.g. 7 days = 7.0 / 365.25).
    pub tte: f64,
    /// Implied volatility as decimal (e.g. 0.15 for 15%).
    pub iv: f64,
    /// Risk-free rate (default: 0.065 for India).
    pub risk_free_rate: f64,
}

/// Output of Black-Scholes pricing.
#[derive(Debug, Clone)]
pub struct BsOutput {
    /// Option theoretical price.
    pub price: f64,
    /// Delta: rate of change of price w.r.t. spot.
    pub delta: f64,
    /// Gamma: rate of change of delta w.r.t. spot.
    pub gamma: f64,
    /// Theta: time decay per calendar day (negative for long options).
    pub theta: f64,
    /// Vega: sensitivity to 1% change in IV.
    pub vega: f64,
}

impl BsInputs {
    /// Create inputs with India's risk-free rate.
    pub fn new(spot: f64, strike: f64, tte: f64, iv: f64) -> Self {
        Self {
            spot,
            strike,
            tte,
            iv,
            risk_free_rate: RISK_FREE_RATE_INDIA,
        }
    }

    /// Price a European Call option.
    pub fn price_ce(&self) -> BsOutput {
        if self.tte <= 0.0 {
            // At expiry: intrinsic value
            let intrinsic = (self.spot - self.strike).max(0.0);
            return BsOutput {
                price: intrinsic,
                delta: if self.spot > self.strike { 1.0 } else { 0.0 },
                gamma: 0.0,
                theta: 0.0,
                vega: 0.0,
            };
        }

        let (d1, d2) = self.d1_d2();
        let norm = Normal::new(0.0, 1.0).unwrap();
        let n_d1 = norm.cdf(d1);
        let n_d2 = norm.cdf(d2);
        let pdf_d1 = normal_pdf(d1);
        let sqrt_t = self.tte.sqrt();
        let discount = (-self.risk_free_rate * self.tte).exp();

        let price = self.spot * n_d1 - self.strike * discount * n_d2;
        let delta = n_d1;
        let gamma = pdf_d1 / (self.spot * self.iv * sqrt_t);
        let theta = (-(self.spot * pdf_d1 * self.iv) / (2.0 * sqrt_t)
            - self.risk_free_rate * self.strike * discount * n_d2)
            / 365.25;
        let vega = self.spot * pdf_d1 * sqrt_t / 100.0;

        BsOutput { price, delta, gamma, theta, vega }
    }

    /// Price a European Put option.
    pub fn price_pe(&self) -> BsOutput {
        if self.tte <= 0.0 {
            let intrinsic = (self.strike - self.spot).max(0.0);
            return BsOutput {
                price: intrinsic,
                delta: if self.spot < self.strike { -1.0 } else { 0.0 },
                gamma: 0.0,
                theta: 0.0,
                vega: 0.0,
            };
        }

        let (d1, d2) = self.d1_d2();
        let norm = Normal::new(0.0, 1.0).unwrap();
        let n_neg_d1 = norm.cdf(-d1);
        let n_neg_d2 = norm.cdf(-d2);
        let pdf_d1 = normal_pdf(d1);
        let sqrt_t = self.tte.sqrt();
        let discount = (-self.risk_free_rate * self.tte).exp();

        let price = self.strike * discount * n_neg_d2 - self.spot * n_neg_d1;
        let delta = norm.cdf(d1) - 1.0;
        let gamma = pdf_d1 / (self.spot * self.iv * sqrt_t);
        let theta = (-(self.spot * pdf_d1 * self.iv) / (2.0 * sqrt_t)
            + self.risk_free_rate * self.strike * discount * n_neg_d2)
            / 365.25;
        let vega = self.spot * pdf_d1 * sqrt_t / 100.0;

        BsOutput { price, delta, gamma, theta, vega }
    }

    /// Compute d1 and d2.
    fn d1_d2(&self) -> (f64, f64) {
        let sqrt_t = self.tte.sqrt();
        let d1 = ((self.spot / self.strike).ln()
            + (self.risk_free_rate + self.iv * self.iv / 2.0) * self.tte)
            / (self.iv * sqrt_t);
        let d2 = d1 - self.iv * sqrt_t;
        (d1, d2)
    }
}

/// Standard normal PDF: (1/√2π) × e^(-x²/2)
fn normal_pdf(x: f64) -> f64 {
    (-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Reference: S=48000, K=48000, T=7d, IV=15%, r=6.5%
    fn atm_inputs() -> BsInputs {
        BsInputs::new(48000.0, 48000.0, 7.0 / 365.25, 0.15)
    }

    #[test]
    fn test_bs_ce_price_reference() {
        let out = atm_inputs().price_ce();
        // ATM CE with 7 DTE, 15% IV for BankNifty spot=48000
        assert!(out.price > 100.0 && out.price < 600.0,
            "CE price {} out of expected range", out.price);
    }

    #[test]
    fn test_bs_pe_price_reference() {
        let out = atm_inputs().price_pe();
        // ATM PE should be similar to CE (put-call parity adjusted)
        assert!(out.price > 50.0 && out.price < 400.0,
            "PE price {} out of expected range", out.price);
    }

    #[test]
    fn test_bs_put_call_parity() {
        let inputs = atm_inputs();
        let ce = inputs.price_ce();
        let pe = inputs.price_pe();
        let discount = (-inputs.risk_free_rate * inputs.tte).exp();
        // C - P = S - K*e^(-rT)
        let lhs = ce.price - pe.price;
        let rhs = inputs.spot - inputs.strike * discount;
        let error_pct = ((lhs - rhs) / rhs).abs() * 100.0;
        assert!(error_pct < 0.01,
            "Put-call parity error: {:.6}% (C={:.4}, P={:.4}, expected diff={:.4})",
            error_pct, ce.price, pe.price, rhs);
    }

    #[test]
    fn test_bs_delta_ce_atm() {
        let out = atm_inputs().price_ce();
        // ATM CE delta should be slightly > 0.5 (positive r pushes it up)
        assert!(out.delta > 0.49 && out.delta < 0.55,
            "CE ATM delta {} out of range", out.delta);
    }

    #[test]
    fn test_bs_delta_pe_atm() {
        let out = atm_inputs().price_pe();
        // ATM PE delta ≈ -0.5
        assert!(out.delta > -0.55 && out.delta < -0.45,
            "PE ATM delta {} out of range", out.delta);
    }

    #[test]
    fn test_bs_gamma_symmetric() {
        let inputs = atm_inputs();
        let ce = inputs.price_ce();
        let pe = inputs.price_pe();
        // CE gamma == PE gamma (exact)
        assert!((ce.gamma - pe.gamma).abs() < 1e-12,
            "Gamma not symmetric: CE={}, PE={}", ce.gamma, pe.gamma);
    }

    #[test]
    fn test_bs_vega_symmetric() {
        let inputs = atm_inputs();
        let ce = inputs.price_ce();
        let pe = inputs.price_pe();
        assert!((ce.vega - pe.vega).abs() < 1e-12,
            "Vega not symmetric: CE={}, PE={}", ce.vega, pe.vega);
    }

    #[test]
    fn test_bs_theta_ce_negative() {
        let out = atm_inputs().price_ce();
        assert!(out.theta < 0.0, "CE theta should be negative, got {}", out.theta);
    }

    #[test]
    fn test_bs_zero_tte_intrinsic() {
        let inputs = BsInputs::new(48200.0, 48000.0, 0.0, 0.15);
        let ce = inputs.price_ce();
        assert!((ce.price - 200.0).abs() < 0.01, "At expiry CE should be intrinsic 200, got {}", ce.price);

        let pe = inputs.price_pe();
        assert!(pe.price.abs() < 0.01, "OTM PE at expiry should be 0, got {}", pe.price);
    }

    #[test]
    fn test_bs_deep_itm_delta() {
        let inputs = BsInputs::new(50000.0, 45000.0, 30.0 / 365.25, 0.15);
        let ce = inputs.price_ce();
        assert!(ce.delta > 0.98, "Deep ITM CE delta should be ~1.0, got {}", ce.delta);
    }

    #[test]
    fn test_bs_deep_otm_delta() {
        let inputs = BsInputs::new(45000.0, 50000.0, 7.0 / 365.25, 0.15);
        let ce = inputs.price_ce();
        assert!(ce.delta < 0.01, "Deep OTM CE delta should be ~0.0, got {}", ce.delta);
    }

    #[test]
    fn test_put_call_parity_multiple_strikes() {
        // Test parity across multiple (S, K) combos
        let combos = vec![
            (48000.0, 47500.0), (48000.0, 48000.0), (48000.0, 48500.0),
            (50000.0, 49000.0), (46000.0, 47000.0),
        ];
        for (spot, strike) in combos {
            let inputs = BsInputs::new(spot, strike, 14.0 / 365.25, 0.20);
            let ce = inputs.price_ce();
            let pe = inputs.price_pe();
            let discount = (-inputs.risk_free_rate * inputs.tte).exp();
            let lhs = ce.price - pe.price;
            let rhs = spot - strike * discount;
            let error = (lhs - rhs).abs();
            assert!(error < 0.01,
                "Parity failed S={}, K={}: |{:.6} - {:.6}| = {:.6}",
                spot, strike, lhs, rhs, error);
        }
    }
}
