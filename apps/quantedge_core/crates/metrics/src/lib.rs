//! QuantEdge Metrics — Performance Analytics
//!
//! Computes backtesting metrics from trade results and equity curves.
//! Phase 5: 45+ core metrics, Monte Carlo simulation, walk-forward analysis.

pub mod core_metrics;
pub mod monte_carlo;

pub use core_metrics::{EquityPoint, MetricExitReason, MetricsEngine, MetricsResult, TradeRecord};
pub use monte_carlo::{MonteCarloConfig, MonteCarloEngine, MonteCarloResult, PercentileBand};
