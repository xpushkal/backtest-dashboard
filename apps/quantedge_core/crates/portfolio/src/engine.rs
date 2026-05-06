//! Portfolio engine — runs N strategies and merges results.
//!
//! Orchestrates per-strategy backtests, builds combined equity curve,
//! and collects portfolio-level data for metrics computation.

use crate::config::{PortfolioConfig, StrategyAllocation};
use chrono::NaiveDate;
use quantedge_core::{RunResult, SimBar, SimRunner};
use quantedge_metrics::{EquityPoint, MetricExitReason, MetricsEngine, TradeRecord};
use std::collections::{BTreeMap, HashMap};

/// Result for a single strategy within the portfolio.
#[derive(Debug, Clone)]
pub struct StrategyResult {
    pub name: String,
    pub underlying: String,
    pub allocation_pct: f64,
    pub allocated_capital: f64,
    pub run_result: RunResult,
    pub equity_curve: Vec<EquityPoint>,
    pub daily_pnls: Vec<f64>,
    pub metrics: quantedge_metrics::MetricsResult,
}

/// Combined portfolio result.
#[derive(Debug, Clone)]
pub struct PortfolioResult {
    pub strategy_results: Vec<StrategyResult>,
    pub combined_equity: Vec<EquityPoint>,
    pub combined_daily_returns: Vec<f64>,
    pub all_dates: Vec<NaiveDate>,
    pub total_trades: usize,
}

/// The portfolio engine orchestrator.
pub struct PortfolioEngine;

impl PortfolioEngine {
    /// Run all strategies and merge results.
    ///
    /// `bars_map`: underlying name → Vec<SimBar> (pre-loaded bar data per underlying).
    pub fn run(
        config: &PortfolioConfig,
        bars_map: &HashMap<String, Vec<SimBar>>,
    ) -> Result<PortfolioResult, String> {
        let start_date = NaiveDate::parse_from_str(&config.date_from, "%Y-%m-%d")
            .map_err(|e| format!("Invalid date_from: {}", e))?;
        let end_date = NaiveDate::parse_from_str(&config.date_to, "%Y-%m-%d")
            .map_err(|e| format!("Invalid date_to: {}", e))?;

        // Run each strategy
        let mut strategy_results = Vec::with_capacity(config.strategies.len());

        for alloc in &config.strategies {
            let result = Self::run_single_strategy(alloc, bars_map, start_date, end_date)?;
            strategy_results.push(result);
        }

        // Build combined equity curve
        let (combined_equity, all_dates, combined_daily_returns) =
            Self::build_combined_equity(&strategy_results, config.total_capital);

        let total_trades: usize = strategy_results
            .iter()
            .map(|r| r.run_result.trades.len())
            .sum();

        Ok(PortfolioResult {
            strategy_results,
            combined_equity,
            combined_daily_returns,
            all_dates,
            total_trades,
        })
    }

