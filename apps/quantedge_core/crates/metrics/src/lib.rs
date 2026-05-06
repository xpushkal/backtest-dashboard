//! QuantEdge Metrics — Performance Analytics
//!
//! Computes backtesting metrics from trade results and equity curves.
//! Phase 5: 45+ core metrics, options metrics, Monte Carlo, walk-forward.

pub mod core_metrics;
pub mod monte_carlo;
pub mod options_metrics;
pub mod walk_forward;

pub use core_metrics::{EquityPoint, MetricExitReason, MetricsEngine, MetricsResult, TradeRecord};
pub use monte_carlo::{MonteCarloConfig, MonteCarloEngine, MonteCarloResult, PercentileBand};
pub use options_metrics::{DteDistribution, OptionsMetrics, OptionsMetricsEngine, OptionsTradeRecord};
pub use walk_forward::{WalkForwardConfig, WalkForwardEngine, WalkForwardResult, WfWindow};
