# Architecture Research

**Domain:** FNO Options Backtesting Platform (Indian Markets)
**Researched:** 2026-04-19
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────┐
│  Presentation Layer (Phoenix LiveView)                       │
│  Strategy Builder · Results Viewer · Optimizer · Data Explorer│
└────────────────────────┬────────────────────────────────────┘
                         │ HTTP + WebSocket (LiveView)
┌────────────────────────▼────────────────────────────────────┐
│  Orchestration Layer (Elixir/Phoenix)                        │
│  ┌──────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │  Oban    │  │  PubSub      │  │  Strategy DSL parser   │ │
│  │  jobs    │  │  progress    │  │  (TOML → validated     │ │
│  └────┬─────┘  └──────────────┘  │   StrategyConfig)      │ │
│       │                          └────────────────────────┘ │
│  ┌────▼──────────────────────────────────────────────────┐  │
│  │  Rustler NIF Bridge  (dirty CPU schedulers)           │  │
│  └────────────────────────┬──────────────────────────────┘  │
└───────────────────────────│──────────────────────────────────┘
                            │ NIF calls (JSON in → JSON out)
┌───────────────────────────▼─────────────────────────────────┐
│  Compute Layer (Rust Core)                                   │
│  ┌────────────────┐  ┌────────────────┐  ┌───────────────┐  │
│  │  quantedge-    │  │  quantedge-    │  │  quantedge-   │  │
│  │  data          │  │  greeks        │  │  metrics      │  │
│  │  (Parquet mmap │  │  (BS, binomial │  │  (75+ metrics │  │
│  │   IV surface)  │  │   SIMD batch)  │  │   Monte Carlo │  │
│  └────────────────┘  └────────────────┘  │   walk-fwd)   │  │
│  ┌────────────────┐  ┌────────────────┐  └───────────────┘  │
│  │  quantedge-    │  │  quantedge-    │                     │
│  │  core          │  │  portfolio     │                     │
│  │  (Leg, Strategy│  │  (Multi-strat  │                     │
│  │   SL, Re-entry)│  │   margin, corr)│                     │
│  └────────────────┘  └────────────────┘                     │
│  ┌────────────────┐                                         │
│  │  quantedge-    │                                         │
│  │  optimizer     │                                         │
│  │  (Grid sweep   │                                         │
│  │   Rayon par.)  │                                         │
│  └────────────────┘                                         │
└─────────────────────────────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│  Storage Layer                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │  Postgres    │  │  DuckDB      │  │  Parquet Files   │   │
│  │  (strategies │  │  (trades,    │  │  (bar data,      │   │
│  │   runs, meta)│  │   equity,    │  │   partitioned    │   │
│  │              │  │   metrics,   │  │   by symbol/      │   │
│  │              │  │   optimizer) │  │   year/month)    │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| Phoenix LiveView | Real-time UI rendering, user interaction, form handling | Server-rendered HTML with WebSocket push updates |
| Oban Workers | Background job execution, retry handling, status tracking | Postgres-backed queue; one worker type per job (backtest, optimizer, portfolio) |
| PubSub | Real-time progress broadcasting | Phoenix.PubSub; topic per run_id |
| Strategy DSL Parser | TOML → validated StrategyConfig struct | `nimble_toml` in Elixir, `toml` crate in Rust |
| Rustler NIF Bridge | Elixir↔Rust communication boundary | JSON serialization; dirty_cpu scheduler; async result return |
| quantedge-data | Bar loading, expiry calendar, IV surface | Memory-mapped Parquet via Polars; cubic spline IV interpolation |
| quantedge-core | Simulation loop, SL state machines, re-entry, momentum filters | Single-threaded inner loop per strategy for L1/L2 cache optimization |
| quantedge-greeks | Black-Scholes pricing, Greeks computation | SIMD-vectorized batch computation where available |
| quantedge-metrics | All 75+ metrics computation, walk-forward, Monte Carlo | Streaming computation on trade/snapshot arrays |
| quantedge-portfolio | Multi-strategy orchestration, margin model, correlation | Per-strategy equity curves → combined portfolio metrics |
| quantedge-optimizer | Parameter grid generation, parallel sweep | Rayon work-stealing across all CPU cores |
| quantedge-nif | Rustler wrapper exposing core/portfolio/optimizer to Elixir | Thin wrapper; JSON I/O boundary |

## Recommended Project Structure

