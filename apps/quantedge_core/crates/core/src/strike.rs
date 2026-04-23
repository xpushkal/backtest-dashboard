//! Strike selection for option legs.
//!
//! Finds the correct bar for a leg's desired strike from a set
//! of bars at a given timestamp. Supports 4 modes:
//! - ATM Offset: exact strike_offset match
//! - Premium: closest premium to target
//! - Percent OTM: closest offset to desired strike_offset
//! - Delta: stubbed (falls back to ATM+0 until Phase 5 Greeks)

use crate::config::{LegConfig, OptionType, StrikeMode};

/// Selects the correct strike for a leg.
pub struct StrikeSelector;

impl StrikeSelector {
    /// Find the bar index matching the desired strike for a leg config.
    ///
    /// Returns `(index, entry_price, spot)` or None if no match.
    pub fn select(
        leg: &LegConfig,
        option_types: &[&str],
        strike_offsets: &[i32],
        closes: &[f64],
        spots: &[f64],
    ) -> Option<(usize, f64, f64)> {
        let target_type = match leg.option_type {
            OptionType::CE => "CE",
            OptionType::PE => "PE",
        };

        match leg.strike_mode {
            StrikeMode::AtmOffset => {
                for i in 0..option_types.len() {
                    if option_types[i] == target_type
                        && strike_offsets[i] == leg.strike_offset
                    {
                        return Some((i, closes[i], spots[i]));
                    }
                }
                None
            }

            StrikeMode::Premium => {
                // Select strike where premium (close) is closest to target
                let target = leg.premium_target.unwrap_or(200.0);
                let mut best: Option<(usize, f64, f64)> = None;
                let mut best_diff = f64::MAX;
                for i in 0..option_types.len() {
                    if option_types[i] == target_type {
                        let diff = (closes[i] - target).abs();
                        if diff < best_diff {
                            best_diff = diff;
                            best = Some((i, closes[i], spots[i]));
                        }
                    }
                }
                best
            }

            StrikeMode::PercentOtm => {
                // Select strike closest to desired offset
                // In Phase 3 we use strike_offset as the proximity key
                let mut best: Option<(usize, f64, f64)> = None;
                let mut best_diff = i32::MAX;
                for i in 0..option_types.len() {
                    if option_types[i] == target_type {
                        let diff = (strike_offsets[i] - leg.strike_offset).abs();
                        if diff < best_diff {
                            best_diff = diff;
                            best = Some((i, closes[i], spots[i]));
                        }
                    }
                }
                best
            }

            StrikeMode::Delta => {
                // Stub: requires Greeks engine (Phase 5)
                // Fallback to ATM+0
                for i in 0..option_types.len() {
                    if option_types[i] == target_type && strike_offsets[i] == 0 {
                        return Some((i, closes[i], spots[i]));
                    }
                }
                // If no ATM+0, pick first matching type
                for i in 0..option_types.len() {
                    if option_types[i] == target_type {
                        return Some((i, closes[i], spots[i]));
                    }
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PositionSide, SlType};

    fn make_leg(option_type: OptionType, offset: i32) -> LegConfig {
        make_leg_with_mode(option_type, StrikeMode::AtmOffset, offset, None)
    }

    fn make_leg_with_mode(
        option_type: OptionType,
        mode: StrikeMode,
        offset: i32,
        premium_target: Option<f64>,
    ) -> LegConfig {
        LegConfig {
            option_type,
            position: PositionSide::Sell,
            lots: 1,
            expiry: "weekly".to_string(),
            strike_mode: mode,
            strike_offset: offset,
            delta_target: None,
            premium_target,
            stop_loss_enabled: false,
            stop_loss_type: SlType::None,
            stop_loss_value: 0.0,
            target_profit_enabled: false,
            target_profit_type: SlType::None,
            target_profit_value: 0.0,
            trail_sl_enabled: false,
            trail_sl_activate_at: 0.0,
            trail_sl_lock_in: 0.0,
            trail_sl_mode: crate::config::TrailSlMode::Trail,
            trail_sl_unit: crate::config::TrailUnit::Percent,
            trail_sl_value: 0.0,
            reentry_on_sl: false,
            reentry_on_target: false,
            reentry_mode: crate::config::ReEntryMode::AfterNBars,
            reentry_cooldown_bars: 5,
            reentry_max_attempts: 2,
            momentum_filter_enabled: false,
        }
    }

    #[test]
    fn test_select_atm_ce() {
        let types = vec!["CE", "PE", "CE", "PE"];
        let offsets = vec![0, 0, 5, 5];
        let closes = vec![200.0, 180.0, 150.0, 120.0];
        let spots = vec![48000.0, 48000.0, 48000.0, 48000.0];

        let leg = make_leg(OptionType::CE, 0);
        let result = StrikeSelector::select(&leg, &types, &offsets, &closes, &spots);
        assert!(result.is_some());
        let (idx, price, spot) = result.unwrap();
        assert_eq!(idx, 0);
        assert_eq!(price, 200.0);
        assert_eq!(spot, 48000.0);
    }

    #[test]
    fn test_select_atm_pe() {
        let types = vec!["CE", "PE"];
        let offsets = vec![0, 0];
        let closes = vec![200.0, 180.0];
        let spots = vec![48000.0, 48000.0];

        let leg = make_leg(OptionType::PE, 0);
        let result = StrikeSelector::select(&leg, &types, &offsets, &closes, &spots);
        let (idx, price, _) = result.unwrap();
        assert_eq!(idx, 1);
        assert_eq!(price, 180.0);
    }

    #[test]
    fn test_select_no_match() {
        let types = vec!["CE"];
        let offsets = vec![5]; // we want offset 0
        let closes = vec![200.0];
        let spots = vec![48000.0];

        let leg = make_leg(OptionType::CE, 0);
        assert!(StrikeSelector::select(&leg, &types, &offsets, &closes, &spots).is_none());
    }

    // ─── Premium Mode Tests ────────────────────────────────

    #[test]
    fn test_select_premium_mode() {
        let leg = make_leg_with_mode(OptionType::CE, StrikeMode::Premium, 0, Some(150.0));
        let types = vec!["CE", "CE", "CE"];
        let offsets = vec![-5, 0, 5];
        let closes = vec![300.0, 200.0, 120.0]; // 120 closest to target 150
        let spots = vec![48000.0; 3];
        let (idx, price, _) = StrikeSelector::select(&leg, &types, &offsets, &closes, &spots).unwrap();
        assert_eq!(idx, 2);
        assert_eq!(price, 120.0);
    }

    #[test]
    fn test_select_premium_mode_exact_match() {
        let leg = make_leg_with_mode(OptionType::CE, StrikeMode::Premium, 0, Some(200.0));
        let types = vec!["CE", "CE"];
        let offsets = vec![0, 5];
        let closes = vec![200.0, 150.0];
        let spots = vec![48000.0; 2];
        let (idx, _, _) = StrikeSelector::select(&leg, &types, &offsets, &closes, &spots).unwrap();
        assert_eq!(idx, 0); // exact match
    }

    // ─── Delta Mode Tests ──────────────────────────────────

    #[test]
    fn test_select_delta_mode_fallback() {
        let leg = make_leg_with_mode(OptionType::CE, StrikeMode::Delta, 0, None);
        let types = vec!["CE", "CE"];
        let offsets = vec![0, 5];
        let closes = vec![200.0, 150.0];
        let spots = vec![48000.0; 2];
        let (idx, _, _) = StrikeSelector::select(&leg, &types, &offsets, &closes, &spots).unwrap();
        assert_eq!(idx, 0); // ATM+0 fallback
    }

    #[test]
    fn test_select_delta_mode_no_atm() {
        // No ATM+0 available, falls back to first matching type
        let leg = make_leg_with_mode(OptionType::CE, StrikeMode::Delta, 0, None);
        let types = vec!["CE", "CE"];
        let offsets = vec![3, 5];
        let closes = vec![180.0, 150.0];
        let spots = vec![48000.0; 2];
        let (idx, _, _) = StrikeSelector::select(&leg, &types, &offsets, &closes, &spots).unwrap();
        assert_eq!(idx, 0); // first CE
    }

    // ─── PercentOtm Mode Tests ─────────────────────────────

    #[test]
    fn test_select_percent_otm_mode() {
        let leg = make_leg_with_mode(OptionType::CE, StrikeMode::PercentOtm, 3, None);
        let types = vec!["CE", "CE", "CE"];
        let offsets = vec![0, 2, 5];
        let closes = vec![200.0, 160.0, 100.0];
        let spots = vec![48000.0; 3];
        let (idx, _, _) = StrikeSelector::select(&leg, &types, &offsets, &closes, &spots).unwrap();
        assert_eq!(idx, 1); // offset 2 closest to target 3
    }
}
