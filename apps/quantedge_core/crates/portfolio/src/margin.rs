//! Simplified SPAN margin model and portfolio margin tracker.
//!
//! Formula: margin = max(3 × premium × quantity, index_factor × spot × quantity × 0.12)
//! Hedge benefit: buy+sell same option_type → 30% margin reduction.

use chrono::{NaiveDate, NaiveTime};
use quantedge_core::PositionSide;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Which margin rule dominated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarginRule {
    ThreeXPremium,
    NotionalPercent,
}

/// Result of a single margin computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginResult {
    pub required: f64,
    pub rule_used: MarginRule,
}

/// Record of a skipped trade due to insufficient margin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginSkip {
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub strategy_name: String,
    pub required_margin: f64,
    pub available_margin: f64,
    pub reason: String,
}

/// The margin computation model.
#[derive(Debug, Clone)]
pub struct MarginModel {
    index_factor: f64,
}

impl MarginModel {
    /// Create a new margin model.
    /// `index_factor`: multiplier for notional margin (default 1.0).
    pub fn new(index_factor: f64) -> Self {
        Self { index_factor }
    }

    /// Default model with index_factor = 1.0.
    pub fn default_model() -> Self {
        Self { index_factor: 1.0 }
    }

    /// Compute margin for a single leg entry.
    pub fn compute_leg_margin(
        &self,
        premium: f64,
        spot: f64,
        lots: u32,
        lot_size: u32,
    ) -> MarginResult {
        let quantity = (lots * lot_size) as f64;
        let three_x = 3.0 * premium * quantity;
        let notional = self.index_factor * spot * quantity * 0.12;

        if three_x >= notional {
            MarginResult {
                required: three_x,
                rule_used: MarginRule::ThreeXPremium,
            }
        } else {
            MarginResult {
                required: notional,
                rule_used: MarginRule::NotionalPercent,
            }
        }
    }

    /// Compute total margin for a multi-leg strategy.
    ///
    /// `legs`: Vec of (premium, spot, lots, lot_size, side).
    /// If the strategy has both Buy and Sell legs on the same option_type,
    /// apply a 30% hedge benefit (multiply total by 0.7).
    pub fn compute_strategy_margin(
        &self,
        legs: &[(f64, f64, u32, u32, PositionSide)],
    ) -> f64 {
        let total: f64 = legs
            .iter()
            .map(|&(premium, spot, lots, lot_size, _)| {
                self.compute_leg_margin(premium, spot, lots, lot_size)
                    .required
            })
            .sum();

        // Check for hedge benefit: both buy and sell sides present
        let has_buy = legs.iter().any(|l| l.4 == PositionSide::Buy);
        let has_sell = legs.iter().any(|l| l.4 == PositionSide::Sell);

        if has_buy && has_sell {
            total * 0.7 // 30% hedge benefit
        } else {
            total
        }
    }
}

/// Tracks margin usage across the entire portfolio in real-time.
#[derive(Debug)]
pub struct PortfolioMarginTracker {
    total_capital: f64,
    margin_model: MarginModel,
    /// strategy_name → currently locked margin
    active_margins: HashMap<String, f64>,
    peak_margin: f64,
    margin_skips: Vec<MarginSkip>,
}

impl PortfolioMarginTracker {
    pub fn new(total_capital: f64, model: MarginModel) -> Self {
        Self {
            total_capital,
            margin_model: model,
            active_margins: HashMap::new(),
            peak_margin: 0.0,
            margin_skips: Vec::new(),
        }
    }

    /// Current total margin in use.
    pub fn current_margin(&self) -> f64 {
        self.active_margins.values().sum()
    }

    /// Available margin = total_capital - current margin.
    pub fn available_margin(&self) -> f64 {
        self.total_capital - self.current_margin()
    }

    /// Check if a new entry can be accepted.
    /// Returns Ok(margin_amount) if accepted, or logs a MarginSkip and returns Err.
    pub fn check_entry(
        &mut self,
        strategy_name: &str,
        required_margin: f64,
        date: NaiveDate,
        time: NaiveTime,
    ) -> Result<f64, MarginSkip> {
        let available = self.available_margin();

        if required_margin <= available {
            // Accept: add to active margins
            let current = self.active_margins.entry(strategy_name.to_string()).or_insert(0.0);
            *current += required_margin;

            // Update peak
            let total = self.current_margin();
            if total > self.peak_margin {
                self.peak_margin = total;
            }

            Ok(required_margin)
        } else {
            // Reject: log margin skip
            let skip = MarginSkip {
                date,
                time,
                strategy_name: strategy_name.to_string(),
                required_margin,
                available_margin: available,
                reason: format!(
                    "Insufficient margin: need ₹{:.0}, available ₹{:.0}",
                    required_margin, available
                ),
            };
            self.margin_skips.push(skip.clone());
            Err(skip)
        }
    }

