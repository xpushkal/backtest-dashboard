//! Performance benchmarks for the backtester.
//!
//! Run with: `cargo bench -p quantedge-core`

use chrono::{Datelike, NaiveDate, NaiveTime};
use criterion::{criterion_group, criterion_main, Criterion};
use quantedge_core::runner::{SimBar, SimRunner};
use quantedge_core::StrategyConfig;

const BENCH_TOML: &str = r#"
[strategy]
name = "Bench Short CE"
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
"#;

/// Generate synthetic bars for N trading days.
fn generate_bench_bars(n_days: u32) -> Vec<SimBar> {
    let mut bars = Vec::with_capacity(n_days as usize * 8);
    let base_date = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
    let mut rng: u64 = 12345;

    for d in 0..n_days {
        let date = base_date + chrono::Duration::days(d as i64);
        let wd = date.weekday();
        if wd == chrono::Weekday::Sat || wd == chrono::Weekday::Sun {
            continue;
        }

        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let base = 180.0 + ((rng >> 33) as f64 / u32::MAX as f64) * 60.0;
        let spot = 48000.0 + d as f64 * 1.5;

        let times = [
            NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(13, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
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
                close: base * factor,
                spot: spot + (i as f64 - 4.0) * 30.0,
            });
        }
    }
    bars
}

fn bench_single_leg_sim(c: &mut Criterion) {
    let config = StrategyConfig::from_toml_str(BENCH_TOML).unwrap();

    // ~1000 trading days ≈ 4 years
    let bars = generate_bench_bars(1400); // ~1000 weekdays
    let bar_count = bars.len();

    c.bench_function(
        &format!("4yr_single_leg_atm_ce ({} bars)", bar_count),
        |b| {
            b.iter(|| SimRunner::run(&config, &bars, 15));
        },
    );
}

fn bench_100_day_sim(c: &mut Criterion) {
    let config = StrategyConfig::from_toml_str(BENCH_TOML).unwrap();
    let bars = generate_bench_bars(140);

    c.bench_function("100_day_single_leg", |b| {
        b.iter(|| SimRunner::run(&config, &bars, 15));
    });
}

// ─── Multi-Leg Straddle Benchmark ───────────────────────────

const STRADDLE_BENCH_TOML: &str = r#"
[strategy]
name = "Bench Straddle"
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

[[legs]]
option_type = "PE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0

[overall]
overall_sl_enabled = true
overall_sl_type = "percent_of_premium"
overall_sl_value = 60.0
overall_target_enabled = true
overall_target_type = "percent_of_premium"
overall_target_value = 50.0
"#;

