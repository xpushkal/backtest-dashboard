//! IV surface interpolation.
//!
//! Provides cubic spline interpolation of implied volatility across
//! strike offsets. Used for looking up IV at non-standard strikes
//! and for Greeks computation.

use super::bar::Bar;

/// IV Surface for a single timestamp.
///
/// Interpolates implied volatility across strike offsets using
/// natural cubic spline. Extrapolates flat (boundary IV) outside
/// the data range.
#[derive(Debug, Clone)]
pub struct IvSurface {
    /// Sorted strike offsets
    offsets: Vec<f64>,
    /// Corresponding IV values
    ivs: Vec<f64>,
    /// Cubic spline coefficients: (a, b, c, d) for each interval
    /// S_i(x) = a_i + b_i*(x - x_i) + c_i*(x - x_i)^2 + d_i*(x - x_i)^3
    coeffs: Vec<(f64, f64, f64, f64)>,
}

impl IvSurface {
    /// Build an IV surface from (strike_offset, iv) pairs.
    ///
    /// Requires at least 3 data points. Returns `None` if insufficient data.
    /// Input need not be sorted — sorting is handled internally.
    pub fn from_points(points: &[(f64, f64)]) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }

        // Sort by strike offset, deduplicate
        let mut sorted: Vec<(f64, f64)> = points.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        sorted.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10);

        if sorted.len() < 3 {
            return None;
        }


        let offsets: Vec<f64> = sorted.iter().map(|p| p.0).collect();
        let ivs: Vec<f64> = sorted.iter().map(|p| p.1).collect();

        // Compute natural cubic spline coefficients using Thomas algorithm
        let coeffs = compute_cubic_spline(&offsets, &ivs);

        Some(Self {
            offsets,
            ivs,
            coeffs,
        })
    }

    /// Interpolate IV at a given strike offset.
    ///
    /// Uses binary search to find the interval, then evaluates
    /// the cubic polynomial. Extrapolates flat outside data range.
    pub fn interpolate(&self, strike_offset: f64) -> f64 {
        let n = self.offsets.len();

        // Flat extrapolation outside range
        if strike_offset <= self.offsets[0] {
            return self.ivs[0];
        }
        if strike_offset >= self.offsets[n - 1] {
            return self.ivs[n - 1];
        }

        // Binary search for the interval
        let i = match self
            .offsets
            .binary_search_by(|probe| probe.partial_cmp(&strike_offset).unwrap())
        {
            Ok(idx) => return self.ivs[idx], // Exact match
            Err(idx) => idx.saturating_sub(1),
        };

        // Evaluate cubic polynomial
        let (a, b, c, d) = self.coeffs[i];
        let dx = strike_offset - self.offsets[i];
        a + b * dx + c * dx * dx + d * dx * dx * dx
    }

    /// Build IV surface from a slice of Bars at the same timestamp.
    ///
    /// Extracts unique (strike_offset, iv) pairs, filters NaN/zero IV.
    pub fn from_bars(bars: &[Bar]) -> Option<Self> {
        let points: Vec<(f64, f64)> = bars
            .iter()
            .filter(|b| b.iv > 0.0 && b.iv.is_finite())
            .map(|b| (b.strike_offset as f64, b.iv))
            .collect();
        Self::from_points(&points)
    }

    /// Get the number of data points in the surface.
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    /// Check if the surface has no data points.
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// Get the range of strike offsets covered.
    pub fn range(&self) -> (f64, f64) {
        (
            *self.offsets.first().unwrap_or(&0.0),
            *self.offsets.last().unwrap_or(&0.0),
        )
    }
}

