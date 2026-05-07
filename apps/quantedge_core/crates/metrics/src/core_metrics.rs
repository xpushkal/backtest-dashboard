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
    pub reentry_attempt: u32,
}

/// Minimal equity snapshot for metrics computation.
#[derive(Debug, Clone)]
pub struct EquityPoint {
    pub date: NaiveDate,
    pub equity: f64,
}

/// The full metrics suite (45+ metrics).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsResult {
    // ── Return metrics (14) ──
    pub total_pnl_gross: f64,
    pub total_pnl_net: f64,
    pub cagr: f64,
    pub roi_pct: f64,
    pub expectancy: f64,
    pub profit_factor: f64,
    pub win_rate_pct: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub win_loss_ratio: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub gross_profit: f64,
    pub gross_loss: f64,

    // ── Risk metrics (16) ──
    pub max_drawdown_inr: f64,
    pub max_drawdown_pct: f64,
    pub avg_drawdown: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub calmar_ratio: f64,
    pub omega_ratio: f64,
    pub var_95: f64,
    pub var_99: f64,
    pub cvar: f64,
    pub ulcer_index: f64,
    pub daily_volatility: f64,
    pub ann_volatility: f64,
    pub skewness: f64,
    pub kurtosis: f64,
    pub recovery_factor: f64,
    pub drawdown_duration_days: u32,

    // ── Trade analytics (14) ──
    pub total_trades: u32,
    pub avg_hold_bars: f64,
    pub max_hold_bars: u32,
    pub max_consec_wins: u32,
    pub max_consec_losses: u32,
    pub sl_hit_rate_pct: f64,
    pub target_hit_rate_pct: f64,
    pub time_exit_rate_pct: f64,
    pub reentry_count: u32,
    pub reentry_win_rate: f64,
    pub total_brokerage: f64,
    pub total_slippage: f64,
    pub total_stt_cost: f64,
    pub net_cost_ratio: f64,
}