/// Generate bars with CE + PE at each timestamp.
fn generate_straddle_bars(n_days: u32) -> Vec<SimBar> {
    let mut bars = Vec::with_capacity(n_days as usize * 16);
    let base_date = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
    let mut rng: u64 = 54321;

    for d in 0..n_days {
        let date = base_date + chrono::Duration::days(d as i64);
        let wd = date.weekday();
        if wd == chrono::Weekday::Sat || wd == chrono::Weekday::Sun {
            continue;
        }

        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let ce_base = 180.0 + ((rng >> 33) as f64 / u32::MAX as f64) * 60.0;
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let pe_base = 160.0 + ((rng >> 33) as f64 / u32::MAX as f64) * 50.0;
        let spot = 48000.0 + d as f64 * 1.5;

        let times = [
            NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(13, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
        ];

        for (i, &t) in times.iter().enumerate() {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let ce_factor = 0.80 + ((rng >> 33) as f64 / u32::MAX as f64) * 0.50;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let pe_factor = 0.80 + ((rng >> 33) as f64 / u32::MAX as f64) * 0.50;
            let s = spot + (i as f64 - 4.0) * 30.0;

            bars.push(SimBar {
                date,
                time: t,
                option_type: "CE".to_string(),
                strike_offset: 0,
                close: ce_base * ce_factor,
                spot: s,
            });
            bars.push(SimBar {
                date,
                time: t,
                option_type: "PE".to_string(),
                strike_offset: 0,
                close: pe_base * pe_factor,
                spot: s,
            });
        }
    }
    bars
}

fn bench_straddle_4yr(c: &mut Criterion) {
    let config = StrategyConfig::from_toml_str(STRADDLE_BENCH_TOML).unwrap();
    let bars = generate_straddle_bars(1400); // ~1000 weekdays = 4 years
    let bar_count = bars.len();

    c.bench_function(
        &format!("4yr_straddle_2leg ({} bars)", bar_count),
        |b| {
            b.iter(|| SimRunner::run(&config, &bars, 15));
        },
    );
}

// ─── Metrics Computation Benchmark ──────────────────────────

fn bench_metrics_computation(c: &mut Criterion) {
    use quantedge_metrics::{EquityPoint, MetricExitReason, MetricsEngine, TradeRecord};

    // Generate 1000 synthetic trades
    let base_date = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
    let trades: Vec<TradeRecord> = (0..1000)
        .map(|i| {
            let pnl = if i % 3 == 0 { -500.0 } else { 300.0 + (i as f64) * 0.5 };
            TradeRecord {
                pnl_gross: pnl + 50.0,
                pnl_net: pnl,
                brokerage: 40.0,
                stt: 10.0,
                slippage_cost: 0.0,
                exit_reason: if pnl >= 0.0 {
                    MetricExitReason::Target
                } else {
                    MetricExitReason::StopLoss
                },
                bars_held: 5,
                exit_date: base_date + chrono::Duration::days(i),
                reentry_attempt: 0,
            }
        })
        .collect();

    // Generate 1000-day equity curve
    let mut equity = 500000.0;
    let equity_points: Vec<EquityPoint> = (0..1000)
        .map(|i| {
            equity += if i % 3 == 0 { -500.0 } else { 300.0 };
            EquityPoint {
                date: base_date + chrono::Duration::days(i),
                equity,
            }
        })
        .collect();

    let start = base_date;
    let end = base_date + chrono::Duration::days(999);

    c.bench_function("metrics_1000_trades", |b| {
        b.iter(|| MetricsEngine::compute(&trades, &equity_points, 500000.0, start, end));
    });
}

// ─── Iron Condor (4-leg) Benchmark ──────────────────────────

const IRON_CONDOR_TOML: &str = r#"
[strategy]
name = "Bench Iron Condor"
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

[[legs]]
option_type = "PE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0

[[legs]]
option_type = "CE"
position = "buy"
lots = 1
strike_mode = "atm_offset"
strike_offset = 2
stop_loss_enabled = false

[[legs]]
option_type = "PE"
position = "buy"
lots = 1
strike_mode = "atm_offset"
strike_offset = -2
stop_loss_enabled = false

[overall]
overall_sl_enabled = true
overall_sl_type = "percent_of_premium"
overall_sl_value = 60.0
overall_target_enabled = true
overall_target_type = "percent_of_premium"
overall_target_value = 50.0
"#;

fn generate_iron_condor_bars(n_days: u32) -> Vec<SimBar> {
    let mut bars = Vec::new();
    let base_date = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
    let mut rng: u64 = 77777;

    for d in 0..n_days {
        let date = base_date + chrono::Duration::days(d as i64);
        if date.weekday() == chrono::Weekday::Sat || date.weekday() == chrono::Weekday::Sun {
            continue;
        }

        let spot = 48000.0 + d as f64 * 1.5;
        let times = [
            NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
        ];

        for (i, &t) in times.iter().enumerate() {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let s = spot + (i as f64 - 1.0) * 30.0;

            // CE ATM (offset 0)
            let ce_0 = 180.0 + ((rng >> 33) as f64 / u32::MAX as f64) * 40.0;
            bars.push(SimBar {
                date, time: t,
                option_type: "CE".to_string(), strike_offset: 0,
                close: ce_0, spot: s,
            });

            // PE ATM (offset 0)
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let pe_0 = 160.0 + ((rng >> 33) as f64 / u32::MAX as f64) * 40.0;
            bars.push(SimBar {
                date, time: t,
                option_type: "PE".to_string(), strike_offset: 0,
                close: pe_0, spot: s,
            });

            // CE OTM (offset +2)
            bars.push(SimBar {
                date, time: t,
                option_type: "CE".to_string(), strike_offset: 2,
                close: ce_0 * 0.3, spot: s,
            });

            // PE OTM (offset -2)
            bars.push(SimBar {
                date, time: t,
                option_type: "PE".to_string(), strike_offset: -2,
                close: pe_0 * 0.3, spot: s,
            });
        }
    }
    bars
}

fn bench_iron_condor_4yr(c: &mut Criterion) {
    let config = StrategyConfig::from_toml_str(IRON_CONDOR_TOML).unwrap();
    let bars = generate_iron_condor_bars(1400);
    let bar_count = bars.len();

    c.bench_function(
        &format!("4yr_iron_condor_4leg ({} bars)", bar_count),
        |b| {
            b.iter(|| SimRunner::run(&config, &bars, 15));
        },
    );
}

criterion_group!(
    benches,
    bench_single_leg_sim,
    bench_100_day_sim,
    bench_straddle_4yr,
    bench_iron_condor_4yr,
    bench_metrics_computation
);
criterion_main!(benches);
