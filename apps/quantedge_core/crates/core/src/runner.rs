//! Simulation runner — the core backtest loop.
//!
//! Iterates over bars grouped by (date, time), opens/closes positions,
//! and captures equity snapshots. This is the heart of the backtester.

use crate::config::{ExitReason, PositionSide, ReEntryMode, StrategyConfig};
use crate::execution::ExecutionEngine;
use crate::leg::Leg;
use crate::position::{ClosedTrade, Position, PositionSnapshot};
use crate::reentry::ReEntryTracker;
use crate::strike::StrikeSelector;
use chrono::{NaiveDate, NaiveTime};
use std::collections::BTreeMap;

/// Result of running a backtest.
#[derive(Debug, Clone)]
pub struct RunResult {
    pub trades: Vec<ClosedTrade>,
    pub snapshots: Vec<PositionSnapshot>,
    pub total_bars: usize,
}

/// A single bar row with the fields needed by the simulation.
/// This is a lightweight view — the runner doesn't depend on the data crate's Bar.
#[derive(Debug, Clone)]
pub struct SimBar {
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub option_type: String,
    pub strike_offset: i32,
    pub close: f64,
    pub spot: f64,
}



/// The simulation runner.
pub struct SimRunner;

impl SimRunner {
    /// Run a full backtest.
    ///
    /// # Arguments
    /// * `config` - Strategy configuration
    /// * `bars` - All bar data (must be sorted by date, time)
    /// * `lot_size` - Lot size for the underlying
    pub fn run(config: &StrategyConfig, bars: &[SimBar], lot_size: u32) -> RunResult {
        if bars.is_empty() {
            return RunResult {
                trades: Vec::new(),
                snapshots: Vec::new(),
                total_bars: 0,
            };
        }

        let mut trades: Vec<ClosedTrade> = Vec::new();
        let mut snapshots: Vec<PositionSnapshot> = Vec::with_capacity(bars.len() / 20);
        let mut position: Option<Position> = None;
        let mut cumulative_pnl: f64 = 0.0;

        // Re-entry trackers: one per leg
        let mut trackers: Vec<ReEntryTracker> = config
            .legs
            .iter()
            .map(|leg| ReEntryTracker::new(leg))
            .collect();
        let mut reentry_armed = false; // true when we have pending re-entries

        // Group bar indices by (date, time)
        let groups = Self::group_by_time(bars);

        for ((date, time), indices) in &groups {
            let date = *date;
            let time = *time;

            // Tick re-entry trackers when no position is open
            if position.is_none() && reentry_armed {
                for tracker in &mut trackers {
                    tracker.tick();
                }
            }

            // 1. ENTRY: no open position
            if position.is_none() {
                let should_enter = if reentry_armed {
                    // Re-entry: check if any tracker says ready
                    let any_ready = trackers.iter().any(|t| t.should_reenter());
                    if any_ready {
                        match trackers[0].state {
                            // SameTime: only re-enter at original entry_time
                            _ if config.legs.iter().any(|l| l.reentry_mode == ReEntryMode::SameTime) => {
                                time == config.entry_time()
                            }
                            _ => true,
                        }
                    } else {
                        false
                    }
                } else {
                    // Normal entry: at entry_time
                    time == config.entry_time()
                };

                if should_enter {
                    if let Some(pos) = Self::try_open(config, bars, indices, lot_size, date, time) {
                        if reentry_armed {
                            for tracker in &mut trackers {
                                tracker.confirm_reentry();
                            }
                            reentry_armed = false;
                        }
                        position = Some(pos);
                    }
                }
            }

            // 2. UPDATE: mark-to-market all legs
            if let Some(ref mut pos) = position {
                Self::update_position(pos, bars, indices);

                // 3. EXIT CHECKS (priority: SL → Target → Time)
                if let Some(reason) = Self::check_exits(config, pos, time) {
                    // Capture re-entry attempt number before closing
                    let attempt = trackers.first().map_or(0, |t| t.completed_attempts);

                    let closed = Self::close_position(
                        pos, config, bars, indices, date, time, reason, lot_size, attempt,
                    );
                    for trade in closed {
                        cumulative_pnl += trade.pnl_net;
                        trades.push(trade);
                    }
                    position = None;

                    // Arm re-entry trackers
                    for tracker in &mut trackers {
                        tracker.on_exit(&reason);
                    }
                    reentry_armed = trackers.iter().any(|t| !t.is_exhausted() && !matches!(t.state, crate::reentry::ReEntryState::Idle));
                }
            }

            // 4. SNAPSHOT
            let unrealized = position.as_ref().map_or(0.0, |p| p.total_unrealized_pnl());
            let spot = bars[indices[0]].spot;
            snapshots.push(PositionSnapshot {
                date,
                time,
                spot,
                equity: config.capital() + cumulative_pnl + unrealized,
                unrealized_pnl: unrealized,
                cumulative_pnl,
            });
        }

        // Close any remaining position at end of data
        if let Some(ref pos) = position {
            if let Some((&(date, time), indices)) = groups.iter().last() {
                let attempt = trackers.first().map_or(0, |t| t.completed_attempts);
                let closed = Self::close_position(
                    pos, config, bars, indices, date, time, ExitReason::EndOfData, lot_size, attempt,
                );
                for trade in closed {
                    cumulative_pnl += trade.pnl_net;
                    trades.push(trade);
                }
            }
        }

        RunResult {
            trades,
            snapshots,
            total_bars: bars.len(),
        }
    }

