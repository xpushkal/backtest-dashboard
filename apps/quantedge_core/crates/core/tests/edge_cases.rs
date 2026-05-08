//! Edge case tests for the simulation engine.
//!
//! Covers all 9 PRD edge cases (OPT-05) plus 3 additional robustness tests.

use chrono::{Datelike, NaiveDate, NaiveTime};
use quantedge_core::runner::{SimBar, SimRunner};
use quantedge_core::StrategyConfig;

// ─── Helper: Generate basic bars ────────────────────────────

fn make_bar(date: NaiveDate, time: NaiveTime, close: f64, spot: f64) -> SimBar {
    SimBar {
        date,
        time,
        option_type: "CE".to_string(),
        strike_offset: 0,
        close,
        high: close,
        low: close,
        spot,
    }
}

fn make_bars_for_days(n_days: u32, entry_h: u32, entry_m: u32) -> Vec<SimBar> {
    let mut bars = Vec::new();
    let base = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
    let mut rng: u64 = 99999;

    for d in 0..n_days {
        let date = base + chrono::Duration::days(d as i64);
        if date.weekday() == chrono::Weekday::Sat || date.weekday() == chrono::Weekday::Sun {
            continue;
        }

        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let base_price = 180.0 + ((rng >> 33) as f64 / u32::MAX as f64) * 60.0;
        let spot = 48000.0 + d as f64 * 2.0;

        let times = [
            NaiveTime::from_hms_opt(entry_h, entry_m, 0).unwrap(),
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
        ];

        for (i, &t) in times.iter().enumerate() {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let factor = 0.80 + ((rng >> 33) as f64 / u32::MAX as f64) * 0.50;
            bars.push(make_bar(date, t, base_price * factor, spot + i as f64 * 30.0));
        }
    }
    bars
}

fn basic_toml(entry_time: &str, exit_time: &str) -> String {
    format!(
        r#"
[strategy]
name = "EdgeTest"
underlying = "BANKNIFTY"
entry_time = "{}"
exit_time = "{}"
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
"#,
        entry_time, exit_time
    )
}

// ─── Test 1: Zero trades (entry_time after all bars) ────────

#[test]
fn test_zero_trades_entry_after_last_bar() {
    let bars = make_bars_for_days(50, 9, 20);
    // Entry time after all bars → no trades
    let config = StrategyConfig::from_toml_str(&basic_toml("15:25", "15:30")).unwrap();
    let result = SimRunner::run(&config, &bars, 15);
    assert!(result.trades.is_empty(), "Expected 0 trades when entry is after last bar");
}

// ─── Test 2: First-bar entry ────────────────────────────────

#[test]
fn test_first_bar_entry() {
    let bars = make_bars_for_days(50, 9, 20);
    let config = StrategyConfig::from_toml_str(&basic_toml("09:20", "15:20")).unwrap();
    let result = SimRunner::run(&config, &bars, 15);
    // Should have trades (bars start at 09:20)
    assert!(!result.trades.is_empty(), "Should open trades at first bar");
    // No panic = success
}

// ─── Test 3: Last-bar exit (EndOfData) ──────────────────────

#[test]
fn test_last_bar_exit() {
    let bars = make_bars_for_days(10, 9, 20);
    // Exit time after the last bar → EndOfData exit
    let config = StrategyConfig::from_toml_str(&basic_toml("09:20", "15:30")).unwrap();
    let result = SimRunner::run(&config, &bars, 15);
    // Should have trades and no panic
    assert!(!result.trades.is_empty());
}

// ─── Test 4: Lot size consistency ───────────────────────────

#[test]
fn test_lot_size_consistency() {
    let bars = make_bars_for_days(100, 9, 20);
    let config = StrategyConfig::from_toml_str(&basic_toml("09:20", "15:20")).unwrap();

    // Run with lot_size=15
    let result = SimRunner::run(&config, &bars, 15);
    // All PnL calculations should be consistent (no mid-run lot change)
    for trade in &result.trades {
        // PnL should be finite
        assert!(trade.pnl_net.is_finite(), "PnL should be finite");
        assert!(trade.pnl_gross.is_finite(), "Gross PnL should be finite");
    }
}

// ─── Test 5: Lot size change mid-trade ──────────────────────

#[test]
fn test_lot_size_change_between_runs() {
    let bars = make_bars_for_days(50, 9, 20);
    let config = StrategyConfig::from_toml_str(&basic_toml("09:20", "15:20")).unwrap();

    // Same bars, different lot sizes
    let result_15 = SimRunner::run(&config, &bars, 15);
    let result_30 = SimRunner::run(&config, &bars, 30);

    // Both should work without panic
    assert!(!result_15.trades.is_empty());
    assert!(!result_30.trades.is_empty());
    // lot_size=30 should have ~2x PnL per trade
    if let (Some(t15), Some(t30)) = (result_15.trades.first(), result_30.trades.first()) {
        if t15.pnl_gross.abs() > 0.01 {
            let ratio = t30.pnl_gross / t15.pnl_gross;
            assert!(
                (ratio - 2.0).abs() < 0.5,
                "Lot size 30 should produce ~2x PnL of lot 15, got ratio={}",
                ratio
            );
        }
    }
}

// ─── Test 6: Zero-price bars ────────────────────────────────

