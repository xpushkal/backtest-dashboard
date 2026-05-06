
> Personal-grade FNO options backtesting platform — ultra-fast, metric-complete, multi-leg, portfolio-aware.

Built on **Rust** (simulation kernel) + **Phoenix/Elixir** (orchestration + real-time UI).
Targets BankNifty, Nifty, and Sensex with 4+ years of 1-minute OHLCV, OI, and IV data.

---

## Why QuantEdge?

| Feature | AlgoTest / Stockmock | QuantEdge |
|---|---|---|
| Full 4yr single strategy | ~10–30 seconds | **< 1 second** |
| 1,000-combo optimizer | Not available | **< 3 minutes** |
| Greeks PnL attribution | No | **Yes (Δ Γ Θ V)** |
| Portfolio backtesting | No | **Yes** |
| Walk-forward / Monte Carlo | No | **Yes** |
| Metrics count | ~15–25 | **75+** |
| Per-leg trailing SL | Limited | **Full state machine** |
| Strategy DSL | GUI only | **TOML + GUI builder** |

---

## Table of Contents

- [Architecture](#architecture)
- [Repository Layout](#repository-layout)
- [Prerequisites](#prerequisites)
- [Getting Started](#getting-started)
- [Data Setup](#data-setup)
- [Running a Backtest](#running-a-backtest)
- [Strategy DSL Reference](#strategy-dsl-reference)
- [Metrics Reference](#metrics-reference)
- [Development Guide](#development-guide)
- [Performance Targets](#performance-targets)
- [Build Phases](#build-phases)
- [Contributing](#contributing)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Phoenix LiveView  (strategy builder · results · optimizer) │
└────────────────────────┬────────────────────────────────────┘
                         │ HTTP + WS (LiveView)
┌────────────────────────▼────────────────────────────────────┐
│  Phoenix / Elixir Application                               │
│  ┌──────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │  Oban    │  │  PubSub      │  │  Strategy DSL parser   │ │
│  │  jobs    │  │  progress    │  │  (TOML → validated     │ │
│  └────┬─────┘  └──────────────┘  │   StrategyConfig)      │ │
│       │                          └────────────────────────┘ │
│  ┌────▼──────────────────────────────────────────────────┐  │
│  │  Rustler NIF bridge  (async dirty CPU threads)        │  │
│  └────────────────────────┬──────────────────────────────┘  │
└───────────────────────────│────────────────────────────────-┘
                            │ NIF calls
┌───────────────────────────▼─────────────────────────────────┐
│  Rust Core (quantedge-core)                                 │
│  ┌────────────────┐  ┌────────────────┐  ┌───────────────┐  │
│  │  quantedge-    │  │  quantedge-    │  │  quantedge-   │  │
│  │  data          │  │  greeks        │  │  metrics      │  │
│  │  (Parquet mmap │  │  (BS / SIMD)   │  │  (75+ metrics │  │
│  │   IV interp)   │  │                │  │   Monte Carlo │  │
│  └────────────────┘  └────────────────┘  │   walk-fwd)   │  │
│                                          └───────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│  Storage                                                    │
│  Postgres (strategies, run metadata)                        │
│  DuckDB   (trade logs, equity curves, optimizer results)    │
│  Parquet  (bar data, partitioned by symbol/year/month)      │
└─────────────────────────────────────────────────────────────┘
```

---

## Repository Layout

```
quantedge/
├── apps/
│   ├── quantedge_core/          # Rust workspace
│   │   ├── crates/
│   │   │   ├── core/            # Simulation kernel, strategy runner
│   │   │   ├── data/            # Parquet reader, bar stream, IV surface
│   │   │   ├── greeks/          # Black-Scholes, binomial, SIMD pricers
│   │   │   ├── metrics/         # All 75+ metrics, Monte Carlo, walk-fwd
│   │   │   └── nif/             # Rustler NIF wrapper (calls into core)
│   │   └── Cargo.toml
│   │
│   └── quantedge_web/           # Phoenix / Elixir umbrella app
│       ├── lib/
│       │   ├── quantedge/
│       │   │   ├── strategies/  # Strategy CRUD, DSL parser
│       │   │   ├── runs/        # Run management, result storage
│       │   │   ├── optimizer/   # Param grid, sweep orchestration
│       │   │   ├── portfolio/   # Multi-strategy portfolio engine
│       │   │   └── workers/     # Oban workers (backtest, optimizer, portfolio)
│       │   └── quantedge_web/
│       │       ├── live/        # LiveView modules (builder, results, optimizer)
│       │       └── components/  # Shared UI components
│       ├── priv/
│       │   └── repo/migrations/ # Postgres migrations
│       └── mix.exs
│
├── data/
│   ├── raw/                     # Original CSVs (gitignored)
│   │   ├── banknifty/
│   │   ├── nifty/
│   │   └── sensex/
│   └── parquet/                 # Converted Parquet files (gitignored)
│       ├── banknifty/{year}/{month:02}.parquet
│       ├── nifty/
│       └── sensex/
│
├── config/
│   ├── strategies/              # Example strategy TOML files
│   │   ├── short_straddle.toml
│   │   ├── short_strangle.toml
│   │   └── iron_condor.toml
│   └── lot_sizes.toml           # NSE lot size lookup by symbol + date range
│
├── scripts/
│   ├── csv_to_parquet.py        # One-time data conversion
│   └── validate_data.py         # Data integrity checks
│
├── docs/
│   ├── PRD.md                   # Full product requirements
│   ├── METRICS.md               # All 75 metrics with formulas
│   └── DSL.md                   # Strategy TOML schema reference
│
└── README.md
```

---

## Prerequisites

### System

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.78+ (stable) | Install via [rustup](https://rustup.rs) |
| Elixir | 1.16+ | Requires OTP 26+ |
| Erlang/OTP | 26+ | Usually installed with Elixir |
| Node.js | 18+ | For Phoenix assets |
| Postgres | 16+ | |
| DuckDB | 0.10+ | Embedded - no separate server needed |
| Python | 3.11+ | For data conversion scripts only |

### Python packages (data conversion only)

```bash
pip install polars pyarrow pandas
```

---

## Getting Started

### 1. Clone and set up

```bash
git clone https://github.com/yourname/quantedge.git
cd quantedge
```

### 2. Install Elixir dependencies

```bash
cd apps/quantedge_web
mix deps.get
mix assets.setup
```

### 3. Build the Rust core

```bash
cd apps/quantedge_core
cargo build --release
```

Rustler will automatically compile and link the NIF when you run `mix compile` from the Phoenix app.

### 4. Set up databases

```bash
# Create and migrate Postgres
cd apps/quantedge_web
mix ecto.create
mix ecto.migrate

# DuckDB database is created automatically on first run at priv/quantedge.duckdb
```

### 5. Configure environment

```bash
cp apps/quantedge_web/.env.example apps/quantedge_web/.env
```

Edit `.env`:

```env
DATABASE_URL=ecto://postgres:postgres@localhost/quantedge_dev
SECRET_KEY_BASE=<generate with: mix phx.gen.secret>
PHX_HOST=localhost
PORT=4000
DATA_DIR=/absolute/path/to/quantedge/data/parquet
```

### 6. Start the server

```bash
cd apps/quantedge_web
mix phx.server
```

Open [http://localhost:4000](http://localhost:4000).

---

## Data Setup

### Step 1: Place your CSV files

```
data/raw/
├── banknifty/
│   ├── 2021.csv
│   ├── 2022.csv
│   └── ...
├── nifty/
└── sensex/
```

Your CSV must follow this schema:

```
timestamp,date,time,weekday,option_type,strike_label,strike_offset,
moneyness,open,high,low,close,volume,strike,oi,spot,iv
```

### Step 2: Convert to Parquet

```bash
python scripts/csv_to_parquet.py \
  --input data/raw/banknifty/ \
  --output data/parquet/banknifty/ \
  --symbol BANKNIFTY

# Repeat for nifty and sensex
```

This partitions data by year/month and validates the schema. Expect ~30–60 seconds per symbol for 4 years of data.

### Step 3: Validate

```bash
python scripts/validate_data.py --data-dir data/parquet/
```

Output shows: bar counts per symbol/year, date gaps, IV coverage, OI completeness.

### Step 4: Verify in UI

Navigate to `/data` in the web UI to see an interactive data explorer confirming all symbols are loaded.

---

## Running a Backtest

### Via the UI

1. Go to `/strategies/new`
2. Build your strategy using the leg editor (see [Strategy DSL Reference](#strategy-dsl-reference) for all options)
3. Click **Run** — configure date range, capital, and cost settings
4. Watch real-time progress, then explore the full results at `/runs/:id`

### Via the API

```bash
curl -X POST http://localhost:4000/api/runs \
  -H "Content-Type: application/json" \
  -d '{
    "strategy_id": "uuid-here",
    "date_from": "2021-01-01",
    "date_to": "2024-12-31",
    "capital": 500000,
    "brokerage_per_lot": 40
  }'
```

### Via Elixir directly (IEx)

```elixir
iex -S mix phx.server

# Run a strategy inline
strategy = QuantEdge.Strategies.get_strategy!("uuid")
{:ok, result} = QuantEdge.Runs.run_backtest(strategy, %{
  date_from: ~D[2021-01-01],
  date_to: ~D[2024-12-31],
  capital: 500_000
})

IO.inspect(result.metrics["sharpe_ratio"])
```

---

## Strategy DSL Reference

Strategies are defined in TOML. The UI generates this automatically, but you can also write and import them directly.

### Minimal example — ATM short straddle

```toml
[strategy]
name = "ATM Short Straddle - Weekly"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
expiry_filter = "weekly"
capital = 500000
brokerage_per_lot = 40.0
slippage_model = "fixed_pts"
slippage_value = 1.0

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
expiry = "weekly"
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0

[[legs]]
option_type = "PE"
position = "sell"
lots = 1
expiry = "weekly"
strike_mode = "atm_offset"
strike_offset = 0
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 100.0

[overall]
overall_sl_enabled = true
overall_sl_type = "percent_of_premium"
overall_sl_value = 60.0
overall_target_enabled = true
overall_target_type = "percent_of_premium"
overall_target_value = 50.0
```

### Advanced example — short strangle with trailing SL and re-entry

```toml
[strategy]
name = "Short Strangle - Trail + Reentry"
underlying = "BANKNIFTY"
entry_time = "09:20"
exit_time = "15:20"
expiry_filter = "weekly"
capital = 500000

[[legs]]
option_type = "CE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = 2          # ATM+2 (one strike OTM)
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 80.0
trail_sl_enabled = true
trail_sl_activate_at = 40.0  # Activate when 40% profit
trail_sl_lock_in = 30.0      # Lock in 30% from peak
trail_sl_type = "percent"
reentry_on_sl = true
reentry_mode = "after_n_bars"
reentry_cooldown_bars = 5
reentry_max_attempts = 2
momentum_filter_enabled = true
momentum_type = "range_breakout"
range_breakout_time = "09:45"
range_breakout_side = "high"

[[legs]]
option_type = "PE"
position = "sell"
lots = 1
strike_mode = "atm_offset"
strike_offset = -2         # ATM-2 (one strike OTM)
stop_loss_enabled = true
stop_loss_type = "percent_of_premium"
stop_loss_value = 80.0
trail_sl_enabled = true
trail_sl_activate_at = 40.0
trail_sl_lock_in = 30.0
trail_sl_type = "percent"
reentry_on_sl = true
reentry_mode = "after_n_bars"
reentry_cooldown_bars = 5
reentry_max_attempts = 2

[overall]
overall_sl_enabled = true
overall_sl_type = "max_loss"
overall_sl_value = 10000.0  # INR hard cap
```

### Strike mode options

| Mode | Field | Example |
|---|---|---|
| `atm_offset` | `strike_offset` (int) | 0 = ATM, 2 = ATM+2, -5 = ATM-5 |
| `delta` | `delta_target` (float) | 0.20 = sell 20-delta strike |
| `premium` | `premium_target` (float) | 200.0 = strike nearest ₹200 premium |
| `percent_otm` | `percent_otm_value` (float) | 2.0 = 2% OTM from spot |

### Stop loss types

| Type | Trigger |
|---|---|
| `points` | Option price moves X points against position |
| `percent_of_premium` | PnL exceeds X% of entry premium |
| `percent_of_margin` | MTM loss exceeds X% of SPAN margin at entry |
| `index_points` | Underlying spot moves X points against delta |
| `delta_breach` | Leg delta crosses configured threshold |
| `combined_premium` | Sum of all leg PnLs crosses threshold |

### Re-entry modes

| Mode | Behaviour |
|---|---|
| `reasap` | Re-enter at open of next available bar |
| `same_time` | Re-enter at the same time on the next trading day |
| `after_n_bars` | Wait `reentry_cooldown_bars` before re-entering |
| `momentum_confirm` | Re-enter only when momentum filter confirms |

---

## Metrics Reference

All 75+ metrics are grouped into six categories. Full formulas are in `docs/METRICS.md`.

### Return metrics
`total_pnl_gross` · `total_pnl_net` · `cagr` · `roi_pct` · `expectancy` · `profit_factor` · `win_rate_pct` · `avg_win` · `avg_loss` · `win_loss_ratio` · `largest_win` · `largest_loss` · `gross_profit` · `gross_loss`

### Risk metrics
`max_drawdown_inr` · `max_drawdown_pct` · `avg_drawdown` · `sharpe_ratio` · `sortino_ratio` · `calmar_ratio` · `omega_ratio` · `var_95` · `var_99` · `cvar` · `ulcer_index` · `daily_volatility` · `ann_volatility` · `skewness` · `kurtosis` · `recovery_factor` · `drawdown_duration_bars`

### Trade analytics
`total_trades` · `avg_hold_bars` · `max_hold_bars` · `max_consec_wins` · `max_consec_losses` · `sl_hit_rate_pct` · `target_hit_rate_pct` · `time_exit_rate_pct` · `reentry_count` · `reentry_win_rate` · `total_brokerage` · `total_slippage` · `total_stt` · `net_cost_ratio`

### Options-specific
`premium_capture_pct` · `total_theta_collected` · `avg_theta_per_day` · `avg_iv_at_entry` · `avg_iv_at_exit` · `iv_crush_pct` · `delta_pnl` · `gamma_pnl` · `theta_pnl` · `vega_pnl` · `avg_net_delta` · `dte_distribution` · `breakeven_range` · `max_profit_theoretical` · `max_loss_theoretical`

### Portfolio metrics
`strategy_correlation_matrix` · `portfolio_sharpe` · `peak_margin_used` · `capital_efficiency` · `net_portfolio_greeks` · `avg_concurrent_trades` · `diversification_benefit`

### Time-based analytics
`monthly_pnl_heatmap` · `day_of_week_pnl` · `expiry_day_performance` · `best_month` · `worst_month` · `pct_profitable_months` · `pct_profitable_weeks` · `walk_forward_results` · `monte_carlo_bands` · `rolling_sharpe_12m` · `equity_curve` · `drawdown_curve` · `iv_regime_performance` · `market_regime_performance`

---

## Development Guide

### Running tests

```bash
# Rust unit tests
cd apps/quantedge_core
cargo test

# Elixir tests
cd apps/quantedge_web
mix test

# Integration tests (requires running Postgres + data loaded)
mix test --tag integration
```

### Benchmarks

```bash
cd apps/quantedge_core
cargo bench

# Key benchmarks:
# simulation_4yr_banknifty   — single strategy full run
# greeks_batch_1000          — Greeks calculation throughput
# metrics_full_suite         — All metrics from 10k trades
```

### Running the optimizer locally

```bash
# From IEx
QuantEdge.Optimizer.run_sweep("strategy-uuid", %{
  strike_offset_ce: [0, 1, 2, 3, 4, 5],
  strike_offset_pe: [0, -1, -2, -3, -4, -5],
  sl_value: [50.0, 60.0, 70.0, 80.0, 100.0],
  target_value: [30.0, 40.0, 50.0]
})
# 6 × 6 × 5 × 4 = 720 combinations, runs in < 2 minutes
```

### Code conventions

- Rust: `cargo fmt` + `cargo clippy` before every commit. No unsafe except in the NIF layer.
- Elixir: `mix format`. Contexts over raw Ecto queries everywhere.
- All simulation logic lives in Rust. Elixir contains zero simulation math.
- All database writes go through Elixir. Rust returns plain data structures to the NIF caller.

---

## Performance Targets

| Scenario | Target |
|---|---|
| Single strategy, 4-year backtest | **< 1 second** |
| 10-strategy portfolio backtest | **< 5 seconds** |
| 1,000-combo optimizer sweep | **< 3 minutes** |
| Walk-forward (12 windows) | **< 20 seconds** |
| Monte Carlo (1,000 simulations) | **< 10 seconds** |
| Data load, 4yr 1 symbol (Parquet) | **< 100 ms** |
| LiveView progress update latency | **< 500 ms** |

### How we hit these numbers

- **Zero-copy Parquet reads** via Arrow2 memory-mapped files — no deserialization on the hot path
- **Single-threaded inner loop** per strategy — maximizes L1/L2 cache hit rate on bar data
- **Rayon parallelism at strategy level** — optimizer sweep spawns N strategies across all CPU cores
- **Greeks computed only when needed** — skipped entirely unless delta-SL mode or attribution report is active
- **Rustler dirty CPU NIFs** — simulation runs on Erlang dirty scheduler, never blocks the BEAM
- **DuckDB for analytics queries** — columnar engine is 10–50× faster than Postgres for time-series aggregations

---

## Build Phases

| Phase | Scope | Est. Duration |
|---|---|---|
| 1 — Data foundation | CSV → Parquet converter, IV interpolator, data explorer UI | 2 weeks |
| 2 — Single-leg engine | Basic CE/PE sim, ATM strike, fixed SL/target, 20 metrics | 2 weeks |
| 3 — Multi-leg + advanced SL | Multi-leg DSL, combined SL, trailing SL state machine, OCO | 3 weeks |
| 4 — Re-entry + momentum | Re-entry state machine (all modes), RSI/EMA/range-breakout filters | 2 weeks |
| 5 — Full metrics suite | All 75 metrics, Greeks attribution, walk-forward, Monte Carlo | 2 weeks |
| 6 — Phoenix + NIF bridge | Rustler NIFs, Oban jobs, Postgres + DuckDB schema, PubSub | 2 weeks |
| 7 — LiveView UI | Strategy builder, run manager, full results viewer | 3 weeks |
| 8 — Portfolio engine | Multi-strategy runner, margin model, correlation matrix | 2 weeks |
| 9 — Optimizer + hardening | Param sweep UI, heatmap, perf profiling, edge case testing | 2 weeks |

---

## Key Dependencies

### Rust

| Crate | Purpose |
|---|---|
| `polars` | Columnar dataframe, Parquet I/O |
| `arrow2` | Zero-copy Arrow memory mapped reads |
| `rayon` | Data parallelism (work-stealing thread pool) |
| `rustler` | Elixir NIF bridge |
| `serde` + `serde_json` | Strategy config deserialization |
| `chrono` | Date/time handling |
| `ndarray` | Matrix ops for Greeks batching |
| `statrs` | Statistical distributions (Monte Carlo) |

### Elixir / Phoenix

| Package | Purpose |
|---|---|
| `phoenix` + `phoenix_live_view` | Web framework + real-time UI |
| `oban` | Background job queue (Postgres-backed) |
| `rustler` | NIF compilation + loading |
| `ecto` + `ecto_sql` | Postgres ORM |
| `duckdbex` | DuckDB Elixir bindings |
| `jason` | JSON encoding/decoding |
| `nimble_toml` | TOML parsing for strategy DSL |

---

## License

Private — personal use only. Not for redistribution.

---

*Built with Rust + Phoenix. Data: NSE FNO 1-minute bars. Instruments: BankNifty, Nifty, Sensex.*