    /// Group bar indices by (date, time) using BTreeMap for sorted order.
    fn group_by_time(bars: &[SimBar]) -> BTreeMap<(NaiveDate, NaiveTime), Vec<usize>> {
        let mut groups: BTreeMap<(NaiveDate, NaiveTime), Vec<usize>> = BTreeMap::new();
        for (i, bar) in bars.iter().enumerate() {
            groups.entry((bar.date, bar.time)).or_default().push(i);
        }
        groups
    }

    /// Try to open a position at the current timestamp.
    fn try_open(
        config: &StrategyConfig,
        bars: &[SimBar],
        indices: &[usize],
        lot_size: u32,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Option<Position> {
        let mut legs = Vec::with_capacity(config.legs.len());

        // Collect bar data at this timestamp
        let option_types: Vec<&str> = indices.iter().map(|&i| bars[i].option_type.as_str()).collect();
        let strike_offsets: Vec<i32> = indices.iter().map(|&i| bars[i].strike_offset).collect();
        let closes: Vec<f64> = indices.iter().map(|&i| bars[i].close).collect();
        let spots: Vec<f64> = indices.iter().map(|&i| bars[i].spot).collect();

        for leg_config in &config.legs {
            if let Some((_idx, raw_price, spot)) = StrikeSelector::select(
                leg_config, &option_types, &strike_offsets, &closes, &spots,
            ) {
                // Apply entry slippage
                let entry_price = ExecutionEngine::apply_slippage(
                    raw_price,
                    leg_config.position,
                    &config.strategy.slippage_model,
                    config.strategy.slippage_value,
                );
                let leg = Leg::new(leg_config, entry_price, spot, lot_size);
                legs.push(leg);
            }
        }

        if legs.is_empty() {
            return None;
        }

        let entry_brokerage: f64 = legs
            .iter()
            .map(|l| config.strategy.brokerage_per_lot * l.lots as f64)
            .sum();

        Some(Position::new(legs, date, time, 0, entry_brokerage))
    }

    /// Update position's legs with current bar prices.
    fn update_position(pos: &mut Position, bars: &[SimBar], indices: &[usize]) {
        for leg in &mut pos.legs {
            let target_type = match leg.config.option_type {
                crate::config::OptionType::CE => "CE",
                crate::config::OptionType::PE => "PE",
            };
            // Find matching bar for this leg
            for &idx in indices {
                if bars[idx].option_type == target_type
                    && bars[idx].strike_offset == leg.config.strike_offset
                {
                    leg.update(bars[idx].close, bars[idx].spot);
                    break;
                }
            }
        }
    }

    /// Check exit conditions in strict priority order.
    ///
    /// OCO (One-Cancels-Other) is implicit in the priority chain:
    /// - If per-leg SL fires (priority 1), target (priority 3) is never checked
    /// - If combined SL fires (priority 2), per-leg target is cancelled
    /// - SL always takes precedence over target on the same bar
    fn check_exits(
        config: &StrategyConfig,
        pos: &Position,
        time: NaiveTime,
    ) -> Option<ExitReason> {
        // Priority 1: Per-leg SL (hardest limit)
        for leg in &pos.legs {
            if let Some(r) = leg.check_sl() {
                return Some(r);
            }
        }

        // Priority 2: Combined / Overall SL
        let monitor = crate::strategy::CombinedSlMonitor::new(&config.overall);
        let total_pnl = pos.total_unrealized_pnl();
        let total_entry_premium: f64 = pos
            .legs
            .iter()
            .map(|l| l.entry_price * l.quantity())
            .sum();
        if let Some(r) = monitor.check_overall_sl(total_pnl, total_entry_premium) {
            return Some(r);
        }

        // Priority 3: Per-leg target
        for leg in &pos.legs {
            if let Some(r) = leg.check_target() {
                return Some(r);
            }
        }

        // Priority 4: Overall target
        if let Some(r) = monitor.check_overall_target(total_pnl, total_entry_premium) {
            return Some(r);
        }

        // Priority 5: Time exit (lowest priority)
        if time >= config.exit_time() {
            return Some(ExitReason::TimeExit);
        }

        None
    }

    fn close_position(
        pos: &Position,
        config: &StrategyConfig,
        bars: &[SimBar],
        indices: &[usize],
        date: NaiveDate,
        time: NaiveTime,
        reason: ExitReason,
        lot_size: u32,
        reentry_attempt: u32,
    ) -> Vec<ClosedTrade> {
        let mut trades = Vec::with_capacity(pos.legs.len());

        for leg in &pos.legs {
            // Find exit price from matching bar
            let target_type = match leg.config.option_type {
                crate::config::OptionType::CE => "CE",
                crate::config::OptionType::PE => "PE",
            };
            let mut exit_price = leg.current_price;
            let mut exit_spot = leg.current_spot;

            for &idx in indices {
                if bars[idx].option_type == target_type
                    && bars[idx].strike_offset == leg.config.strike_offset
                {
                    exit_price = bars[idx].close;
                    exit_spot = bars[idx].spot;
                    break;
                }
            }

            // Apply exit slippage (opposite direction)
            let exit_side = match leg.config.position {
                PositionSide::Buy => PositionSide::Sell,
                PositionSide::Sell => PositionSide::Buy,
            };
            let slipped_exit = ExecutionEngine::apply_slippage(
                exit_price,
                exit_side,
                &config.strategy.slippage_model,
                config.strategy.slippage_value,
            );

            let brokerage = ExecutionEngine::calculate_brokerage(
                config.strategy.brokerage_per_lot,
                leg.lots,
            );

            // STT: on the sell side of the transaction
            let sell_price = match leg.config.position {
                PositionSide::Sell => leg.entry_price,   // sold at entry
                PositionSide::Buy => slipped_exit,       // sold at exit
            };
            let stt = ExecutionEngine::calculate_stt(
                sell_price,
                leg.lots * lot_size,
                config.strategy.stt_on_sell,
            );

            let slippage_cost = ExecutionEngine::calculate_slippage_cost(
                leg.entry_price,
                exit_price,
                &config.strategy.slippage_model,
                config.strategy.slippage_value,
                leg.lots,
                lot_size,
            );

            let trade = ClosedTrade::from_leg(
                leg,
                pos.entry_date,
                pos.entry_time,
                date,
                time,
                slipped_exit,
                exit_spot,
                reason,
                brokerage,
                stt,
                slippage_cost,
                reentry_attempt,
            );
            trades.push(trade);
        }

        trades
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StrategyConfig;

    const TEST_TOML: &str = r#"
[strategy]
name = "Test Short CE"
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

    /// Generate bars for one trading day.
    fn make_day_bars(
        date: NaiveDate,
        entry_close: f64,
        exit_close: f64,
        sl_trigger_close: Option<f64>,
    ) -> Vec<SimBar> {
        let mut bars = Vec::new();
        let spot = 48000.0;

        // Entry bar at 09:20
        bars.push(SimBar {
            date,
            time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            option_type: "CE".to_string(),
            strike_offset: 0,
            close: entry_close,
            spot,
        });

        // Mid-day bars (09:21 to 15:19)
        // If SL trigger, one bar shows the SL hit price
        if let Some(sl_price) = sl_trigger_close {
            bars.push(SimBar {
                date,
                time: NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
                option_type: "CE".to_string(),
                strike_offset: 0,
                close: sl_price,
                spot: spot + 200.0,
            });
        } else {
            // Normal mid-day bar (price trending down = profit for sell)
            let mid = (entry_close + exit_close) / 2.0;
            bars.push(SimBar {
                date,
                time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                option_type: "CE".to_string(),
                strike_offset: 0,
                close: mid,
                spot,
            });
        }

        // Exit bar at 15:20
        bars.push(SimBar {
            date,
            time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
            option_type: "CE".to_string(),
            strike_offset: 0,
            close: exit_close,
            spot: spot - 50.0,
        });

        bars
    }

    #[test]
    fn test_single_day_time_exit() {
        let config = StrategyConfig::from_toml_str(TEST_TOML).unwrap();
        let bars = make_day_bars(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            200.0, 185.0, None,
        );
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].exit_reason, ExitReason::TimeExit);
        assert!(result.trades[0].pnl_gross > 0.0); // sell at 200, exit ~185 → profit
    }

