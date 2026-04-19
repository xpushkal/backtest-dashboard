//! Performance benchmarks for the backtester.
//!
//! Run with: `cargo bench -p quantedge-core`

use chrono::{NaiveDate, NaiveTime};
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

criterion_group!(benches, bench_single_leg_sim, bench_100_day_sim);
criterion_main!(benches);
