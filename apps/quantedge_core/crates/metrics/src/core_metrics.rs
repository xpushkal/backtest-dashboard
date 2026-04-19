//! Core metrics engine — 20 metrics for Phase 2.
//!
//! Computes return, risk, and trade analytics metrics from
//! closed trades and equity snapshots.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Exit reason imported from core crate (kept as string for decoupling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricExitReason {
    StopLoss,
    Target,
    TimeExit,
    EndOfData,
}

/// Minimal trade info needed for metrics computation.
/// Decoupled from core::ClosedTrade to avoid circular dependency.
#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub pnl_gross: f64,
    pub pnl_net: f64,
    pub brokerage: f64,
    pub stt: f64,
    pub slippage_cost: f64,
    pub exit_reason: MetricExitReason,
    pub bars_held: u32,
    pub exit_date: NaiveDate,
}

/// Minimal equity snapshot for metrics computation.
#[derive(Debug, Clone)]
pub struct EquityPoint {
    pub date: NaiveDate,
    pub equity: f64,
}

/// The 20 core metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsResult {
    // ── Return metrics (11) ──
    pub total_pnl_gross: f64,
    pub total_pnl_net: f64,
    pub cagr: f64,
    pub roi_pct: f64,
    pub expectancy: f64,
    pub profit_factor: f64,
    pub win_rate_pct: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub largest_win: f64,
    pub largest_loss: f64,

    // ── Risk metrics (3) ──
    pub max_drawdown_inr: f64,
    pub max_drawdown_pct: f64,
    pub sharpe_ratio: f64,

    // ── Trade analytics (6) ──
    pub total_trades: u32,
    pub avg_hold_bars: f64,
    pub sl_hit_rate_pct: f64,
    pub target_hit_rate_pct: f64,
    pub time_exit_rate_pct: f64,
    pub total_brokerage: f64,
}

/// Computes all 20 core metrics.
pub struct MetricsEngine;

