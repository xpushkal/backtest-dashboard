//! Portfolio-level metrics computation (MET-05).
//!
//! 7 portfolio metrics: portfolio_sharpe, peak_margin, capital_efficiency,
//! diversification_benefit, avg_correlation, avg_concurrent_trades, portfolio_sortino.

use crate::correlation::CorrelationMatrix;
use crate::engine::{PortfolioResult, StrategyResult};
use crate::margin::PortfolioMarginTracker;
use serde::{Deserialize, Serialize};

const RISK_FREE_RATE: f64 = 0.065; // 6.5% for India
const TRADING_DAYS_PER_YEAR: f64 = 252.0;

/// Portfolio-level metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioMetrics {
    pub portfolio_sharpe: f64,
    pub portfolio_sortino: f64,
    pub portfolio_max_drawdown_pct: f64,
    pub portfolio_total_pnl: f64,
    pub portfolio_cagr: f64,
    pub peak_margin_used: f64,
    pub capital_efficiency: f64,
    pub avg_concurrent_trades: f64,
    pub diversification_benefit: f64,
    pub avg_correlation: f64,
    pub total_margin_skips: usize,
    pub avg_margin_utilization: f64,
}

impl PortfolioMetrics {
    /// Compute portfolio-level metrics from results, correlation, and margin tracking.
    pub fn compute(
        portfolio: &PortfolioResult,
        correlation: &CorrelationMatrix,
        margin_tracker: &PortfolioMarginTracker,
        total_capital: f64,
    ) -> Self {
        let daily_returns = &portfolio.combined_daily_returns;

        // Portfolio Sharpe
        let portfolio_sharpe = compute_sharpe(daily_returns);

        // Portfolio Sortino
        let portfolio_sortino = compute_sortino(daily_returns);

        // Portfolio total PnL
        let portfolio_total_pnl: f64 = portfolio
            .strategy_results
            .iter()
            .map(|sr| sr.metrics.total_pnl_net)
            .sum();

        // Portfolio max drawdown
        let portfolio_max_drawdown_pct = compute_max_drawdown(&portfolio.combined_equity);

        // CAGR
        let portfolio_cagr = compute_cagr(&portfolio.combined_equity, total_capital);

        // Margin metrics
        let peak_margin_used = margin_tracker.peak_margin();
        let capital_efficiency = if peak_margin_used > 0.0 {
            portfolio_total_pnl / peak_margin_used
        } else {
            0.0
        };

        // Diversification benefit = 1 - (σ_portfolio / Σ(w_i * σ_i))
        let diversification_benefit =
            compute_diversification_benefit(&portfolio.strategy_results, daily_returns);

        // Average correlation
        let avg_correlation = correlation.avg_correlation();

        // Average concurrent trades
        let avg_concurrent_trades =
            compute_avg_concurrent(&portfolio.strategy_results);

        // Margin utilization (average over all dates)
        let avg_margin_utilization = if total_capital > 0.0 {
            peak_margin_used / total_capital
        } else {
            0.0
        };

        Self {
            portfolio_sharpe,
            portfolio_sortino,
            portfolio_max_drawdown_pct,
            portfolio_total_pnl,
            portfolio_cagr,
            peak_margin_used,
            capital_efficiency,
            avg_concurrent_trades,
            diversification_benefit,
            avg_correlation,
            total_margin_skips: margin_tracker.skips().len(),
            avg_margin_utilization,
        }
    }
}

/// Annualized Sharpe ratio using India's 6.5% risk-free rate.
fn compute_sharpe(daily_returns: &[f64]) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }

    let mean: f64 = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;
    let variance: f64 = daily_returns
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>()
        / (daily_returns.len() - 1) as f64;
    let std_dev = variance.sqrt();

    if std_dev < 1e-12 {
        return 0.0;
    }

    let ann_return = mean * TRADING_DAYS_PER_YEAR;
    let ann_std = std_dev * TRADING_DAYS_PER_YEAR.sqrt();

    (ann_return - RISK_FREE_RATE) / ann_std
}

/// Annualized Sortino ratio (downside deviation only).
fn compute_sortino(daily_returns: &[f64]) -> f64 {
    if daily_returns.len() < 2 {
        return 0.0;
    }

    let mean: f64 = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;
    let downside_variance: f64 = daily_returns
        .iter()
        .filter(|&&r| r < 0.0)
        .map(|r| r.powi(2))
        .sum::<f64>()
        / daily_returns.len() as f64;
    let downside_std = downside_variance.sqrt();

    if downside_std < 1e-12 {
        return 0.0;
    }

    let ann_return = mean * TRADING_DAYS_PER_YEAR;
    let ann_downside = downside_std * TRADING_DAYS_PER_YEAR.sqrt();

    (ann_return - RISK_FREE_RATE) / ann_downside
}

