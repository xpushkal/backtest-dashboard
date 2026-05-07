//! Bar data types and Parquet reader.
//!
//! Provides the `Bar` struct (1-minute option chain bar) and `BarStream`
//! for loading bars from partitioned Parquet files using Polars memory-mapped I/O.

use chrono::{Datelike, NaiveDate, NaiveTime};
use polars::prelude::*;
use std::path::{Path, PathBuf};

/// A single 1-minute bar from the option chain.
///
/// Fields match the Parquet/CSV schema exactly.
#[derive(Debug, Clone)]
pub struct Bar {
    pub timestamp: String,
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub weekday: String,
    pub option_type: String,
    pub strike_label: String,
    pub strike_offset: i32,
    pub moneyness: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub strike: f64,
    pub oi: f64,
    pub spot: f64,
    pub iv: f64,
}

/// Configuration for loading bars from Parquet.
#[derive(Debug, Clone)]
pub struct BarLoadConfig {
    pub symbol: String,
    pub expiry_type: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub data_dir: String,
}

impl Default for BarLoadConfig {
    fn default() -> Self {
        Self {
            symbol: "BANKNIFTY".to_string(),
            expiry_type: "weekly".to_string(),
            start_date: NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            data_dir: "data/parquet".to_string(),
        }
    }
}

/// Loads bars from partitioned Parquet files.
///
/// Uses Polars `scan_parquet` for lazy evaluation with
/// predicate pushdown (date range filter) and projection pushdown
/// (only required columns).
pub struct BarStream;

impl BarStream {
    /// Load all bars matching the given configuration.
    ///
    /// Scans Parquet files in `{data_dir}/{symbol}/{expiry_type}/{year}/{month:02}.parquet`,
    /// filters by date range, and returns sorted `Vec<Bar>`.
    pub fn load(config: &BarLoadConfig) -> Result<Vec<Bar>, Box<dyn std::error::Error>> {
        let parquet_files = Self::find_parquet_files(config)?;

        if parquet_files.is_empty() {
            return Ok(Vec::new());
        }

        // Scan all matching Parquet files lazily
        let mut frames: Vec<LazyFrame> = Vec::new();
        for path in &parquet_files {
            let pl_path = polars::prelude::PlRefPath::try_from_path(path.as_path())?;
            let lf = LazyFrame::scan_parquet(pl_path, ScanArgsParquet::default())?;
            frames.push(lf);
        }

        // Concatenate all lazy frames
        let combined = if frames.len() == 1 {
            frames.into_iter().next().unwrap()
        } else {
            concat(
                frames,
                UnionArgs {
                    parallel: true,
                    rechunk: false,
                    to_supertypes: true,
                    ..Default::default()
                },
            )?
        };

        // Apply date filter (predicate pushdown)
        let filtered = combined.filter(
            col("date")
                .gt_eq(lit(config.start_date))
                .and(col("date").lt_eq(lit(config.end_date))),
        );

        // Sort by date, time, strike_offset, option_type
        let sorted = filtered.sort(
            ["date", "time", "strike_offset", "option_type"],
            SortMultipleOptions::default(),
        );

        // Collect
        let df = sorted.collect()?;

        // Convert to Vec<Bar>
        Self::dataframe_to_bars(&df)
    }

    /// List available Parquet files for a symbol/expiry_type.
    ///
    /// Returns `Vec<(year, month)>` pairs.
    pub fn list_available(
        data_dir: &str,
        symbol: &str,
        expiry_type: &str,
    ) -> Result<Vec<(i32, u32)>, Box<dyn std::error::Error>> {
        let base = Path::new(data_dir)
            .join(symbol.to_lowercase())
            .join(expiry_type);

        if !base.exists() {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();

        for year_entry in std::fs::read_dir(&base)? {
            let year_entry = year_entry?;
            if !year_entry.file_type()?.is_dir() {
                continue;
            }
            let year: i32 = year_entry
                .file_name()
                .to_string_lossy()
                .parse()
                .unwrap_or(0);
            if year == 0 {
                continue;
            }

            for month_entry in std::fs::read_dir(year_entry.path())? {
                let month_entry = month_entry?;
                let fname = month_entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".parquet") {
                    let month: u32 = fname
                        .trim_end_matches(".parquet")
                        .parse()
                        .unwrap_or(0);
                    if month > 0 && month <= 12 {
                        result.push((year, month));
                    }
                }
            }
        }

        result.sort();
        Ok(result)
    }

    /// Find all Parquet files that could contain data for the given config.
    fn find_parquet_files(config: &BarLoadConfig) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let base = Path::new(&config.data_dir)
            .join(config.symbol.to_lowercase())
            .join(&config.expiry_type);

        if !base.exists() {
            return Ok(Vec::new());
        }

        let start_year = config.start_date.year();
        let end_year = config.end_date.year();
        let start_month = config.start_date.month();
        let end_month = config.end_date.month();

        let mut files = Vec::new();

        for year in start_year..=end_year {
            let year_dir = base.join(format!("{}", year));
            if !year_dir.exists() {
                continue;
            }

            let m_start = if year == start_year { start_month } else { 1 };
            let m_end = if year == end_year { end_month } else { 12 };

            for month in m_start..=m_end {
                let parquet_path = year_dir.join(format!("{:02}.parquet", month));
                if parquet_path.exists() {
                    files.push(parquet_path);
                }
            }
        }

