//! Momentum filters for entry gating and re-entry confirmation.
//!
//! Four filter types: RSI, EMA Cross, Range Breakout, and Supertrend.
//! Filters produce per-bar signals (Bullish/Bearish/Neutral).
//! Range breakout is pre-computed in O(N) for O(1) per-bar lookup.

use crate::runner::SimBar;
use chrono::{NaiveDate, NaiveTime};
use std::collections::HashMap;

/// Momentum signal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MomentumSignal {
    Bullish,
    Bearish,
    Neutral,
}

/// Configuration for a momentum filter.
#[derive(Debug, Clone)]
pub enum MomentumFilterConfig {
    Rsi {
        period: usize,
        bullish_above: f64,
        bearish_below: f64,
    },
    EmaCross {
        fast: usize,
        slow: usize,
    },
    RangeBreakout {
        range_end_time: NaiveTime,
    },
    Supertrend {
        period: usize,
        multiplier: f64,
    },
}

/// Pre-computed momentum signals for a bar series.
pub struct MomentumEngine {
    /// One signal per unique (date, time) group, in chronological order.
    signals: Vec<MomentumSignal>,
    /// Map from (date, time) → index into signals vec.
    index_map: HashMap<(NaiveDate, NaiveTime), usize>,
}

impl MomentumEngine {
    /// Build momentum engine from config and bar data.
    ///
    /// Extracts spot prices from bars (one per unique time group),
    /// then computes signals for the chosen filter type.
    pub fn from_config_and_bars(
        config: &MomentumFilterConfig,
        bars: &[SimBar],
    ) -> Self {
        // Extract one spot price per unique (date, time) group
        let mut groups: Vec<((NaiveDate, NaiveTime), f64)> = Vec::new();
        let mut index_map = HashMap::new();
        let mut last_key: Option<(NaiveDate, NaiveTime)> = None;

        for bar in bars {
            let key = (bar.date, bar.time);
            if last_key.as_ref() != Some(&key) {
                index_map.insert(key, groups.len());
                groups.push((key, bar.spot));
                last_key = Some(key);
            }
        }

        let spots: Vec<f64> = groups.iter().map(|(_, s)| *s).collect();
        let keys: Vec<(NaiveDate, NaiveTime)> = groups.iter().map(|(k, _)| *k).collect();

        let signals = match config {
            MomentumFilterConfig::Rsi { period, bullish_above, bearish_below } => {
                Self::compute_rsi(&spots, *period, *bullish_above, *bearish_below)
            }
            MomentumFilterConfig::EmaCross { fast, slow } => {
                Self::compute_ema_cross(&spots, *fast, *slow)
            }
            MomentumFilterConfig::RangeBreakout { range_end_time } => {
                Self::compute_range_breakout(&keys, &spots, *range_end_time)
            }
            MomentumFilterConfig::Supertrend { period, multiplier } => {
                Self::compute_supertrend(&spots, *period, *multiplier)
            }
        };

        Self { signals, index_map }
    }

    /// Get signal for a given (date, time) key.
    pub fn signal_at(&self, date: NaiveDate, time: NaiveTime) -> MomentumSignal {
        self.index_map
            .get(&(date, time))
            .and_then(|&idx| self.signals.get(idx))
            .copied()
            .unwrap_or(MomentumSignal::Neutral)
    }

    /// Get all signals (for testing).
    pub fn signals(&self) -> &[MomentumSignal] {
        &self.signals
    }

    // ── RSI ──────────────────────────────────────────────────

    fn compute_rsi(
        spots: &[f64],
        period: usize,
        bullish_above: f64,
        bearish_below: f64,
    ) -> Vec<MomentumSignal> {
        if spots.len() < 2 {
            return vec![MomentumSignal::Neutral; spots.len()];
        }

        let mut signals = vec![MomentumSignal::Neutral; spots.len()];
        let mut avg_gain = 0.0;
        let mut avg_loss = 0.0;

        // First `period` changes: seed with SMA
        let init_len = period.min(spots.len() - 1);
        for i in 1..=init_len {
            let change = spots[i] - spots[i - 1];
            if change > 0.0 {
                avg_gain += change;
            } else {
                avg_loss += change.abs();
            }
        }
        avg_gain /= period as f64;
        avg_loss /= period as f64;

        if init_len >= period {
            let rsi = if avg_loss == 0.0 {
                100.0
            } else {
                100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
            };
            signals[period] = Self::rsi_signal(rsi, bullish_above, bearish_below);
        }

        // Subsequent bars: EMA smoothing
        for i in (period + 1)..spots.len() {
            let change = spots[i] - spots[i - 1];
            let (gain, loss) = if change > 0.0 {
                (change, 0.0)
            } else {
                (0.0, change.abs())
            };

            avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
            avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;

            let rsi = if avg_loss == 0.0 {
                100.0
            } else {
                100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
            };
            signals[i] = Self::rsi_signal(rsi, bullish_above, bearish_below);
        }

        signals
    }