    #[test]
    fn test_sl_triggered_intraday() {
        let config = StrategyConfig::from_toml_str(TEST_TOML).unwrap();
        // Entry at ~199 (after slippage), SL=100%, so SL at ~398
        // SL trigger bar at 420 → definitely hits 100% SL
        let bars = make_day_bars(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            200.0, 150.0, Some(420.0),
        );
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].exit_reason, ExitReason::StopLoss);
        assert!(result.trades[0].pnl_gross < 0.0);
    }

    #[test]
    fn test_no_entry_no_bars() {
        let config = StrategyConfig::from_toml_str(TEST_TOML).unwrap();
        let result = SimRunner::run(&config, &[], 15);
        assert_eq!(result.trades.len(), 0);
        assert_eq!(result.snapshots.len(), 0);
    }

    #[test]
    fn test_no_entry_wrong_time() {
        let config = StrategyConfig::from_toml_str(TEST_TOML).unwrap();
        // Bars only at 10:00, not at 09:20 → no entry
        let bars = vec![SimBar {
            date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            time: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            option_type: "CE".to_string(),
            strike_offset: 0,
            close: 200.0,
            spot: 48000.0,
        }];
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades.len(), 0);
    }

    #[test]
    fn test_multi_day_trades() {
        let config = StrategyConfig::from_toml_str(TEST_TOML).unwrap();
        let mut bars = Vec::new();
        for day in 15..18 {
            bars.extend(make_day_bars(
                NaiveDate::from_ymd_opt(2024, 1, day).unwrap(),
                200.0, 185.0, None,
            ));
        }
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades.len(), 3); // one trade per day
    }

    #[test]
    fn test_brokerage_deducted() {
        let config = StrategyConfig::from_toml_str(TEST_TOML).unwrap();
        let bars = make_day_bars(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            200.0, 185.0, None,
        );
        let result = SimRunner::run(&config, &bars, 15);
        assert!(result.trades[0].brokerage > 0.0);
        assert!(result.trades[0].pnl_net < result.trades[0].pnl_gross);
    }

    #[test]
    fn test_snapshots_captured() {
        let config = StrategyConfig::from_toml_str(TEST_TOML).unwrap();
        let bars = make_day_bars(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            200.0, 185.0, None,
        );
        let result = SimRunner::run(&config, &bars, 15);
        assert!(!result.snapshots.is_empty());
        // Equity should be close to capital (may differ by unrealized PnL + costs)
        let first_equity = result.snapshots[0].equity;
        assert!(first_equity > 400000.0 && first_equity < 600000.0);
    }

    #[test]
    fn test_end_of_data_close() {
        let config = StrategyConfig::from_toml_str(TEST_TOML).unwrap();
        // Only entry bar, no exit bar → position open at end
        let bars = vec![
            SimBar {
                date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
                option_type: "CE".to_string(),
                strike_offset: 0,
                close: 200.0,
                spot: 48000.0,
            },
            SimBar {
                date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                option_type: "CE".to_string(),
                strike_offset: 0,
                close: 190.0,
                spot: 47950.0,
            },
        ];
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].exit_reason, ExitReason::EndOfData);
    }

    // ─── Multi-Leg Tests ────────────────────────────────────

    const STRADDLE_TOML: &str = r#"
