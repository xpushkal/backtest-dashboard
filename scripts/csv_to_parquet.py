#!/usr/bin/env python3
"""
CSV to Parquet Conversion Script for QuantEdge.

Converts raw option chain CSVs into partitioned Parquet files,
organized by symbol/expiry_type/year/month.

Usage:
    python scripts/csv_to_parquet.py --input-dir data/raw/ --output-dir data/parquet/

The script reads transition dates from config/expiry_calendar.toml to determine
whether each row belongs in the weekly/ or monthly/ partition.
"""

import argparse
import os
import sys
from datetime import date
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
        print("Error: tomllib (Python 3.11+) or tomli required for TOML parsing.")
        sys.exit(1)


# Expected column schema
EXPECTED_COLUMNS = {
    "timestamp": pl.Utf8,
    "date": pl.Date,
    "time": pl.Utf8,
    "weekday": pl.Utf8,
    "option_type": pl.Utf8,
    "strike_label": pl.Utf8,
    "strike_offset": pl.Int32,
    "moneyness": pl.Utf8,
    "open": pl.Float64,
    "high": pl.Float64,
    "low": pl.Float64,
    "close": pl.Float64,
    "volume": pl.Int64,
    "strike": pl.Float64,
    "oi": pl.Float64,
    "spot": pl.Float64,
    "iv": pl.Float64,
}


def load_transition_dates(config_path: str = "config/expiry_calendar.toml") -> dict:
    """Load expiry transition dates from TOML config.

    Returns dict mapping symbol -> cutoff_date (date before which = weekly, on/after = monthly).
    """
    with open(config_path, "rb") as f:
        config = tomllib.load(f)

    transitions = {}
    for symbol, data in config.items():
        rules = data.get("transitions", [])
        # Find the transition point: where type changes from weekly to monthly
        for i, rule in enumerate(rules):
            if rule["type"] == "monthly" and i > 0:
                # The cutoff is the 'from' date of the monthly rule
                cutoff = date.fromisoformat(rule["from"])
                transitions[symbol.upper()] = cutoff
                break

    return transitions


def infer_symbol(filename: str) -> str | None:
    """Infer symbol from CSV filename."""
    name = filename.lower()
    if "banknifty" in name or "bnf" in name:
        return "BANKNIFTY"
    elif "nifty" in name:
        return "NIFTY"
    elif "sensex" in name:
        return "SENSEX"
    return None


def validate_schema(df: pl.DataFrame, filename: str) -> bool:
    """Validate DataFrame has the expected columns."""
    missing = set(EXPECTED_COLUMNS.keys()) - set(df.columns)
    if missing:
        print(f"  ✗ {filename}: Missing columns: {missing}")
        return False
    return True


