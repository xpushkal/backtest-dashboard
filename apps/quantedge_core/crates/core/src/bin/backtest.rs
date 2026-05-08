//! QuantEdge Backtester CLI
//!
//! Run backtests from the command line:
//! `cargo run --bin backtest -- --strategy config/strategies/example_short_call.toml`

use chrono::{Datelike, NaiveDate};
use clap::Parser;
use quantedge_core::config::ExitReason;
use quantedge_core::runner::{RunResult, SimBar, SimRunner};
use quantedge_core::StrategyConfig;
use quantedge_metrics::{EquityPoint, MetricExitReason, MetricsEngine, TradeRecord};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "backtest", about = "QuantEdge Backtester — run strategy backtests")]
struct Args {
    /// Path to strategy TOML file
    #[arg(short, long)]
    strategy: String,

    /// Path to Parquet data directory
    #[arg(short, long, default_value = "data/parquet")]
    data_dir: String,

    /// Path to lot sizes config
    #[arg(long, default_value = "config/lot_sizes.toml")]
    lot_config: String,

    /// Output format: table or json
    #[arg(long, default_value = "table")]
    output: String,

    /// Use synthetic test data (for testing without real data)
    #[arg(long)]
    synthetic: bool,

    /// Number of synthetic trading days to generate
    #[arg(long, default_value = "100")]
    synthetic_days: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let start = Instant::now();

    // 1. Parse strategy
    let config = StrategyConfig::from_toml(&args.strategy)?;
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" QuantEdge Backtester");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(" Strategy:   {}", config.name());
    println!(" Underlying: {}", config.underlying());
    println!(" Capital:    ₹{:.0}", config.capital());

    // 2. Load data
    let (bars, lot_size) = if args.synthetic {
        println!(" Data:       {} synthetic days", args.synthetic_days);
        (generate_synthetic_bars(args.synthetic_days), 15_u32)
    } else {
        // Real Parquet data loading (requires data directory)
        println!(" Data:       {} (not yet connected — use --synthetic)", args.data_dir);
        println!();
        println!(" ⚠  Real data loading will be connected when BarStream");
        println!("    is integrated with SimBar in a future phase.");
        println!("    Use --synthetic flag for now.");
        return Ok(());
    };

    println!(" Bars:       {}", bars.len());
    println!(" Lot size:   {}", lot_size);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // 3. Run simulation
    let sim_start = Instant::now();
    let result = SimRunner::run(&config, &bars, lot_size);
    let sim_elapsed = sim_start.elapsed();

    // 4. Convert to metrics input
    let trade_records = convert_trades(&result);
    let equity_points = convert_snapshots(&result);

    let start_date = bars.first().map_or(
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        |b| b.date,
    );
    let end_date = bars.last().map_or(
        NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        |b| b.date,
    );

    // 5. Compute metrics
    let metrics = MetricsEngine::compute(
        &trade_records,
        &equity_points,
        config.capital(),
        start_date,
        end_date,
    );

    // 6. Print results
    if args.output == "json" {
        println!("{}", serde_json::to_string_pretty(&metrics)?);
    } else {
        println!();
        println!(" ── Return Metrics ─────────────────────────────────");
        println!("  Total PnL (net):     ₹{:>12.2}", metrics.total_pnl_net);
        println!("  Total PnL (gross):   ₹{:>12.2}", metrics.total_pnl_gross);
        println!("  CAGR:                {:>12.2}%", metrics.cagr * 100.0);
        println!("  ROI:                 {:>12.2}%", metrics.roi_pct);
        println!("  Expectancy:          ₹{:>12.2}", metrics.expectancy);
        println!("  Profit Factor:       {:>12.2}", metrics.profit_factor);
        println!("  Win Rate:            {:>12.1}%", metrics.win_rate_pct);
        println!("  Avg Win:             ₹{:>12.2}", metrics.avg_win);
        println!("  Avg Loss:            ₹{:>12.2}", metrics.avg_loss);
        println!("  Largest Win:         ₹{:>12.2}", metrics.largest_win);
        println!("  Largest Loss:        ₹{:>12.2}", metrics.largest_loss);
        println!();
        println!(" ── Risk Metrics ──────────────────────────────────");
        println!("  Max Drawdown (INR):  ₹{:>12.2}", metrics.max_drawdown_inr);
        println!("  Max Drawdown (%):    {:>12.2}%", metrics.max_drawdown_pct);
        println!("  Sharpe Ratio:        {:>12.2}", metrics.sharpe_ratio);
        println!();
        println!(" ── Trade Analytics ────────────────────────────────");
        println!("  Total Trades:        {:>12}", metrics.total_trades);
        println!("  Avg Hold (bars):     {:>12.1}", metrics.avg_hold_bars);
        println!("  SL Hit Rate:         {:>12.1}%", metrics.sl_hit_rate_pct);
        println!("  Target Hit Rate:     {:>12.1}%", metrics.target_hit_rate_pct);
        println!("  Time Exit Rate:      {:>12.1}%", metrics.time_exit_rate_pct);
        println!("  Total Costs:         ₹{:>12.2}", metrics.total_brokerage);
        println!();
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!(" Simulation:  {:?}", sim_elapsed);
        println!(" Total:       {:?}", start.elapsed());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }

    Ok(())
}

