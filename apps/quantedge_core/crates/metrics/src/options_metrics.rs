//! Options-specific metrics engine.
//!
//! Computes premium capture, theta collected, IV analysis, DTE distribution,
//! and aggregate Greeks PnL. These metrics are unique to options backtesting
//! and complement the core risk/return metrics.

use serde::{Deserialize, Serialize};

/// Options-specific trade record with IV, DTE, and premium data.
#[derive(Debug, Clone)]
pub struct OptionsTradeRecord {
    pub pnl_net: f64,
    pub entry_iv: f64,        // decimal (e.g. 0.15)
    pub exit_iv: f64,
    pub dte_at_entry: f64,    // days
    pub entry_premium: f64,   // entry price × quantity (absolute value)
    pub exit_premium: f64,    // exit price × quantity (absolute value)
    pub is_sell: bool,
    pub delta_at_entry: f64,
    pub theta_pnl: f64,       // from attribution
    pub days_held: f64,
}

/// DTE distribution breakdown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DteDistribution {
    pub avg_dte: f64,
    pub min_dte: f64,
    pub max_dte: f64,
    /// % of trades entered with DTE < 3
    pub pct_below_3: f64,
    /// % of trades entered with 3 ≤ DTE ≤ 7
    pub pct_3_to_7: f64,
    /// % of trades entered with DTE > 7
    pub pct_above_7: f64,
}

/// Options-specific metrics result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OptionsMetrics {
    // ── Premium & Theta (3) ──
    /// (premium_collected - premium_returned) / premium_collected × 100
    pub premium_capture_pct: f64,
    /// Sum of theta_pnl for sell positions.
    pub total_theta_collected: f64,
    /// total_theta / total_days_held
    pub avg_theta_per_day: f64,

    // ── IV Analysis (4) ──
    pub avg_iv_at_entry: f64,
    pub avg_iv_at_exit: f64,
    /// (avg_iv_entry - avg_iv_exit) / avg_iv_entry × 100
    pub iv_crush_pct: f64,
    /// Average position delta across all trades.
    pub avg_net_delta: f64,

    // ── Distribution (1) ──
    pub dte_distribution: DteDistribution,

    // ── Greeks PnL aggregate (4) ──
    pub total_delta_pnl: f64,
    pub total_gamma_pnl: f64,
    pub total_theta_pnl: f64,
    pub total_vega_pnl: f64,
}

/// Options metrics engine.
pub struct OptionsMetricsEngine;

