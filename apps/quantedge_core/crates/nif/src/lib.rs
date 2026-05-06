//! Rustler NIF bridge for Elixir — QuantEdge simulation kernel.
//!
//! All NIFs use `dirty_cpu` scheduling to avoid blocking the BEAM.
//! Input: JSON strings. Output: `Result<String, String>` → `{:ok, json} | {:error, reason}`.

use quantedge_core::{
    ExitReason, RunResult, SimBar, SimRunner, StrategyConfig,
};
use quantedge_data::bar::{BarLoadConfig, BarStream};
use quantedge_metrics::{
    EquityPoint, MetricExitReason, MetricsEngine, TradeRecord,
};
use serde::{Deserialize, Serialize};

/// Backtest request from Elixir.
#[derive(Deserialize)]
struct BacktestRequest {
    symbol: String,
    date_from: String,
    date_to: String,
    capital: f64,
    lot_size: u32,
    data_dir: String,
}

/// Simplified trade JSON output.
#[derive(Serialize)]
struct TradeJson {
    entry_date: String,
    exit_date: String,
    entry_time: String,
    exit_time: String,
    option_type: String,
    position_side: String,
    entry_price: f64,
    exit_price: f64,
    entry_spot: f64,
    exit_spot: f64,
    lots: u32,
    lot_size: u32,
    pnl_gross: f64,
    pnl_net: f64,
    brokerage: f64,
    exit_reason: String,
    bars_held: u32,
}

/// Equity point JSON output.
#[derive(Serialize)]
struct EquityJson {
    date: String,
    equity: f64,
    drawdown_pct: f64,
}

/// Full backtest response.
#[derive(Serialize)]
struct BacktestResponse {
    trades: Vec<TradeJson>,
    equity_curve: Vec<EquityJson>,
    metrics: serde_json::Value,
    total_bars: usize,
}

#[rustler::nif(schedule = "DirtyCpu")]
fn run_backtest(strategy_toml: String, opts_json: String) -> Result<String, String> {
    // Parse request options
    let request: BacktestRequest =
        serde_json::from_str(&opts_json).map_err(|e| format!("Invalid opts: {}", e))?;

    // Parse strategy config from TOML
    let config = StrategyConfig::from_toml_str(&strategy_toml)
        .map_err(|e| format!("Invalid TOML: {}", e))?;

    // Load bar data from Parquet
    let start_date = chrono::NaiveDate::parse_from_str(&request.date_from, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date_from: {}", e))?;
    let end_date = chrono::NaiveDate::parse_from_str(&request.date_to, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date_to: {}", e))?;

    let bar_config = BarLoadConfig {
        symbol: request.symbol.clone(),
        expiry_type: "weekly".to_string(),
        start_date,
        end_date,
        data_dir: request.data_dir.clone(),
    };

    let bars_raw = BarStream::load(&bar_config)
        .map_err(|e| format!("Data load error: {}", e))?;

    // Convert data::Bar → core::SimBar
    let bars: Vec<SimBar> = bars_raw
        .iter()
        .map(|b| SimBar {
            date: b.date,
            time: b.time,
            option_type: b.option_type.clone(),
            strike_offset: b.strike_offset,
            close: b.close,
            spot: b.spot,
        })
        .collect();

    // Run simulation
    let result = SimRunner::run(&config, &bars, request.lot_size);

    // Compute metrics
    let trade_records = convert_trades(&result);
    let equity_points = convert_snapshots(&result, request.capital);
    let metrics = MetricsEngine::compute(
        &trade_records,
        &equity_points,
        request.capital,
        start_date,
        end_date,
    );

    // Build response
    let trade_jsons: Vec<TradeJson> = result
        .trades
        .iter()
        .map(|t| TradeJson {
            entry_date: t.entry_date.to_string(),
            exit_date: t.exit_date.to_string(),
            entry_time: t.entry_time.to_string(),
            exit_time: t.exit_time.to_string(),
            option_type: format!("{:?}", t.option_type),
            position_side: format!("{:?}", t.position_side),
            entry_price: t.entry_price,
            exit_price: t.exit_price,
            entry_spot: t.entry_spot,
            exit_spot: t.exit_spot,
            lots: t.lots,
            lot_size: t.lot_size,
            pnl_gross: t.pnl_gross,
            pnl_net: t.pnl_net,
            brokerage: t.brokerage,
            exit_reason: format!("{:?}", t.exit_reason),
            bars_held: t.bars_held,
        })
        .collect();

    let equity_jsons = build_equity_json(&equity_points, request.capital);

    let response = BacktestResponse {
        trades: trade_jsons,
        equity_curve: equity_jsons,
        metrics: serde_json::to_value(&metrics).unwrap_or_default(),
        total_bars: result.total_bars,
    };

    serde_json::to_string(&response).map_err(|e| format!("Serialize error: {}", e))
}

#[rustler::nif(schedule = "DirtyCpu")]
fn run_optimizer(strategy_toml: String, param_grid_json: String) -> Result<String, String> {
    use quantedge_optimizer::{OptimizerSweep, ParamGrid};

    // 1. Parse strategy config from TOML
    let config = StrategyConfig::from_toml_str(&strategy_toml)
        .map_err(|e| format!("Invalid TOML: {}", e))?;

    // 2. Parse param grid from JSON
    let grid = ParamGrid::from_json_str(&param_grid_json)
        .map_err(|e| format!("Invalid param grid: {}", e))?;

    // 3. Load bar data
    let start_date = chrono::NaiveDate::from_ymd_opt(2021, 1, 1)
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2021, 1, 1).unwrap());
    let end_date = chrono::NaiveDate::from_ymd_opt(2024, 12, 31)
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap());

    let bar_config = BarLoadConfig {
        symbol: config.strategy.underlying.clone(),
        expiry_type: "weekly".to_string(),
        start_date,
        end_date,
        data_dir: "Data/parquet".to_string(),
    };

    let bars_raw = BarStream::load(&bar_config)
        .map_err(|e| format!("Data load error: {}", e))?;

    let bars: Vec<SimBar> = bars_raw
        .iter()
        .map(|b| SimBar {
            date: b.date,
            time: b.time,
            option_type: b.option_type.clone(),
            strike_offset: b.strike_offset,
            close: b.close,
            spot: b.spot,
        })
        .collect();

    // 4. Run optimizer sweep (Rayon parallel)
    let capital = config.strategy.capital;
    let results = OptimizerSweep::run(
        &config, &bars, &grid, 15, capital, start_date, end_date,
    );

    // 5. Serialize results (top 100) to JSON
    let result_jsons: Vec<serde_json::Value> = results
        .iter()
        .take(100)
        .map(|r| {
            serde_json::json!({
                "combo_index": r.combo_index,
                "params": r.params,
                "sharpe": r.metrics.sharpe_ratio,
                "total_pnl": r.metrics.total_pnl_net,
                "max_dd_pct": r.metrics.max_drawdown_pct,
                "trade_count": r.trade_count,
                "win_rate": r.metrics.win_rate_pct,
                "profit_factor": r.metrics.profit_factor,
                "cagr": r.metrics.cagr,
            })
        })
        .collect();

    serde_json::to_string(&result_jsons).map_err(|e| format!("Serialize error: {}", e))
}