    fn rsi_signal(rsi: f64, bullish_above: f64, bearish_below: f64) -> MomentumSignal {
        if rsi > bullish_above {
            MomentumSignal::Bullish
        } else if rsi < bearish_below {
            MomentumSignal::Bearish
        } else {
            MomentumSignal::Neutral
        }
    }

    // ── EMA Cross ────────────────────────────────────────────

    fn compute_ema_cross(spots: &[f64], fast: usize, slow: usize) -> Vec<MomentumSignal> {
        if spots.is_empty() {
            return Vec::new();
        }

        let fast_ema = Self::ema(spots, fast);
        let slow_ema = Self::ema(spots, slow);

        fast_ema
            .iter()
            .zip(slow_ema.iter())
            .map(|(f, s)| {
                if f > s {
                    MomentumSignal::Bullish
                } else if f < s {
                    MomentumSignal::Bearish
                } else {
                    MomentumSignal::Neutral
                }
            })
            .collect()
    }

    fn ema(prices: &[f64], period: usize) -> Vec<f64> {
        let mut result = vec![0.0; prices.len()];
        if prices.is_empty() || period == 0 {
            return result;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        result[0] = prices[0];

        for i in 1..prices.len() {
            result[i] = prices[i] * multiplier + result[i - 1] * (1.0 - multiplier);
        }

        result
    }

    // ── Range Breakout (RE-04: O(N) setup, O(1) lookup) ─────

    fn compute_range_breakout(
        keys: &[(NaiveDate, NaiveTime)],
        spots: &[f64],
        range_end_time: NaiveTime,
    ) -> Vec<MomentumSignal> {
        // Phase 1: O(N) pre-compute range high/low per day
        let mut ranges: HashMap<NaiveDate, (f64, f64)> = HashMap::new();

        for (i, (date, time)) in keys.iter().enumerate() {
            if *time <= range_end_time {
                let entry = ranges.entry(*date).or_insert((f64::MIN, f64::MAX));
                entry.0 = entry.0.max(spots[i]); // high
                entry.1 = entry.1.min(spots[i]); // low
            }
        }

        // Phase 2: O(1) per-bar signal lookup
        keys.iter()
            .enumerate()
            .map(|(i, (date, time))| {
                if *time <= range_end_time {
                    return MomentumSignal::Neutral; // still forming range
                }

                if let Some(&(high, low)) = ranges.get(date) {
                    if high == f64::MIN {
                        return MomentumSignal::Neutral;
                    }
                    if spots[i] > high {
                        MomentumSignal::Bullish
                    } else if spots[i] < low {
                        MomentumSignal::Bearish
                    } else {
                        MomentumSignal::Neutral
                    }
                } else {
                    MomentumSignal::Neutral
                }
            })
            .collect()
    }

    // ── Supertrend ───────────────────────────────────────────

    fn compute_supertrend(
        spots: &[f64],
        period: usize,
        multiplier: f64,
    ) -> Vec<MomentumSignal> {
        if spots.len() < period + 1 {
            return vec![MomentumSignal::Neutral; spots.len()];
        }

        let mut signals = vec![MomentumSignal::Neutral; spots.len()];

        // Compute ATR using simple true range (|close[i] - close[i-1]|)
        let mut atr = vec![0.0; spots.len()];
        // Seed ATR with SMA of first `period` true ranges
        let mut sum = 0.0;
        for i in 1..=period.min(spots.len() - 1) {
            sum += (spots[i] - spots[i - 1]).abs();
        }
        if period > 0 && period < spots.len() {
            atr[period] = sum / period as f64;
        }

        // EMA-smoothed ATR
        for i in (period + 1)..spots.len() {
            let tr = (spots[i] - spots[i - 1]).abs();
            atr[i] = (atr[i - 1] * (period as f64 - 1.0) + tr) / period as f64;
        }

        // Supertrend computation
        let mut upper_band = vec![0.0; spots.len()];
        let mut lower_band = vec![0.0; spots.len()];
        let mut in_uptrend = true;

        for i in period..spots.len() {
            let mid = spots[i]; // using close as midpoint
            let basic_upper = mid + multiplier * atr[i];
            let basic_lower = mid - multiplier * atr[i];

            upper_band[i] = if i > period && basic_upper < upper_band[i - 1] {
                basic_upper
            } else if i > period {
                upper_band[i - 1].min(basic_upper)
            } else {
                basic_upper
            };

            lower_band[i] = if i > period && basic_lower > lower_band[i - 1] {
                basic_lower
            } else if i > period {
                lower_band[i - 1].max(basic_lower)
            } else {
                basic_lower
            };

            if spots[i] > upper_band[i] {
                in_uptrend = true;
            } else if spots[i] < lower_band[i] {
                in_uptrend = false;
            }

            signals[i] = if in_uptrend {
                MomentumSignal::Bullish
            } else {
                MomentumSignal::Bearish
            };
        }

        signals
    }
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spot_bars(spots: &[(NaiveDate, NaiveTime, f64)]) -> Vec<SimBar> {
        spots
            .iter()
            .map(|(date, time, spot)| SimBar {
                date: *date,
                time: *time,
                option_type: "CE".to_string(),
                strike_offset: 0,
                close: 200.0, high: 200.0, low: 200.0,
                spot: *spot,
            })
            .collect()
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }
    fn t(h: u32, min: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, min, 0).unwrap()
    }

