//! Lot size configuration.
//!
//! Provides config-driven lot size lookups per (symbol, date),
//! handling historical changes (e.g., BankNifty 15→30).

use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::HashMap;

/// A single lot size entry from lot_sizes.toml.
#[derive(Debug, Deserialize, Clone)]
pub struct LotSizeEntry {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub size: u32,
}

/// Lot size lookup table for Indian FNO instruments.
///
/// Loaded from config/lot_sizes.toml. Each symbol has one or more
/// date-ranged entries covering its lot size history.
#[derive(Debug, Clone)]
pub struct LotSizes {
    entries: HashMap<String, Vec<LotSizeEntry>>,
}

impl LotSizes {
    /// Load from a TOML config file.
    ///
    /// The TOML format uses array-of-tables:
    /// ```toml
    /// [[BANKNIFTY]]
    /// from = "2000-01-01"
    /// to   = "2024-11-19"
    /// size = 15
    /// ```
    pub fn from_toml(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let raw: HashMap<String, Vec<LotSizeEntry>> = toml::from_str(&content)?;
        let entries = raw
            .into_iter()
            .map(|(k, v)| (k.to_uppercase(), v))
            .collect();
        Ok(Self { entries })
    }

    /// Get the lot size for a symbol on a given date.
    ///
    /// Returns `None` if no entry covers the date or the symbol is unknown.
    ///
    /// # Important
    /// Lot size should be stored on Position at ENTRY time — never re-looked up
    /// during the trade lifetime.
    pub fn get(&self, symbol: &str, date: NaiveDate) -> Option<u32> {
        let symbol_upper = symbol.to_uppercase();
        let entries = self.entries.get(&symbol_upper)?;
        entries
            .iter()
            .find(|e| date >= e.from && date <= e.to)
            .map(|e| e.size)
    }

    /// List all symbols that have lot size entries.
    pub fn symbols(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_lots() -> LotSizes {
        let mut entries = HashMap::new();
        entries.insert(
            "BANKNIFTY".to_string(),
            vec![
                LotSizeEntry {
                    from: NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
                    to: NaiveDate::from_ymd_opt(2024, 11, 19).unwrap(),
                    size: 15,
                },
                LotSizeEntry {
                    from: NaiveDate::from_ymd_opt(2024, 11, 20).unwrap(),
                    to: NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
                    size: 30,
                },
            ],
        );
        entries.insert(
            "NIFTY".to_string(),
            vec![
                LotSizeEntry {
                    from: NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
                    to: NaiveDate::from_ymd_opt(2024, 7, 24).unwrap(),
                    size: 50,
                },
                LotSizeEntry {
                    from: NaiveDate::from_ymd_opt(2024, 7, 25).unwrap(),
                    to: NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
                    size: 75,
                },
            ],
        );
        entries.insert(
            "SENSEX".to_string(),
            vec![LotSizeEntry {
                from: NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
                to: NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
                size: 10,
            }],
        );
        LotSizes { entries }
    }

    #[test]
    fn test_banknifty_old_lot_size() {
        let lots = make_test_lots();
        assert_eq!(
            lots.get("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
            Some(15)
        );
    }

    #[test]
    fn test_banknifty_new_lot_size() {
        let lots = make_test_lots();
        assert_eq!(
            lots.get("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 11, 20).unwrap()),
            Some(30)
        );
    }

    #[test]
    fn test_banknifty_edge_day_before_change() {
        let lots = make_test_lots();
        assert_eq!(
            lots.get("BANKNIFTY", NaiveDate::from_ymd_opt(2024, 11, 19).unwrap()),
            Some(15)
        );
    }

    #[test]
    fn test_nifty_old_lot_size() {
        let lots = make_test_lots();
        assert_eq!(
            lots.get("NIFTY", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            Some(50)
        );
    }

    #[test]
    fn test_nifty_new_lot_size() {
        let lots = make_test_lots();
        assert_eq!(
            lots.get("NIFTY", NaiveDate::from_ymd_opt(2024, 7, 25).unwrap()),
            Some(75)
        );
    }

    #[test]
    fn test_sensex_lot_size() {
        let lots = make_test_lots();
        assert_eq!(
            lots.get("SENSEX", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
            Some(10)
        );
    }

    #[test]
    fn test_unknown_symbol() {
        let lots = make_test_lots();
        assert_eq!(
            lots.get("UNKNOWN", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
            None
        );
    }

    #[test]
    fn test_case_insensitive() {
        let lots = make_test_lots();
        assert_eq!(
            lots.get("banknifty", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()),
            Some(15)
        );
    }
}