    /// Release margin when a position is closed.
    pub fn release_margin(&mut self, strategy_name: &str, amount: f64) {
        if let Some(current) = self.active_margins.get_mut(strategy_name) {
            *current = (*current - amount).max(0.0);
            if *current == 0.0 {
                self.active_margins.remove(strategy_name);
            }
        }
    }

    /// Margin utilization ratio (0.0 - 1.0).
    pub fn utilization(&self) -> f64 {
        if self.total_capital > 0.0 {
            self.current_margin() / self.total_capital
        } else {
            0.0
        }
    }

    /// Peak margin used across the entire backtest.
    pub fn peak_margin(&self) -> f64 {
        self.peak_margin
    }

    /// All margin skips that occurred.
    pub fn skips(&self) -> &[MarginSkip] {
        &self.margin_skips
    }

    /// Access to the margin model.
    pub fn model(&self) -> &MarginModel {
        &self.margin_model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leg_margin_three_x_wins() {
        let model = MarginModel::new(1.0);
        // premium=200, spot=45000, lots=1, lot_size=15
        // 3x: 3*200*15 = 9000
        // notional: 1.0*45000*15*0.12 = 81000
        // notional wins here
        let result = model.compute_leg_margin(200.0, 45000.0, 1, 15);
        assert_eq!(result.rule_used, MarginRule::NotionalPercent);
        assert!((result.required - 81000.0).abs() < 0.01);
    }

    #[test]
    fn test_leg_margin_premium_wins() {
        let model = MarginModel::new(1.0);
        // premium=5000, spot=100, lots=1, lot_size=15
        // 3x: 3*5000*15 = 225000
        // notional: 1.0*100*15*0.12 = 180
        // 3x premium wins
        let result = model.compute_leg_margin(5000.0, 100.0, 1, 15);
        assert_eq!(result.rule_used, MarginRule::ThreeXPremium);
        assert!((result.required - 225000.0).abs() < 0.01);
    }

    #[test]
    fn test_hedge_benefit() {
        let model = MarginModel::new(1.0);
        let legs = vec![
            (200.0, 45000.0, 1_u32, 15_u32, PositionSide::Sell),
            (150.0, 45000.0, 1_u32, 15_u32, PositionSide::Buy),
        ];
        let hedged = model.compute_strategy_margin(&legs);
        let unhedged_legs = vec![
            (200.0, 45000.0, 1_u32, 15_u32, PositionSide::Sell),
            (150.0, 45000.0, 1_u32, 15_u32, PositionSide::Sell),
        ];
        let unhedged = model.compute_strategy_margin(&unhedged_legs);
        // Hedged should be 70% of the sum
        assert!(hedged < unhedged);
    }

    #[test]
    fn test_margin_tracker_accept() {
        let model = MarginModel::default_model();
        let mut tracker = PortfolioMarginTracker::new(200_000.0, model);
        let date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let time = NaiveTime::from_hms_opt(9, 20, 0).unwrap();

        let result = tracker.check_entry("Strategy1", 80_000.0, date, time);
        assert!(result.is_ok());
        assert!((tracker.current_margin() - 80_000.0).abs() < 0.01);
    }

    #[test]
    fn test_margin_tracker_reject() {
        let model = MarginModel::default_model();
        let mut tracker = PortfolioMarginTracker::new(100_000.0, model);
        let date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let time = NaiveTime::from_hms_opt(9, 20, 0).unwrap();

        // First entry takes 80K
        tracker.check_entry("Strategy1", 80_000.0, date, time).unwrap();
        // Second entry needs 50K but only 20K available
        let result = tracker.check_entry("Strategy2", 50_000.0, date, time);
        assert!(result.is_err());
        assert_eq!(tracker.skips().len(), 1);
    }

    #[test]
    fn test_margin_release() {
        let model = MarginModel::default_model();
        let mut tracker = PortfolioMarginTracker::new(200_000.0, model);
        let date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let time = NaiveTime::from_hms_opt(9, 20, 0).unwrap();

        tracker.check_entry("S1", 80_000.0, date, time).unwrap();
        assert!((tracker.current_margin() - 80_000.0).abs() < 0.01);

        tracker.release_margin("S1", 80_000.0);
        assert!((tracker.current_margin() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_peak_margin_tracking() {
        let model = MarginModel::default_model();
        let mut tracker = PortfolioMarginTracker::new(500_000.0, model);
        let date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let time = NaiveTime::from_hms_opt(9, 20, 0).unwrap();

        tracker.check_entry("S1", 100_000.0, date, time).unwrap();
        tracker.check_entry("S2", 150_000.0, date, time).unwrap();
        // Peak = 250K
        tracker.release_margin("S1", 100_000.0);
        // Current = 150K but peak stays at 250K
        assert!((tracker.peak_margin() - 250_000.0).abs() < 0.01);
    }
}
