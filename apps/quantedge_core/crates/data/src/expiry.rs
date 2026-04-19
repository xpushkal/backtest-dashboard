//! Expiry calendar resolution.
//!
//! Resolves the next expiry date for any (symbol, date) pair,
//! handling weekly→monthly transitions for BankNifty, Nifty, Sensex.

use chrono::{Datelike, NaiveDate, Weekday};
use serde::Deserialize;
use std::collections::HashMap;

/// A single transition rule from expiry_calendar.toml.
#[derive(Debug, Deserialize, Clone)]
pub struct TransitionRule {
    pub from: NaiveDate,
    pub to: NaiveDate,
    #[serde(rename = "type")]
    pub expiry_type: String,
    pub day: String,
}

/// Wrapper for TOML deserialization.
#[derive(Debug, Deserialize)]
struct SymbolConfig {
    transitions: Vec<TransitionRule>,
}

/// Expiry calendar for Indian FNO markets.
///
/// Resolves the next expiry date given a symbol and trading date.
/// The single source of truth for expiry resolution — no other code
/// should compute expiry dates.
#[derive(Debug, Clone)]
pub struct ExpiryCalendar {
    rules: HashMap<String, Vec<TransitionRule>>,
}

impl ExpiryCalendar {
    /// Load from a TOML config file.
    ///
    /// # Example
    /// ```ignore
    /// let cal = ExpiryCalendar::from_toml("config/expiry_calendar.toml").unwrap();
    /// ```
    pub fn from_toml(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let raw: HashMap<String, SymbolConfig> = toml::from_str(&content)?;
        let rules = raw
            .into_iter()
            .map(|(k, v)| (k.to_uppercase(), v.transitions))
            .collect();
        Ok(Self { rules })
    }

    /// Resolve the next expiry date for the given symbol and date.
    ///
    /// Returns `(expiry_date, expiry_type, dte)` where:
    /// - `expiry_date`: the actual NaiveDate of the expiry
    /// - `expiry_type`: "weekly" or "monthly"
    /// - `dte`: days to expiry (expiry_date - date, always >= 0)
    pub fn next_expiry(
        &self,
        symbol: &str,
        date: NaiveDate,
    ) -> Option<(NaiveDate, String, i64)> {
        let symbol_upper = symbol.to_uppercase();
        let rules = self.rules.get(&symbol_upper)?;

        // Find the applicable rule for this date
        let rule = rules.iter().find(|r| date >= r.from && date <= r.to)?;

        let expiry = match rule.expiry_type.as_str() {
            "weekly" => {
                let weekday = parse_weekday(&rule.day)?;
                next_weekday(date, weekday)
            }
            "monthly" => {
                let weekday = parse_last_weekday(&rule.day)?;
                let candidate = last_weekday_of_month(date.year(), date.month(), weekday);
                if date > candidate {
                    // Past this month's expiry, go to next month
                    let (next_year, next_month) = if date.month() == 12 {
                        (date.year() + 1, 1)
                    } else {
                        (date.year(), date.month() + 1)
                    };
                    last_weekday_of_month(next_year, next_month, weekday)
                } else {
                    candidate
                }
            }
            _ => return None,
        };

        let dte = (expiry - date).num_days();
        Some((expiry, rule.expiry_type.clone(), dte))
    }

    /// Get the expiry type (weekly/monthly) for a symbol on a given date.
    pub fn expiry_type(&self, symbol: &str, date: NaiveDate) -> Option<String> {
        let symbol_upper = symbol.to_uppercase();
        let rules = self.rules.get(&symbol_upper)?;
        let rule = rules.iter().find(|r| date >= r.from && date <= r.to)?;
        Some(rule.expiry_type.clone())
    }
}

