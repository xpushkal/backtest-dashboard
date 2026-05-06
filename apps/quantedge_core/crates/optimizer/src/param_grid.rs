//! Parameter grid definition and cartesian product combo generation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single parameter range for sweeping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamRange {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

impl ParamRange {
    /// Generate all values in this range.
    pub fn values(&self) -> Vec<f64> {
        if self.step <= 0.0 || self.min > self.max {
            return vec![self.min];
        }
        let mut vals = Vec::new();
        let mut v = self.min;
        while v <= self.max + 1e-9 {
            vals.push((v * 1000.0).round() / 1000.0); // round to 3dp
            v += self.step;
        }
        if vals.is_empty() {
            vals.push(self.min);
        }
        vals
    }

    /// Number of values in this range.
    pub fn count(&self) -> usize {
        self.values().len()
    }
}

/// A grid of parameter ranges to sweep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamGrid {
    pub params: Vec<ParamRange>,
}

/// A single combination of parameter values.
#[derive(Debug, Clone, Serialize)]
pub struct ParamCombo {
    pub index: usize,
    pub values: HashMap<String, f64>,
}

impl ParamGrid {
    /// Parse from JSON string (array of ParamRange).
    pub fn from_json_str(s: &str) -> Result<Self, String> {
        let params: Vec<ParamRange> =
            serde_json::from_str(s).map_err(|e| format!("Invalid param grid JSON: {}", e))?;
        Ok(Self { params })
    }

    /// Total number of combinations (product of all range counts).
    pub fn total_combos(&self) -> usize {
        if self.params.is_empty() {
            return 1;
        }
        self.params.iter().map(|p| p.count()).product()
    }

    /// Generate all parameter combinations (cartesian product).
    pub fn generate_combos(&self) -> Vec<ParamCombo> {
        if self.params.is_empty() {
            return vec![ParamCombo {
                index: 0,
                values: HashMap::new(),
            }];
        }

        let all_values: Vec<Vec<f64>> = self.params.iter().map(|p| p.values()).collect();
        let total = self.total_combos();
        let mut combos = Vec::with_capacity(total);

        for idx in 0..total {
            let mut values = HashMap::new();
            let mut remainder = idx;

            for (i, param) in self.params.iter().enumerate().rev() {
                let vals = &all_values[i];
                let val_idx = remainder % vals.len();
                remainder /= vals.len();
                values.insert(param.name.clone(), vals[val_idx]);
            }

            combos.push(ParamCombo { index: idx, values });
        }

        combos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_range_values() {
        let r = ParamRange { name: "sl".into(), min: 20.0, max: 50.0, step: 10.0 };
        assert_eq!(r.values(), vec![20.0, 30.0, 40.0, 50.0]);
        assert_eq!(r.count(), 4);
    }

    #[test]
    fn test_param_range_single_value() {
        let r = ParamRange { name: "x".into(), min: 5.0, max: 5.0, step: 1.0 };
        assert_eq!(r.values(), vec![5.0]);
    }

    #[test]
    fn test_grid_total_combos() {
        let grid = ParamGrid {
            params: vec![
                ParamRange { name: "a".into(), min: 1.0, max: 3.0, step: 1.0 },
                ParamRange { name: "b".into(), min: 10.0, max: 40.0, step: 10.0 },
            ],
        };
        assert_eq!(grid.total_combos(), 12); // 3 × 4
    }

    #[test]
    fn test_grid_generate_combos() {
        let grid = ParamGrid {
            params: vec![
                ParamRange { name: "a".into(), min: 1.0, max: 2.0, step: 1.0 },
                ParamRange { name: "b".into(), min: 10.0, max: 20.0, step: 10.0 },
            ],
        };
        let combos = grid.generate_combos();
        assert_eq!(combos.len(), 4); // 2 × 2
        // Verify all combos have both params
        for c in &combos {
            assert!(c.values.contains_key("a"));
            assert!(c.values.contains_key("b"));
        }
    }

    #[test]
    fn test_empty_grid() {
        let grid = ParamGrid { params: vec![] };
        assert_eq!(grid.total_combos(), 1);
        let combos = grid.generate_combos();
        assert_eq!(combos.len(), 1);
        assert!(combos[0].values.is_empty());
    }

    #[test]
    fn test_from_json() {
        let json = r#"[{"name":"sl_value","min":20,"max":50,"step":10}]"#;
        let grid = ParamGrid::from_json_str(json).unwrap();
        assert_eq!(grid.params.len(), 1);
        assert_eq!(grid.total_combos(), 4);
    }

    #[test]
    fn test_720_combo_grid() {
        // 6×6×5×4 = 720
        let grid = ParamGrid {
            params: vec![
                ParamRange { name: "a".into(), min: 1.0, max: 6.0, step: 1.0 },
                ParamRange { name: "b".into(), min: 1.0, max: 6.0, step: 1.0 },
                ParamRange { name: "c".into(), min: 1.0, max: 5.0, step: 1.0 },
                ParamRange { name: "d".into(), min: 1.0, max: 4.0, step: 1.0 },
            ],
        };
        assert_eq!(grid.total_combos(), 720);
        let combos = grid.generate_combos();
        assert_eq!(combos.len(), 720);
    }
}