    // ── RSI Tests ────────────────────────────────────────────

    #[test]
    fn test_rsi_bullish_uptrend() {
        // Steadily rising spots → RSI > 50 → Bullish
        let spots: Vec<f64> = (0..20).map(|i| 48000.0 + i as f64 * 50.0).collect();
        let signals = MomentumEngine::compute_rsi(&spots, 14, 50.0, 50.0);
        // After warmup period, should be Bullish
        let last = signals.last().unwrap();
        assert_eq!(*last, MomentumSignal::Bullish);
    }

    #[test]
    fn test_rsi_bearish_downtrend() {
        // Steadily falling spots → RSI < 50 → Bearish
        let spots: Vec<f64> = (0..20).map(|i| 48000.0 - i as f64 * 50.0).collect();
        let signals = MomentumEngine::compute_rsi(&spots, 14, 50.0, 50.0);
        let last = signals.last().unwrap();
        assert_eq!(*last, MomentumSignal::Bearish);
    }

    #[test]
    fn test_rsi_neutral_flat() {
        // Alternating up/down → RSI near 50 → Neutral
        let spots: Vec<f64> = (0..30)
            .map(|i| if i % 2 == 0 { 48000.0 } else { 48010.0 })
            .collect();
        let signals = MomentumEngine::compute_rsi(&spots, 14, 60.0, 40.0);
        let last = signals.last().unwrap();
        assert_eq!(*last, MomentumSignal::Neutral);
    }

    // ── EMA Cross Tests ──────────────────────────────────────

    #[test]
    fn test_ema_cross_bullish() {
        // Rising prices: fast EMA > slow EMA → Bullish
        let spots: Vec<f64> = (0..30).map(|i| 48000.0 + i as f64 * 100.0).collect();
        let signals = MomentumEngine::compute_ema_cross(&spots, 9, 21);
        let last = signals.last().unwrap();
        assert_eq!(*last, MomentumSignal::Bullish);
    }

    #[test]
    fn test_ema_cross_bearish() {
        // Falling prices: fast EMA < slow EMA → Bearish
        let spots: Vec<f64> = (0..30).map(|i| 48000.0 - i as f64 * 100.0).collect();
        let signals = MomentumEngine::compute_ema_cross(&spots, 9, 21);
        let last = signals.last().unwrap();
        assert_eq!(*last, MomentumSignal::Bearish);
    }

    // ── Range Breakout Tests ─────────────────────────────────

    #[test]
    fn test_range_during_formation() {
        // Before range_end_time → always Neutral
        let spots = vec![
            (d(2024, 1, 15), t(9, 15), 48000.0),
            (d(2024, 1, 15), t(9, 30), 48050.0),
            (d(2024, 1, 15), t(9, 45), 48020.0),
        ];
        let bars = make_spot_bars(&spots);
        let config = MomentumFilterConfig::RangeBreakout {
            range_end_time: t(9, 45),
        };
        let engine = MomentumEngine::from_config_and_bars(&config, &bars);
        for s in engine.signals() {
            assert_eq!(*s, MomentumSignal::Neutral);
        }
    }