#[test]
fn test_zero_price_bars() {
    let base_date = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
    let mut bars = Vec::new();

    for d in 0..30 {
        let date = base_date + chrono::Duration::days(d);
        if date.weekday() == chrono::Weekday::Sat || date.weekday() == chrono::Weekday::Sun {
            continue;
        }

        // First bar has zero close
        bars.push(make_bar(
            date,
            NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            0.0, // zero price!
            48000.0,
        ));
        bars.push(make_bar(
            date,
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            180.0,
            48000.0,
        ));
        bars.push(make_bar(
            date,
            NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
            170.0,
            48000.0,
        ));
    }

    let config = StrategyConfig::from_toml_str(&basic_toml("09:20", "15:20")).unwrap();
    let result = SimRunner::run(&config, &bars, 15);

    // Should not panic. PnL should not contain NaN.
    for trade in &result.trades {
        assert!(!trade.pnl_net.is_nan(), "PnL should not be NaN");
    }
}

// ─── Test 7: Max concurrent trades limit ────────────────────

#[test]
fn test_max_concurrent_limit() {
    let toml = r#"
[strategy]
name = "MaxConcurrent"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0
brokerage_per_lot = 40.0
max_concurrent_trades = 1

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = false

[overall]
overall_sl_enabled = false
"#;

    let bars = make_bars_for_days(100, 9, 20);
    let config = StrategyConfig::from_toml_str(toml).unwrap();
    let result = SimRunner::run(&config, &bars, 15);

    // With max_concurrent=1, should never have overlapping trades
    // Each trade should close before the next opens
    for w in result.trades.windows(2) {
        assert!(
            w[0].exit_date <= w[1].entry_date,
            "Trade {} exit {} should be <= trade {} entry {}",
            0, w[0].exit_date, 1, w[1].entry_date
        );
    }
}

// ─── Test 8: Re-entry exhaustion ────────────────────────────

#[test]
fn test_reentry_exhaustion() {
    let toml = r#"
[strategy]
name = "ReentryTest"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0
brokerage_per_lot = 40.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 10.0
reentry_on_sl = true
reentry_mode = "asap"
reentry_max_attempts = 2
reentry_cooldown_bars = 0

[overall]
overall_sl_enabled = false
"#;

    let bars = make_bars_for_days(200, 9, 20);
    let config = StrategyConfig::from_toml_str(toml).unwrap();
    let result = SimRunner::run(&config, &bars, 15);

    // Should not panic. Verify reentry_attempt never exceeds max_attempts
    for trade in &result.trades {
        assert!(
            trade.reentry_attempt <= 2,
            "Re-entry attempt {} exceeds max 2",
            trade.reentry_attempt
        );
    }
}

// ─── Test 9: Empty bar data ─────────────────────────────────

#[test]
fn test_empty_bars() {
    let config = StrategyConfig::from_toml_str(&basic_toml("09:20", "15:20")).unwrap();
    let bars: Vec<SimBar> = vec![];
    let result = SimRunner::run(&config, &bars, 15);
    assert!(result.trades.is_empty());
}

// ─── Test 10: Single bar ────────────────────────────────────

#[test]
fn test_single_bar() {
    let config = StrategyConfig::from_toml_str(&basic_toml("09:20", "15:20")).unwrap();
    let bars = vec![make_bar(
        NaiveDate::from_ymd_opt(2021, 1, 4).unwrap(),
        NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
        200.0,
        48000.0,
    )];
    let result = SimRunner::run(&config, &bars, 15);
    // Should not panic — may or may not produce a trade
    for trade in &result.trades {
        assert!(trade.pnl_net.is_finite());
    }
}

// ─── Test 11: Weekend gap ───────────────────────────────────

#[test]
fn test_weekend_gap() {
    let mut bars = Vec::new();
    // Friday
    let fri = NaiveDate::from_ymd_opt(2021, 1, 8).unwrap();
    bars.push(make_bar(fri, NaiveTime::from_hms_opt(9, 20, 0).unwrap(), 200.0, 48000.0));
    bars.push(make_bar(fri, NaiveTime::from_hms_opt(15, 20, 0).unwrap(), 190.0, 48050.0));

    // Monday (skip Sat/Sun)
    let mon = NaiveDate::from_ymd_opt(2021, 1, 11).unwrap();
    bars.push(make_bar(mon, NaiveTime::from_hms_opt(9, 20, 0).unwrap(), 195.0, 48100.0));
    bars.push(make_bar(mon, NaiveTime::from_hms_opt(15, 20, 0).unwrap(), 185.0, 48150.0));

    let config = StrategyConfig::from_toml_str(&basic_toml("09:20", "15:20")).unwrap();
    let result = SimRunner::run(&config, &bars, 15);
    // Should handle the Friday→Monday gap without panic
    for trade in &result.trades {
        assert!(trade.pnl_net.is_finite());
    }
}

// ─── Test 12: Large bar set performance ─────────────────────

#[test]
fn test_large_bar_set_no_panic() {
    let bars = make_bars_for_days(1500, 9, 20);
    let config = StrategyConfig::from_toml_str(&basic_toml("09:20", "15:20")).unwrap();

    let start = std::time::Instant::now();
    let result = SimRunner::run(&config, &bars, 15);
    let elapsed = start.elapsed();

    assert!(!result.trades.is_empty());
    assert!(elapsed.as_secs() < 5, "Should complete 1500-day run in <5s, took {:?}", elapsed);
}
