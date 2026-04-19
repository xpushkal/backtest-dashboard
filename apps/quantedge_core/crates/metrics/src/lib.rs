//! QuantEdge Metrics — Performance Analytics
//!
//! Computes backtesting metrics from trade results and equity curves.
//! Phase 2: 20 core metrics. Phase 5: expanded to 75+.

pub mod core_metrics;

pub use core_metrics::{EquityPoint, MetricExitReason, MetricsEngine, MetricsResult, TradeRecord};