    /// Run a single strategy within the portfolio.
    fn run_single_strategy(
        alloc: &StrategyAllocation,
        bars_map: &HashMap<String, Vec<SimBar>>,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<StrategyResult, String> {
        let strategy_config = alloc
            .strategy_config
            .as_ref()
            .ok_or_else(|| format!("Strategy '{}' config not parsed", alloc.name))?;

        let bars = bars_map
            .get(&alloc.underlying)
            .ok_or_else(|| format!("No bar data for underlying '{}'", alloc.underlying))?;

        // Determine lot_size from strategy config
        let lot_size = alloc.lot_size;

        // Run simulation
        let run_result = SimRunner::run(strategy_config, bars, lot_size);

        // Build equity curve from snapshots
        let equity_points: Vec<EquityPoint> = run_result
            .snapshots
            .iter()
            .map(|s| EquityPoint {
                date: s.date,
                equity: alloc.allocated_capital + s.cumulative_pnl + s.unrealized_pnl,
            })
            .collect();

        // Compute daily PnLs from equity curve
        let daily_pnls = Self::compute_daily_pnls(&equity_points);

        // Build trade records for metrics
        let trade_records: Vec<TradeRecord> = run_result
            .trades
            .iter()
            .map(|t| TradeRecord {
                pnl_gross: t.pnl_gross,
                pnl_net: t.pnl_net,
                brokerage: t.brokerage,
                stt: t.stt,
                slippage_cost: t.slippage_cost,
                exit_reason: match t.exit_reason {
                    quantedge_core::ExitReason::StopLoss
                    | quantedge_core::ExitReason::CombinedSl => MetricExitReason::StopLoss,
                    quantedge_core::ExitReason::Target
                    | quantedge_core::ExitReason::CombinedTarget => MetricExitReason::Target,
                    quantedge_core::ExitReason::TimeExit => MetricExitReason::TimeExit,
                    quantedge_core::ExitReason::EndOfData => MetricExitReason::EndOfData,
                },
                bars_held: t.bars_held,
                exit_date: t.exit_date,
                reentry_attempt: t.reentry_attempt,
            })
            .collect();

        // Compute per-strategy metrics
        let metrics =
            MetricsEngine::compute(&trade_records, &equity_points, alloc.allocated_capital, start_date, end_date);

        Ok(StrategyResult {
            name: alloc.name.clone(),
            underlying: alloc.underlying.clone(),
            allocation_pct: alloc.allocation_pct,
            allocated_capital: alloc.allocated_capital,
            run_result,
            equity_curve: equity_points,
            daily_pnls,
            metrics,
        })
    }

    /// Build a combined equity curve by summing all strategy equities per date.
    fn build_combined_equity(
        strategy_results: &[StrategyResult],
        total_capital: f64,
    ) -> (Vec<EquityPoint>, Vec<NaiveDate>, Vec<f64>) {
        // Collect all equity points into a date → per-strategy equity map
        let mut date_map: BTreeMap<NaiveDate, Vec<f64>> = BTreeMap::new();

        for (idx, sr) in strategy_results.iter().enumerate() {
            for ep in &sr.equity_curve {
                let entry = date_map.entry(ep.date).or_insert_with(|| {
                    // Initialize with allocated capital per strategy (forward-fill default)
                    strategy_results
                        .iter()
                        .map(|s| s.allocated_capital)
                        .collect()
                });
                entry[idx] = ep.equity;
            }
        }

        // Forward-fill: for dates where a strategy has no equity point, use last known value
        let mut last_values: Vec<f64> = strategy_results
            .iter()
            .map(|s| s.allocated_capital)
            .collect();

        let mut combined_equity = Vec::new();
        let mut all_dates = Vec::new();

        for (date, values) in &date_map {
            // Update last known values (forward-fill)
            for (i, v) in values.iter().enumerate() {
                if *v != last_values[i] {
                    last_values[i] = *v;
                }
            }

            let portfolio_equity: f64 = last_values.iter().sum();
            combined_equity.push(EquityPoint {
                date: *date,
                equity: portfolio_equity,
            });
            all_dates.push(*date);
        }

        // Compute daily returns
        let daily_returns = Self::compute_daily_returns(&combined_equity, total_capital);

        (combined_equity, all_dates, daily_returns)
    }

    /// Compute daily PnLs from equity curve.
    fn compute_daily_pnls(equity: &[EquityPoint]) -> Vec<f64> {
        if equity.len() < 2 {
            return vec![];
        }
        equity
            .windows(2)
            .map(|w| w[1].equity - w[0].equity)
            .collect()
    }

    /// Compute daily returns from equity curve.
    fn compute_daily_returns(equity: &[EquityPoint], _initial_capital: f64) -> Vec<f64> {
        if equity.len() < 2 {
            return vec![];
        }
        equity
            .windows(2)
            .map(|w| {
                if w[0].equity > 0.0 {
                    (w[1].equity / w[0].equity) - 1.0
                } else {
                    0.0
                }
            })
            .collect()
    }
}
