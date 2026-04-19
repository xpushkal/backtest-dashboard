//! QuantEdge Core — Simulation Engine
//!
//! Single-leg and multi-leg options backtesting with configurable
//! stop loss, targets, and position management.

pub mod config;

pub use config::{
    ExitReason, LegConfig, OptionType, OverallConfig, PositionSide, SlType, SlippageModel,
    StrikeMode, StrategyConfig, StrategyMeta,
};
