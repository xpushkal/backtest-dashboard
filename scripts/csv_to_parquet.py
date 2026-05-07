#!/usr/bin/env python3
"""Convert QuantEdge CSV data files to partitioned Parquet.

Structure: Data/parquet/{symbol_lowercase}/{expiry_type}/{year}/{MM}.parquet
"""

import csv
import os
import sys
from collections import defaultdict
from datetime import datetime

import pyarrow as pa
import pyarrow.parquet as pq

# CSV columns
COLUMNS = [
    "timestamp", "date", "time", "weekday", "option_type",
    "strike_label", "strike_offset", "moneyness", "open", "high",
    "low", "close", "volume", "strike", "oi", "spot", "iv"
]

FLOAT_COLS = {"open", "high", "low", "close", "volume", "strike", "oi", "spot", "iv"}
INT_COLS = {"strike_offset"}

FILES = {
    "NIFTY": "Data/NIFTY 4 Years.csv",
    "SENSEX": "Data/SENSEX 4 Years.csv",
}

EXPIRY_TYPE = "weekly"


def parse_row(row):
    """Parse a CSV row into typed dict."""
    out = {}
    for col in COLUMNS:
        val = row.get(col, "")
        if col in FLOAT_COLS:
            try:
                out[col] = float(val) if val else 0.0
            except ValueError:
                out[col] = 0.0
        elif col in INT_COLS:
            try:
                out[col] = int(val) if val else 0
            except ValueError:
                out[col] = 0
        else:
            out[col] = val
    return out


def convert_symbol(symbol, csv_path, base_dir):
    """Convert one CSV file to partitioned Parquet."""
    print(f"  Reading {csv_path}...")

    # Group rows by (year, month)
    buckets = defaultdict(list)

    with open(csv_path, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        row_count = 0
        for row in reader:
            parsed = parse_row(row)
            date_str = parsed["date"]
            try:
                dt = datetime.strptime(date_str, "%Y-%m-%d")
            except ValueError:
                continue
            buckets[(dt.year, dt.month)].append(parsed)
            row_count += 1
            if row_count % 500000 == 0:
                print(f"    ...{row_count:,} rows")

    print(f"  Total rows: {row_count:,} across {len(buckets)} month partitions")

    # Write each month as a Parquet file
    symbol_lower = symbol.lower()
    for (year, month), rows in sorted(buckets.items()):
        out_dir = os.path.join(base_dir, symbol_lower, EXPIRY_TYPE, str(year))
        os.makedirs(out_dir, exist_ok=True)
        out_path = os.path.join(out_dir, f"{month:02d}.parquet")

        # Build Arrow table
        arrays = {
            "date": pa.array([datetime.strptime(r["date"], "%Y-%m-%d").date() for r in rows], type=pa.date32()),
            "time": pa.array([r["time"] for r in rows], type=pa.utf8()),
            "weekday": pa.array([r["weekday"] for r in rows], type=pa.utf8()),
            "option_type": pa.array([r["option_type"] for r in rows], type=pa.utf8()),
            "strike_label": pa.array([r["strike_label"] for r in rows], type=pa.utf8()),
            "strike_offset": pa.array([r["strike_offset"] for r in rows], type=pa.int32()),
            "moneyness": pa.array([r["moneyness"] for r in rows], type=pa.utf8()),
            "open": pa.array([r["open"] for r in rows], type=pa.float64()),
            "high": pa.array([r["high"] for r in rows], type=pa.float64()),
            "low": pa.array([r["low"] for r in rows], type=pa.float64()),
            "close": pa.array([r["close"] for r in rows], type=pa.float64()),
            "volume": pa.array([r["volume"] for r in rows], type=pa.float64()),
            "strike": pa.array([r["strike"] for r in rows], type=pa.float64()),
            "oi": pa.array([r["oi"] for r in rows], type=pa.float64()),
            "spot": pa.array([r["spot"] for r in rows], type=pa.float64()),
            "iv": pa.array([r["iv"] for r in rows], type=pa.float64()),
        }

        table = pa.table(arrays)
        pq.write_table(table, out_path, compression="snappy")
        print(f"    ✓ {out_path} ({len(rows):,} rows)")


def main():
    base_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), "Data", "parquet")
    os.makedirs(base_dir, exist_ok=True)

    for symbol, csv_path in FILES.items():
        full_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), csv_path)
        if not os.path.exists(full_path):
            print(f"  ⚠ Skipping {symbol}: {full_path} not found")
            continue
        print(f"\n{'='*60}")
        print(f"  Converting {symbol}")
        print(f"{'='*60}")
        convert_symbol(symbol, full_path, base_dir)

    print(f"\n✅ Done! Parquet files in: {base_dir}")


if __name__ == "__main__":
    main()
