//! QuantEdge Greeks — Black-Scholes Pricing, Greeks & PnL Attribution
//!
//! Provides European option pricing, first-order Greeks
//! (delta, gamma, theta, vega), and PnL attribution decomposition.

pub mod attribution;
pub mod black_scholes;
pub mod greeks_engine;

pub use attribution::{AttributionInputs, AttributionSummary, PnlAttribution};
pub use black_scholes::{BsInputs, BsOutput, RISK_FREE_RATE_INDIA};
pub use greeks_engine::{GreeksEngine, TradeGreeks};