[strategy]
name = "Short Straddle"
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
stop_loss_value = 150.0

[[legs]]
option_type = "PE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 150.0

[overall]
overall_sl_enabled = true
overall_sl_type = "percent_of_premium"
overall_sl_value = 60.0
overall_target_enabled = true
overall_target_type = "percent_of_premium"
overall_target_value = 50.0
"#;

    /// Generate bars for a straddle day (CE + PE at each timestamp).
    fn make_straddle_bars(
        date: NaiveDate,
        ce_entry: f64,
        pe_entry: f64,
        ce_mid: f64,
        pe_mid: f64,
        ce_exit: f64,
        pe_exit: f64,
    ) -> Vec<SimBar> {
        let spot = 48000.0;
        vec![
            // Entry
            SimBar { date, time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(), option_type: "CE".to_string(), strike_offset: 0, close: ce_entry, spot },
            SimBar { date, time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(), option_type: "PE".to_string(), strike_offset: 0, close: pe_entry, spot },
            // Mid-day
            SimBar { date, time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(), option_type: "CE".to_string(), strike_offset: 0, close: ce_mid, spot: spot + 100.0 },
            SimBar { date, time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(), option_type: "PE".to_string(), strike_offset: 0, close: pe_mid, spot: spot + 100.0 },
            // Exit
            SimBar { date, time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(), option_type: "CE".to_string(), strike_offset: 0, close: ce_exit, spot: spot - 50.0 },
            SimBar { date, time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(), option_type: "PE".to_string(), strike_offset: 0, close: pe_exit, spot: spot - 50.0 },
        ]
    }

    #[test]
    fn test_straddle_opens_both_legs() {
        let config = StrategyConfig::from_toml_str(STRADDLE_TOML).unwrap();
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        // Both legs profitable, no SL hit
        let bars = make_straddle_bars(date, 200.0, 180.0, 190.0, 170.0, 180.0, 160.0);
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades.len(), 2); // one per leg
    }

    #[test]
    fn test_combined_sl_triggers() {
        let config = StrategyConfig::from_toml_str(STRADDLE_TOML).unwrap();
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        // CE entry=200, PE entry=180. Total premium = (200+180)*15 = 5700 (with slippage: ~199+179)
        // Mid: CE=350, PE=300 → big loss on both legs.
        // CE PnL ≈ (199-350)*15 = -2265, PE PnL ≈ (179-300)*15 = -1815
        // Total PnL ≈ -4080. Total entry premium ≈ (199+179)*15 = 5670
        // Loss% = 4080/5670*100 ≈ 72% > 60% → CombinedSl
        let bars = make_straddle_bars(date, 200.0, 180.0, 350.0, 300.0, 180.0, 160.0);
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades.len(), 2);
        assert_eq!(result.trades[0].exit_reason, ExitReason::CombinedSl);
        assert_eq!(result.trades[1].exit_reason, ExitReason::CombinedSl);
    }

    #[test]
    fn test_overall_target_triggers() {
        let config = StrategyConfig::from_toml_str(STRADDLE_TOML).unwrap();
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        // Both legs very profitable → overall target 50% hit
        // CE entry~199, PE entry~179. Mid: CE=90, PE=80
        // CE PnL = (199-90)*15 = 1635, PE PnL = (179-80)*15 = 1485
        // Total PnL = 3120. Premium = (199+179)*15 = 5670. 
        // Profit% = 3120/5670*100 ≈ 55% > 50% → CombinedTarget
        let bars = make_straddle_bars(date, 200.0, 180.0, 90.0, 80.0, 85.0, 75.0);
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades.len(), 2);
        assert_eq!(result.trades[0].exit_reason, ExitReason::CombinedTarget);
        assert_eq!(result.trades[1].exit_reason, ExitReason::CombinedTarget);
    }

    // ─── OCO Tests ──────────────────────────────────────────

    #[test]
    fn test_oco_sl_cancels_target() {
        // Scenario: per-leg SL fires (priority 1), target would also fire (priority 3)
        // Result: StopLoss returned, not Target
        let toml = r#"
[strategy]
name = "OCO Test"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "points"
stop_loss_value = 50.0
target_profit_enabled = true
target_profit_type = "points"
target_profit_value = 10.0

[overall]
"#;
        let config = StrategyConfig::from_toml_str(toml).unwrap();
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        // Entry at 200, mid at 280 → SL fires (80pt move > 50pt), but also target would fire
        // SL has priority → StopLoss
        let bars = vec![
            SimBar { date, time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(), option_type: "CE".to_string(), strike_offset: 0, close: 200.0, spot: 48000.0 },
            SimBar { date, time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(), option_type: "CE".to_string(), strike_offset: 0, close: 280.0, spot: 48280.0 },
            SimBar { date, time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(), option_type: "CE".to_string(), strike_offset: 0, close: 260.0, spot: 48260.0 },
        ];
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades[0].exit_reason, ExitReason::StopLoss);
    }

    #[test]
    fn test_time_exit_lowest_priority() {
        // No SL or target configured → time exit
        let toml = r#"
[strategy]
name = "Time Exit Test"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0

[overall]
"#;
        let config = StrategyConfig::from_toml_str(toml).unwrap();
        let bars = make_day_bars(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            200.0, 185.0, None,
        );
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades[0].exit_reason, ExitReason::TimeExit);
    }

    // ─── Re-entry Integration Tests ──────────────────────────

    const REENTRY_TOML_ASAP: &str = r#"