/// Compute natural cubic spline coefficients using the Thomas algorithm.
///
/// Given n data points, produces n-1 sets of (a, b, c, d) coefficients.
/// Natural boundary conditions: S''(x_0) = S''(x_{n-1}) = 0.
fn compute_cubic_spline(x: &[f64], y: &[f64]) -> Vec<(f64, f64, f64, f64)> {
    let n = x.len();
    assert!(n >= 3);

    let m = n - 1; // number of intervals

    // Step 1: Compute interval widths h_i
    let h: Vec<f64> = (0..m).map(|i| x[i + 1] - x[i]).collect();

    // Step 2: Set up tridiagonal system for second derivatives
    // Natural BC: c_0 = 0, c_{n-1} = 0
    let mut alpha = vec![0.0; n];
    for i in 1..m {
        alpha[i] = (3.0 / h[i]) * (y[i + 1] - y[i]) - (3.0 / h[i - 1]) * (y[i] - y[i - 1]);
    }

    // Step 3: Solve tridiagonal system (Thomas algorithm)
    let mut c = vec![0.0; n];
    let mut l = vec![1.0; n];
    let mut mu = vec![0.0; n];
    let mut z = vec![0.0; n];

    for i in 1..m {
        l[i] = 2.0 * (x[i + 1] - x[i - 1]) - h[i - 1] * mu[i - 1];
        mu[i] = h[i] / l[i];
        z[i] = (alpha[i] - h[i - 1] * z[i - 1]) / l[i];
    }

    // Back-substitution
    for j in (0..m).rev() {
        c[j] = z[j] - mu[j] * c[j + 1];
    }

    // Step 4: Compute a, b, d from c
    let mut coeffs = Vec::with_capacity(m);
    for i in 0..m {
        let a = y[i];
        let b = (y[i + 1] - y[i]) / h[i] - h[i] * (c[i + 1] + 2.0 * c[i]) / 3.0;
        let d = (c[i + 1] - c[i]) / (3.0 * h[i]);
        coeffs.push((a, b, c[i], d));
    }

    coeffs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_at_data_points() {
        let points = vec![(-5.0, 0.20), (0.0, 0.18), (5.0, 0.22), (10.0, 0.25)];
        let surface = IvSurface::from_points(&points).unwrap();
        assert!((surface.interpolate(0.0) - 0.18).abs() < 1e-10);
        assert!((surface.interpolate(-5.0) - 0.20).abs() < 1e-10);
        assert!((surface.interpolate(10.0) - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_interpolation_between_points() {
        let points = vec![(-5.0, 0.20), (0.0, 0.18), (5.0, 0.22)];
        let surface = IvSurface::from_points(&points).unwrap();
        let iv_at_2 = surface.interpolate(2.0);
        // Should be between min and max IV (reasonable interpolation)
        assert!(iv_at_2 > 0.15 && iv_at_2 < 0.30);
    }

    #[test]
    fn test_extrapolation_flat() {
        let points = vec![(-5.0, 0.20), (0.0, 0.18), (5.0, 0.22)];
        let surface = IvSurface::from_points(&points).unwrap();
        assert!((surface.interpolate(-10.0) - 0.20).abs() < 1e-10);
        assert!((surface.interpolate(15.0) - 0.22).abs() < 1e-10);
    }

    #[test]
    fn test_too_few_points() {
        let points = vec![(0.0, 0.18), (5.0, 0.22)];
        assert!(IvSurface::from_points(&points).is_none());
    }

    #[test]
    fn test_unsorted_input() {
        let points = vec![(5.0, 0.22), (-5.0, 0.20), (0.0, 0.18)];
        let surface = IvSurface::from_points(&points).unwrap();
        assert!((surface.interpolate(0.0) - 0.18).abs() < 1e-10);
    }

    #[test]
    fn test_len_and_range() {
        let points = vec![(-5.0, 0.20), (0.0, 0.18), (5.0, 0.22)];
        let surface = IvSurface::from_points(&points).unwrap();
        assert_eq!(surface.len(), 3);
        assert!(!surface.is_empty());
        assert_eq!(surface.range(), (-5.0, 5.0));
    }

    #[test]
    fn test_smile_shape() {
        // Typical vol smile: higher IV at wings, lower at ATM
        let points = vec![
            (-10.0, 0.25),
            (-5.0, 0.21),
            (0.0, 0.18),
            (5.0, 0.20),
            (10.0, 0.24),
        ];
        let surface = IvSurface::from_points(&points).unwrap();

        // ATM should be the lowest
        let iv_atm = surface.interpolate(0.0);
        let iv_m5 = surface.interpolate(-5.0);
        let iv_p5 = surface.interpolate(5.0);
        assert!(iv_atm < iv_m5);
        assert!(iv_atm < iv_p5);
    }

    #[test]
    fn test_continuity() {
        // Spline should be continuous — adjacent interpolations should be close
        let points = vec![(-5.0, 0.20), (0.0, 0.18), (5.0, 0.22), (10.0, 0.25)];
        let surface = IvSurface::from_points(&points).unwrap();

        let iv_a = surface.interpolate(2.49);
        let iv_b = surface.interpolate(2.51);
        assert!((iv_a - iv_b).abs() < 0.01);
    }
}
