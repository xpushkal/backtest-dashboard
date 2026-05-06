//! Monte Carlo simulation engine.
//!
//! Shuffles trade PnL sequences to produce percentile equity bands
//! and probability statistics. Uses Fisher-Yates shuffle for O(N)
//! per simulation.

use serde::{Deserialize, Serialize};

/// Monte Carlo simulation configuration.
#[derive(Debug, Clone)]
pub struct MonteCarloConfig {
    /// Number of simulations to run (default: 1000).
    pub n_simulations: u32,
    /// Optional seed for reproducibility.
    pub seed: Option<u64>,
}

impl Default for MonteCarloConfig {
    fn default() -> Self {
        Self {
            n_simulations: 1000,
            seed: None,
        }
    }
}

/// Percentile equity at a given trade index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercentileBand {
    pub trade_index: u32,
    pub p5: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p95: f64,
}

/// Monte Carlo simulation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResult {
    /// Percentile bands at each trade index.
    pub equity_bands: Vec<PercentileBand>,
    /// Final equity distribution percentiles.
    pub final_p5: f64,
    pub final_p25: f64,
    pub final_p50: f64,
    pub final_p75: f64,
    pub final_p95: f64,
    /// Max drawdown distribution.
    pub mdd_p50: f64,
    pub mdd_p95: f64,
    /// Probability statistics.
    pub prob_positive_return: f64,
    pub prob_mdd_gt_10pct: f64,
    pub prob_mdd_gt_20pct: f64,
    /// Metadata.
    pub n_simulations: u32,
    pub n_trades: u32,
}

/// Monte Carlo simulation engine.
pub struct MonteCarloEngine;

impl MonteCarloEngine {
    /// Run Monte Carlo simulation by shuffling trade PnL sequence.
    ///
    /// 1. For each sim: shuffle PnLs, build equity curve, record final equity + max DD
    /// 2. Compute percentile bands across all sims at each trade index
    /// 3. Compute probability statistics from distributions
    pub fn simulate(
        trade_pnls: &[f64],
        capital: f64,
        config: &MonteCarloConfig,
    ) -> MonteCarloResult {
        let n_trades = trade_pnls.len();
        let n_sims = config.n_simulations as usize;

        if n_trades == 0 {
            return MonteCarloResult {
                equity_bands: vec![],
                final_p5: capital, final_p25: capital, final_p50: capital,
                final_p75: capital, final_p95: capital,
                mdd_p50: 0.0, mdd_p95: 0.0,
                prob_positive_return: 0.0, prob_mdd_gt_10pct: 0.0, prob_mdd_gt_20pct: 0.0,
                n_simulations: config.n_simulations, n_trades: 0,
            };
        }

        // Pre-allocate: equity curves per sim at each trade index
        // Matrix: [trade_index][sim_index]
        let mut equity_at: Vec<Vec<f64>> = vec![Vec::with_capacity(n_sims); n_trades];
        let mut final_equities: Vec<f64> = Vec::with_capacity(n_sims);
        let mut max_dds: Vec<f64> = Vec::with_capacity(n_sims);

        // Seed-based deterministic PRNG (simple LCG for speed)
        let mut rng_state: u64 = config.seed.unwrap_or(42);

        let mut shuffled = trade_pnls.to_vec();

        for _ in 0..n_sims {
            // Fisher-Yates shuffle
            for i in (1..n_trades).rev() {
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let j = (rng_state >> 33) as usize % (i + 1);
                shuffled.swap(i, j);
            }

            // Build equity curve and track max DD
            let mut equity = capital;
            let mut peak = capital;
            let mut max_dd = 0.0_f64;

            for (idx, &pnl) in shuffled.iter().enumerate() {
                equity += pnl;
                if equity > peak { peak = equity; }
                let dd_pct = if peak > 0.0 { (peak - equity) / peak * 100.0 } else { 0.0 };
                if dd_pct > max_dd { max_dd = dd_pct; }
                equity_at[idx].push(equity);
            }

            final_equities.push(equity);
            max_dds.push(max_dd);
        }

        // Compute percentile bands
        let mut equity_bands: Vec<PercentileBand> = Vec::with_capacity(n_trades);
        for idx in 0..n_trades {
            equity_at[idx].sort_by(|a, b| a.partial_cmp(b).unwrap());
            equity_bands.push(PercentileBand {
                trade_index: idx as u32,
                p5: Self::percentile(&equity_at[idx], 5.0),
                p25: Self::percentile(&equity_at[idx], 25.0),
                p50: Self::percentile(&equity_at[idx], 50.0),
                p75: Self::percentile(&equity_at[idx], 75.0),
                p95: Self::percentile(&equity_at[idx], 95.0),
            });
        }

        // Final equity percentiles
        final_equities.sort_by(|a, b| a.partial_cmp(b).unwrap());
        max_dds.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let prob_positive = final_equities.iter().filter(|&&e| e > capital).count() as f64 / n_sims as f64;
        let prob_mdd_10 = max_dds.iter().filter(|&&d| d > 10.0).count() as f64 / n_sims as f64;
        let prob_mdd_20 = max_dds.iter().filter(|&&d| d > 20.0).count() as f64 / n_sims as f64;

        MonteCarloResult {
            equity_bands,
            final_p5: Self::percentile(&final_equities, 5.0),
            final_p25: Self::percentile(&final_equities, 25.0),
            final_p50: Self::percentile(&final_equities, 50.0),
            final_p75: Self::percentile(&final_equities, 75.0),
            final_p95: Self::percentile(&final_equities, 95.0),
            mdd_p50: Self::percentile(&max_dds, 50.0),
            mdd_p95: Self::percentile(&max_dds, 95.0),
            prob_positive_return: prob_positive,
            prob_mdd_gt_10pct: prob_mdd_10,
            prob_mdd_gt_20pct: prob_mdd_20,
            n_simulations: config.n_simulations,
            n_trades: n_trades as u32,
        }
    }

