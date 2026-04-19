#!/usr/bin/env python3
"""
Data Validation Script for QuantEdge.

Implements 6 integrity checks on converted Parquet data:
1. Weekly cutoff alignment - bars before/after transition dates go to correct partitions
2. Date gap detection - identifies missing trading days
3. Duplicate detection - flags duplicate (date, time, strike_offset, option_type) tuples
4. IV coverage check - ensures sufficient IV data points per timestamp
5. Spot continuity - validates spot price doesn't jump >10% between adjacent bars
6. Lot size coverage - verifies lot_sizes.toml covers all dates in data

Usage:
    python scripts/validate_data.py --data-dir data/parquet/ [--symbol BANKNIFTY]
"""

import argparse
import os
import sys
from datetime import date, timedelta
from pathlib import Path

try:
    import polars as pl
except ImportError:
    print("Error: polars is required. Install with: pip install polars>=1.0")
    sys.exit(1)

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        print("Error: tomllib (Python 3.11+) or tomli required.")
        sys.exit(1)


class ValidationResult:
    """Holds results for a single validation check."""

    def __init__(self, name: str):
        self.name = name
        self.passed = True
        self.warnings: list[str] = []
        self.errors: list[str] = []

    def warn(self, msg: str):
        self.warnings.append(msg)

    def error(self, msg: str):
        self.passed = False
        self.errors.append(msg)

    def summary(self) -> str:
        status = "✓ PASS" if self.passed else "✗ FAIL"
        return f"  [{status}] {self.name} ({len(self.errors)} errors, {len(self.warnings)} warnings)"


def load_expiry_config(config_path: str) -> dict:
    """Load expiry calendar transitions."""
    with open(config_path, "rb") as f:
        config = tomllib.load(f)
    transitions = {}
    for symbol, data in config.items():
        rules = data.get("transitions", [])
        for rule in rules:
            if rule["type"] == "monthly":
                cutoff = date.fromisoformat(rule["from"])
                transitions[symbol.upper()] = cutoff
                break
    return transitions


def load_lot_sizes(config_path: str) -> dict:
    """Load lot size configurations."""
    with open(config_path, "rb") as f:
        config = tomllib.load(f)
    return {k.upper(): v for k, v in config.items()}


# ─── CHECK 1: Weekly Cutoff Alignment ───────────────────────

def check_weekly_cutoff(
    data_dir: str, symbol: str, transitions: dict
) -> ValidationResult:
    """Verify bars are in correct weekly/monthly partitions."""
    result = ValidationResult("Weekly cutoff alignment")

    cutoff = transitions.get(symbol.upper())
    if cutoff is None:
        result.warn(f"No transition config for {symbol}")
        return result

    # Check weekly partition doesn't have dates >= cutoff
    weekly_dir = Path(data_dir) / symbol.lower() / "weekly"
    if weekly_dir.exists():
        for parquet_file in weekly_dir.rglob("*.parquet"):
            try:
                df = pl.scan_parquet(str(parquet_file)).select("date").collect()
                if "date" in df.columns:
                    max_date = df["date"].max()
                    if max_date and max_date >= cutoff:
                        result.error(
                            f"Weekly partition {parquet_file.name} has dates >= cutoff "
                            f"({cutoff}): max_date={max_date}"
                        )
            except Exception as e:
                result.warn(f"Could not read {parquet_file}: {e}")

    # Check monthly partition doesn't have dates < cutoff
    monthly_dir = Path(data_dir) / symbol.lower() / "monthly"
    if monthly_dir.exists():
        for parquet_file in monthly_dir.rglob("*.parquet"):
            try:
                df = pl.scan_parquet(str(parquet_file)).select("date").collect()
                if "date" in df.columns:
                    min_date = df["date"].min()
                    if min_date and min_date < cutoff:
                        result.error(
                            f"Monthly partition {parquet_file.name} has dates < cutoff "
                            f"({cutoff}): min_date={min_date}"
                        )
            except Exception as e:
                result.warn(f"Could not read {parquet_file}: {e}")

    return result


# ─── CHECK 2: Date Gap Detection ────────────────────────────