#[rustler::nif(schedule = "DirtyCpu")]
fn run_portfolio(portfolio_json: String, _opts_json: String) -> Result<String, String> {
    use quantedge_portfolio::{
        CorrelationMatrix, MarginModel, PortfolioConfig, PortfolioEngine,
        PortfolioMarginTracker, PortfolioMetrics,
    };
    use std::collections::HashMap;

    // 1. Parse portfolio config from JSON
    let config = PortfolioConfig::from_json_str(&portfolio_json)
        .map_err(|e| format!("Portfolio config error: {}", e))?;

    // 2. Load bar data per unique underlying (deduplicate loads)
    let start_date = chrono::NaiveDate::parse_from_str(&config.date_from, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date_from: {}", e))?;
    let end_date = chrono::NaiveDate::parse_from_str(&config.date_to, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date_to: {}", e))?;

    let mut bars_map: HashMap<String, Vec<SimBar>> = HashMap::new();
    for alloc in &config.strategies {
        if bars_map.contains_key(&alloc.underlying) {
            continue;
        }

        let bar_config = BarLoadConfig {
            symbol: alloc.underlying.clone(),
            expiry_type: "weekly".to_string(),
            start_date,
            end_date,
            data_dir: config.data_dir.clone(),
        };

        let bars_raw = BarStream::load(&bar_config)
            .map_err(|e| format!("Data load error for {}: {}", alloc.underlying, e))?;

        let bars: Vec<SimBar> = bars_raw
            .iter()
            .map(|b| SimBar {
                date: b.date,
                time: b.time,
                option_type: b.option_type.clone(),
                strike_offset: b.strike_offset,
                close: b.close,
                spot: b.spot,
            })
            .collect();

        bars_map.insert(alloc.underlying.clone(), bars);
    }

    // 3. Run portfolio engine
    let portfolio_result = PortfolioEngine::run(&config, &bars_map)
        .map_err(|e| format!("Portfolio engine error: {}", e))?;

    // 4. Compute correlation matrix
    let daily_pnls: Vec<Vec<f64>> = portfolio_result
        .strategy_results
        .iter()
        .map(|sr| sr.daily_pnls.clone())
        .collect();
    let strategy_names: Vec<String> = portfolio_result
        .strategy_results
        .iter()
        .map(|sr| sr.name.clone())
        .collect();
    let correlation = CorrelationMatrix::compute(&daily_pnls, &strategy_names);

    // 5. Portfolio margin tracking (simplified — compute peak margin from results)
    let margin_model = MarginModel::default_model();
    let margin_tracker = PortfolioMarginTracker::new(config.total_capital, margin_model);

    // 6. Compute portfolio metrics
    let portfolio_metrics = PortfolioMetrics::compute(
        &portfolio_result,
        &correlation,
        &margin_tracker,
        config.total_capital,
    );

    // 7. Build response JSON
    let per_strategy: Vec<serde_json::Value> = portfolio_result
        .strategy_results
        .iter()
        .map(|sr| {
            let trades: Vec<serde_json::Value> = sr
                .run_result
                .trades
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "entry_date": t.entry_date.to_string(),
                        "exit_date": t.exit_date.to_string(),
                        "entry_time": t.entry_time.to_string(),
                        "exit_time": t.exit_time.to_string(),
                        "option_type": format!("{:?}", t.option_type),
                        "position_side": format!("{:?}", t.position_side),
                        "entry_price": t.entry_price,
                        "exit_price": t.exit_price,
                        "pnl_gross": t.pnl_gross,
                        "pnl_net": t.pnl_net,
                        "exit_reason": format!("{:?}", t.exit_reason),
                        "bars_held": t.bars_held,
                    })
                })
                .collect();

            let equity: Vec<serde_json::Value> = sr
                .equity_curve
                .iter()
                .map(|ep| {
                    serde_json::json!({
                        "date": ep.date.to_string(),
                        "equity": ep.equity,
                    })
                })
                .collect();

            serde_json::json!({
                "name": sr.name,
                "underlying": sr.underlying,
                "allocation_pct": sr.allocation_pct,
                "allocated_capital": sr.allocated_capital,
                "trades": trades,
                "equity_curve": equity,
                "metrics": serde_json::to_value(&sr.metrics).unwrap_or_default(),
                "total_trades": sr.run_result.trades.len(),
            })
        })
        .collect();

    let combined_equity: Vec<serde_json::Value> = portfolio_result
        .combined_equity
        .iter()
        .map(|ep| {
            serde_json::json!({
                "date": ep.date.to_string(),
                "equity": ep.equity,
            })
        })
        .collect();

    let response = serde_json::json!({
        "strategies": per_strategy,
        "combined_equity": combined_equity,
        "portfolio_metrics": serde_json::to_value(&portfolio_metrics).unwrap_or_default(),
        "correlation_matrix": correlation.to_json(),
        "total_trades": portfolio_result.total_trades,
    });

    serde_json::to_string(&response).map_err(|e| format!("Serialize error: {}", e))
}