/// Convert core ClosedTrades to metrics TradeRecords.
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

/// Convert core PositionSnapshots to metrics EquityPoints.
fn convert_snapshots(result: &RunResult) -> Vec<EquityPoint> {
    result
        .snapshots
        .iter()
        .map(|s| EquityPoint {
            date: s.date,
            equity: s.equity,
        })
        .collect()
}

/// Generate synthetic trading data for testing.
fn generate_synthetic_bars(n_days: u32) -> Vec<SimBar> {
    use chrono::NaiveTime;

    let mut bars = Vec::with_capacity(n_days as usize * 10);
    let base_date = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
    let mut rng_state: u64 = 42;

    for day_offset in 0..n_days {
        let date = base_date + chrono::Duration::days(day_offset as i64);

        // Skip weekends
        let weekday = date.weekday();
        if weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun {
            continue;
        }

        // Simple LCG for deterministic "randomness"
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rand_val = (rng_state >> 33) as f64 / (u32::MAX as f64);

        let base_premium = 180.0 + rand_val * 60.0; // 180-240
        let spot = 47000.0 + (day_offset as f64) * 2.0;

        // Entry bar at 09:20
        bars.push(SimBar {
            date,
            time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            option_type: "CE".to_string(),
            strike_offset: 0,
            close: base_premium, high: base_premium, low: base_premium,
            spot,
        });

        // Mid-morning
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mid_rand = (rng_state >> 33) as f64 / (u32::MAX as f64);
        let mid_price = base_premium * (0.85 + mid_rand * 0.60); // 85%-145% of entry

        bars.push(SimBar {
            date,
            time: NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
            option_type: "CE".to_string(),
            strike_offset: 0,
            close: mid_price, high: mid_price, low: mid_price,
            spot: spot + (mid_rand - 0.5) * 200.0,
        });

        // Midday
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let noon_rand = (rng_state >> 33) as f64 / (u32::MAX as f64);
        let noon_price = base_premium * (0.80 + noon_rand * 0.65);

        bars.push(SimBar {
            date,
            time: NaiveTime::from_hms_opt(13, 0, 0).unwrap(),
            option_type: "CE".to_string(),
            strike_offset: 0,
            close: noon_price, high: noon_price, low: noon_price,
            spot: spot + (noon_rand - 0.5) * 300.0,
        });

        // Exit bar at 15:20
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let exit_rand = (rng_state >> 33) as f64 / (u32::MAX as f64);
        let exit_price = base_premium * (0.75 + exit_rand * 0.70);

        bars.push(SimBar {
            date,
            time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
            option_type: "CE".to_string(),
            strike_offset: 0,
            close: exit_price, high: exit_price, low: exit_price,
            spot: spot + (exit_rand - 0.5) * 400.0,
        });
    }

    bars
}
