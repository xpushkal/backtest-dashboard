//! Strategy configuration types and TOML parsing.
//!
//! Parses strategy TOML files into strongly-typed Rust structs.
//! Every backtest starts by loading a `StrategyConfig`.

use chrono::NaiveTime;
use serde::{Deserialize, Serialize};

// ─── Enums ──────────────────────────────────────────────────

/// Option type: Call or Put.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionType {
    #[serde(alias = "ce", alias = "Ce")]
    CE,
    #[serde(alias = "pe", alias = "Pe")]
    PE,
}

/// Position direction: Buy (long) or Sell (short).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionSide {
    Buy,
    Sell,
}

/// Strike selection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrikeMode {
    AtmOffset,
    Delta,
    Premium,
    PercentOtm,
}

/// Stop-loss type variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlType {
    Points,
    PercentOfPremium,
    PercentOfMargin,
    IndexPoints,
    DeltaBreach,
    CombinedPremium,
    None,
}

impl Default for SlType {
    fn default() -> Self {
        SlType::None
    }
}

/// Slippage model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlippageModel {
    None,
    FixedPts,
    Percent,
    VolumeBased,
}

impl Default for SlippageModel {
    fn default() -> Self {
        SlippageModel::None
    }
}

/// Trailing SL mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrailSlMode {
    /// Lock-in profit floor: ratchets up, never down
    Lock,
    /// Dynamic trail: distance from high-water mark
    Trail,
}

impl Default for TrailSlMode {
    fn default() -> Self {
        TrailSlMode::Trail
    }
}

/// Trail unit for Trail mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrailUnit {
    Points,
    Percent,
}

impl Default for TrailUnit {
    fn default() -> Self {
        TrailUnit::Percent
    }
}

/// Re-entry mode after SL/target hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReEntryMode {
    /// Re-enter at next bar open.
    Asap,
    /// Re-enter at same clock time next trading day.
    SameTime,
    /// Wait N bars then re-enter.
    AfterNBars,
    /// Wait for momentum filter to confirm.
    MomentumConfirm,
}

impl Default for ReEntryMode {
    fn default() -> Self {
        ReEntryMode::AfterNBars
    }
}

/// Reason a position was exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    StopLoss,
    Target,
    TimeExit,
    EndOfData,
    CombinedSl,
    CombinedTarget,
}

// ─── Config Structs ─────────────────────────────────────────

/// Top-level strategy configuration.
///
/// Parsed from a TOML file with `[strategy]`, `[[legs]]`, and `[overall]` sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    #[serde(flatten)]
    pub strategy: StrategyMeta,
    pub legs: Vec<LegConfig>,
    #[serde(default)]
    pub overall: OverallConfig,
}

/// The `[strategy]` section metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMeta {
    pub name: String,
    pub underlying: String,
    #[serde(deserialize_with = "deserialize_time")]
    pub entry_time: NaiveTime,
    #[serde(deserialize_with = "deserialize_time")]
    pub exit_time: NaiveTime,
    #[serde(default = "default_expiry_filter")]
    pub expiry_filter: String,
    #[serde(default = "default_true")]
    pub trade_on_expiry: bool,
    #[serde(default = "default_one_u32")]
    pub max_concurrent_trades: u32,
    pub capital: f64,
    #[serde(default = "default_brokerage")]
    pub brokerage_per_lot: f64,
    #[serde(default)]
    pub slippage_model: SlippageModel,
    #[serde(default)]
    pub slippage_value: f64,
    #[serde(default = "default_true")]
    pub stt_on_sell: bool,
}

/// A single leg configuration from `[[legs]]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegConfig {
    pub option_type: OptionType,
    pub position: PositionSide,
    #[serde(default = "default_one_u32")]
    pub lots: u32,
    #[serde(default = "default_expiry_filter")]
    pub expiry: String,
    #[serde(default)]
    pub strike_mode: StrikeMode,
    #[serde(default)]
    pub strike_offset: i32,
    pub delta_target: Option<f64>,
    pub premium_target: Option<f64>,

    // Per-leg SL
    #[serde(default)]
    pub stop_loss_enabled: bool,
    #[serde(default)]
    pub stop_loss_type: SlType,
    #[serde(default)]
    pub stop_loss_value: f64,

    // Per-leg Target
    #[serde(default)]
    pub target_profit_enabled: bool,
    #[serde(default)]
    pub target_profit_type: SlType,
    #[serde(default)]
    pub target_profit_value: f64,

    // Trailing SL
    #[serde(default)]
    pub trail_sl_enabled: bool,
    #[serde(default)]
    pub trail_sl_activate_at: f64,
    #[serde(default)]
    pub trail_sl_lock_in: f64,
    #[serde(default)]
    pub trail_sl_mode: TrailSlMode,
    #[serde(default)]
    pub trail_sl_unit: TrailUnit,
    #[serde(default)]
    pub trail_sl_value: f64,

    // Re-entry (stubbed for Phase 2)
    #[serde(default)]
    pub reentry_on_sl: bool,
    #[serde(default)]
    pub reentry_on_target: bool,
    #[serde(default)]
    pub reentry_mode: ReEntryMode,
    #[serde(default = "default_five_u32")]
    pub reentry_cooldown_bars: u32,
    #[serde(default = "default_two_u32")]
    pub reentry_max_attempts: u32,

    // Momentum (stubbed for Phase 2)
    #[serde(default)]
    pub momentum_filter_enabled: bool,
}

