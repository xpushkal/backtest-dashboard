//! Correlation matrix computation between strategy daily returns.
//!
//! Uses ndarray for NxN Pearson correlation.

use ndarray::Array2;
use serde::{Deserialize, Serialize};

/// NxN correlation matrix between strategy daily PnL series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    pub strategy_names: Vec<String>,
    pub matrix: Vec<Vec<f64>>, // NxN stored as Vec<Vec> for easy serialization
}

impl CorrelationMatrix {
    /// Compute Pearson correlation between each pair of strategy daily PnL series.
    ///
    /// All series must have the same length.
    pub fn compute(strategy_daily_pnls: &[Vec<f64>], strategy_names: &[String]) -> Self {
        let n = strategy_daily_pnls.len();

        if n == 0 {
            return Self {
                strategy_names: vec![],
                matrix: vec![],
            };
        }

        let mut matrix = vec![vec![0.0_f64; n]; n];

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    matrix[i][j] = 1.0;
                } else if j > i {
                    let corr = pearson_correlation(&strategy_daily_pnls[i], &strategy_daily_pnls[j]);
                    matrix[i][j] = corr;
                    matrix[j][i] = corr; // symmetric
                }
            }
        }

        Self {
            strategy_names: strategy_names.to_vec(),
            matrix,
        }
    }

    /// Get correlation between two strategies by name.
    pub fn get(&self, a: &str, b: &str) -> Option<f64> {
        let idx_a = self.strategy_names.iter().position(|n| n == a)?;
        let idx_b = self.strategy_names.iter().position(|n| n == b)?;
        Some(self.matrix[idx_a][idx_b])
    }

    /// Average pairwise correlation (excluding diagonal).
    pub fn avg_correlation(&self) -> f64 {
        let n = self.matrix.len();
        if n < 2 {
            return 0.0;
        }

        let mut sum = 0.0;
        let mut count = 0;
        for i in 0..n {
            for j in (i + 1)..n {
                sum += self.matrix[i][j];
                count += 1;
            }
        }

        if count > 0 {
            sum / count as f64
        } else {
            0.0
        }
    }

    /// Convert to ndarray for advanced operations.
    pub fn to_ndarray(&self) -> Array2<f64> {
        let n = self.matrix.len();
        let mut arr = Array2::zeros((n, n));
        for i in 0..n {
            for j in 0..n {
                arr[[i, j]] = self.matrix[i][j];
            }
        }
        arr
    }

    /// Serialize to JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "strategy_names": self.strategy_names,
            "matrix": self.matrix
        })
    }
}

/// Pearson correlation coefficient between two series.
fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len());
    if n < 2 {
        return 0.0;
    }

    let mean_x: f64 = x[..n].iter().sum::<f64>() / n as f64;
    let mean_y: f64 = y[..n].iter().sum::<f64>() / n as f64;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..n {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    let denom = (var_x * var_y).sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        cov / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_correlation() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![2.0, 4.0, 6.0, 8.0, 10.0]; // perfectly correlated
        let corr = pearson_correlation(&a, &b);
        assert!((corr - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_negative_correlation() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![5.0, 4.0, 3.0, 2.0, 1.0]; // perfectly negative
        let corr = pearson_correlation(&a, &b);
        assert!((corr - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_correlation_matrix_symmetric() {
        let pnls = vec![
            vec![1.0, 2.0, -1.0, 3.0, 0.5],
            vec![0.5, 1.5, -0.5, 2.0, 0.3],
            vec![-1.0, 0.0, 1.0, -0.5, 2.0],
        ];
        let names = vec!["A".into(), "B".into(), "C".into()];
        let matrix = CorrelationMatrix::compute(&pnls, &names);

        // Check diagonal = 1.0
        for i in 0..3 {
            assert!((matrix.matrix[i][i] - 1.0).abs() < 1e-10);
        }

        // Check symmetric
        for i in 0..3 {
            for j in 0..3 {
                assert!((matrix.matrix[i][j] - matrix.matrix[j][i]).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_avg_correlation() {
        let pnls = vec![
            vec![1.0, 2.0, 3.0],
            vec![1.0, 2.0, 3.0], // same as first = corr 1.0
        ];
        let names = vec!["A".into(), "B".into()];
        let matrix = CorrelationMatrix::compute(&pnls, &names);
        assert!((matrix.avg_correlation() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_get_by_name() {
        let pnls = vec![
            vec![1.0, 2.0, 3.0],
            vec![3.0, 2.0, 1.0],
        ];
        let names = vec!["Alpha".into(), "Beta".into()];
        let matrix = CorrelationMatrix::compute(&pnls, &names);
        let corr = matrix.get("Alpha", "Beta").unwrap();
        assert!((corr - (-1.0)).abs() < 1e-10);
    }
}
