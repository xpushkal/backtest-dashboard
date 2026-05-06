//! QuantEdge Greeks — Black-Scholes Pricing & Greeks Engine
//!
//! Provides European option pricing and first-order Greeks
//! (delta, gamma, theta, vega) using the Black-Scholes model.

pub mod black_scholes;
pub mod greeks_engine;

pub use black_scholes::{BsInputs, BsOutput, RISK_FREE_RATE_INDIA};
pub use greeks_engine::{GreeksEngine, TradeGreeks};