#[rustler::nif(schedule = "DirtyCpu")]
fn load_data_summary(symbol: String, date_from: String, date_to: String) -> Result<String, String> {
    let start = chrono::NaiveDate::parse_from_str(&date_from, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date: {}", e))?;
    let end = chrono::NaiveDate::parse_from_str(&date_to, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date: {}", e))?;

    let config = BarLoadConfig {
        symbol: symbol.clone(),
        expiry_type: "weekly".to_string(),
        start_date: start,
        end_date: end,
        data_dir: "Data/parquet".to_string(),
    };

    match BarStream::load(&config) {
        Ok(bars) => {
            let summary = serde_json::json!({
                "symbol": symbol,
                "bar_count": bars.len(),
                "date_from": date_from,
                "date_to": date_to,
            });
            serde_json::to_string(&summary).map_err(|e| e.to_string())
        }
        Err(e) => Err(format!("Failed to load data: {}", e)),
    }
}

// ─── Helpers ────────────────────────────────────────────────

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

fn convert_snapshots(result: &RunResult, capital: f64) -> Vec<EquityPoint> {
    result
        .snapshots
        .iter()
        .map(|s| EquityPoint {
            date: s.date,
            equity: capital + s.cumulative_pnl + s.unrealized_pnl,
        })
        .collect()
}

fn build_equity_json(points: &[EquityPoint], capital: f64) -> Vec<EquityJson> {
    let mut peak = capital;
    points
        .iter()
        .map(|p| {
            if p.equity > peak {
                peak = p.equity;
            }
            let dd_pct = if peak > 0.0 {
                (peak - p.equity) / peak * 100.0
            } else {
                0.0
            };
            EquityJson {
                date: p.date.to_string(),
                equity: p.equity,
                drawdown_pct: dd_pct,
            }
        })
        .collect()
}

rustler::init!("Elixir.QuantEdge.NIF");