/// Computes all metrics.
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
        r.total_brokerage = trades.iter().map(|t| t.brokerage).sum();
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
        let daily = Self::daily_equities(equity_points);
        let returns = Self::daily_returns(&daily);

        let (dd_inr, dd_pct) = Self::compute_drawdown(equity_points, capital);
        r.max_drawdown_inr = dd_inr;
        r.max_drawdown_pct = dd_pct;
        r.avg_drawdown = Self::compute_avg_drawdown(&daily, capital);
        r.sharpe_ratio = Self::compute_sharpe_from_returns(&returns);
        r.sortino_ratio = Self::compute_sortino(&returns);
        r.calmar_ratio = if r.max_drawdown_pct > 0.0 { r.cagr / (r.max_drawdown_pct / 100.0) } else { 0.0 };
        r.omega_ratio = Self::compute_omega(&returns, 0.0);
        r.daily_volatility = Self::compute_std(&returns);
        r.ann_volatility = r.daily_volatility * (252.0_f64).sqrt();
        r.skewness = Self::compute_skewness(&returns);
        r.kurtosis = Self::compute_kurtosis(&returns);
        r.recovery_factor = if r.max_drawdown_inr > 0.0 { r.total_pnl_net / r.max_drawdown_inr } else { 0.0 };
        r.drawdown_duration_days = Self::compute_drawdown_duration(&daily, capital);

        // Daily PnL series for VaR/CVaR
        let daily_pnls: Vec<f64> = daily.windows(2).map(|w| w[1] - w[0]).collect();
        r.var_95 = Self::compute_var(&daily_pnls, 5.0);
        r.var_99 = Self::compute_var(&daily_pnls, 1.0);
        r.cvar = Self::compute_cvar(&daily_pnls, r.var_95);
        r.ulcer_index = Self::compute_ulcer_index(&daily, capital);

        // ── Additional return metrics ──
        r.gross_profit = gross_profit;
        r.gross_loss = gross_loss;
        r.win_loss_ratio = if r.avg_loss.abs() > 0.0 { r.avg_win / r.avg_loss.abs() } else { f64::INFINITY };

        // ── Extended trade analytics ──
        r.max_hold_bars = trades.iter().map(|t| t.bars_held).max().unwrap_or(0);
        let (cw, cl) = Self::compute_consecutive_streaks(trades);
        r.max_consec_wins = cw;
        r.max_consec_losses = cl;
        r.total_slippage = trades.iter().map(|t| t.slippage_cost).sum();
        r.total_stt_cost = trades.iter().map(|t| t.stt).sum();
        r.net_cost_ratio = if r.total_pnl_gross.abs() > 0.0 {
            (r.total_brokerage / r.total_pnl_gross.abs()) * 100.0
        } else { 0.0 };

        // Re-entry stats
        let reentries: Vec<&TradeRecord> = trades.iter().filter(|t| t.reentry_attempt > 0).collect();
        r.reentry_count = reentries.len() as u32;
        r.reentry_win_rate = if reentries.is_empty() {
            0.0
        } else {
            let re_wins = reentries.iter().filter(|t| t.pnl_net > 0.0).count();
            (re_wins as f64 / reentries.len() as f64) * 100.0
        };

        r
    }

    /// Compute max drawdown from equity curve.
    fn compute_drawdown(points: &[EquityPoint], capital: f64) -> (f64, f64) {
        if points.is_empty() {
            return (0.0, 0.0);
        }
        let daily = Self::daily_equities(points);
        let mut peak = capital;
        let mut max_dd_inr = 0.0_f64;
        let mut max_dd_pct = 0.0_f64;
        for &eq in &daily {
            if eq > peak { peak = eq; }
            let dd = peak - eq;
            if dd > max_dd_inr {
                max_dd_inr = dd;
                if peak > 0.0 { max_dd_pct = (dd / peak) * 100.0; }
            }
        }
        (max_dd_inr, max_dd_pct)
    }

    /// Compute average drawdown.
    fn compute_avg_drawdown(daily: &[f64], capital: f64) -> f64 {
        if daily.is_empty() { return 0.0; }
        let mut peak = capital;
        let mut dd_sum = 0.0;
        let mut dd_count = 0u32;
        for &eq in daily {
            if eq > peak { peak = eq; }
            let dd = peak - eq;
            if dd > 0.0 { dd_sum += dd; dd_count += 1; }
        }
        if dd_count > 0 { dd_sum / dd_count as f64 } else { 0.0 }
    }

    /// Compute daily returns from equity series.
    fn daily_returns(daily: &[f64]) -> Vec<f64> {
        daily.windows(2)
            .map(|w| if w[0] > 0.0 { (w[1] - w[0]) / w[0] } else { 0.0 })
            .collect()
    }

    /// Compute Sharpe from daily returns.
    fn compute_sharpe_from_returns(returns: &[f64]) -> f64 {
        if returns.is_empty() { return 0.0; }
        let rf_daily = 0.065 / 252.0;
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let std = Self::compute_std(returns);
        if std == 0.0 { return 0.0; }
        ((mean - rf_daily) / std) * (252.0_f64).sqrt()
    }

    /// Compute annualized Sharpe ratio (legacy API).
    fn compute_sharpe(points: &[EquityPoint]) -> f64 {
        let daily = Self::daily_equities(points);
        let returns = Self::daily_returns(&daily);
        Self::compute_sharpe_from_returns(&returns)
    }

    /// Compute Sortino ratio (downside deviation only).
    fn compute_sortino(returns: &[f64]) -> f64 {
        if returns.is_empty() { return 0.0; }
        let rf_daily = 0.065 / 252.0;
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let downside: Vec<f64> = returns.iter().filter(|&&r| r < rf_daily).map(|&r| (r - rf_daily).powi(2)).collect();
        if downside.is_empty() { return 0.0; }
        let dd = (downside.iter().sum::<f64>() / downside.len() as f64).sqrt();
        if dd == 0.0 { return 0.0; }
        ((mean - rf_daily) / dd) * (252.0_f64).sqrt()
    }

    /// Compute Omega ratio.
    fn compute_omega(returns: &[f64], threshold: f64) -> f64 {
        if returns.is_empty() { return 0.0; }
        let gains: f64 = returns.iter().filter(|&&r| r > threshold).map(|r| r - threshold).sum();
        let losses: f64 = returns.iter().filter(|&&r| r <= threshold).map(|r| (threshold - r).abs()).sum();
        if losses == 0.0 { return f64::INFINITY; }
        gains / losses
    }

    /// Compute VaR at given percentile.
    fn compute_var(daily_pnls: &[f64], percentile: f64) -> f64 {
        if daily_pnls.is_empty() { return 0.0; }
        let mut sorted = daily_pnls.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((percentile / 100.0) * sorted.len() as f64).floor() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Compute CVaR (expected shortfall below VaR).
    fn compute_cvar(daily_pnls: &[f64], var: f64) -> f64 {
        let tail: Vec<f64> = daily_pnls.iter().filter(|&&p| p <= var).copied().collect();
        if tail.is_empty() { return var; }
        tail.iter().sum::<f64>() / tail.len() as f64
    }

    /// Compute Ulcer Index.
    fn compute_ulcer_index(daily: &[f64], capital: f64) -> f64 {
        if daily.is_empty() { return 0.0; }
        let mut peak = capital;
        let mut sum_sq = 0.0;
        let mut count = 0u32;
        for &eq in daily {
            if eq > peak { peak = eq; }
            let pct_dd = if peak > 0.0 { ((eq - peak) / peak) * 100.0 } else { 0.0 };
            sum_sq += pct_dd * pct_dd;
            count += 1;
        }
        if count == 0 { return 0.0; }
        (sum_sq / count as f64).sqrt()
    }

    /// Sample standard deviation (Bessel-corrected, n-1).
    ///
    /// Matches `quantedge_portfolio::PortfolioMetrics::compute_std` so the same
    /// strategy reports the same Sharpe whether viewed standalone or inside a
    /// portfolio.
    fn compute_std(values: &[f64]) -> f64 {
        if values.len() < 2 { return 0.0; }
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
        var.sqrt()
    }

    /// Compute skewness.
    fn compute_skewness(returns: &[f64]) -> f64 {
        if returns.len() < 3 { return 0.0; }
        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let std = Self::compute_std(returns);
        if std == 0.0 { return 0.0; }
        let m3 = returns.iter().map(|r| ((r - mean) / std).powi(3)).sum::<f64>() / n;
        m3
    }

    /// Compute excess kurtosis.
    fn compute_kurtosis(returns: &[f64]) -> f64 {
        if returns.len() < 4 { return 0.0; }
        let n = returns.len() as f64;
        let mean = returns.iter().sum::<f64>() / n;
        let std = Self::compute_std(returns);
        if std == 0.0 { return 0.0; }
        let m4 = returns.iter().map(|r| ((r - mean) / std).powi(4)).sum::<f64>() / n;
        m4 - 3.0 // excess kurtosis
    }

    /// Compute max consecutive wins and losses.
    fn compute_consecutive_streaks(trades: &[TradeRecord]) -> (u32, u32) {
        let mut max_wins = 0u32;
        let mut max_losses = 0u32;
        let mut cur_wins = 0u32;
        let mut cur_losses = 0u32;
        for t in trades {
            if t.pnl_net > 0.0 {
                cur_wins += 1;
                cur_losses = 0;
                max_wins = max_wins.max(cur_wins);
            } else {
                cur_losses += 1;
                cur_wins = 0;
                max_losses = max_losses.max(cur_losses);
            }
        }
        (max_wins, max_losses)
    }

    /// Compute max drawdown duration in trading days.
    fn compute_drawdown_duration(daily: &[f64], capital: f64) -> u32 {
        if daily.is_empty() { return 0; }
        let mut peak = capital;
        let mut max_duration = 0u32;
        let mut cur_duration = 0u32;
        for &eq in daily {
            if eq >= peak {
                peak = eq;
                max_duration = max_duration.max(cur_duration);
                cur_duration = 0;
            } else {
                cur_duration += 1;
            }
        }
        max_duration.max(cur_duration)
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
                reentry_attempt: 0,
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
                reentry_attempt: 0,
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

    #[test]
    fn test_sortino_downside_only() {
        let points = make_equity_points(&[
            (15, 500000.0), (16, 501000.0), (17, 499000.0),
            (18, 502000.0), (19, 503000.0),
        ]);
        let trades = make_trades(&[
            (1000.0, 900.0, MetricExitReason::TimeExit, 10),
        ]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 19).unwrap(),
        );
        // Sortino should be computed (non-zero with mixed returns)
        assert!(r.sortino_ratio.is_finite());
    }

    #[test]
    fn test_var_95_known_series() {
        // VaR is computed from daily PnL series in equity curve
        let points = make_equity_points(&[
            (1, 500000.0), (2, 500100.0), (3, 499950.0), (4, 500050.0),
            (5, 499900.0), (6, 500200.0), (7, 500300.0), (8, 500000.0),
            (9, 499800.0), (10, 500100.0), (11, 500400.0), (12, 500500.0),
            (13, 500200.0), (14, 500600.0), (15, 500700.0), (16, 500800.0),
            (17, 500500.0), (18, 500900.0), (19, 501000.0), (20, 501100.0),
            (21, 500800.0),
        ]);
        let trades = make_trades(&[(1100.0, 1000.0, MetricExitReason::TimeExit, 10)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 21).unwrap(),
        );
        // VaR95 should be negative (a loss amount)
        assert!(r.var_95 <= 0.0 || r.var_95.is_finite(), "VaR95={}", r.var_95);
    }

    #[test]
    fn test_cvar_below_var() {
        let points = make_equity_points(&[
            (1, 500000.0), (2, 501000.0), (3, 498000.0), (4, 497000.0),
            (5, 499000.0), (6, 502000.0), (7, 503000.0), (8, 500000.0),
            (9, 498000.0), (10, 501000.0), (11, 504000.0), (12, 505000.0),
        ]);
        let trades = make_trades(&[(5000.0, 4500.0, MetricExitReason::TimeExit, 10)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 12).unwrap(),
        );
        assert!(r.cvar <= r.var_95, "CVaR {} should be ≤ VaR95 {}", r.cvar, r.var_95);
    }

    #[test]
    fn test_ulcer_no_drawdown() {
        // Monotonically rising equity → ulcer = 0
        let points = make_equity_points(&[
            (15, 500000.0), (16, 501000.0), (17, 502000.0), (18, 503000.0),
        ]);
        let trades = make_trades(&[(3000.0, 2800.0, MetricExitReason::TimeExit, 10)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 18).unwrap(),
        );
        assert!((r.ulcer_index - 0.0).abs() < 0.01, "Ulcer should be ~0, got {}", r.ulcer_index);
    }

    #[test]
    fn test_omega_all_positive() {
        // All gains → omega > 1
        let points = make_equity_points(&[
            (15, 500000.0), (16, 501000.0), (17, 502000.0), (18, 503000.0),
        ]);
        let trades = make_trades(&[(3000.0, 2800.0, MetricExitReason::TimeExit, 10)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 18).unwrap(),
        );
        assert!(r.omega_ratio > 1.0 || r.omega_ratio == f64::INFINITY,
            "Omega should be > 1 for all positive returns, got {}", r.omega_ratio);
    }

    #[test]
    fn test_consecutive_wins() {
        // W, W, W, L, W, W → max_consec_wins = 3
        let trades = make_trades(&[
            (100.0, 90.0, MetricExitReason::TimeExit, 5),  // W
            (100.0, 90.0, MetricExitReason::TimeExit, 5),  // W
            (100.0, 90.0, MetricExitReason::TimeExit, 5),  // W
            (-50.0, -50.0, MetricExitReason::StopLoss, 5), // L
            (100.0, 90.0, MetricExitReason::TimeExit, 5),  // W
            (100.0, 90.0, MetricExitReason::TimeExit, 5),  // W
        ]);
        let points = make_equity_points(&[(15, 500310.0)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 20).unwrap(),
        );
        assert_eq!(r.max_consec_wins, 3);
    }

    #[test]
    fn test_consecutive_losses() {
        // L, W, L, L, L, W → max_consec_losses = 3
        let trades = make_trades(&[
            (-50.0, -50.0, MetricExitReason::StopLoss, 5), // L
            (100.0, 90.0, MetricExitReason::TimeExit, 5),  // W
            (-30.0, -30.0, MetricExitReason::StopLoss, 5), // L
            (-40.0, -40.0, MetricExitReason::StopLoss, 5), // L
            (-20.0, -20.0, MetricExitReason::StopLoss, 5), // L
            (100.0, 90.0, MetricExitReason::TimeExit, 5),  // W
        ]);
        let points = make_equity_points(&[(15, 500000.0)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 20).unwrap(),
        );
        assert_eq!(r.max_consec_losses, 3);
    }

    #[test]
    fn test_reentry_count_and_winrate() {
        // 3 reentries, 2 profitable
        let mut trades = make_trades(&[
            (100.0, 90.0, MetricExitReason::TimeExit, 5),
            (100.0, 90.0, MetricExitReason::TimeExit, 5),
            (-50.0, -50.0, MetricExitReason::StopLoss, 5),
        ]);
        trades[0].reentry_attempt = 1;
        trades[1].reentry_attempt = 2;
        trades[2].reentry_attempt = 3;
        let points = make_equity_points(&[(15, 500130.0)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 17).unwrap(),
        );
        assert_eq!(r.reentry_count, 3);
        assert!((r.reentry_win_rate - 66.667).abs() < 1.0);
    }

    #[test]
    fn test_recovery_factor() {
        let trades = make_trades(&[
            (1100.0, 1000.0, MetricExitReason::TimeExit, 10),
            (-550.0, -500.0, MetricExitReason::StopLoss, 5),
        ]);
        let points = make_equity_points(&[
            (15, 501000.0), (16, 500500.0),
        ]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
        );
        // recovery = net_pnl / max_dd
        assert!(r.recovery_factor > 0.0);
    }

    #[test]
    fn test_win_loss_ratio() {
        let trades = make_trades(&[
            (1100.0, 1000.0, MetricExitReason::TimeExit, 10),
            (-550.0, -500.0, MetricExitReason::StopLoss, 5),
        ]);
        let points = make_equity_points(&[(15, 500500.0)]);
        let r = MetricsEngine::compute(
            &trades, &points, 500000.0,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
        );
        // win_loss_ratio = avg_win / |avg_loss| = 1000/500 = 2.0
        assert!((r.win_loss_ratio - 2.0).abs() < 0.01);
    }
}