[strategy]
name = "Reentry ASAP Test"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0
brokerage_per_lot = 0.0
slippage_model = "fixed_pts"
slippage_value = 0.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 50.0
reentry_on_sl = true
reentry_mode = "asap"
reentry_max_attempts = 2
reentry_cooldown_bars = 0

[overall]
"#;

    #[test]
    fn test_asap_reentry_produces_extra_trade() {
        // Day 1: SL hit at 11:00, ASAP re-entry opens new position
        // At exit bar (15:20), the re-entry position closes
        let d1 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let mut bars = Vec::new();

        // Entry at 09:20, price=200
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 200.0, spot: 48000.0 });
        // SL trigger at 11:00: price=310 → 55% loss > 50% SL
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 310.0, spot: 48200.0 });
        // After re-entry, bar at 13:00
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(13, 0, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 280.0, spot: 48100.0 });
        // Exit at 15:20
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 250.0, spot: 48050.0 });

        let config = StrategyConfig::from_toml_str(REENTRY_TOML_ASAP).unwrap();
        let result = SimRunner::run(&config, &bars, 15);

        // Should have 2 trades: initial (SL) + re-entry (TimeExit)
        assert_eq!(result.trades.len(), 2);
        assert_eq!(result.trades[0].exit_reason, ExitReason::StopLoss);
        assert_eq!(result.trades[0].reentry_attempt, 0);
        assert_eq!(result.trades[1].exit_reason, ExitReason::TimeExit);
        assert_eq!(result.trades[1].reentry_attempt, 1);
    }

    #[test]
    fn test_no_reentry_on_time_exit() {
        // reentry_on_sl=true but position exits via TimeExit → no re-entry
        let d1 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let bars = make_day_bars(d1, 200.0, 185.0, None);

        let config = StrategyConfig::from_toml_str(REENTRY_TOML_ASAP).unwrap();
        let result = SimRunner::run(&config, &bars, 15);
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].exit_reason, ExitReason::TimeExit);
    }

    #[test]
    fn test_max_attempts_limits_reentry() {
        // max_attempts=2: initial + 2 re-entries = 3 total trades max
        let d1 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let mut bars = Vec::new();

        // Entry at 09:20 → SL at 10:00 → re-entry → SL at 11:00 → re-entry → SL at 12:00 → exhausted
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 200.0, spot: 48000.0 });
        // SL #1 at 10:00
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 310.0, spot: 48200.0 });
        // Re-entry #1 opens at 10:00 (ASAP), SL #2 at 11:00
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 480.0, spot: 48400.0 });
        // Re-entry #2 opens at 11:00 (ASAP), SL #3 at 12:00
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 730.0, spot: 48600.0 });
        // No more re-entry (exhausted), bar at 15:20
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 600.0, spot: 48500.0 });

        let config = StrategyConfig::from_toml_str(REENTRY_TOML_ASAP).unwrap();
        let result = SimRunner::run(&config, &bars, 15);

        // 3 SL trades: initial + 2 re-entries, then exhausted
        assert_eq!(result.trades.len(), 3);
        assert_eq!(result.trades[0].reentry_attempt, 0);
        assert_eq!(result.trades[1].reentry_attempt, 1);
        assert_eq!(result.trades[2].reentry_attempt, 2);
    }

    #[test]
    fn test_after_n_bars_reentry_waits() {
        let toml = r#"
[strategy]
name = "Reentry AfterN Test"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0
brokerage_per_lot = 0.0
slippage_model = "fixed_pts"
slippage_value = 0.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 50.0
reentry_on_sl = true
reentry_mode = "after_n_bars"
reentry_cooldown_bars = 2
reentry_max_attempts = 1

[overall]
"#;
        let d1 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let mut bars = Vec::new();

        // Entry at 09:20
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 200.0, spot: 48000.0 });
        // SL at 10:00
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 310.0, spot: 48200.0 });
        // Cooldown bar 1 at 10:30
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(10, 30, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 290.0, spot: 48150.0 });
        // Cooldown bar 2 at 11:00 → ready
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 280.0, spot: 48100.0 });
        // Re-entry opens at 11:30
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(11, 30, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 270.0, spot: 48080.0 });
        // Exit at 15:20
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 250.0, spot: 48050.0 });

        let config = StrategyConfig::from_toml_str(toml).unwrap();
        let result = SimRunner::run(&config, &bars, 15);

        // 2 trades: initial SL + re-entry TimeExit
        assert_eq!(result.trades.len(), 2);
        assert_eq!(result.trades[0].exit_reason, ExitReason::StopLoss);
        assert_eq!(result.trades[1].exit_reason, ExitReason::TimeExit);
        assert_eq!(result.trades[1].reentry_attempt, 1);
    }

    #[test]
    fn test_reentry_on_target_hit() {
        let toml = r#"
[strategy]
name = "Reentry on Target"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0
brokerage_per_lot = 0.0
slippage_model = "fixed_pts"
slippage_value = 0.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 0
target_profit_enabled = true
target_profit_type = "percent_of_premium"
target_profit_value = 30.0
reentry_on_target = true
reentry_mode = "asap"
reentry_max_attempts = 1
reentry_cooldown_bars = 0

[overall]
"#;
        let d1 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let mut bars = Vec::new();

        // Entry at 09:20, price=200
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 200.0, spot: 48000.0 });
        // Target hit at 11:00: 200→135 = 32.5% profit for sell → exceeds 30%
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 135.0, spot: 47800.0 });
        // After re-entry at 13:00
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(13, 0, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 120.0, spot: 47700.0 });
        // Exit at 15:20
        bars.push(SimBar { date: d1, time: NaiveTime::from_hms_opt(15, 20, 0).unwrap(),
            option_type: "CE".to_string(), strike_offset: 0, close: 100.0, spot: 47600.0 });

        let config = StrategyConfig::from_toml_str(toml).unwrap();
        let result = SimRunner::run(&config, &bars, 15);

        // 2 trades: target hit + re-entry time exit
        assert_eq!(result.trades.len(), 2);
        assert_eq!(result.trades[0].exit_reason, ExitReason::Target);
        assert_eq!(result.trades[1].exit_reason, ExitReason::TimeExit);
    }
}
