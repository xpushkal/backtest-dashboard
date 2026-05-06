//! Portfolio configuration types and TOML/JSON parsing.
//!
//! Defines `PortfolioConfig` which describes N strategies with capital allocation.

use quantedge_core::StrategyConfig;
use serde::{Deserialize, Serialize};

/// A single strategy within the portfolio with its allocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyAllocation {
    pub name: String,
    pub underlying: String,
    pub allocation_pct: f64,
    /// Lot size for this underlying (e.g. 15 for BankNifty, 75 for Nifty).
    #[serde(default = "default_lot_size")]
    pub lot_size: u32,
    /// Computed: total_capital * allocation_pct / 100.0
    #[serde(default)]
    pub allocated_capital: f64,
    /// Strategy TOML string (inline).
    pub toml: String,
    /// Parsed strategy config (populated after parsing).
    #[serde(skip)]
    pub strategy_config: Option<StrategyConfig>,
}

fn default_lot_size() -> u32 {
    15
}

/// Top-level portfolio configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioConfig {
    pub name: String,
    pub total_capital: f64,
    pub date_from: String,
    pub date_to: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    pub strategies: Vec<StrategyAllocation>,
}

fn default_data_dir() -> String {
    "Data/parquet".to_string()
}

impl PortfolioConfig {
    /// Parse from a JSON string (primary NIF input path).
    pub fn from_json_str(s: &str) -> Result<Self, String> {
        let mut config: PortfolioConfig =
            serde_json::from_str(s).map_err(|e| format!("Invalid portfolio JSON: {}", e))?;
        config.validate()?;
        config.compute_allocations();
        config.parse_strategy_tomls()?;
        Ok(config)
    }

    /// Validate that allocations sum to 100% and strategies are non-empty.
    fn validate(&self) -> Result<(), String> {
        if self.strategies.is_empty() {
            return Err("Portfolio must have at least 1 strategy".to_string());
        }
        if self.total_capital <= 0.0 {
            return Err("total_capital must be positive".to_string());
        }

        let sum: f64 = self.strategies.iter().map(|s| s.allocation_pct).sum();
        if (sum - 100.0).abs() > 0.5 {
            return Err(format!(
                "allocation_pct must sum to 100.0 (got {:.1})",
                sum
            ));
        }

        for s in &self.strategies {
            if s.allocation_pct <= 0.0 {
                return Err(format!(
                    "Strategy '{}' has non-positive allocation: {}",
                    s.name, s.allocation_pct
                ));
            }
        }
        Ok(())
    }

    /// Compute allocated_capital for each strategy.
    fn compute_allocations(&mut self) {
        for s in &mut self.strategies {
            s.allocated_capital = self.total_capital * s.allocation_pct / 100.0;
        }
    }

    /// Parse each strategy's TOML string into a StrategyConfig.
    fn parse_strategy_tomls(&mut self) -> Result<(), String> {
        for s in &mut self.strategies {
            let config = StrategyConfig::from_toml_str(&s.toml)
                .map_err(|e| format!("Strategy '{}' TOML error: {}", s.name, e))?;
            s.strategy_config = Some(config);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_strategy_toml() -> String {
        r#"
[strategy]
name = "Test Strategy"
underlying = "BANKNIFTY"
capital = 100000
entry_time = "09:20"
exit_time = "15:15"
lot_size = 15

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_value = 0
sl_type = "points"
sl_value = 30.0
        "#
        .to_string()
    }

    fn sample_json(alloc1: f64, alloc2: f64) -> String {
        format!(
            r#"{{
                "name": "Test Portfolio",
                "total_capital": 500000,
                "date_from": "2021-01-01",
                "date_to": "2024-12-31",
                "strategies": [
                    {{
                        "name": "BN Straddle",
                        "underlying": "BANKNIFTY",
                        "allocation_pct": {},
                        "toml": {:?}
                    }},
                    {{
                        "name": "NF Condor",
                        "underlying": "NIFTY",
                        "allocation_pct": {},
                        "toml": {:?}
                    }}
                ]
            }}"#,
            alloc1,
            sample_strategy_toml(),
            alloc2,
            sample_strategy_toml()
        )
    }

    #[test]
    fn test_parse_valid_portfolio() {
        let config = PortfolioConfig::from_json_str(&sample_json(60.0, 40.0)).unwrap();
        assert_eq!(config.strategies.len(), 2);
        assert_eq!(config.strategies[0].allocated_capital, 300_000.0);
        assert_eq!(config.strategies[1].allocated_capital, 200_000.0);
        assert!(config.strategies[0].strategy_config.is_some());
    }

    #[test]
    fn test_reject_bad_allocation_sum() {
        let result = PortfolioConfig::from_json_str(&sample_json(60.0, 60.0));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("sum to 100.0"));
    }

    #[test]
    fn test_reject_zero_strategies() {
        let json = r#"{"name":"Empty","total_capital":100000,"date_from":"2021-01-01","date_to":"2024-12-31","strategies":[]}"#;
        let result = PortfolioConfig::from_json_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_negative_capital() {
        let json = r#"{"name":"Bad","total_capital":-100,"date_from":"2021-01-01","date_to":"2024-12-31","strategies":[{"name":"X","underlying":"BANKNIFTY","allocation_pct":100.0,"toml":"[strategy]\nname=\"X\""}]}"#;
        let result = PortfolioConfig::from_json_str(json);
        assert!(result.is_err());
    }
}