```
quantedge/
├── apps/
│   ├── quantedge_core/              # Rust workspace
│   │   ├── crates/
│   │   │   ├── data/                # BarStream, ExpiryCalendar, IvSurface, LotSizes
│   │   │   ├── core/                # Leg, Strategy, SlStateMachine, ReEntryState, Runner
│   │   │   ├── greeks/              # BlackScholes, Greeks, IvSolver
│   │   │   ├── metrics/             # All 75+ metrics, WalkForward, MonteCarlo
│   │   │   ├── portfolio/           # PortfolioEngine, MarginModel, CorrelationMatrix
│   │   │   ├── optimizer/           # ParamGrid, OptimizerSweep
│   │   │   └── nif/                 # Rustler NIF wrapper
│   │   └── Cargo.toml               # Workspace Cargo.toml
│   │
│   └── quantedge_web/               # Phoenix umbrella app
│       ├── lib/
│       │   ├── quantedge/           # Business logic contexts
│       │   │   ├── strategies/      # Strategy CRUD, DSL parser
│       │   │   ├── runs/            # Run management, result storage
│       │   │   ├── optimizer/       # Optimizer orchestration
│       │   │   ├── portfolio/       # Portfolio management
│       │   │   └── workers/         # Oban workers
│       │   └── quantedge_web/       # Web layer
│       │       ├── live/            # LiveView modules
│       │       └── components/      # Shared UI components
│       ├── priv/repo/migrations/    # Postgres migrations
│       └── mix.exs
│
├── config/
│   ├── strategies/                  # Example TOML strategy files
│   ├── expiry_calendar.toml         # NSE expiry transitions
│   └── lot_sizes.toml               # NSE lot size history
│
├── data/
│   ├── raw/                         # Original CSVs (gitignored)
│   └── parquet/                     # Converted Parquet (gitignored)
│       └── {symbol}/{weekly|monthly}/{year}/{month}.parquet
│
├── scripts/
│   ├── csv_to_parquet.py            # Data conversion
│   └── validate_data.py             # Data integrity checks
│
└── docs/
    ├── PRD.md                       # Product requirements
    ├── METRICS.md                   # All 75 metrics with formulas
    └── DSL.md                       # Strategy TOML schema
```

### Structure Rationale

- **Rust workspace with separate crates:** Each crate has a single responsibility; compile times improve because changing `metrics` doesn't recompile `data`
- **Umbrella Phoenix app:** Keeps web concerns separate from business logic; contexts enforce clean data access patterns
- **Config as TOML files:** Version-controllable, human-readable; expiry calendar and lot sizes are data, not code
- **Parquet partitioned by symbol/type/year/month:** Enables loading only needed time ranges; memory-mapped reads efficient per-file

## Architectural Patterns

### Pattern 1: NIF Boundary as JSON Gateway

**What:** All communication between Elixir and Rust goes through JSON serialization at the NIF boundary.
**When to use:** Long-running compute tasks where Elixir sends a strategy config and receives results.
**Trade-offs:** +Simple debugging (inspect JSON), +Clean separation, -Serialization overhead (~1-2ms)

```rust
// NIF side
#[rustler::nif(schedule = "DirtyCpu")]
fn run_backtest(strategy_json: String, opts_json: String) -> Result<String, String> {
    let config: StrategyConfig = serde_json::from_str(&strategy_json)?;
    let result = runner::run(&config, &opts)?;
    Ok(serde_json::to_string(&result)?)
}
```

### Pattern 2: Single-Threaded Inner Loop, Rayon Outer Loop

**What:** Each individual backtest runs single-threaded for cache efficiency; parallelism happens at the strategy level (optimizer sweep).
**When to use:** When iterating over a time series where each bar depends on previous state.
**Trade-offs:** +Maximizes L1/L2 cache hits on bar data, +No synchronization overhead, -Can't speed up a single backtest with more cores

```rust
// Inner loop: single-threaded, cache-friendly
fn run_single(strategy: &Strategy, bars: &[Bar]) -> RunResult { /* sequential */ }

// Outer loop: parallel across strategies
param_grid.into_par_iter()
    .map(|params| run_single(&strategy.with_params(&params), &bars))
    .collect()
```

### Pattern 3: Event-Driven Exit Priority Chain

**What:** Exits are checked in strict priority order on each bar: per-leg SL → combined SL → per-leg target → overall target → time exit.
**When to use:** Multi-leg strategies with overlapping exit conditions.
**Trade-offs:** +Deterministic behavior, +Matches real trading semantics, -Must carefully define priority order upfront

