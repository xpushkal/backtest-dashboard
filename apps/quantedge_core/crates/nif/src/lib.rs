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
fn run_optimizer(_strategy_toml: String, _param_grid_json: String) -> Result<String, String> {
    // TODO: Phase 8 — Parameter optimizer implementation
    Err("Optimizer not yet implemented".to_string())
}

#[rustler::nif(schedule = "DirtyCpu")]
fn run_portfolio(_strategies_json: String, _opts_json: String) -> Result<String, String> {
    // TODO: Phase 8 — Portfolio engine implementation
    Err("Portfolio not yet implemented".to_string())
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
