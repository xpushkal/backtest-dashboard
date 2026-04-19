//! QuantEdge Core — Simulation Engine
//!
//! Single-leg and multi-leg options backtesting with configurable
//! stop loss, targets, and position management.

pub mod config;
pub mod leg;
pub mod position;

pub use config::{
    ExitReason, LegConfig, OptionType, OverallConfig, PositionSide, SlType, SlippageModel,
    StrikeMode, StrategyConfig, StrategyMeta,
};
pub use leg::{Leg, SlState};
pub use position::{ClosedTrade, Position, PositionSnapshot};