        Ok(files)
    }

    /// Convert a Polars DataFrame to Vec<Bar>.
    ///
    /// Tolerant of two minor schema variations seen in the wild:
    /// - `timestamp` column may be missing (we compute it from date+time)
    /// - `volume` may be stored as either i64 or f64
    fn dataframe_to_bars(df: &DataFrame) -> Result<Vec<Bar>, Box<dyn std::error::Error>> {
        let n = df.height();
        let mut bars = Vec::with_capacity(n);

        let dates = df.column("date")?.date()?;
        let times = df.column("time")?.str()?;
        let weekdays = df.column("weekday")?.str()?;
        let option_types = df.column("option_type")?.str()?;
        let strike_labels = df.column("strike_label")?.str()?;
        let strike_offsets = df.column("strike_offset")?.i32()?;
        let moneyness_col = df.column("moneyness")?.str()?;
        let opens = df.column("open")?.f64()?;
        let highs = df.column("high")?.f64()?;
        let lows = df.column("low")?.f64()?;
        let closes = df.column("close")?.f64()?;
        let strikes = df.column("strike")?.f64()?;
        let ois = df.column("oi")?.f64()?;
        let spots = df.column("spot")?.f64()?;
        let ivs = df.column("iv")?.f64()?;

        // Volume tolerance: accept i64 or f64
        let volume_col = df.column("volume")?;
        let volume_i64 = volume_col.i64().ok();
        let volume_f64 = volume_col.f64().ok();

        // Timestamp is optional — compute from date+time when missing
        let timestamps_opt = df.column("timestamp").ok().and_then(|c| c.str().ok().cloned());

        for i in 0..n {
            // Convert Polars date (days since epoch) to NaiveDate
            let date_val = dates.get(i).unwrap_or(0);
            let date = NaiveDate::from_num_days_from_ce_opt(
                date_val + 719_163, // 1970-01-01 is day 719163 in CE
            )
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());

            let time_str = times.get(i).unwrap_or("00:00:00");
            let time = NaiveTime::parse_from_str(time_str, "%H:%M:%S")
                .or_else(|_| NaiveTime::parse_from_str(time_str, "%H:%M"))
                .unwrap_or_else(|_| NaiveTime::from_hms_opt(0, 0, 0).unwrap());

            let timestamp = match &timestamps_opt {
                Some(col) => col.get(i).unwrap_or("").to_string(),
                None => format!("{} {}", date, time),
            };

            let volume = if let Some(col) = &volume_i64 {
                col.get(i).unwrap_or(0)
            } else if let Some(col) = &volume_f64 {
                col.get(i).unwrap_or(0.0) as i64
            } else {
                0
            };

            bars.push(Bar {
                timestamp,
                date,
                time,
                weekday: weekdays.get(i).unwrap_or("").to_string(),
                option_type: option_types.get(i).unwrap_or("").to_string(),
                strike_label: strike_labels.get(i).unwrap_or("").to_string(),
                strike_offset: strike_offsets.get(i).unwrap_or(0),
                moneyness: moneyness_col.get(i).unwrap_or("").to_string(),
                open: opens.get(i).unwrap_or(0.0),
                high: highs.get(i).unwrap_or(0.0),
                low: lows.get(i).unwrap_or(0.0),
                close: closes.get(i).unwrap_or(0.0),
                volume,
                strike: strikes.get(i).unwrap_or(0.0),
                oi: ois.get(i).unwrap_or(0.0),
                spot: spots.get(i).unwrap_or(0.0),
                iv: ivs.get(i).unwrap_or(0.0),
            });
        }

        Ok(bars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bar_struct_fields() {
        let bar = Bar {
            timestamp: "2024-01-08 09:15:00".to_string(),
            date: NaiveDate::from_ymd_opt(2024, 1, 8).unwrap(),
            time: NaiveTime::from_hms_opt(9, 15, 0).unwrap(),
            weekday: "Monday".to_string(),
            option_type: "CE".to_string(),
            strike_label: "ATM".to_string(),
            strike_offset: 0,
            moneyness: "ATM".to_string(),
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 153.0,
            volume: 10000,
            strike: 48000.0,
            oi: 500000.0,
            spot: 48000.0,
            iv: 0.18,
        };
        assert_eq!(bar.option_type, "CE");
        assert_eq!(bar.strike_offset, 0);
        assert!(bar.iv > 0.0);
    }

    #[test]
    fn test_bar_load_config_default() {
        let config = BarLoadConfig::default();
        assert_eq!(config.symbol, "BANKNIFTY");
        assert_eq!(config.expiry_type, "weekly");
        assert_eq!(config.data_dir, "data/parquet");
    }

    #[test]
    fn test_list_available_empty_dir() {
        // Non-existent directory should return empty
        let result = BarStream::list_available("/tmp/nonexistent_qe_test", "BANKNIFTY", "weekly");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_load_empty_dir() {
        let config = BarLoadConfig {
            data_dir: "/tmp/nonexistent_qe_test".to_string(),
            ..Default::default()
        };
        let result = BarStream::load(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
