# Phase 1: Data Foundation — Research

**Phase:** 1 — Data Foundation
**Researched:** 2026-04-19
**Confidence:** HIGH

## Objective

Research how to implement the data pipeline for QuantEdge: CSV→Parquet conversion, ExpiryCalendar, BarStream (memory-mapped Parquet reads), IvSurface interpolation, LotSizes, and data validation.

## Key Findings

### 1. CSV to Parquet Conversion

**Recommended approach:** Python + Polars (one-time script)

The PRD specifies Python for the conversion script (`scripts/csv_to_parquet.py`). This is correct — Python Polars is the most ergonomic for one-time batch processing:

```python
import polars as pl

# Read CSV, partition by symbol/type/year/month, write Parquet
df = pl.read_csv("data/raw/banknifty_weekly.csv")
df = df.with_columns(pl.col("date").str.to_date("%Y-%m-%d"))
# Partition and write...
```

**Parquet layout** (from PRD):
```
data/parquet/{symbol}/{weekly|monthly}/{year}/{month:02}.parquet
```

**Key config:** Snappy compression (default), row group size ~1M rows for efficient memory-mapping.

### 2. Rust BarStream with Polars

**Crate:** `polars` v0.53+ with features `["lazy", "parquet", "streaming"]`

**Memory-mapped Parquet reads:**
```rust
use polars::prelude::*;

// LazyFrame scanning - predicate/projection pushdown
let lf = LazyFrame::scan_parquet(
    "data/parquet/banknifty/weekly/2023/01.parquet",
    ScanArgsParquet::default(),
)?;

let bars: Vec<Bar> = lf
    .select([col("timestamp"), col("open"), col("high"), col("low"), col("close"), ...])
    .collect()?
    .into_struct("bar")?;
```

**Performance target:** <100ms for 4yr single symbol, achievable with:
- Memory-mapped Parquet (Polars internal mmap)
- Projection pushdown (only read needed columns)
- Predicate pushdown (filter by date range)

### 3. ExpiryCalendar

**Implementation:** Pure Rust with `chrono` + `toml` config parsing.

**Algorithm for `next_expiry(symbol, date)`:**
1. Load transition rules from `config/expiry_calendar.toml`
2. Find which rule applies for the given date
3. If `type = "weekly"`: find the next occurrence of the configured `day` from the given date
4. If `type = "monthly"`: find the last occurrence of configured `day` in the current or next month
5. Return `NaiveDate` (never a string)

**Edge case: transition week overlap:**
- BankNifty transition: Oct 28 – Nov 1, 2024
- Both weekly and monthly data exist — `expiry_filter` setting in strategy config decides which rows to use

### 4. IvSurface Interpolation

**Recommended crate:** Custom implementation using `ndarray` + simple cubic spline

For this project, a full volatility surface library (like `volsurf` or `surface-lib`) is overkill. The data already has IV per bar, so the interpolation is:

1. For a given bar's timestamp, collect all available IV values across strike offsets
2. Fit a 1D cubic spline across `strike_offset → IV`
3. Optionally interpolate across DTE for multi-expiry lookups

**Simple approach:** Since each bar already has IV, the IvSurface is primarily for:
- Interpolating IV at non-standard strike offsets
- Providing IV for Greeks computation at arbitrary strikes

**Implementation:** Custom cubic spline in Rust (~100 lines), no external dependency needed for 1D natural cubic spline.

### 5. LotSizes

**Implementation:** Config-driven lookup from `config/lot_sizes.toml`.

```rust
struct LotSizes {
    entries: HashMap<String, Vec<LotSizeEntry>>,
}

struct LotSizeEntry {
    from: NaiveDate,
    to: NaiveDate,
    size: u32,
}

impl LotSizes {
    fn get(&self, symbol: &str, date: NaiveDate) -> u32 { ... }
}
```

**Critical rule:** Lot size is stored on Position at ENTRY time — never re-looked up.

### 6. Data Validation

**Script:** `scripts/validate_data.py` (Python + Polars)

Six checks:
1. **Weekly cutoff:** No weekly data after transition date
2. **Date gaps:** No missing trading days (excluding holidays)
3. **Duplicates:** No duplicate bars (same timestamp + strike + option_type)
4. **IV coverage:** ≥95% of bars have non-null, non-zero IV
5. **Spot continuity:** Spot price within ±10% day-to-day
6. **Lot size coverage:** Every (symbol, date) has a lot size entry

## Validation Architecture

### What to Test

| Component | Test Type | What to Verify |
|-----------|-----------|----------------|
| ExpiryCalendar | Unit | Weekly expiry resolution for all 3 symbols |
| ExpiryCalendar | Unit | Monthly expiry resolution post-transition |
| ExpiryCalendar | Unit | Transition week overlap handling |
| LotSizes | Unit | Correct lot for pre/post change dates |
| LotSizes | Unit | BankNifty 15→30 edge (2024-11-19 vs 2024-11-20) |
| BarStream | Integration | Load 4yr data in <100ms |
| BarStream | Unit | Correct column types after Parquet read |
| IvSurface | Unit | Cubic spline matches known values |
| Parquet conversion | Integration | Round-trip: CSV→Parquet→read matches original data |
| Validation | Integration | All 6 checks pass on clean data, fail on bad data |

### Performance Benchmarks

| Operation | Target | How to Measure |
|-----------|--------|----------------|
| 4yr BankNifty load | <100ms | `criterion` bench with real Parquet files |
| ExpiryCalendar lookup | <1μs | `criterion` bench, 1000 lookups |
| IvSurface interpolation | <10μs per point | `criterion` bench |

## Dependencies

### Rust Crates for Phase 1

```toml
[dependencies]
polars = { version = "0.53", features = ["lazy", "parquet", "streaming", "csv"] }
chrono = "0.4"
toml = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
criterion = "0.5"
```

### Python Packages for Scripts

```
polars>=1.0
pyarrow>=14.0
```

## Sources

- Polars docs (docs.rs/polars) — scan_parquet, LazyFrame, Parquet I/O
- ndarray-interp crate — cubic spline interpolation
- Chronos crate — NaiveDate, weekday calculations
- PRD Phase 1 section — exact schema, config format, validation checks

---
*Phase 1 Research*
*Researched: 2026-04-19*

## RESEARCH COMPLETE