def check_date_gaps(data_dir: str, symbol: str) -> ValidationResult:
    """Identify gaps > 3 calendar days (accounts for weekends + holidays)."""
    result = ValidationResult("Date gap detection")

    symbol_dir = Path(data_dir) / symbol.lower()
    if not symbol_dir.exists():
        result.warn(f"No data directory for {symbol}")
        return result

    all_dates = set()
    for parquet_file in symbol_dir.rglob("*.parquet"):
        try:
            df = pl.scan_parquet(str(parquet_file)).select("date").collect()
            if "date" in df.columns:
                dates = df["date"].unique().sort().to_list()
                all_dates.update(dates)
        except Exception as e:
            result.warn(f"Could not read {parquet_file}: {e}")

    if len(all_dates) < 2:
        result.warn("Insufficient dates for gap analysis")
        return result

    sorted_dates = sorted(all_dates)
    gaps = []
    for i in range(1, len(sorted_dates)):
        gap = (sorted_dates[i] - sorted_dates[i - 1]).days
        if gap > 5:  # >5 calendar days = suspicious gap (allows for long weekends)
            gaps.append((sorted_dates[i - 1], sorted_dates[i], gap))

    for prev, curr, gap_days in gaps:
        result.warn(f"Gap of {gap_days} days: {prev} → {curr}")

    if len(gaps) > 20:
        result.error(f"Too many date gaps ({len(gaps)}) — data may be incomplete")

    return result


# ─── CHECK 3: Duplicate Detection ───────────────────────────

def check_duplicates(data_dir: str, symbol: str) -> ValidationResult:
    """Flag duplicate (date, time, strike_offset, option_type) tuples."""
    result = ValidationResult("Duplicate detection")

    symbol_dir = Path(data_dir) / symbol.lower()
    if not symbol_dir.exists():
        result.warn(f"No data directory for {symbol}")
        return result

    total_dupes = 0
    for parquet_file in symbol_dir.rglob("*.parquet"):
        try:
            df = pl.read_parquet(str(parquet_file))
            key_cols = ["date", "time", "strike_offset", "option_type"]
            available_keys = [c for c in key_cols if c in df.columns]
            if len(available_keys) < 3:
                continue

            n_total = len(df)
            n_unique = df.select(available_keys).unique().height
            dupes = n_total - n_unique

            if dupes > 0:
                total_dupes += dupes
                result.warn(
                    f"{parquet_file.relative_to(Path(data_dir))}: "
                    f"{dupes} duplicates out of {n_total} rows"
                )
        except Exception as e:
            result.warn(f"Could not read {parquet_file}: {e}")

    if total_dupes > 0:
        result.error(f"Total duplicates found: {total_dupes}")

    return result


# ─── CHECK 4: IV Coverage ───────────────────────────────────

def check_iv_coverage(data_dir: str, symbol: str) -> ValidationResult:
    """Ensure >=3 valid IV points per timestamp for spline interpolation."""
    result = ValidationResult("IV coverage")

    symbol_dir = Path(data_dir) / symbol.lower()
    if not symbol_dir.exists():
        result.warn(f"No data directory for {symbol}")
        return result

    low_iv_count = 0
    total_timestamps = 0

    for parquet_file in symbol_dir.rglob("*.parquet"):
        try:
            df = pl.read_parquet(str(parquet_file))
            if "iv" not in df.columns or "timestamp" not in df.columns:
                continue

            # Group by timestamp, count valid IV points
            iv_counts = (
                df.filter(pl.col("iv") > 0.0)
                .group_by("timestamp")
                .agg(pl.count().alias("iv_count"))
            )

            total_timestamps += iv_counts.height
            low_iv = iv_counts.filter(pl.col("iv_count") < 3)
            low_iv_count += low_iv.height

        except Exception as e:
            result.warn(f"Could not read {parquet_file}: {e}")

    if total_timestamps > 0:
        coverage_pct = ((total_timestamps - low_iv_count) / total_timestamps) * 100
        if coverage_pct < 95:
            result.error(
                f"IV coverage {coverage_pct:.1f}% < 95% threshold "
                f"({low_iv_count}/{total_timestamps} timestamps with <3 IV points)"
            )
        elif coverage_pct < 99:
            result.warn(f"IV coverage {coverage_pct:.1f}% (good but not perfect)")

    return result


# ─── CHECK 5: Spot Continuity ───────────────────────────────

def check_spot_continuity(data_dir: str, symbol: str) -> ValidationResult:
    """Validate spot price doesn't jump >10% between adjacent bars."""
    result = ValidationResult("Spot continuity")

    symbol_dir = Path(data_dir) / symbol.lower()
    if not symbol_dir.exists():
        result.warn(f"No data directory for {symbol}")
        return result

    big_jumps = 0
    for parquet_file in symbol_dir.rglob("*.parquet"):
        try:
            df = pl.read_parquet(str(parquet_file))
            if "spot" not in df.columns or "date" not in df.columns:
                continue

            # Get unique spot per (date, time)
            spots = (
                df.select(["date", "time", "spot"])
                .unique()
                .sort(["date", "time"])
            )

            spot_values = spots["spot"].to_list()
            for i in range(1, len(spot_values)):
                if spot_values[i - 1] > 0:
                    pct_change = abs(spot_values[i] - spot_values[i - 1]) / spot_values[i - 1]
                    if pct_change > 0.10:
                        big_jumps += 1

        except Exception as e:
            result.warn(f"Could not read {parquet_file}: {e}")

    if big_jumps > 0:
        result.error(f"{big_jumps} spot price jumps >10% detected")

    return result