    #[test]
    fn test_range_breakout_bullish() {
        // Range high = 48050. After range, spot = 48100 → Bullish
        let spots = vec![
            (d(2024, 1, 15), t(9, 15), 48000.0),
            (d(2024, 1, 15), t(9, 30), 48050.0),
            (d(2024, 1, 15), t(9, 45), 48020.0),
            (d(2024, 1, 15), t(10, 0), 48100.0), // above range high
        ];
        let bars = make_spot_bars(&spots);
        let config = MomentumFilterConfig::RangeBreakout {
            range_end_time: t(9, 45),
        };
        let engine = MomentumEngine::from_config_and_bars(&config, &bars);
        assert_eq!(engine.signal_at(d(2024, 1, 15), t(10, 0)), MomentumSignal::Bullish);
    }

    #[test]
    fn test_range_breakout_bearish() {
        // Range low = 48000. After range, spot = 47950 → Bearish
        let spots = vec![
            (d(2024, 1, 15), t(9, 15), 48000.0),
            (d(2024, 1, 15), t(9, 30), 48050.0),
            (d(2024, 1, 15), t(9, 45), 48020.0),
            (d(2024, 1, 15), t(10, 0), 47950.0), // below range low
        ];
        let bars = make_spot_bars(&spots);
        let config = MomentumFilterConfig::RangeBreakout {
            range_end_time: t(9, 45),
        };
        let engine = MomentumEngine::from_config_and_bars(&config, &bars);
        assert_eq!(engine.signal_at(d(2024, 1, 15), t(10, 0)), MomentumSignal::Bearish);
    }

    #[test]
    fn test_range_breakout_o1_lookup() {
        // Verify O(1) lookup via HashMap — same signal returned consistently
        let spots = vec![
            (d(2024, 1, 15), t(9, 15), 48000.0),
            (d(2024, 1, 15), t(9, 45), 48050.0),
            (d(2024, 1, 15), t(10, 0), 48100.0),
        ];
        let bars = make_spot_bars(&spots);
        let config = MomentumFilterConfig::RangeBreakout {
            range_end_time: t(9, 45),
        };
        let engine = MomentumEngine::from_config_and_bars(&config, &bars);
        // Lookup same key 1000 times — HashMap O(1)
        for _ in 0..1000 {
            let _ = engine.signal_at(d(2024, 1, 15), t(10, 0));
        }
        assert_eq!(engine.signal_at(d(2024, 1, 15), t(10, 0)), MomentumSignal::Bullish);
    }

    // ── Supertrend Tests ─────────────────────────────────────

    #[test]
    fn test_supertrend_bullish_uptrend() {
        // Strong uptrend → Bullish
        let spots: Vec<f64> = (0..20).map(|i| 48000.0 + i as f64 * 200.0).collect();
        let signals = MomentumEngine::compute_supertrend(&spots, 7, 3.0);
        let last = signals.last().unwrap();
        assert_eq!(*last, MomentumSignal::Bullish);
    }

    #[test]
    fn test_supertrend_bearish_downtrend() {
        // Strong downtrend → Bearish
        let spots: Vec<f64> = (0..20).map(|i| 48000.0 - i as f64 * 200.0).collect();
        let signals = MomentumEngine::compute_supertrend(&spots, 7, 3.0);
        let last = signals.last().unwrap();
        assert_eq!(*last, MomentumSignal::Bearish);
    }

    // ── Full Engine Tests ────────────────────────────────────

    #[test]
    fn test_engine_from_bars_rsi() {
        let d1 = d(2024, 1, 15);
        let spots: Vec<(NaiveDate, NaiveTime, f64)> = (0..20)
            .map(|i| (d1, t(9, 20 + i), 48000.0 + i as f64 * 30.0))
            .collect();
        let bars = make_spot_bars(&spots);
        let config = MomentumFilterConfig::Rsi {
            period: 14,
            bullish_above: 50.0,
            bearish_below: 50.0,
        };
        let engine = MomentumEngine::from_config_and_bars(&config, &bars);
        assert_eq!(engine.signals().len(), 20);
        // Uptrend → last signal should be Bullish
        assert_eq!(*engine.signals().last().unwrap(), MomentumSignal::Bullish);
    }
}