    /// Compute percentile from sorted array.
    fn percentile(sorted: &[f64], p: f64) -> f64 {
        if sorted.is_empty() { return 0.0; }
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_config(n: u32) -> MonteCarloConfig {
        MonteCarloConfig { n_simulations: n, seed: Some(12345) }
    }

    #[test]
    fn test_1000_simulations() {
        let pnls: Vec<f64> = (0..100).map(|i| if i % 3 == 0 { -100.0 } else { 150.0 }).collect();
        let r = MonteCarloEngine::simulate(&pnls, 500000.0, &seeded_config(1000));
        assert_eq!(r.n_simulations, 1000);
        assert_eq!(r.n_trades, 100);
    }

    #[test]
    fn test_deterministic_seed() {
        let pnls = vec![100.0, -50.0, 200.0, -100.0, 150.0];
        let r1 = MonteCarloEngine::simulate(&pnls, 100000.0, &seeded_config(100));
        let r2 = MonteCarloEngine::simulate(&pnls, 100000.0, &seeded_config(100));
        assert_eq!(r1.final_p50, r2.final_p50);
        assert_eq!(r1.mdd_p95, r2.mdd_p95);
    }

    #[test]
    fn test_positive_only_pnl() {
        let pnls = vec![100.0; 50];
        let r = MonteCarloEngine::simulate(&pnls, 100000.0, &seeded_config(500));
        assert!((r.prob_positive_return - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_negative_only_pnl() {
        let pnls = vec![-100.0; 50];
        let r = MonteCarloEngine::simulate(&pnls, 100000.0, &seeded_config(500));
        assert!((r.prob_positive_return - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_percentile_ordering() {
        let pnls: Vec<f64> = (0..100).map(|i| if i % 2 == 0 { 200.0 } else { -100.0 }).collect();
        let r = MonteCarloEngine::simulate(&pnls, 500000.0, &seeded_config(500));
        assert!(r.final_p5 <= r.final_p25);
        assert!(r.final_p25 <= r.final_p50);
        assert!(r.final_p50 <= r.final_p75);
        assert!(r.final_p75 <= r.final_p95);
    }

    #[test]
    fn test_mdd_percentile_ordering() {
        let pnls: Vec<f64> = (0..100).map(|i| if i % 3 == 0 { -500.0 } else { 200.0 }).collect();
        let r = MonteCarloEngine::simulate(&pnls, 500000.0, &seeded_config(500));
        assert!(r.mdd_p50 <= r.mdd_p95);
    }

    #[test]
    fn test_single_trade() {
        let pnls = vec![100.0];
        let r = MonteCarloEngine::simulate(&pnls, 100000.0, &seeded_config(100));
        assert_eq!(r.equity_bands.len(), 1);
        // All percentiles should be the same (only one outcome)
        assert!((r.final_p5 - r.final_p95).abs() < 0.01);
    }

    #[test]
    fn test_equity_bands_length() {
        let pnls = vec![100.0, -50.0, 200.0, -100.0, 150.0];
        let r = MonteCarloEngine::simulate(&pnls, 100000.0, &seeded_config(100));
        assert_eq!(r.equity_bands.len(), 5);
    }

    #[test]
    fn test_zero_variance_pnl() {
        let pnls = vec![100.0; 20];
        let r = MonteCarloEngine::simulate(&pnls, 100000.0, &seeded_config(100));
        // All same PnL → all percentiles equal
        assert!((r.final_p5 - r.final_p95).abs() < 0.01);
    }

    #[test]
    fn test_prob_mdd_thresholds() {
        let pnls: Vec<f64> = (0..200).map(|i| if i % 2 == 0 { 500.0 } else { -300.0 }).collect();
        let r = MonteCarloEngine::simulate(&pnls, 500000.0, &seeded_config(500));
        assert!(r.prob_mdd_gt_10pct >= r.prob_mdd_gt_20pct,
            "P(MDD>10%) {} should be ≥ P(MDD>20%) {}", r.prob_mdd_gt_10pct, r.prob_mdd_gt_20pct);
    }

    #[test]
    fn test_empty_trades() {
        let r = MonteCarloEngine::simulate(&[], 100000.0, &MonteCarloConfig::default());
        assert_eq!(r.n_trades, 0);
        assert!(r.equity_bands.is_empty());
    }
}