def convert_csv_to_parquet(
    input_dir: str,
    output_dir: str,
    transitions: dict,
) -> dict:
    """Convert all CSVs in input_dir to partitioned Parquet files.

    Returns summary dict with stats.
    """
    input_path = Path(input_dir)
    output_path = Path(output_dir)
    csv_files = sorted(input_path.glob("*.csv"))

    if not csv_files:
        print(f"No CSV files found in {input_dir}")
        return {"files_created": 0, "total_rows": 0}

    stats = {"files_created": 0, "total_rows": 0, "per_symbol": {}}

    for csv_file in csv_files:
        symbol = infer_symbol(csv_file.name)
        if symbol is None:
            print(f"  ⚠ Skipping {csv_file.name}: cannot infer symbol")
            continue

        print(f"\n  Processing: {csv_file.name} → {symbol}")

        # Read CSV
        df = pl.read_csv(str(csv_file), try_parse_dates=True)

        # Try to parse date column if it's still string
        if df.schema.get("date") == pl.Utf8:
            df = df.with_columns(pl.col("date").str.to_date("%Y-%m-%d"))

        # Cast columns to expected types where possible
        if "strike_offset" in df.columns and df.schema.get("strike_offset") != pl.Int32:
            df = df.with_columns(pl.col("strike_offset").cast(pl.Int32))
        if "volume" in df.columns and df.schema.get("volume") != pl.Int64:
            df = df.with_columns(pl.col("volume").cast(pl.Int64))

        if not validate_schema(df, csv_file.name):
            continue

        total_rows = len(df)
        stats["total_rows"] += total_rows

        if symbol not in stats["per_symbol"]:
            stats["per_symbol"][symbol] = 0
        stats["per_symbol"][symbol] += total_rows

        # Determine cutoff date for this symbol
        cutoff = transitions.get(symbol)
        if cutoff is None:
            print(f"  ⚠ No transition config for {symbol}, treating all as weekly")
            cutoff = date(2099, 12, 31)

        # Add year/month columns for partitioning
        df = df.with_columns([
            pl.col("date").dt.year().alias("year"),
            pl.col("date").dt.month().alias("month"),
        ])

        # Split into weekly and monthly partitions
        cutoff_date = date(cutoff.year, cutoff.month, cutoff.day)

        weekly_df = df.filter(pl.col("date") < pl.lit(cutoff_date))
        monthly_df = df.filter(pl.col("date") >= pl.lit(cutoff_date))

        # Write partitioned Parquet files
        for expiry_type, partition_df in [("weekly", weekly_df), ("monthly", monthly_df)]:
            if len(partition_df) == 0:
                continue

            # Group by year/month
            groups = partition_df.group_by(["year", "month"]).agg(pl.all().sort_by("date"))

            # Get unique year/month combinations
            year_months = partition_df.select(["year", "month"]).unique().sort(["year", "month"])

            for row in year_months.iter_rows(named=True):
                year = row["year"]
                month = row["month"]

                # Filter data for this year/month
                month_df = partition_df.filter(
                    (pl.col("year") == year) & (pl.col("month") == month)
                ).drop(["year", "month"])

                # Create output directory
                out_dir = output_path / symbol.lower() / expiry_type / str(year)
                out_dir.mkdir(parents=True, exist_ok=True)

                # Write Parquet file (Snappy compression is default)
                out_file = out_dir / f"{month:02d}.parquet"
                month_df.write_parquet(str(out_file))
                stats["files_created"] += 1

        print(f"    Rows: {total_rows:,}")

    return stats


def print_summary(stats: dict) -> None:
    """Print conversion summary."""
    print("\n" + "━" * 45)
    print(" Conversion Summary")
    print("━" * 45)
    print(f"\n  Total rows processed: {stats['total_rows']:,}")
    print(f"  Parquet files created: {stats['files_created']}")

    if stats["per_symbol"]:
        print("\n  Per symbol:")
        for symbol, count in sorted(stats["per_symbol"].items()):
            print(f"    {symbol}: {count:,} rows")

    print("\n" + "━" * 45)


def main():
    parser = argparse.ArgumentParser(
        description="Convert raw option chain CSVs to partitioned Parquet files."
    )
    parser.add_argument(
        "--input-dir",
        default="data/raw/",
        help="Path to directory containing raw CSV files (default: data/raw/)",
    )
    parser.add_argument(
        "--output-dir",
        default="data/parquet/",
        help="Path to output Parquet directory (default: data/parquet/)",
    )
    parser.add_argument(
        "--config",
        default="config/expiry_calendar.toml",
        help="Path to expiry calendar TOML config (default: config/expiry_calendar.toml)",
    )
    args = parser.parse_args()

    print("━" * 45)
    print(" QuantEdge CSV → Parquet Converter")
    print("━" * 45)
    print(f"\n  Input:  {args.input_dir}")
    print(f"  Output: {args.output_dir}")
    print(f"  Config: {args.config}")

    # Load transition dates
    try:
        transitions = load_transition_dates(args.config)
        print(f"\n  Transition dates loaded:")
        for symbol, cutoff in sorted(transitions.items()):
            print(f"    {symbol}: weekly → monthly on {cutoff}")
    except FileNotFoundError:
        print(f"\n  ✗ Config file not found: {args.config}")
        sys.exit(1)

    # Convert
    stats = convert_csv_to_parquet(args.input_dir, args.output_dir, transitions)

    # Summary
    print_summary(stats)

    if stats["files_created"] == 0:
        print("  ⚠ No files created. Check input directory and CSV format.")
        sys.exit(1)

    print("  ✓ Conversion complete!")


if __name__ == "__main__":
    main()
