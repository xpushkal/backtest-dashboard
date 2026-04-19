//! Strike selection for option legs.
//!
//! Finds the correct bar for a leg's desired strike from a set
//! of bars at a given timestamp.

use crate::config::{LegConfig, OptionType, StrikeMode};

/// Selects the correct strike for a leg.
pub struct StrikeSelector;

impl StrikeSelector {
    /// Find the bar index matching the desired strike for a leg config.
    ///
    /// For ATM offset mode: find bar where `strike_offset == leg.strike_offset`
    /// and `option_type` matches.
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
            // Delta, Premium, PercentOtm → Phase 3
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PositionSide, SlType};

    fn make_leg(option_type: OptionType, offset: i32) -> LegConfig {
        LegConfig {
            option_type,
            position: PositionSide::Sell,
            lots: 1,
            expiry: "weekly".to_string(),
            strike_mode: StrikeMode::AtmOffset,
            strike_offset: offset,
            delta_target: None,
            premium_target: None,
            stop_loss_enabled: false,
            stop_loss_type: SlType::None,
            stop_loss_value: 0.0,
            target_profit_enabled: false,
            target_profit_type: SlType::None,
            target_profit_value: 0.0,
            trail_sl_enabled: false,
            trail_sl_activate_at: 0.0,
            trail_sl_lock_in: 0.0,
            trail_sl_type: "percent".to_string(),
            trail_sl_value: 0.0,
            reentry_on_sl: false,
            reentry_on_target: false,
            reentry_mode: "after_n_bars".to_string(),
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
}
