//! QuantEdge Core — Simulation Engine
//!
//! Single-leg and multi-leg options backtesting with configurable
//! stop loss, targets, and position management.

pub mod config;
pub mod execution;
pub mod leg;
pub mod position;
pub mod runner;
pub mod sl_types;
pub mod strategy;
pub mod strike;

pub use config::{
    ExitReason, LegConfig, OptionType, OverallConfig, PositionSide, SlType, SlippageModel,
    StrikeMode, StrategyConfig, StrategyMeta,
};
pub use execution::ExecutionEngine;
pub use leg::{Leg, SlState};
pub use position::{ClosedTrade, Position, PositionSnapshot};
pub use runner::{RunResult, SimBar, SimRunner};
pub use sl_types::{is_sl_triggered, is_target_triggered, SlContext};
pub use strategy::CombinedSlMonitor;
pub use strike::StrikeSelector;
