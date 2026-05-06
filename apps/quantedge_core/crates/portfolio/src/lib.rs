//! Portfolio engine: multi-strategy, margin model, correlation.
//!
//! Runs N strategies with shared capital, margin tracking,
//! correlation analysis, and portfolio-level metrics.

pub mod config;
pub mod correlation;
pub mod engine;
pub mod margin;
pub mod metrics;

pub use config::{PortfolioConfig, StrategyAllocation};
pub use correlation::CorrelationMatrix;
pub use engine::{PortfolioEngine, PortfolioResult, StrategyResult};
pub use margin::{MarginModel, MarginResult, MarginRule, MarginSkip, PortfolioMarginTracker};
pub use metrics::PortfolioMetrics;