/// Max drawdown percentage from equity curve.
fn compute_max_drawdown(equity: &[quantedge_metrics::EquityPoint]) -> f64 {
    if equity.is_empty() {
        return 0.0;
    }

    let mut peak = equity[0].equity;
    let mut max_dd_pct = 0.0;

    for ep in equity {
        if ep.equity > peak {
            peak = ep.equity;
        }
        if peak > 0.0 {
            let dd = (peak - ep.equity) / peak * 100.0;
            if dd > max_dd_pct {
                max_dd_pct = dd;
            }
        }
    }

    max_dd_pct
}

/// CAGR from equity curve.
fn compute_cagr(
    equity: &[quantedge_metrics::EquityPoint],
    initial_capital: f64,
) -> f64 {
    if equity.len() < 2 || initial_capital <= 0.0 {
        return 0.0;
    }

    let final_equity = equity.last().unwrap().equity;
    let first_date = equity.first().unwrap().date;
    let last_date = equity.last().unwrap().date;
    let days = (last_date - first_date).num_days() as f64;
    let years = days / 365.25;

    if years < 0.01 {
        return 0.0;
    }

    ((final_equity / initial_capital).powf(1.0 / years) - 1.0) * 100.0
}

/// Diversification benefit: 1 - (σ_portfolio / Σ(w_i × σ_i)).
fn compute_diversification_benefit(
    strategy_results: &[StrategyResult],
    portfolio_daily_returns: &[f64],
) -> f64 {
    if strategy_results.is_empty() || portfolio_daily_returns.len() < 2 {
        return 0.0;
    }

    // Portfolio annualized volatility
    let port_vol = annualized_volatility(portfolio_daily_returns);

    // Weighted sum of individual strategy volatilities
    let total_alloc: f64 = strategy_results.iter().map(|s| s.allocation_pct).sum();
    let weighted_vol_sum: f64 = strategy_results
        .iter()
        .map(|sr| {
            let weight = sr.allocation_pct / total_alloc;
            let vol = annualized_volatility(&sr.daily_pnls);
            weight * vol
        })
        .sum();

    if weighted_vol_sum < 1e-12 {
        return 0.0;
    }

    (1.0 - port_vol / weighted_vol_sum).max(0.0)
}

/// Annualized volatility from daily returns/PnLs.
fn annualized_volatility(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }

    let mean: f64 = data.iter().sum::<f64>() / data.len() as f64;
    let variance: f64 = data
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>()
        / (data.len() - 1) as f64;

    variance.sqrt() * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Average number of concurrent open positions across strategies.
fn compute_avg_concurrent(strategy_results: &[StrategyResult]) -> f64 {
    if strategy_results.is_empty() {
        return 0.0;
    }

    // Simple approximation: average number of strategies that had trades
    let total_bars: f64 = strategy_results
        .iter()
        .map(|sr| sr.run_result.total_bars as f64)
        .sum::<f64>()
        / strategy_results.len() as f64;

    if total_bars < 1.0 {
        return 0.0;
    }

    let total_bars_held: f64 = strategy_results
        .iter()
        .flat_map(|sr| sr.run_result.trades.iter())
        .map(|t| t.bars_held as f64)
        .sum();

    total_bars_held / total_bars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sharpe_positive_returns() {
        // Varying positive daily returns
        let returns: Vec<f64> = (0..252).map(|i| 0.001 + (i as f64 % 5.0) * 0.0002).collect();
        let sharpe = compute_sharpe(&returns);
        // Should be positive
        assert!(sharpe > 0.0, "Expected positive Sharpe, got {}", sharpe);
    }

    #[test]
    fn test_sharpe_zero_returns() {
        let returns: Vec<f64> = vec![0.0; 252];
        let sharpe = compute_sharpe(&returns);
        assert!((sharpe - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_max_drawdown() {
        use quantedge_metrics::EquityPoint;
        use chrono::NaiveDate;

        let equity = vec![
            EquityPoint { date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(), equity: 100_000.0 },
            EquityPoint { date: NaiveDate::from_ymd_opt(2023, 2, 1).unwrap(), equity: 120_000.0 },
            EquityPoint { date: NaiveDate::from_ymd_opt(2023, 3, 1).unwrap(), equity: 90_000.0 },  // 25% DD from 120K
            EquityPoint { date: NaiveDate::from_ymd_opt(2023, 4, 1).unwrap(), equity: 110_000.0 },
        ];
        let dd = compute_max_drawdown(&equity);
        assert!((dd - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_diversification_benefit_nonnegative() {
        let result = compute_diversification_benefit(&[], &[0.01, -0.005]);
        assert!(result >= 0.0);
    }
}
