#!/bin/bash
# Performance regression check
# Usage: ./scripts/perf_check.sh

set -e

echo "🔍 Running performance benchmarks..."
cd "$(dirname "$0")/.."

cargo bench -p quantedge-core 2>&1 | tee /tmp/quantedge_bench_output.txt

echo ""
echo "✅ Benchmarks complete. Check results above against baseline thresholds:"
echo ""
echo "  | Benchmark                | Target    |"
echo "  |--------------------------|-----------|"
echo "  | 4yr_single_leg_atm_ce    | < 1000ms  |"
echo "  | 100_day_single_leg       | < 50ms    |"
echo "  | 4yr_straddle_2leg        | < 1500ms  |"
echo "  | 4yr_iron_condor_4leg     | < 2500ms  |"
echo "  | metrics_1000_trades      | < 50ms    |"
echo ""
echo "Review /tmp/quantedge_bench_output.txt for detailed timing."
