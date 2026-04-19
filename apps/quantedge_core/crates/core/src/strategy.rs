//! Combined SL/Target monitor for multi-leg strategies.
//!
//! Evaluates overall (portfolio-level) SL and target conditions
//! based on the aggregate PnL across all legs.

use crate::config::{ExitReason, OverallConfig, SlType};

/// Monitor for combined/overall SL and target checks.
///
/// Operates on the SUM of all leg PnLs vs overall thresholds.
#[derive(Debug, Clone)]
pub struct CombinedSlMonitor {
    pub overall: OverallConfig,
}

impl CombinedSlMonitor {
    pub fn new(overall: &OverallConfig) -> Self {
        Self {
            overall: overall.clone(),
        }
    }

    /// Check if combined (overall) SL is triggered.
    ///
    /// # Arguments
    /// * `total_pnl` - Sum of unrealized PnL across all legs
    /// * `total_entry_premium` - Sum of (entry_price × quantity) across all legs
    pub fn check_overall_sl(
        &self,
        total_pnl: f64,
        total_entry_premium: f64,
    ) -> Option<ExitReason> {
        if !self.overall.overall_sl_enabled {
            return None;
        }

        let triggered = match self.overall.overall_sl_type {
            SlType::PercentOfPremium => {
                let loss_pct = if total_entry_premium > 0.0 {
                    (-total_pnl / total_entry_premium) * 100.0
                } else {
                    0.0
                };
                loss_pct >= self.overall.overall_sl_value
            }
            SlType::CombinedPremium | SlType::Points => {
                -total_pnl >= self.overall.overall_sl_value
            }
            _ => false,
        };

        if triggered {
            Some(ExitReason::CombinedSl)
        } else {
            None
        }
    }

    /// Check if combined (overall) target is hit.
    pub fn check_overall_target(
        &self,
        total_pnl: f64,
        total_entry_premium: f64,
    ) -> Option<ExitReason> {
        if !self.overall.overall_target_enabled {
            return None;
        }

        let triggered = match self.overall.overall_target_type {
            SlType::PercentOfPremium => {
                let profit_pct = if total_entry_premium > 0.0 {
                    (total_pnl / total_entry_premium) * 100.0
                } else {
                    0.0
                };
                profit_pct >= self.overall.overall_target_value
            }
            SlType::Points => total_pnl >= self.overall.overall_target_value,
            _ => false,
        };

        if triggered {
            Some(ExitReason::CombinedTarget)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_overall(sl_enabled: bool, sl_type: SlType, sl_val: f64, tgt_enabled: bool, tgt_type: SlType, tgt_val: f64) -> OverallConfig {
        OverallConfig {
            overall_sl_enabled: sl_enabled,
            overall_sl_type: sl_type,
            overall_sl_value: sl_val,
            overall_target_enabled: tgt_enabled,
            overall_target_type: tgt_type,
            overall_target_value: tgt_val,
        }
    }

    #[test]
    fn test_overall_sl_disabled() {
        let mon = CombinedSlMonitor::new(&make_overall(false, SlType::PercentOfPremium, 60.0, false, SlType::None, 0.0));
        assert!(mon.check_overall_sl(-10000.0, 5000.0).is_none());
    }

    #[test]
    fn test_overall_sl_percent_triggered() {
        let mon = CombinedSlMonitor::new(&make_overall(true, SlType::PercentOfPremium, 60.0, false, SlType::None, 0.0));
        // total_pnl = -3500, premium = 5000. Loss% = 3500/5000*100 = 70% > 60%
        assert_eq!(mon.check_overall_sl(-3500.0, 5000.0), Some(ExitReason::CombinedSl));
    }

    #[test]
    fn test_overall_sl_percent_not_triggered() {
        let mon = CombinedSlMonitor::new(&make_overall(true, SlType::PercentOfPremium, 60.0, false, SlType::None, 0.0));
        // Loss% = 2000/5000*100 = 40% < 60%
        assert!(mon.check_overall_sl(-2000.0, 5000.0).is_none());
    }

    #[test]
    fn test_overall_sl_combined_premium() {
        let mon = CombinedSlMonitor::new(&make_overall(true, SlType::CombinedPremium, 5000.0, false, SlType::None, 0.0));
        assert_eq!(mon.check_overall_sl(-5500.0, 8000.0), Some(ExitReason::CombinedSl));
        assert!(mon.check_overall_sl(-4000.0, 8000.0).is_none());
    }

    #[test]
    fn test_overall_target_percent() {
        let mon = CombinedSlMonitor::new(&make_overall(false, SlType::None, 0.0, true, SlType::PercentOfPremium, 50.0));
        // Profit% = 3000/5000*100 = 60% > 50%
        assert_eq!(mon.check_overall_target(3000.0, 5000.0), Some(ExitReason::CombinedTarget));
    }

    #[test]
    fn test_overall_target_not_triggered() {
        let mon = CombinedSlMonitor::new(&make_overall(false, SlType::None, 0.0, true, SlType::PercentOfPremium, 50.0));
        assert!(mon.check_overall_target(2000.0, 5000.0).is_none());
    }

    #[test]
    fn test_overall_target_disabled() {
        let mon = CombinedSlMonitor::new(&make_overall(false, SlType::None, 0.0, false, SlType::None, 0.0));
        assert!(mon.check_overall_target(100000.0, 5000.0).is_none());
    }
}
