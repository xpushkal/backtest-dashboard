# ⚡ VelocityBT — High-Performance F&O Backtesting Engine

> A personal, ultra-fast options backtesting platform for Indian derivatives (BankNifty, Nifty, Sensex) — built with Rust + Phoenix (Elixir). Designed to be more advanced than AlgoTest, Stockmock, and Stoxxo combined.

---

## 📌 Table of Contents

- [Overview](#overview)
- [Tech Stack](#tech-stack)
- [Architecture](#architecture)
- [Data Layer](#data-layer)
- [Strategy DSL](#strategy-dsl)
- [Features](#features)
  - [Strike Selection Engine](#strike-selection-engine)
  - [SL / Exit Types](#sl--exit-types)
  - [Re-entry Modes](#re-entry-modes)
  - [Trailing Stop Logic](#trailing-stop-logic)
  - [Portfolio Engine](#portfolio-engine)
- [Metrics Suite (75 Metrics)](#metrics-suite-75-metrics)
- [Performance Targets](#performance-targets)
- [Build Order](#build-order)
- [Project Structure](#project-structure)
- [Getting Started](#getting-started)
- [Roadmap](#roadmap)

---

## Overview

VelocityBT is a personal-use, high-performance backtesting platform for Indian F&O derivatives. It supports:

- Multi-leg options strategies (straddle, strangle, iron condor, etc.)
- 4 years of 1-minute resolution BankNifty, Nifty, and Sensex data
- 75+ metrics across 6 categories
- Parameter optimizer sweeps (1,000+ combinations in < 2 minutes)
- Real-time streaming results via Phoenix LiveView
- Portfolio-level backtesting with SPAN margin approximation

---

## Tech Stack

| Layer | Technology | Purpose |
|---|---|---|
| Core Engine | **Rust** | Tick replay, Greeks, PnL, SL logic, simulation loop |
| Parallelism | **Rayon** | Parallel parameter sweeps |
| NIF Bridge | **Rustler** | Zero-overhead Phoenix ↔ Rust calls |
| Orchestration | **Phoenix / Elixir** | Strategy DSL, job queues, real-time UI |
| UI | **Phoenix LiveView** | Real-time streaming results dashboard |
| Job Queue | **Oban** | Async Rust NIF dispatch |
| Primary DB | **PostgreSQL** | Strategy definitions, run metadata, user config |
| Result Store | **DuckDB** | Raw trade logs, time-series data (50–100M rows) |
| Data Format | **Parquet** | Columnar, memory-mapped, partitioned by symbol/date |

### Key Rust Crates

```toml
polars     = "0.39"   # Columnar data, lazy eval
rayon      = "1.10"   # Parallel param sweeps
rustler    = "0.32"   # Phoenix NIF bridge
ndarray    = "0.15"   # Matrix ops for Greeks
serde      = { features = ["derive"] }
chrono     = "0.4"
arrow2     = "0.18"   # Zero-copy memory-mapped Parquet reads
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Phoenix LiveView (UI)                     │
│         Strategy Builder → Results Dashboard → Compare      │
└────────────────────────┬────────────────────────────────────┘
                         │ PubSub (real-time streaming)
┌────────────────────────▼────────────────────────────────────┐
│                  Phoenix / Elixir Layer                      │
│   GenServers (strategy processes) │ Oban (job queue)        │
│   Portfolio Aggregator            │ DuckDB / Postgres        │
└────────────────────────┬────────────────────────────────────┘
                         │ Rustler NIF (zero serialization)
┌────────────────────────▼────────────────────────────────────┐
│                    Rust Core Engine                          │
│   Tick Replay │ Greeks │ SL State Machine │ Metrics         │
│   Strike Selector │ IV Interpolator │ Rayon Parallelism      │
└────────────────────────┬────────────────────────────────────┘
                         │ Memory-mapped reads
┌────────────────────────▼────────────────────────────────────┐
│                     Parquet Data Store                       │
│   Partitioned by (symbol, year, month)                      │
│   BankNifty │ Nifty │ Sensex │ 4 years │ 1-min resolution   │
└─────────────────────────────────────────────────────────────┘
```

### Phoenix LiveView Flow

```
User defines strategy (visual builder)
           ↓
Elixir serializes → TOML → stores in Postgres
           ↓
Oban job dispatches → Rust NIF (async dirty thread)
           ↓
Rust emits progress events → Phoenix PubSub → LiveView
           ↓
Results stream to browser: equity curve, trade log, stats
           ↓
Compare view: overlay multiple run results
```

---

## Data Layer

### Source Data
- **Format**: 1-minute OHLCV + IV CSV files
- **Symbols**: BankNifty, Nifty, Sensex
- **Duration**: 4 years (~10–15 million rows total)

### Internal Format
- **Parquet**, partitioned by `(symbol, year, month)`
- Memory-mapped reads via `polars` / `arrow2` in Rust
- Full 4-year load time: **< 100ms**

> ⚠️ **Important**: Do NOT use raw CSV at runtime. CSV I/O adds 2–5 seconds per backtest run, which compounds badly during optimizer sweeps. Convert to Parquet immediately.

### IV Surface
- Per-row IV from source data is used to build an **IV surface interpolator**
- Method: Cubic spline over `(strike_offset, time_to_expiry)`
- Ensures accurate mid-bar Greeks at all times

---

## Strategy DSL

Strategies are defined in **TOML** and stored in Postgres.

```toml
[strategy]
name = "Short Strangle on Expiry Day"
underlying = "BANKNIFTY"
entry_time = "09:20"
expiry_filter = "weekly"

[[legs]]
type = "CE"
moneyness = "ATM+2"
action = "sell"
lots = 1

[[legs]]
type = "PE"
moneyness = "ATM-2"
action = "sell"
lots = 1

[stop_loss]
type = "combined_premium"   # per_leg | index_points | delta
value = 50.0                # % of combined premium received

[trailing_sl]
activate_at = 40            # Activate when 40% profit reached
lock_in = 30                # Lock in 30% profit from peak

[target]
type = "percent_of_premium"
value = 60.0

[re_entry]
enabled = true
max_attempts = 2
cooldown_bars = 5
filter = "momentum"         # momentum | none | range_breakout

[exit]
time = "15:20"
expiry_day_early_exit = true
```

---

## Features

### Strike Selection Engine

A dedicated Rust module supporting 4 strike selection modes:

| Mode | Description |
|---|---|
| **ATM ± N offset** | ATM, ATM+1, ATM+2, ITM-3, etc. |
| **Delta-based** | Sell the 0.20 delta strike |
| **Premium-based** | Select strike nearest to ₹X premium |
| **Percentage OTM** | X% out of the money from spot |

---

### SL / Exit Types

All implemented as **Rust enum variants**, resolved per-bar in the simulation loop:

| Type | Description |
|---|---|
| `PerLegSL` | Absolute points / % of premium / % of spot |
| `CombinedPremiumSL` | Total debit/credit crosses threshold |
| `IndexBasedSL` | Underlying moves X points against position |
| `DeltaSL` | Net position delta exceeds threshold |
| `TrailingSL` | Activate at profit %, trail from peak high-water mark |
| `TimedExit` | Hard exit at time, or N bars before expiry |
| `OCOBracket` | First of SL or target fires, cancels the other |
| `MomentumFilter` | RSI / EMA cross gating entries and re-entries |

---

### Re-entry Modes

| Mode | Behavior |
|---|---|
| **RE ASAP** | Re-enter on the next bar immediately |
| **RE at same time** | Re-enter at same time next session |
| **RE after N bars** | Cooldown period before re-entry |
| **RE on momentum** | RSI/EMA confirmation required |

---

### Trailing Stop Logic

The trailing SL is a **stateful component** tracked per-leg or combined:

1. Position enters profit zone
2. At `activate_at` % profit → trailing begins
3. High-water mark is tracked continuously
4. SL trails by `lock_in` % from peak
5. If price retraces past the trail → position exits

---

### Portfolio Engine

Managed by **Elixir GenServer processes** (one per strategy), with a central aggregator:

- Runs multiple strategies concurrently as supervised processes
- Computes combined margin using **SPAN approximation** (NSE SPAN model)
- Tracks **net Greeks** across all open positions in real time
- Enforces **capital allocation** limits per strategy
- Generates **correlation matrix** of strategy returns
- Correctly handles overlapping positions on the same underlying

---

## Metrics Suite (75 Metrics)

### Returns
Total PnL, CAGR, Win Rate, Profit Factor, Expectancy, Avg Win, Avg Loss, Largest Win, Largest Loss, Consecutive Wins, Consecutive Losses, Avg Trade Duration, Total Trades, Winning Trades, Losing Trades

### Risk
Max Drawdown (Absolute), Max Drawdown (%), Sharpe Ratio, Sortino Ratio, Calmar Ratio, VaR 95%, VaR 99%, CVaR (Expected Shortfall), Ulcer Index, Recovery Factor, Max Runup, Payoff Ratio, Risk/Reward Ratio

### Options-Specific
Theta Collected per Day, Avg IV at Entry, Avg IV at Exit, IV Crush Capture Rate, Delta PnL Attribution, Gamma PnL Attribution, Theta PnL Attribution, Vega PnL Attribution, Premium Capture %, Avg DTE at Entry, Avg Greeks at Entry/Exit

### Time Analysis
Monthly PnL Heatmap, Weekly PnL Heatmap, Day-of-Week Performance, DTE Bucket Performance (0–7, 7–15, 15–30 days), Time-of-Day Entry Comparison, Expiry Day vs Non-Expiry Performance

### Portfolio
Combined Margin Utilization, Net Delta (portfolio), Net Theta (portfolio), Net Vega (portfolio), Strategy Correlation Matrix, Capital Utilization %, Margin-Adjusted Returns

### Trade-Level
Entry Price, Exit Price, Slippage Estimate, Per-Trade Greeks at Entry/Exit, MTM per Bar, Leg-wise PnL Breakdown, Trade Tag / Strategy Label

---

## Performance Targets

| Task | Target |
|---|---|
| Single strategy, 4-year backtest | **< 1 second** |
| 1,000-parameter optimizer sweep | **< 2 minutes** |
| Portfolio backtest (5 strategies) | **< 5 seconds** |
| Walk-forward validation (12 windows) | **< 30 seconds** |

*Benchmarked on a modern 8-core machine with Rayon parallelism.*

---

## Build Order

> Follow this sequence — each stage builds on the previous.

```
Phase 1 — Data Foundation
  ├── CSV → Parquet converter (partitioned by symbol/date)
  ├── IV surface interpolator (cubic spline)
  └── Schema validation & data integrity checks

Phase 2 — Single-Leg Simulator
  ├── ATM CE/PE selection
  ├── Fixed SL + target logic
  └── Basic metrics (PnL, win rate, drawdown)

Phase 3 — Multi-Leg Engine
  ├── Straddle / strangle / iron condor
  ├── Combined SL state machine
  └── OCO bracket orders

Phase 4 — Advanced SL & Re-entry
  ├── Trailing SL with high-water mark tracking
  ├── All 4 re-entry modes
  └── Momentum / range breakout filters

Phase 5 — Phoenix / Rustler Bridge
  ├── NIF wrapping of Rust engine
  ├── Oban async job queue
  └── PubSub progress streaming

Phase 6 — Portfolio Engine
  ├── Multi-strategy GenServer supervision
  ├── SPAN margin approximation
  └── Correlation matrix + net Greeks

Phase 7 — LiveView Frontend
  ├── Visual strategy builder
  ├── Real-time results dashboard
  └── Multi-run compare view

Phase 8 — Optimizer
  ├── Parameter sweep UI
  ├── Heatmap visualization
  └── Walk-forward validation
```

---

## Project Structure

```
velocitybt/
├── rust_engine/                   # Rust core
│   ├── src/
│   │   ├── data/
│   │   │   ├── loader.rs          # Parquet loader, memory-mapped reads
│   │   │   ├── iv_surface.rs      # Cubic spline IV interpolator
│   │   │   └── schema.rs          # Internal data types
│   │   ├── strategy/
│   │   │   ├── dsl.rs             # TOML strategy deserializer
│   │   │   ├── strike_selector.rs # 4 strike selection modes
│   │   │   ├── legs.rs            # Leg definitions
│   │   │   └── re_entry.rs        # Re-entry state machines
│   │   ├── simulation/
│   │   │   ├── engine.rs          # Main tick-replay loop
│   │   │   ├── sl_machine.rs      # All SL/exit type handlers
│   │   │   ├── trailing.rs        # Trailing SL high-water mark
│   │   │   └── portfolio.rs       # Multi-strategy aggregation
│   │   ├── greeks/
│   │   │   ├── black_scholes.rs   # BS pricing + Greeks
│   │   │   └── attribution.rs     # Delta/Gamma/Theta/Vega PnL
│   │   ├── metrics/
│   │   │   ├── returns.rs         # Return metrics
│   │   │   ├── risk.rs            # Risk metrics (Sharpe, VaR, etc.)
│   │   │   ├── options.rs         # Options-specific metrics
│   │   │   └── time_analysis.rs   # Heatmaps, DTE buckets
│   │   └── nif/
│   │       └── bridge.rs          # Rustler NIF exports
│   └── Cargo.toml
│
├── phoenix_app/                   # Elixir/Phoenix layer
│   ├── lib/
│   │   ├── velocitybt/
│   │   │   ├── strategies/        # Strategy CRUD, TOML serialization
│   │   │   ├── backtest/
│   │   │   │   ├── runner.ex      # Oban job → Rust NIF dispatch
│   │   │   │   └── portfolio.ex   # Portfolio GenServer supervisor
│   │   │   └── results/
│   │   │       ├── store.ex       # DuckDB result persistence
│   │   │       └── analytics.ex   # Query helpers for metrics
│   │   └── velocitybt_web/
│   │       ├── live/
│   │       │   ├── strategy_builder_live.ex
│   │       │   ├── backtest_live.ex
│   │       │   └── compare_live.ex
│   │       └── components/
│   │           ├── equity_curve.ex
│   │           ├── trade_log.ex
│   │           └── metrics_panel.ex
│   ├── priv/
│   │   └── repo/migrations/
│   └── mix.exs
│
├── data/
│   ├── raw/                       # Original CSV files (do not modify)
│   │   ├── banknifty/
│   │   ├── nifty/
│   │   └── sensex/
│   └── parquet/                   # Converted Parquet files
│       ├── banknifty/
│       ├── nifty/
│       └── sensex/
│
├── scripts/
│   ├── csv_to_parquet.py          # Data conversion script
│   └── validate_data.py           # Data integrity checker
│
└── README.md
```

---

## Getting Started

### Prerequisites

- **Rust** >= 1.75 (`rustup install stable`)
- **Elixir** >= 1.16 + **Erlang/OTP** >= 26
- **Phoenix** >= 1.7
- **PostgreSQL** >= 15
- **DuckDB** >= 0.10
- **Python** >= 3.10 (for data conversion scripts)

### 1. Convert Data to Parquet

```bash
cd scripts
pip install polars pyarrow
python csv_to_parquet.py --input ../data/raw --output ../data/parquet
```

### 2. Build the Rust Engine

```bash
cd rust_engine
cargo build --release
```

### 3. Set Up Phoenix App

```bash
cd phoenix_app
mix deps.get
mix ecto.setup
mix phx.server
```

### 4. Open the UI

```
http://localhost:4000
```

---

## Roadmap

- [x] Architecture design
- [x] Strategy DSL spec
- [x] Metrics suite definition
- [ ] Parquet data pipeline
- [ ] Single-leg simulator (Rust)
- [ ] Multi-leg engine (Rust)
- [ ] Trailing SL + re-entry state machines
- [ ] Rustler NIF bridge
- [ ] Phoenix Oban job queue
- [ ] Portfolio engine (Elixir)
- [ ] LiveView strategy builder UI
- [ ] LiveView results dashboard
- [ ] Parameter optimizer + heatmap
- [ ] Walk-forward validation
- [ ] DuckDB result analytics

---

## Notes

- This is a **personal-use project** — not intended for commercial deployment
- SPAN margin calculations are approximations; not certified NSE figures
- Greeks are computed using Black-Scholes; model risk applies for deep ITM/OTM options
- Always validate backtest results against a known benchmark before trading

---

*Built with Rust 🦀 + Elixir 💧 for maximum performance and reliability.*