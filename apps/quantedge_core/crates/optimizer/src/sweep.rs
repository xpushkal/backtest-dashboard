//! Optimizer sweep engine — runs all parameter combos via Rayon.

use crate::param_grid::{ParamCombo, ParamGrid};
use quantedge_core::{ExitReason, RunResult, SimBar, SimRunner, StrategyConfig};
use quantedge_metrics::{EquityPoint, MetricExitReason, MetricsEngine, MetricsResult, TradeRecord};
use rayon::prelude::*;
use std::collections::HashMap;

/// Result of a single optimizer combo.
#[derive(Debug, Clone)]
pub struct OptimizerResult {
    pub combo_index: usize,
    pub params: HashMap<String, f64>,
    pub metrics: MetricsResult,
    pub trade_count: u32,
}

/// The optimizer sweep engine.
pub struct OptimizerSweep;

impl OptimizerSweep {
    /// Run all parameter combinations in parallel using Rayon.
    ///
    /// Returns results sorted by Sharpe ratio descending.
    pub fn run(
        base_config: &StrategyConfig,
        bars: &[SimBar],
        grid: &ParamGrid,
        lot_size: u32,
        capital: f64,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Vec<OptimizerResult> {
        let combos = grid.generate_combos();

        let mut results: Vec<OptimizerResult> = combos
            .par_iter()
            .map(|combo| {
                Self::run_single(base_config, bars, combo, lot_size, capital, start_date, end_date)
            })
            .collect();

        // Sort by Sharpe descending (NaN goes to bottom)
        results.sort_by(|a, b| {
            b.metrics
                .sharpe_ratio
                .partial_cmp(&a.metrics.sharpe_ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Run a single combo: apply overrides → simulate → compute metrics.
    fn run_single(
        base_config: &StrategyConfig,
        bars: &[SimBar],
        combo: &ParamCombo,
        lot_size: u32,
        capital: f64,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> OptimizerResult {
        let mut config = base_config.clone();
        apply_overrides(&mut config, &combo.values);

        let result = SimRunner::run(&config, bars, lot_size);
        let trade_records = convert_trades(&result);
        let equity_points = convert_equity(&result, capital);
        let metrics = MetricsEngine::compute(&trade_records, &equity_points, capital, start_date, end_date);

        OptimizerResult {
            combo_index: combo.index,
            params: combo.values.clone(),
            metrics,
            trade_count: result.trades.len() as u32,
        }
    }
}

/// Apply parameter overrides to a strategy config.
fn apply_overrides(config: &mut StrategyConfig, overrides: &HashMap<String, f64>) {
    for (name, value) in overrides {
        match name.as_str() {
            "sl_value" => {
                for leg in &mut config.legs {
                    leg.stop_loss_value = *value;
                }
            }
            "target_value" => {
                config.overall.overall_target_value = *value;
                config.overall.overall_target_enabled = true;
            }
            "strike_offset" => {
                for leg in &mut config.legs {
                    leg.strike_offset = *value as i32;
                }
            }
            "lots" => {
                for leg in &mut config.legs {
                    leg.lots = (*value as u32).max(1);
                }
            }
            _ => {} // Unknown — skip
        }
    }
}

/// Convert RunResult trades to MetricsEngine TradeRecords.
fn convert_trades(result: &RunResult) -> Vec<TradeRecord> {
    result
        .trades
        .iter()
        .map(|t| TradeRecord {
            pnl_gross: t.pnl_gross,
            pnl_net: t.pnl_net,
            brokerage: t.brokerage,
            stt: t.stt,
            slippage_cost: t.slippage_cost,
            exit_reason: match t.exit_reason {
                ExitReason::StopLoss | ExitReason::CombinedSl => MetricExitReason::StopLoss,
                ExitReason::Target | ExitReason::CombinedTarget => MetricExitReason::Target,
                ExitReason::TimeExit => MetricExitReason::TimeExit,
                ExitReason::EndOfData => MetricExitReason::EndOfData,
            },
            bars_held: t.bars_held,
            exit_date: t.exit_date,
            reentry_attempt: t.reentry_attempt,
        })
        .collect()
}

/// Convert RunResult snapshots to equity points.
fn convert_equity(result: &RunResult, capital: f64) -> Vec<EquityPoint> {
    result
        .snapshots
        .iter()
        .map(|s| EquityPoint {
            date: s.date,
            equity: capital + s.cumulative_pnl + s.unrealized_pnl,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param_grid::ParamRange;
    use chrono::{Datelike, NaiveDate, NaiveTime};

    fn bench_toml() -> &'static str {
        r#"
[strategy]
name = "Test CE"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0
brokerage_per_lot = 40.0
slippage_model = "fixed_pts"
slippage_value = 1.0
stt_on_sell = true

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0

[overall]
overall_sl_enabled = false
        "#
    }

    fn generate_test_bars(n_days: u32) -> Vec<SimBar> {
        let mut bars = Vec::new();
        let base_date = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
        let mut rng: u64 = 12345;

        for d in 0..n_days {
            let date = base_date + chrono::Duration::days(d as i64);
            if date.weekday() == chrono::Weekday::Sat || date.weekday() == chrono::Weekday::Sun {
                continue;
            }

            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let base = 180.0 + ((rng >> 33) as f64 / u32::MAX as f64) * 60.0;
            let spot = 48000.0 + d as f64 * 1.5;

            let times = [
                NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
                NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
            ];

            for (i, &t) in times.iter().enumerate() {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let factor = 0.80 + ((rng >> 33) as f64 / u32::MAX as f64) * 0.50;
                bars.push(SimBar {
                    date,
                    time: t,
                    option_type: "CE".to_string(),
                    strike_offset: 0,
                    close: base * factor, high: base * factor, low: base * factor,
                    spot: spot + (i as f64 - 1.0) * 30.0,
                });
            }
        }
        bars
    }

    #[test]
    fn test_sweep_basic() {
        let config = StrategyConfig::from_toml_str(bench_toml()).unwrap();
        let bars = generate_test_bars(100);
        let grid = ParamGrid {
            params: vec![
                ParamRange { name: "sl_value".into(), min: 50.0, max: 100.0, step: 50.0 },
            ],
        };
        let start = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
        let end = NaiveDate::from_ymd_opt(2021, 5, 15).unwrap();

        let results = OptimizerSweep::run(&config, &bars, &grid, 15, 500000.0, start, end);
        assert_eq!(results.len(), 2); // 50 and 100
        // Should be sorted by Sharpe
        assert!(results[0].metrics.sharpe_ratio >= results[1].metrics.sharpe_ratio
            || results[0].metrics.sharpe_ratio.is_nan());
    }

    #[test]
    fn test_sweep_12_combos() {
        let config = StrategyConfig::from_toml_str(bench_toml()).unwrap();
        let bars = generate_test_bars(50);
        let grid = ParamGrid {
            params: vec![
                ParamRange { name: "sl_value".into(), min: 30.0, max: 60.0, step: 10.0 },
                ParamRange { name: "lots".into(), min: 1.0, max: 3.0, step: 1.0 },
            ],
        };
        let start = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
        let end = NaiveDate::from_ymd_opt(2021, 3, 1).unwrap();

        let results = OptimizerSweep::run(&config, &bars, &grid, 15, 500000.0, start, end);
        assert_eq!(results.len(), 12); // 4 × 3
    }

    #[test]
    fn test_overrides_applied() {
        let mut config = StrategyConfig::from_toml_str(bench_toml()).unwrap();
        let mut overrides = HashMap::new();
        overrides.insert("sl_value".to_string(), 42.0);
        overrides.insert("lots".to_string(), 3.0);
        apply_overrides(&mut config, &overrides);
        assert!((config.legs[0].stop_loss_value - 42.0).abs() < 0.01);
        assert_eq!(config.legs[0].lots, 3);
    }
}