impl Default for StrikeMode {
    fn default() -> Self {
        StrikeMode::AtmOffset
    }
}

/// The `[overall]` section for portfolio-level SL/target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallConfig {
    #[serde(default)]
    pub overall_sl_enabled: bool,
    #[serde(default)]
    pub overall_sl_type: SlType,
    #[serde(default)]
    pub overall_sl_value: f64,
    #[serde(default)]
    pub overall_target_enabled: bool,
    #[serde(default)]
    pub overall_target_type: SlType,
    #[serde(default)]
    pub overall_target_value: f64,
}

impl Default for OverallConfig {
    fn default() -> Self {
        Self {
            overall_sl_enabled: false,
            overall_sl_type: SlType::None,
            overall_sl_value: 0.0,
            overall_target_enabled: false,
            overall_target_type: SlType::None,
            overall_target_value: 0.0,
        }
    }
}

// ─── Serde Helpers ──────────────────────────────────────────

fn default_true() -> bool { true }
fn default_one_u32() -> u32 { 1 }
fn default_two_u32() -> u32 { 2 }
fn default_five_u32() -> u32 { 5 }
fn default_brokerage() -> f64 { 40.0 }
fn default_expiry_filter() -> String { "weekly".to_string() }

// default_reentry_mode removed — ReEntryMode implements Default

fn deserialize_time<'de, D>(deserializer: D) -> Result<NaiveTime, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    NaiveTime::parse_from_str(&s, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(&s, "%H:%M:%S"))
        .map_err(serde::de::Error::custom)
}

// ─── Parsing & Validation ───────────────────────────────────

/// Intermediate TOML structure for parsing.
#[derive(Debug, Deserialize)]
struct TomlFile {
    strategy: StrategyMeta,
    legs: Vec<LegConfig>,
    #[serde(default)]
    overall: OverallConfig,
}

impl StrategyConfig {
    /// Load from a TOML file path.
    pub fn from_toml(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml_str(&content)
    }

    /// Parse from a TOML string.
    pub fn from_toml_str(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let raw: TomlFile = toml::from_str(content)?;
        let config = Self {
            strategy: raw.strategy,
            legs: raw.legs,
            overall: raw.overall,
        };
        config.validate()?;
        Ok(config)
    }

    /// Validate strategy constraints.
    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.legs.is_empty() {
            return Err("Strategy must have at least one leg".into());
        }
        if self.strategy.capital <= 0.0 {
            return Err("Capital must be positive".into());
        }
        if self.strategy.entry_time >= self.strategy.exit_time {
            return Err("entry_time must be before exit_time".into());
        }
        for (i, leg) in self.legs.iter().enumerate() {
            if leg.lots == 0 {
                return Err(format!("Leg {}: lots must be > 0", i + 1).into());
            }
            // Trailing SL validation
            if leg.trail_sl_enabled && leg.trail_sl_activate_at <= 0.0 {
                return Err(format!(
                    "Leg {}: trailing SL enabled but activate_at is 0. Set trail_sl_activate_at > 0.",
                    i + 1
                ).into());
            }
            // SL value must be positive when enabled
            if leg.stop_loss_enabled && leg.stop_loss_value <= 0.0 {
                return Err(format!(
                    "Leg {}: stop loss enabled but value is 0. Set stop_loss_value > 0.",
                    i + 1
                ).into());
            }
            // Target value must be positive when enabled
            if leg.target_profit_enabled && leg.target_profit_value <= 0.0 {
                return Err(format!(
                    "Leg {}: target profit enabled but value is 0. Set target_profit_value > 0.",
                    i + 1
                ).into());
            }
        }
        // Overall SL with fewer than 2 legs is meaningless
        if self.overall.overall_sl_enabled && self.legs.len() < 2 {
            return Err("Overall SL requires at least 2 legs. Use per-leg SL for single-leg strategies.".into());
        }
        // Re-entry validation
        for (i, leg) in self.legs.iter().enumerate() {
            if leg.reentry_mode == ReEntryMode::MomentumConfirm && !leg.momentum_filter_enabled {
                return Err(format!(
                    "Leg {}: reentry_mode is 'momentum_confirm' but no momentum filter is configured. \
                     Set momentum_filter_enabled = true.",
                    i + 1
                ).into());
            }
            if leg.reentry_mode == ReEntryMode::AfterNBars
                && (leg.reentry_on_sl || leg.reentry_on_target)
                && leg.reentry_cooldown_bars == 0
            {
                return Err(format!(
                    "Leg {}: reentry_mode is 'after_n_bars' but cooldown_bars is 0. \
                     Use mode 'asap' for immediate re-entry, or set cooldown_bars > 0.",
                    i + 1
                ).into());
            }
        }
        Ok(())
    }

    // Convenience accessors delegating to strategy meta
    pub fn name(&self) -> &str { &self.strategy.name }
    pub fn underlying(&self) -> &str { &self.strategy.underlying }
    pub fn entry_time(&self) -> NaiveTime { self.strategy.entry_time }
    pub fn exit_time(&self) -> NaiveTime { self.strategy.exit_time }
    pub fn capital(&self) -> f64 { self.strategy.capital }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_TOML: &str = r#"