### Pattern 4: Three-Tier Storage Split

**What:** Postgres for transactional data, DuckDB for analytical queries, Parquet for raw bar data.
**When to use:** When you have different access patterns — CRUD vs aggregation vs sequential scan.
**Trade-offs:** +Each store optimized for its access pattern, -Three systems to manage, -Data consistency across stores

## Data Flow

### Backtest Request Flow

```
[User clicks "Run"]
    ↓
[LiveView] → [Oban.insert(BacktestWorker)]
    ↓
[BacktestWorker.perform/1]
    ↓
[NIF.run_backtest(strategy_json, opts_json)]  ← dirty CPU scheduler
    ↓
[Rust: load bars → run simulation → compute metrics]
    ↓
[NIF returns result_json]
    ↓
[Worker: parse result → store to DuckDB (trades, equity) + Postgres (summary)]
    ↓
[PubSub.broadcast("run:#{id}", {:completed, result})]
    ↓
[LiveView receives push → re-render results page]
```

### Optimizer Flow

```
[User configures param grid]
    ↓
[Oban.insert(OptimizerWorker)]
    ↓
[NIF.run_optimizer(strategy_json, param_grid_json)]
    ↓
[Rust: Rayon parallel over param combos → collect results]
    ↓
[NIF returns all combo results]
    ↓
[Worker: store to DuckDB (optimizer_results) + Postgres (status)]
    ↓
[PubSub → LiveView renders heatmap]
```

### Key Data Flows

1. **Bar data flow:** Parquet on disk → memory-mapped read → Polars DataFrame → Vec<Bar> → simulation loop
2. **Result flow:** Rust RunResult → JSON → Elixir → DuckDB (trades/equity) + Postgres (summary)
3. **Progress flow:** Rust progress callback → NIF → Elixir PubSub → LiveView WebSocket → browser update

## Anti-Patterns

### Anti-Pattern 1: Simulation Logic in Elixir

**What people do:** Put parts of the simulation loop in Elixir "for convenience"
**Why it's wrong:** 100-1000× slower than Rust for numeric computation; breaks the clean boundary
**Do this instead:** ALL simulation math in Rust. Elixir handles I/O, persistence, UI only.

### Anti-Pattern 2: Passing DataFrames Through NIF

**What people do:** Serialize entire DataFrames from Rust → Elixir → Rust
**Why it's wrong:** Serialization cost can exceed computation cost; negates zero-copy benefits
**Do this instead:** Load data entirely in Rust; pass only final results (summary JSON) to Elixir

### Anti-Pattern 3: Shared Mutable State in Simulation

**What people do:** Use Arc<Mutex<>> for position state across threads
**Why it's wrong:** Destroys cache locality; synchronization overhead; non-deterministic
**Do this instead:** Each backtest owns its own state; parallelize at strategy level, not bar level

### Anti-Pattern 4: Blocking the BEAM

**What people do:** Run NIFs on normal scheduler or hold locks too long
**Why it's wrong:** Blocks all other Elixir processes; LiveView becomes unresponsive
**Do this instead:** Always use `schedule = "DirtyCpu"` for NIFs >1ms; or spawn external process

## Integration Points

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Elixir ↔ Rust | JSON via Rustler NIF (dirty_cpu) | Strategy config in, result summary out; keep payloads small |
| LiveView ↔ Oban | Oban job insertion + PubSub | LiveView inserts job, subscribes to PubSub topic for updates |
| Elixir ↔ Postgres | Ecto contexts | Standard Phoenix context pattern |
| Elixir ↔ DuckDB | Raw SQL via duckdbex | No ORM; write queries directly; batch inserts for trades |
| Rust data ↔ core | Direct function calls | Same process; data crate exposes BarStream to core |
| Rust core ↔ metrics | Direct function calls | Metrics computed from RunResult (trades + snapshots) |

## Sources

- Elixir Forum — Rustler NIF patterns, dirty scheduler best practices
- Phoenix LiveView documentation — PubSub patterns for real-time updates
- Oban documentation — worker patterns, job lifecycle
- Polars official docs — Parquet I/O, memory-mapped reading
- Rust performance patterns — cache-friendly iteration, Rayon parallelism
- DuckDB documentation — embedded usage, Parquet direct querying

---
*Architecture research for: FNO Options Backtesting Platform*
*Researched: 2026-04-19*
