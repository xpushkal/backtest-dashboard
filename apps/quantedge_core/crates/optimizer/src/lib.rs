//! Parameter optimizer: grid sweep, Rayon parallelism.

pub mod param_grid;
pub mod sweep;

pub use param_grid::{ParamCombo, ParamGrid, ParamRange};
pub use sweep::{OptimizerResult, OptimizerSweep};