impl MetricsEngine {
    /// Compute metrics from trade records and equity snapshots.
    pub fn compute(
        trades: &[TradeRecord],
        equity_points: &[EquityPoint],
        capital: f64,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> MetricsResult {
        if trades.is_empty() {
            return MetricsResult::default();
        }

        let n = trades.len() as f64;
        let mut r = MetricsResult::default();

        // ── Return metrics ──
        r.total_trades = trades.len() as u32;
        r.total_pnl_gross = trades.iter().map(|t| t.pnl_gross).sum();
        r.total_pnl_net = trades.iter().map(|t| t.pnl_net).sum();
        r.total_brokerage = trades.iter().map(|t| t.brokerage + t.stt + t.slippage_cost).sum();
        r.roi_pct = (r.total_pnl_net / capital) * 100.0;
        r.expectancy = r.total_pnl_net / n;

        let wins: Vec<f64> = trades.iter().filter(|t| t.pnl_net > 0.0).map(|t| t.pnl_net).collect();
        let losses: Vec<f64> = trades.iter().filter(|t| t.pnl_net <= 0.0).map(|t| t.pnl_net).collect();

        r.win_rate_pct = (wins.len() as f64 / n) * 100.0;
        r.avg_win = if wins.is_empty() { 0.0 } else { wins.iter().sum::<f64>() / wins.len() as f64 };
        r.avg_loss = if losses.is_empty() { 0.0 } else { losses.iter().sum::<f64>() / losses.len() as f64 };
        r.largest_win = wins.iter().copied().fold(0.0_f64, f64::max);
        r.largest_loss = losses.iter().copied().fold(0.0_f64, f64::min);

        let gross_profit: f64 = wins.iter().sum();
        let gross_loss: f64 = losses.iter().map(|l| l.abs()).sum();
        r.profit_factor = if gross_loss > 0.0 { gross_profit / gross_loss } else { f64::INFINITY };

        // CAGR
        let days = (end_date - start_date).num_days().max(1) as f64;
        let years = days / 365.25;
        let final_equity = capital + r.total_pnl_net;
        r.cagr = if years > 0.0 && final_equity > 0.0 {
            (final_equity / capital).powf(1.0 / years) - 1.0
        } else {
            0.0
        };

        // ── Trade analytics ──
        r.avg_hold_bars = trades.iter().map(|t| t.bars_held as f64).sum::<f64>() / n;

        let sl_count = trades.iter().filter(|t| t.exit_reason == MetricExitReason::StopLoss).count();
        let tgt_count = trades.iter().filter(|t| t.exit_reason == MetricExitReason::Target).count();
        let time_count = trades.iter().filter(|t| t.exit_reason == MetricExitReason::TimeExit).count();

        r.sl_hit_rate_pct = (sl_count as f64 / n) * 100.0;
        r.target_hit_rate_pct = (tgt_count as f64 / n) * 100.0;
        r.time_exit_rate_pct = (time_count as f64 / n) * 100.0;

        // ── Risk metrics from equity curve ──
        let (dd_inr, dd_pct) = Self::compute_drawdown(equity_points, capital);
        r.max_drawdown_inr = dd_inr;
        r.max_drawdown_pct = dd_pct;
        r.sharpe_ratio = Self::compute_sharpe(equity_points);

        r
    }

    /// Compute max drawdown from equity curve.
    fn compute_drawdown(points: &[EquityPoint], capital: f64) -> (f64, f64) {
        if points.is_empty() {
            return (0.0, 0.0);
        }

        // Get end-of-day equity (last point per date)
        let daily = Self::daily_equities(points);

        let mut peak = capital;
        let mut max_dd_inr = 0.0_f64;
        let mut max_dd_pct = 0.0_f64;

        for &eq in &daily {
            if eq > peak {
                peak = eq;
            }
            let dd = peak - eq;
            if dd > max_dd_inr {
                max_dd_inr = dd;
                if peak > 0.0 {
                    max_dd_pct = (dd / peak) * 100.0;
                }
            }
        }
        (max_dd_inr, max_dd_pct)
    }

    /// Compute annualized Sharpe ratio.
    /// Uses 6.5% risk-free rate (India).
    fn compute_sharpe(points: &[EquityPoint]) -> f64 {
        let daily = Self::daily_equities(points);
        if daily.len() < 2 {
            return 0.0;
        }

        let rf_daily = 0.065 / 252.0;

        let returns: Vec<f64> = daily
            .windows(2)
            .map(|w| if w[0] > 0.0 { (w[1] - w[0]) / w[0] } else { 0.0 })
            .collect();

        if returns.is_empty() {
            return 0.0;
        }

        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
        let std = variance.sqrt();

        if std == 0.0 {
            return 0.0;
        }

        ((mean - rf_daily) / std) * (252.0_f64).sqrt()
    }

    /// Extract end-of-day equities from time-stamped points.
    fn daily_equities(points: &[EquityPoint]) -> Vec<f64> {
        let mut by_date: BTreeMap<NaiveDate, f64> = BTreeMap::new();
        for pt in points {
            by_date.insert(pt.date, pt.equity);
        }
        by_date.values().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trades(specs: &[(f64, f64, MetricExitReason, u32)]) -> Vec<TradeRecord> {
        specs
            .iter()
            .enumerate()
            .map(|(i, &(gross, net, ref reason, bars))| TradeRecord {
                pnl_gross: gross,
                pnl_net: net,
                brokerage: (gross - net).abs() * 0.7,
                stt: (gross - net).abs() * 0.1,
                slippage_cost: (gross - net).abs() * 0.2,
                exit_reason: *reason,
                bars_held: bars,
                exit_date: NaiveDate::from_ymd_opt(2024, 1, 15 + i as u32).unwrap(),
            })
            .collect()
    }

    fn make_equity_points(equities: &[(i32, f64)]) -> Vec<EquityPoint> {
        equities
            .iter()
            .map(|&(day, eq)| EquityPoint {
                date: NaiveDate::from_ymd_opt(2024, 1, day as u32).unwrap(),
                equity: eq,
            })
            .collect()
    }

    #[test]
    fn test_basic_return_metrics() {
        let trades = make_trades(&[
            (1100.0, 1000.0, MetricExitReason::TimeExit, 10),
            (-550.0, -500.0, MetricExitReason::StopLoss, 5),
            (880.0, 800.0, MetricExitReason::TimeExit, 12),
            (-330.0, -300.0, MetricExitReason::StopLoss, 3),
            (660.0, 600.0, MetricExitReason::Target, 8),
        ]);
        let points = make_equity_points(&[
            (15, 501000.0), (16, 500500.0), (17, 501300.0), (18, 501000.0), (19, 501600.0),
        ]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 19).unwrap(),
        );
        assert_eq!(r.total_trades, 5);
        assert!((r.total_pnl_net - 1600.0).abs() < 0.01);
        assert!((r.win_rate_pct - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_profit_factor() {
        let trades = make_trades(&[
            (2500.0, 2400.0, MetricExitReason::TimeExit, 10),
            (-850.0, -800.0, MetricExitReason::StopLoss, 5),
        ]);
        let points = make_equity_points(&[(15, 502400.0), (16, 501600.0)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
        );
        assert!((r.profit_factor - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_expectancy() {
        let trades = make_trades(&[
            (1100.0, 1000.0, MetricExitReason::TimeExit, 10),
            (-550.0, -500.0, MetricExitReason::StopLoss, 5),
            (880.0, 800.0, MetricExitReason::TimeExit, 12),
            (-330.0, -300.0, MetricExitReason::StopLoss, 3),
            (660.0, 600.0, MetricExitReason::Target, 8),
        ]);
        let points = make_equity_points(&[(15, 501600.0)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 19).unwrap(),
        );
        assert!((r.expectancy - 320.0).abs() < 0.01);
    }

    #[test]
    fn test_exit_rate_breakdown() {
        let trades = make_trades(&[
            (100.0, 90.0, MetricExitReason::StopLoss, 10),
            (100.0, 90.0, MetricExitReason::StopLoss, 10),
            (100.0, 90.0, MetricExitReason::StopLoss, 10),
            (100.0, 90.0, MetricExitReason::Target, 10),
            (100.0, 90.0, MetricExitReason::TimeExit, 10),
        ]);
        let points = make_equity_points(&[(15, 500450.0)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 19).unwrap(),
        );
        assert!((r.sl_hit_rate_pct - 60.0).abs() < 0.01);
        assert!((r.target_hit_rate_pct - 20.0).abs() < 0.01);
        assert!((r.time_exit_rate_pct - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_drawdown_simple() {
        let points = make_equity_points(&[
            (15, 500000.0), (16, 510000.0), (17, 490000.0), (18, 520000.0),
        ]);
        let r = MetricsEngine::compute(
            &[TradeRecord {
                pnl_gross: 20000.0, pnl_net: 20000.0, brokerage: 0.0, stt: 0.0,
                slippage_cost: 0.0, exit_reason: MetricExitReason::TimeExit,
                bars_held: 10, exit_date: NaiveDate::from_ymd_opt(2024, 1, 18).unwrap(),
            }],
            &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 18).unwrap(),
        );
        // Peak at 510k, trough at 490k → DD = 20k
        assert!((r.max_drawdown_inr - 20000.0).abs() < 0.01);
        // DD% = 20000/510000 * 100 ≈ 3.92%
        assert!((r.max_drawdown_pct - (20000.0 / 510000.0 * 100.0)).abs() < 0.01);
    }

    #[test]
    fn test_zero_trades() {
        let r = MetricsEngine::compute(
            &[], &[], 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 19).unwrap(),
        );
        assert_eq!(r.total_trades, 0);
        assert_eq!(r.total_pnl_net, 0.0);
        assert_eq!(r.win_rate_pct, 0.0);
        assert_eq!(r.sharpe_ratio, 0.0);
    }

    #[test]
    fn test_all_wins() {
        let trades = make_trades(&[
            (1100.0, 1000.0, MetricExitReason::TimeExit, 10),
            (550.0, 500.0, MetricExitReason::TimeExit, 10),
            (330.0, 300.0, MetricExitReason::TimeExit, 10),
        ]);
        let points = make_equity_points(&[(15, 501800.0)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 17).unwrap(),
        );
        assert!((r.win_rate_pct - 100.0).abs() < 0.01);
        assert!(r.profit_factor == f64::INFINITY);
        assert_eq!(r.avg_loss, 0.0);
    }

    #[test]
    fn test_cagr_calculation() {
        // Capital=500k, PnL=100k over ~1 year → CAGR ≈ 20%
        let trades = make_trades(&[
            (110000.0, 100000.0, MetricExitReason::TimeExit, 100),
        ]);
        let points = make_equity_points(&[(1, 600000.0)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        );
        // CAGR = (600k/500k)^(1/~1yr) - 1 ≈ 0.20
        assert!((r.cagr - 0.2).abs() < 0.02);
    }
}