# ─── CHECK 6: Lot Size Coverage ─────────────────────────────

def check_lot_size_coverage(
    data_dir: str, symbol: str, lot_sizes: dict
) -> ValidationResult:
    """Verify lot_sizes.toml covers all dates in the data."""
    result = ValidationResult("Lot size coverage")

    entries = lot_sizes.get(symbol.upper(), [])
    if not entries:
        result.error(f"No lot size entries for {symbol}")
        return result

    symbol_dir = Path(data_dir) / symbol.lower()
    if not symbol_dir.exists():
        result.warn(f"No data directory for {symbol}")
        return result

    all_dates = set()
    for parquet_file in symbol_dir.rglob("*.parquet"):
        try:
            df = pl.scan_parquet(str(parquet_file)).select("date").collect()
            if "date" in df.columns:
                all_dates.update(df["date"].unique().to_list())
        except Exception:
            pass

    if not all_dates:
        result.warn("No dates found in data")
        return result

    uncovered = 0
    for d in sorted(all_dates):
        covered = False
        for entry in entries:
            entry_from = date.fromisoformat(entry["from"])
            entry_to = date.fromisoformat(entry["to"])
            if entry_from <= d <= entry_to:
                covered = True
                break
        if not covered:
            uncovered += 1

    if uncovered > 0:
        result.error(f"{uncovered} dates not covered by lot_sizes.toml")

    return result


# ─── MAIN ────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Validate converted Parquet data for QuantEdge."
    )
    parser.add_argument(
        "--data-dir",
        default="data/parquet/",
        help="Path to Parquet data directory (default: data/parquet/)",
    )
    parser.add_argument(
        "--symbol",
        default=None,
        help="Validate specific symbol only (default: all found symbols)",
    )
    parser.add_argument(
        "--expiry-config",
        default="config/expiry_calendar.toml",
        help="Path to expiry calendar config",
    )
    parser.add_argument(
        "--lot-config",
        default="config/lot_sizes.toml",
        help="Path to lot sizes config",
    )
    args = parser.parse_args()

    print("━" * 50)
    print(" QuantEdge Data Validation")
    print("━" * 50)

    # Detect symbols
    data_path = Path(args.data_dir)
    if not data_path.exists():
        print(f"\n  ✗ Data directory not found: {args.data_dir}")
        print("  Run csv_to_parquet.py first to convert data.")
        sys.exit(1)

    if args.symbol:
        symbols = [args.symbol.upper()]
    else:
        symbols = [
            d.name.upper()
            for d in data_path.iterdir()
            if d.is_dir() and not d.name.startswith(".")
        ]

    if not symbols:
        print(f"\n  ✗ No symbol directories found in {args.data_dir}")
        sys.exit(1)

    # Load configs
    try:
        transitions = load_expiry_config(args.expiry_config)
    except FileNotFoundError:
        print(f"  ⚠ Expiry config not found: {args.expiry_config}")
        transitions = {}

    try:
        lot_sizes = load_lot_sizes(args.lot_config)
    except FileNotFoundError:
        print(f"  ⚠ Lot sizes config not found: {args.lot_config}")
        lot_sizes = {}

    # Run validations
    all_passed = True
    for symbol in sorted(symbols):
        print(f"\n  ── {symbol} ──")

        checks = [
            check_weekly_cutoff(args.data_dir, symbol, transitions),
            check_date_gaps(args.data_dir, symbol),
            check_duplicates(args.data_dir, symbol),
            check_iv_coverage(args.data_dir, symbol),
            check_spot_continuity(args.data_dir, symbol),
            check_lot_size_coverage(args.data_dir, symbol, lot_sizes),
        ]

        for check in checks:
            print(check.summary())
            if check.errors:
                for err in check.errors[:3]:  # Show max 3 errors
                    print(f"      → {err}")
            if check.warnings:
                for warn in check.warnings[:3]:  # Show max 3 warnings
                    print(f"      ⚠ {warn}")
            if not check.passed:
                all_passed = False

    # Summary
    print("\n" + "━" * 50)
    if all_passed:
        print(" ✓ All validations passed!")
    else:
        print(" ✗ Some validations failed — review errors above")
    print("━" * 50)

    sys.exit(0 if all_passed else 1)


if __name__ == "__main__":
    main()