[strategy]
name = "Short ATM Call Weekly"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
expiry_filter = "weekly"
trade_on_expiry = true
max_concurrent_trades = 1
capital = 500000.0
brokerage_per_lot = 40.0
slippage_model = "fixed_pts"
slippage_value = 1.0
stt_on_sell = true

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
expiry = "weekly"
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0
target_profit_enabled = false

[overall]
overall_sl_enabled = false
overall_target_enabled = false
"#;

    #[test]
    fn test_parse_example_strategy() {
        let config = StrategyConfig::from_toml_str(EXAMPLE_TOML).unwrap();
        assert_eq!(config.name(), "Short ATM Call Weekly");
        assert_eq!(config.underlying(), "BANKNIFTY");
        assert_eq!(config.entry_time(), NaiveTime::from_hms_opt(9, 20, 0).unwrap());
        assert_eq!(config.exit_time(), NaiveTime::from_hms_opt(15, 20, 0).unwrap());
        assert_eq!(config.capital(), 500000.0);
        assert_eq!(config.legs.len(), 1);
        assert_eq!(config.legs[0].option_type, OptionType::CE);
        assert_eq!(config.legs[0].position, PositionSide::Sell);
        assert_eq!(config.legs[0].lots, 1);
        assert_eq!(config.legs[0].strike_offset, 0);
        assert!(config.legs[0].stop_loss_enabled);
        assert_eq!(config.legs[0].stop_loss_type, SlType::PercentOfPremium);
        assert_eq!(config.legs[0].stop_loss_value, 100.0);
    }

    #[test]
    fn test_parse_ce_pe_enum() {
        let config = StrategyConfig::from_toml_str(EXAMPLE_TOML).unwrap();
        assert_eq!(config.legs[0].option_type, OptionType::CE);
    }

    #[test]
    fn test_validate_no_legs() {
        let toml = r#"
[strategy]
name = "Bad"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[overall]
"#;
        let result = StrategyConfig::from_toml_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_zero_capital() {
        let toml = r#"
[strategy]
name = "Bad"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 0.0

[[legs]]
option_type = "CE"
position = "sell"

[overall]
"#;
        let result = StrategyConfig::from_toml_str(toml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Capital must be positive"));
    }

    #[test]
    fn test_validate_entry_after_exit() {
        let toml = r#"
[strategy]
name = "Bad"
underlying = "BANKNIFTY"
entry_time = "15:20"
exit_time = "09:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"

[overall]
"#;
        let result = StrategyConfig::from_toml_str(toml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("entry_time must be before exit_time"));
    }

    #[test]
    fn test_default_fields() {
        let config = StrategyConfig::from_toml_str(EXAMPLE_TOML).unwrap();
        assert!(!config.legs[0].trail_sl_enabled);
        assert!(!config.legs[0].reentry_on_sl);
        assert!(!config.legs[0].momentum_filter_enabled);
        assert!(!config.overall.overall_sl_enabled);
    }

    #[test]
    fn test_sltype_enum_variants() {
        assert_eq!(SlType::default(), SlType::None);
        let all = [
            SlType::Points,
            SlType::PercentOfPremium,
            SlType::PercentOfMargin,
            SlType::IndexPoints,
            SlType::DeltaBreach,
            SlType::CombinedPremium,
            SlType::None,
        ];
        assert_eq!(all.len(), 7);
    }

    // ─── Phase 3 Validation Tests ───────────────────────────

    #[test]
    fn test_validate_trailing_sl_no_activate() {
        let toml = r#"
[strategy]
name = "Bad Trail"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
trail_sl_enabled = true
trail_sl_activate_at = 0.0

[overall]
"#;
        let result = StrategyConfig::from_toml_str(toml);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("trailing SL enabled but activate_at is 0"), "Got: {}", err);
    }

    #[test]
    fn test_validate_sl_zero_value() {
        let toml = r#"
[strategy]
name = "Bad SL"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
stop_loss_enabled = true
stop_loss_type = "points"
stop_loss_value = 0.0

[overall]
"#;
        let result = StrategyConfig::from_toml_str(toml);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("stop loss enabled but value is 0"), "Got: {}", err);
    }

    #[test]
    fn test_validate_overall_sl_single_leg() {
        let toml = r#"
[strategy]
name = "Bad Overall"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1

[overall]
overall_sl_enabled = true
overall_sl_type = "percent_of_premium"
overall_sl_value = 60.0
"#;
        let result = StrategyConfig::from_toml_str(toml);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Overall SL requires at least 2 legs"), "Got: {}", err);
    }

    #[test]
    fn test_validate_straddle_passes() {
        let toml = r#"
[strategy]
name = "Short Straddle"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0

[[legs]]
option_type = "PE"
position = "sell"
lots = 1
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0

[overall]
overall_sl_enabled = true
overall_sl_type = "percent_of_premium"
overall_sl_value = 60.0
overall_target_enabled = true
overall_target_type = "percent_of_premium"
overall_target_value = 50.0
"#;
        let config = StrategyConfig::from_toml_str(toml);
        assert!(config.is_ok(), "Straddle should validate: {:?}", config.err());
    }

    #[test]
    fn test_validate_iron_condor_passes() {
        let toml = r#"
[strategy]
name = "Iron Condor"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 5
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0

[[legs]]
option_type = "CE"
position = "buy"
lots = 1
strike_mode = "atm_offset"
strike_offset = 10

[[legs]]
option_type = "PE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = -5
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0

[[legs]]
option_type = "PE"
position = "buy"
lots = 1
strike_mode = "atm_offset"
strike_offset = -10

[overall]
overall_sl_enabled = true
overall_sl_type = "combined_premium"
overall_sl_value = 5000.0
"#;
        let config = StrategyConfig::from_toml_str(toml).unwrap();
        assert_eq!(config.legs.len(), 4);
    }

    #[test]
    fn test_invalid_toml_rejected() {
        let result = StrategyConfig::from_toml_str("[strategy]\nname = 123");
        assert!(result.is_err());
    }

    #[test]
    fn test_trail_sl_mode_parsing() {
        let toml = r#"
[strategy]
name = "Trail Lock Test"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
trail_sl_enabled = true
trail_sl_activate_at = 30.0
trail_sl_lock_in = 50.0
trail_sl_mode = "lock"
trail_sl_unit = "percent"
trail_sl_value = 0.0

[overall]
"#;
        let config = StrategyConfig::from_toml_str(toml).unwrap();
        assert_eq!(config.legs[0].trail_sl_mode, TrailSlMode::Lock);
        assert_eq!(config.legs[0].trail_sl_unit, TrailUnit::Percent);
    }

    #[test]
    fn test_reentry_momentum_confirm_without_filter_rejected() {
        let toml = r#"
[strategy]
name = "Invalid MomentumConfirm"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
reentry_on_sl = true
reentry_mode = "momentum_confirm"
momentum_filter_enabled = false

[overall]
"#;
        let result = StrategyConfig::from_toml_str(toml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("momentum_confirm"));
    }

    #[test]
    fn test_reentry_after_n_bars_zero_cooldown_rejected() {
        let toml = r#"
[strategy]
name = "Invalid AfterNBars"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
reentry_on_sl = true
reentry_mode = "after_n_bars"
reentry_cooldown_bars = 0

[overall]
"#;
        let result = StrategyConfig::from_toml_str(toml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cooldown_bars"));
    }

    #[test]
    fn test_reentry_asap_valid() {
        let toml = r#"
[strategy]
name = "Valid ASAP Reentry"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
capital = 500000.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 50.0
reentry_on_sl = true
reentry_mode = "asap"
reentry_max_attempts = 2

[overall]
"#;
        let result = StrategyConfig::from_toml_str(toml);
        assert!(result.is_ok());
    }
}
