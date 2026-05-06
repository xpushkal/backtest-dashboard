//! Walk-forward analysis engine.
//!
//! Slices existing trade results into in-sample/out-of-sample windows
//! and computes per-window Sharpe ratios to assess strategy robustness.
//! Does NOT re-run simulations — only recomputes metrics per time slice.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

/// Walk-forward configuration.
#[derive(Debug, Clone)]
pub struct WalkForwardConfig {
    /// In-sample period in months (default: 6).
    pub is_months: u32,
    /// Out-of-sample period in months (default: 2).
    pub oos_months: u32,
    /// Slide step in months (default: 2).
    pub slide_months: u32,
}

impl Default for WalkForwardConfig {
    fn default() -> Self {
        Self {
            is_months: 6,
            oos_months: 2,
            slide_months: 2,
        }
    }
}

/// One walk-forward window result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfWindow {
    pub window_id: u32,
    pub is_start: NaiveDate,
    pub is_end: NaiveDate,
    pub oos_start: NaiveDate,
    pub oos_end: NaiveDate,
    pub is_sharpe: f64,
    pub oos_sharpe: f64,
    /// OOS/IS Sharpe ratio (< 1 indicates degradation).
    pub degradation_ratio: f64,
    pub is_trades: u32,
    pub oos_trades: u32,
    pub is_pnl: f64,
    pub oos_pnl: f64,
}

/// Walk-forward summary result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardResult {
    pub windows: Vec<WfWindow>,
    /// Mean of degradation ratios across all windows.
    pub avg_degradation: f64,
    /// % of windows where OOS Sharpe > 0.
    pub pct_positive_oos: f64,
    /// avg_degradation × pct_positive_oos / 100 — composite robustness score.
    pub robustness_score: f64,
}

/// Minimal trade data for walk-forward analysis.
#[derive(Debug, Clone)]
pub struct WfTradeRecord {
    pub pnl_net: f64,
    pub exit_date: NaiveDate,
}

/// Minimal equity data for walk-forward analysis.
#[derive(Debug, Clone)]
pub struct WfEquityPoint {
    pub date: NaiveDate,
    pub equity: f64,
}

pub struct WalkForwardEngine;

impl WalkForwardEngine {
    /// Run walk-forward analysis.
    pub fn analyze(
        trades: &[WfTradeRecord],
        equity: &[WfEquityPoint],
        _capital: f64,
        data_start: NaiveDate,
        data_end: NaiveDate,
        config: &WalkForwardConfig,
    ) -> WalkForwardResult {
        let windows_dates = Self::generate_windows(data_start, data_end, config);

        if windows_dates.is_empty() {
            return WalkForwardResult {
                windows: vec![],
                avg_degradation: 0.0,
                pct_positive_oos: 0.0,
                robustness_score: 0.0,
            };
        }

        let mut windows: Vec<WfWindow> = Vec::new();

        for (id, (is_start, is_end, oos_start, oos_end)) in windows_dates.iter().enumerate() {
            let is_trades: Vec<&WfTradeRecord> = trades.iter()
                .filter(|t| t.exit_date >= *is_start && t.exit_date < *is_end)
                .collect();
            let oos_trades: Vec<&WfTradeRecord> = trades.iter()
                .filter(|t| t.exit_date >= *oos_start && t.exit_date < *oos_end)
                .collect();

            let is_equity: Vec<&WfEquityPoint> = equity.iter()
                .filter(|e| e.date >= *is_start && e.date < *is_end)
                .collect();
            let oos_equity: Vec<&WfEquityPoint> = equity.iter()
                .filter(|e| e.date >= *oos_start && e.date < *oos_end)
                .collect();

            let is_sharpe = Self::compute_sharpe_from_equity(&is_equity);
            let oos_sharpe = Self::compute_sharpe_from_equity(&oos_equity);

            let degradation = if is_sharpe.abs() > 0.001 {
                oos_sharpe / is_sharpe
            } else {
                0.0
            };

            let is_pnl: f64 = is_trades.iter().map(|t| t.pnl_net).sum();
            let oos_pnl: f64 = oos_trades.iter().map(|t| t.pnl_net).sum();

            windows.push(WfWindow {
                window_id: id as u32,
                is_start: *is_start,
                is_end: *is_end,
                oos_start: *oos_start,
                oos_end: *oos_end,
                is_sharpe,
                oos_sharpe,
                degradation_ratio: degradation,
                is_trades: is_trades.len() as u32,
                oos_trades: oos_trades.len() as u32,
                is_pnl,
                oos_pnl,
            });
        }

        let n = windows.len() as f64;
        let avg_degradation = windows.iter().map(|w| w.degradation_ratio).sum::<f64>() / n;
        let positive_oos = windows.iter().filter(|w| w.oos_sharpe > 0.0).count() as f64;
        let pct_positive_oos = (positive_oos / n) * 100.0;
        let robustness_score = avg_degradation * (pct_positive_oos / 100.0);

        WalkForwardResult {
            windows,
            avg_degradation,
            pct_positive_oos,
            robustness_score,
        }
    }

    /// Generate window date ranges.
    fn generate_windows(
        start: NaiveDate,
        end: NaiveDate,
        config: &WalkForwardConfig,
    ) -> Vec<(NaiveDate, NaiveDate, NaiveDate, NaiveDate)> {
        let mut windows = Vec::new();
        let mut cursor = start;

        loop {
            let is_start = cursor;
            let is_end = Self::add_months(is_start, config.is_months);
            let oos_start = is_end;
            let oos_end = Self::add_months(oos_start, config.oos_months);

            if oos_end > end {
                break;
            }

            windows.push((is_start, is_end, oos_start, oos_end));
            cursor = Self::add_months(cursor, config.slide_months);
        }

        windows
    }