/// Parse a weekday name like "Wednesday" to chrono::Weekday.
fn parse_weekday(s: &str) -> Option<Weekday> {
    match s.to_lowercase().as_str() {
        "monday" => Some(Weekday::Mon),
        "tuesday" => Some(Weekday::Tue),
        "wednesday" => Some(Weekday::Wed),
        "thursday" => Some(Weekday::Thu),
        "friday" => Some(Weekday::Fri),
        "saturday" => Some(Weekday::Sat),
        "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

/// Parse "last_thursday" or "last_friday" to the weekday part.
fn parse_last_weekday(s: &str) -> Option<Weekday> {
    let s_lower = s.to_lowercase();
    if let Some(day_part) = s_lower.strip_prefix("last_") {
        parse_weekday(day_part)
    } else {
        parse_weekday(&s_lower)
    }
}

/// Find the next occurrence of a weekday on or after the given date.
fn next_weekday(date: NaiveDate, target: Weekday) -> NaiveDate {
    let current = date.weekday();
    let days_ahead = (target.num_days_from_monday() as i64
        - current.num_days_from_monday() as i64
        + 7)
        % 7;
    // If today is the target day, return today (expiry day itself)
    if days_ahead == 0 {
        return date;
    }
    date + chrono::Duration::days(days_ahead)
}

/// Find the last occurrence of a weekday in a given month.
fn last_weekday_of_month(year: i32, month: u32, target: Weekday) -> NaiveDate {
    // Start from the last day of the month and work backwards
    let last_day = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap() - chrono::Duration::days(1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap() - chrono::Duration::days(1)
    };

    let mut d = last_day;
    while d.weekday() != target {
        d -= chrono::Duration::days(1);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_weekday() {
        assert_eq!(parse_weekday("Wednesday"), Some(Weekday::Wed));
        assert_eq!(parse_weekday("thursday"), Some(Weekday::Thu));
        assert_eq!(parse_weekday("Friday"), Some(Weekday::Fri));
    }

    #[test]
    fn test_parse_last_weekday() {
        assert_eq!(parse_last_weekday("last_thursday"), Some(Weekday::Thu));
        assert_eq!(parse_last_weekday("last_friday"), Some(Weekday::Fri));
        assert_eq!(parse_last_weekday("last_wednesday"), Some(Weekday::Wed));
    }

    #[test]
    fn test_next_weekday() {
        // Monday 2024-01-08 → next Wednesday = 2024-01-10
        let mon = NaiveDate::from_ymd_opt(2024, 1, 8).unwrap();
        let result = next_weekday(mon, Weekday::Wed);
        assert_eq!(result, NaiveDate::from_ymd_opt(2024, 1, 10).unwrap());
    }

    #[test]
    fn test_next_weekday_same_day() {
        // Wednesday 2024-01-10 → should return same day
        let wed = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
        let result = next_weekday(wed, Weekday::Wed);
        assert_eq!(result, wed);
    }

    #[test]
    fn test_last_weekday_of_month() {
        // Last Thursday of January 2024 = 2024-01-25
        let result = last_weekday_of_month(2024, 1, Weekday::Thu);
        assert_eq!(result, NaiveDate::from_ymd_opt(2024, 1, 25).unwrap());
    }

    #[test]
    fn test_last_thursday_november_2024() {
        // Last Thursday of November 2024 = 2024-11-28
        let result = last_weekday_of_month(2024, 11, Weekday::Thu);
        assert_eq!(result, NaiveDate::from_ymd_opt(2024, 11, 28).unwrap());
    }

    #[test]
    fn test_last_wednesday_november_2024() {
        // Last Wednesday of November 2024 = 2024-11-27
        let result = last_weekday_of_month(2024, 11, Weekday::Wed);
        assert_eq!(result, NaiveDate::from_ymd_opt(2024, 11, 27).unwrap());
    }

    #[test]
    fn test_last_friday_december_2024() {
        // Last Friday of December 2024 = 2024-12-27
        let result = last_weekday_of_month(2024, 12, Weekday::Fri);
        assert_eq!(result, NaiveDate::from_ymd_opt(2024, 12, 27).unwrap());
    }
}