impl OptionsMetricsEngine {
    /// Compute options-specific metrics from trade records.
    pub fn compute(trades: &[OptionsTradeRecord]) -> OptionsMetrics {
        if trades.is_empty() {
            return OptionsMetrics::default();
        }

        let n = trades.len() as f64;
        let mut r = OptionsMetrics::default();

        // ── Premium capture (sell positions only) ──
        let sell_trades: Vec<&OptionsTradeRecord> = trades.iter().filter(|t| t.is_sell).collect();
        if !sell_trades.is_empty() {
            let total_entry_premium: f64 = sell_trades.iter().map(|t| t.entry_premium).sum();
            let total_exit_premium: f64 = sell_trades.iter().map(|t| t.exit_premium).sum();
            if total_entry_premium > 0.0 {
                r.premium_capture_pct =
                    ((total_entry_premium - total_exit_premium) / total_entry_premium) * 100.0;
            }
        }

        // ── Theta collected (sell positions) ──
        r.total_theta_collected = sell_trades.iter().map(|t| t.theta_pnl).sum();
        let total_sell_days: f64 = sell_trades.iter().map(|t| t.days_held).sum();
        r.avg_theta_per_day = if total_sell_days > 0.0 {
            r.total_theta_collected / total_sell_days
        } else {
            0.0
        };

        // ── IV analysis ──
        r.avg_iv_at_entry = trades.iter().map(|t| t.entry_iv).sum::<f64>() / n;
        r.avg_iv_at_exit = trades.iter().map(|t| t.exit_iv).sum::<f64>() / n;
        r.iv_crush_pct = if r.avg_iv_at_entry > 0.0 {
            ((r.avg_iv_at_entry - r.avg_iv_at_exit) / r.avg_iv_at_entry) * 100.0
        } else {
            0.0
        };
        r.avg_net_delta = trades.iter().map(|t| t.delta_at_entry).sum::<f64>() / n;

        // ── DTE distribution ──
        let dtes: Vec<f64> = trades.iter().map(|t| t.dte_at_entry).collect();
        let dte_sum: f64 = dtes.iter().sum();
        r.dte_distribution.avg_dte = dte_sum / n;
        r.dte_distribution.min_dte = dtes.iter().copied().fold(f64::INFINITY, f64::min);
        r.dte_distribution.max_dte = dtes.iter().copied().fold(0.0_f64, f64::max);

        let below_3 = dtes.iter().filter(|&&d| d < 3.0).count() as f64;
        let in_3_7 = dtes.iter().filter(|&&d| d >= 3.0 && d <= 7.0).count() as f64;
        let above_7 = dtes.iter().filter(|&&d| d > 7.0).count() as f64;
        r.dte_distribution.pct_below_3 = (below_3 / n) * 100.0;
        r.dte_distribution.pct_3_to_7 = (in_3_7 / n) * 100.0;
        r.dte_distribution.pct_above_7 = (above_7 / n) * 100.0;

        r
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sell_trade(entry_prem: f64, exit_prem: f64, entry_iv: f64, exit_iv: f64, dte: f64) -> OptionsTradeRecord {
        OptionsTradeRecord {
            pnl_net: entry_prem - exit_prem,
            entry_iv,
            exit_iv,
            dte_at_entry: dte,
            entry_premium: entry_prem,
            exit_premium: exit_prem,
            is_sell: true,
            delta_at_entry: 0.5,
            theta_pnl: 50.0,
            days_held: 1.0,
        }
    }

    #[test]
    fn test_premium_capture_full() {
        // Entry premium = 200, exit = 0 → 100% capture
        let trades = vec![make_sell_trade(200.0, 0.0, 0.15, 0.10, 5.0)];
        let r = OptionsMetricsEngine::compute(&trades);
        assert!((r.premium_capture_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_premium_capture_partial() {
        // Entry = 200, exit = 80 → 60% capture
        let trades = vec![make_sell_trade(200.0, 80.0, 0.15, 0.12, 5.0)];
        let r = OptionsMetricsEngine::compute(&trades);
        assert!((r.premium_capture_pct - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_iv_crush() {
        // IV drops from 0.20 to 0.12 → 40% crush
        let trades = vec![make_sell_trade(200.0, 100.0, 0.20, 0.12, 5.0)];
        let r = OptionsMetricsEngine::compute(&trades);
        assert!((r.iv_crush_pct - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_theta_collected_sell() {
        let mut trades = vec![];
        for _ in 0..5 {
            let mut t = make_sell_trade(200.0, 100.0, 0.15, 0.12, 5.0);
            t.theta_pnl = 50.0;
            t.days_held = 5.0;
            trades.push(t);
        }
        let r = OptionsMetricsEngine::compute(&trades);
        assert!((r.total_theta_collected - 250.0).abs() < 0.01); // 5 × 50
        assert!((r.avg_theta_per_day - 10.0).abs() < 0.01); // 250 / 25
    }

    #[test]
    fn test_dte_distribution_weekly() {
        let trades: Vec<OptionsTradeRecord> = (0..5)
            .map(|_| make_sell_trade(100.0, 50.0, 0.15, 0.12, 5.0))
            .collect();
        let r = OptionsMetricsEngine::compute(&trades);
        assert!((r.dte_distribution.pct_3_to_7 - 100.0).abs() < 0.01);
        assert!((r.dte_distribution.pct_below_3 - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_avg_net_delta() {
        let mut trades = vec![
            make_sell_trade(100.0, 50.0, 0.15, 0.12, 5.0),
            make_sell_trade(100.0, 50.0, 0.15, 0.12, 5.0),
        ];
        trades[0].delta_at_entry = 0.5;
        trades[1].delta_at_entry = -0.3;
        let r = OptionsMetricsEngine::compute(&trades);
        assert!((r.avg_net_delta - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_empty_options_trades() {
        let r = OptionsMetricsEngine::compute(&[]);
        assert!((r.premium_capture_pct - 0.0).abs() < 0.01);
        assert!((r.avg_iv_at_entry - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_dte_distribution_mixed() {
        let trades = vec![
            make_sell_trade(100.0, 50.0, 0.15, 0.12, 1.0),  // below 3
            make_sell_trade(100.0, 50.0, 0.15, 0.12, 5.0),  // 3-7
            make_sell_trade(100.0, 50.0, 0.15, 0.12, 14.0), // above 7
        ];
        let r = OptionsMetricsEngine::compute(&trades);
        assert!((r.dte_distribution.pct_below_3 - 33.333).abs() < 1.0);
        assert!((r.dte_distribution.pct_3_to_7 - 33.333).abs() < 1.0);
        assert!((r.dte_distribution.pct_above_7 - 33.333).abs() < 1.0);
    }
}