    /// Add months to a date.
    fn add_months(date: NaiveDate, months: u32) -> NaiveDate {
        let total_months = date.month0() + months;
        let new_year = date.year() + (total_months / 12) as i32;
        let new_month = (total_months % 12) + 1;
        let day = date.day().min(28); // Safe for all months
        NaiveDate::from_ymd_opt(new_year, new_month, day).unwrap()
    }

    /// Compute Sharpe from equity points (6.5% risk-free, India).
    fn compute_sharpe_from_equity(points: &[&WfEquityPoint]) -> f64 {
        if points.len() < 2 { return 0.0; }

        let equities: Vec<f64> = points.iter().map(|p| p.equity).collect();
        let returns: Vec<f64> = equities.windows(2)
            .map(|w| if w[0] > 0.0 { (w[1] - w[0]) / w[0] } else { 0.0 })
            .collect();

        if returns.is_empty() { return 0.0; }

        let rf_daily = 0.065 / 252.0;
        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
        let std = var.sqrt();

        if std == 0.0 { return 0.0; }
        ((mean - rf_daily) / std) * (252.0_f64).sqrt()
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_4yr_trades() -> Vec<WfTradeRecord> {
        // Generate trades every 3 days for 4 years
        let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        (0..480).map(|i| {
            let date = start + chrono::Duration::days(i * 3);
            WfTradeRecord {
                pnl_net: if i % 3 == 0 { -100.0 } else { 150.0 },
                exit_date: date,
            }
        }).collect()
    }

    fn make_4yr_equity() -> Vec<WfEquityPoint> {
        let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let mut equity = 500000.0;
        (0..1460).map(|i| {
            let date = start + chrono::Duration::days(i);
            equity += if i % 3 == 0 { -50.0 } else { 100.0 };
            WfEquityPoint { date, equity }
        }).collect()
    }

    #[test]
    fn test_window_generation_4yr() {
        let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let config = WalkForwardConfig::default();
        let windows = WalkForwardEngine::generate_windows(start, end, &config);
        // 4 years = 48 months; IS=6, OOS=2, slide=2 → ~21 windows
        assert!(windows.len() >= 10 && windows.len() <= 25,
            "Expected 10-25 windows, got {}", windows.len());
    }

    #[test]
    fn test_windows_non_overlapping() {
        let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let config = WalkForwardConfig::default();
        let windows = WalkForwardEngine::generate_windows(start, end, &config);
        // IS and OOS within same window must be contiguous
        for (is_start, is_end, oos_start, oos_end) in &windows {
            assert_eq!(*is_end, *oos_start,
                "IS end ({}) must equal OOS start ({})", is_end, oos_start);
            assert!(*is_start < *is_end);
            assert!(*oos_start < *oos_end);
        }
    }

    #[test]
    fn test_is_oos_contiguous() {
        let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let windows = WalkForwardEngine::generate_windows(start, end, &WalkForwardConfig::default());
        for w in &windows {
            assert_eq!(w.1, w.2, "IS end must equal OOS start");
        }
    }

    #[test]
    fn test_degradation_ratio() {
        let trades = make_4yr_trades();
        let equity = make_4yr_equity();
        let r = WalkForwardEngine::analyze(
            &trades, &equity, 500000.0,
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            &WalkForwardConfig::default(),
        );
        assert!(!r.windows.is_empty());
        for w in &r.windows {
            assert!(w.degradation_ratio.is_finite());
        }
    }

    #[test]
    fn test_empty_oos_handled() {
        // Short data window that produces windows but no OOS trades
        let trades = vec![WfTradeRecord {
            pnl_net: 100.0,
            exit_date: NaiveDate::from_ymd_opt(2020, 3, 1).unwrap(),
        }];
        let equity = vec![
            WfEquityPoint { date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), equity: 500000.0 },
            WfEquityPoint { date: NaiveDate::from_ymd_opt(2020, 12, 1).unwrap(), equity: 500100.0 },
        ];
        let r = WalkForwardEngine::analyze(
            &trades, &equity, 500000.0,
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            &WalkForwardConfig::default(),
        );
        // Should handle gracefully
        for w in &r.windows {
            assert!(w.oos_sharpe.is_finite());
        }
    }

    #[test]
    fn test_short_data() {
        // 3 months of data → 0 windows (IS=6 + OOS=2 = 8 months needed)
        let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2020, 4, 1).unwrap();
        let r = WalkForwardEngine::analyze(
            &[], &[], 500000.0, start, end, &WalkForwardConfig::default(),
        );
        assert!(r.windows.is_empty());
    }

    #[test]
    fn test_robustness_score_range() {
        let trades = make_4yr_trades();
        let equity = make_4yr_equity();
        let r = WalkForwardEngine::analyze(
            &trades, &equity, 500000.0,
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            &WalkForwardConfig::default(),
        );
        assert!(r.robustness_score.is_finite());
        assert!(r.pct_positive_oos >= 0.0 && r.pct_positive_oos <= 100.0);
    }

    #[test]
    fn test_custom_config() {
        let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let config = WalkForwardConfig { is_months: 3, oos_months: 1, slide_months: 1 };
        let windows = WalkForwardEngine::generate_windows(start, end, &config);
        // More frequent windows with smaller config
        assert!(windows.len() > 20, "Expected >20 windows with small config, got {}", windows.len());
    }
}
