# QuantEdge — Phase Execution Reference

> Everything you need to build each phase. No fluff. Source of truth for implementation.

---

## Quick Index

| Phase | What You Build | Key Rust Structs | Est. |
|---|---|---|---|
| [P1 — Data Foundation](#p1--data-foundation) | Parquet pipeline, expiry calendar, IV interpolator | `BarStream`, `ExpiryCalendar`, `IvSurface` | 2w |
| [P2 — Single-leg Engine](#p2--single-leg-engine) | CE/PE sim, ATM strike, fixed SL, 20 metrics | `Leg`, `Position`, `SlStateMachine`, `MetricsEngine` | 2w |
| [P3 — Multi-leg + Advanced SL](#p3--multi-leg--advanced-sl) | Multi-leg DSL, combined SL, trailing SL, OCO | `Strategy`, `PortfolioSnapshot`, `TrailState` | 3w |
| [P4 — Re-entry + Momentum](#p4--re-entry--momentum) | Re-entry state machine, momentum filters | `ReEntryState`, `MomentumFilter`, `RangeBreakout` | 2w |
| [P5 — Full Metrics Suite](#p5--full-metrics-suite) | All 75 metrics, Greeks PnL, walk-fwd, Monte Carlo | `MetricsSuite`, `WalkForward`, `MonteCarlo` | 2w |
| [P6 — Phoenix NIF Bridge](#p6--phoenix-nif-bridge) | Rustler NIFs, Oban jobs, Postgres + DuckDB | `QuantEdge.NIF`, `BacktestWorker`, schemas | 2w |
| [P7 — LiveView UI](#p7--liveview-ui) | Strategy builder, run manager, results viewer | LiveView modules, components | 3w |
| [P8 — Portfolio Engine](#p8--portfolio-engine) | Multi-strategy, margin model, correlation | `PortfolioEngine`, `MarginModel`, `CorrelationMatrix` | 2w |
| [P9 — Optimizer + Hardening](#p9--optimizer--hardening) | Param sweep, heatmap, profiling, edge cases | `OptimizerSweep`, `ParamGrid` | 2w |

---

## Data Schema (Your CSV)

Every Parquet file must conform to this schema. Validate before any phase.

```
timestamp     : Utf8       "2023-05-15 11:07:00+05:30"
date          : Date32     "2023-05-15"
time          : Utf8       "11:07:00"
weekday       : Utf8       "Monday"
option_type   : Utf8       "CE" | "PE"
strike_label  : Utf8       "ATM" | "ATM+5" | "ATM-10"
strike_offset : Int32      0, 5, -10  (offset from ATM in strike units)
moneyness     : Utf8       "ATM" | "OTM" | "ITM"
open          : Float64
high          : Float64
low           : Float64
close         : Float64
volume        : Int64
strike        : Float64    absolute strike price
oi            : Float64    open interest
spot          : Float64    underlying spot at bar close
iv            : Float64    implied volatility (annualised decimal e.g. 0.18)
```

---

## BankNifty Expiry Transition — Always Keep This in Mind

```
BANKNIFTY  weekly → monthly   cutoff: 2024-11-01  (Wednesday → last Thursday)
NIFTY      weekly → monthly   cutoff: 2024-10-03  (Thursday  → last Thursday)
SENSEX     weekly → monthly   cutoff: 2024-09-18  (Friday    → last Friday)
```

**Rule:** Every open leg stores an actual `NaiveDate` expiry, never an expiry type string.
**Rule:** `ExpiryCalendar` is the only place that resolves expiry dates. No other code does this.
**Rule:** Parquet is partitioned into `/weekly/` and `/monthly/` sub-trees. Data loader picks the right tree per bar date.

Overlap window `Oct 28 – Nov 1, 2024` — both partitions queried; `expiry_filter` setting decides which rows are used.

---

## Parquet Layout

```
data/
├── parquet/
│   ├── banknifty/
│   │   ├── weekly/
│   │   │   └── {year}/{month:02}.parquet   ← ends 2024-10
│   │   └── monthly/
│   │       └── {year}/{month:02}.parquet   ← full period
│   ├── nifty/
│   │   ├── weekly/  └── monthly/
│   └── sensex/
│       ├── weekly/  └── monthly/
└── raw/                                    ← original CSVs (gitignored)
```

---

## config/ Files Required Before Phase 1

### `config/expiry_calendar.toml`
```toml
[BANKNIFTY]
transitions = [
  { from = "2000-01-01", to = "2024-10-31", type = "weekly",  day = "Wednesday"      },
  { from = "2024-11-01", to = "2099-12-31", type = "monthly", day = "last_thursday"  },
]

[NIFTY]
transitions = [
  { from = "2000-01-01", to = "2024-10-02", type = "weekly",  day = "Thursday"       },
  { from = "2024-10-03", to = "2099-12-31", type = "monthly", day = "last_thursday"  },
]

[SENSEX]
transitions = [
  { from = "2000-01-01", to = "2024-09-17", type = "weekly",  day = "Friday"         },
  { from = "2024-09-18", to = "2099-12-31", type = "monthly", day = "last_friday"    },
]
```

### `config/lot_sizes.toml`
```toml
[[BANKNIFTY]]
from = "2000-01-01"
to   = "2024-11-19"
size = 15

[[BANKNIFTY]]
from = "2024-11-20"
to   = "2099-12-31"
size = 30

[[NIFTY]]
from = "2000-01-01"
to   = "2024-07-24"
size = 50

[[NIFTY]]
from = "2024-07-25"
to   = "2099-12-31"
size = 75
```

---

## Strategy TOML — Full Reference

```toml
# ── Top-level ────────────────────────────────────────────────
[strategy]
name                   = "Short Straddle Weekly"
underlying             = "BANKNIFTY"          # BANKNIFTY | NIFTY | SENSEX
entry_time             = "09:20"              # HH:MM IST
exit_time              = "15:20"
expiry_filter          = "weekly"             # weekly | monthly | nearest | specific_dte
trade_on_expiry        = true
max_concurrent_trades  = 1
capital                = 500000.0             # INR
brokerage_per_lot      = 40.0                 # INR per lot per side
slippage_model         = "fixed_pts"          # none | fixed_pts | percent | volume_based
slippage_value         = 1.0
stt_on_sell            = true

# ── Leg (repeat [[legs]] block for each leg) ─────────────────
[[legs]]
option_type            = "CE"                 # CE | PE
position               = "sell"               # buy | sell
lots                   = 1
expiry                 = "weekly"
strike_mode            = "atm_offset"         # atm_offset | delta | premium | percent_otm
strike_offset          = 0                    # 0=ATM, 2=ATM+2, -3=ATM-3
delta_target           = 0.20                 # used if strike_mode = delta
premium_target         = 200.0               # INR, used if strike_mode = premium

# Per-leg SL
stop_loss_enabled      = true
stop_loss_type         = "percent_of_premium" # points | percent_of_premium | percent_of_margin
                                              # | index_points | delta_breach | combined_premium
stop_loss_value        = 100.0

# Per-leg Target
target_profit_enabled  = false
target_profit_type     = "percent_of_premium"
target_profit_value    = 50.0

# Trailing SL
trail_sl_enabled       = true
trail_sl_activate_at   = 40.0                 # % profit to activate trailing
trail_sl_lock_in       = 30.0                 # % profit to lock from peak
trail_sl_type          = "percent"            # points | percent
trail_sl_value         = 10.0                 # trail distance

# Re-entry
reentry_on_sl          = true
reentry_on_target      = false
reentry_mode           = "after_n_bars"       # reasap | same_time | after_n_bars | momentum_confirm
reentry_cooldown_bars  = 5
reentry_max_attempts   = 2

# Momentum filter
momentum_filter_enabled = false
momentum_type           = "range_breakout"    # rsi | ema_cross | range_breakout | supertrend
momentum_direction      = "either"            # bullish | bearish | either
range_breakout_time     = "09:45"
range_breakout_side     = "high"              # high | low | either

# ── Overall strategy settings ─────────────────────────────────
[overall]
overall_sl_enabled     = true
overall_sl_type        = "percent_of_premium" # max_loss | percent_of_premium | percent_of_margin
overall_sl_value       = 60.0
overall_target_enabled = true
overall_target_type    = "percent_of_premium"
overall_target_value   = 50.0
trail_options_enabled  = false
trail_lock_type        = "lock"               # lock | trail
trail_lock_value       = 30.0
trail_activate_value   = 40.0
```

---

## P1 — Data Foundation

**Goal:** All data loaded, validated, accessible as zero-copy bar streams in Rust.

### Deliverables
- [ ] `scripts/csv_to_parquet.py` — converts raw CSVs, partitions by symbol/weekly-or-monthly/year/month
- [ ] `crates/data/src/expiry_calendar.rs` — `ExpiryCalendar` struct
- [ ] `crates/data/src/bar_stream.rs` — `BarStream`, memory-mapped Parquet reader
- [ ] `crates/data/src/iv_surface.rs` — `IvSurface` cubic spline interpolator
- [ ] `config/expiry_calendar.toml` + `config/lot_sizes.toml`
- [ ] `scripts/validate_data.py` — 6 checks (see below)

### Key Rust Structs

```rust
// crates/data/src/bar_stream.rs
pub struct Bar {
    pub timestamp:    NaiveDateTime,
    pub option_type:  OptionType,      // CE | PE
    pub strike:       f64,
    pub strike_offset: i32,
    pub open: f64, pub high: f64, pub low: f64, pub close: f64,
    pub volume: i64,
    pub oi:     f64,
    pub spot:   f64,
    pub iv:     f64,
    pub expiry: NaiveDate,             // resolved by ExpiryCalendar at load time
}

// crates/data/src/expiry_calendar.rs
pub struct ExpiryCalendar { /* transitions per symbol */ }
impl ExpiryCalendar {
    pub fn get_next_expiry(&self, symbol: &str, date: NaiveDate) -> NaiveDate;
    pub fn get_dte(&self, symbol: &str, date: NaiveDate) -> u32;
    pub fn is_expiry_day(&self, symbol: &str, date: NaiveDate) -> bool;
    pub fn get_expiry_type(&self, symbol: &str, date: NaiveDate) -> ExpiryType;
}

// crates/data/src/iv_surface.rs
pub struct IvSurface { /* spline knots per expiry */ }
impl IvSurface {
    pub fn get_iv(&self, strike_offset: i32, dte: u32) -> f64;
}
```

### Validation Checks (`validate_data.py`)
1. Weekly Parquet ends on/before cutoff date per symbol
2. Monthly Parquet has no date gaps > 1 trading day
3. No duplicate `(date, time, option_type, strike)` across weekly + monthly for same symbol
4. IV coverage ≥ 95% (null IV rows → Greeks will fail)
5. Spot continuity — no consecutive bar gap > 5%
6. `lot_sizes.toml` covers every `(symbol, date)` in dataset

### Acceptance Criteria
- Full 4-year load for one symbol: **< 100ms**
- `validate_data.py` exits 0 on clean data
- `ExpiryCalendar` unit tests: weekly pre-cutoff, monthly post-cutoff, transition week, expiry-day detection

---

## P2 — Single-Leg Engine

**Goal:** End-to-end backtest for one leg — CE or PE, ATM strike, fixed SL/target, basic metrics.

### Deliverables
- [ ] `crates/core/src/leg.rs` — `Leg`, `LegState`, `SlStateMachine`
- [ ] `crates/core/src/position.rs` — `Position`, `PositionSnapshot`
- [ ] `crates/core/src/strike_selector.rs` — `StrikeSelector` (ATM offset for now)
- [ ] `crates/core/src/execution.rs` — `ExecutionEngine` (slippage, brokerage, STT)
- [ ] `crates/metrics/src/basic.rs` — 20 core metrics
- [ ] CLI binary: `cargo run --bin backtest -- --strategy config/strategies/example.toml`

### Simulation Loop (Inner Loop — Keep It Tight)

```rust
// crates/core/src/runner.rs
pub fn run(strategy: &StrategyConfig, bars: &[Bar]) -> RunResult {
    let mut position: Option<Position> = None;
    let mut snapshots: Vec<PositionSnapshot> = Vec::with_capacity(bars.len());
    let mut trades: Vec<ClosedTrade> = Vec::new();

    for bar in bars {
        // 1. Check entry condition
        if position.is_none() && should_enter(strategy, bar) {
            position = Some(open_position(strategy, bar));
        }

        // 2. Update open position
        if let Some(ref mut pos) = position {
            pos.update(bar);  // mark-to-market, update SL state machines

            // 3. Check exits (order matters: SL > Target > Time)
            if let Some(exit) = check_exits(strategy, pos, bar) {
                trades.push(close_position(pos, bar, exit));
                position = None;
            }
        }

        // 4. Capture snapshot for equity curve
        snapshots.push(PositionSnapshot::from(&position, bar));
    }
    RunResult { trades, snapshots }
}
```

### SL State Machine States
```rust
pub enum SlState {
    Active,               // monitoring, no trigger yet
    TrailActivated {      // profit crossed activate_at threshold
        high_water: f64,  // tracks peak PnL
    },
    Triggered { reason: ExitReason },
}

pub enum ExitReason {
    StopLoss, Target, TimeExit, EndOfData
}
```

### 20 Core Metrics for P2
`total_pnl_net` · `total_pnl_gross` · `win_rate_pct` · `profit_factor` · `expectancy` · `total_trades` · `avg_win` · `avg_loss` · `largest_win` · `largest_loss` · `max_drawdown_inr` · `max_drawdown_pct` · `sharpe_ratio` · `cagr` · `roi_pct` · `sl_hit_rate_pct` · `target_hit_rate_pct` · `time_exit_rate_pct` · `avg_hold_bars` · `total_brokerage`

### Acceptance Criteria
- Single ATM short call, 4yr BankNifty: **< 1 second**
- Brokerage + STT correctly deducted from every trade
- Equity curve matches manual spot-check on 5 sampled trades
- All 20 metrics match Excel calculation on 10-trade sample

---

## P3 — Multi-Leg + Advanced SL

**Goal:** Full multi-leg strategies. Combined SL. Trailing SL complete state machine. OCO.

### Deliverables
- [ ] `crates/core/src/strategy.rs` — `Strategy` (N legs), `CombinedSlMonitor`
- [ ] `crates/core/src/sl_types.rs` — all 7 SL types as enum variants
- [ ] Trailing SL with lock + trail modes (high-water mark tracking)
- [ ] OCO (one-cancels-other) between SL and target at leg level
- [ ] Index-points SL (monitors spot, not option price)
- [ ] Delta-breach SL (requires bar-level Greeks — see Greeks engine P5, stub for now)

### All SL Types

```rust
pub enum StopLossType {
    Points(f64),                  // option price moves X pts against entry
    PercentOfPremium(f64),        // PnL exceeds X% of entry premium
    PercentOfMargin(f64),         // MTM loss exceeds X% of SPAN margin
    IndexPoints(f64),             // spot moves X points against position delta
    DeltaBreach(f64),             // leg delta crosses threshold (stub in P3, live in P5)
    CombinedPremium(f64),         // sum of all leg PnLs crosses threshold
    None,
}

pub enum TrailSlMode {
    Lock {
        activate_at_pct: f64,     // % profit to activate
        lock_in_pct: f64,         // % profit to lock from peak — SL rises, never falls
    },
    Trail {
        activate_at_pct: f64,
        trail_by: TrailUnit,      // Points(f64) | Percent(f64)
    },
}
```

### Combined SL Priority Order
When multiple exits are possible on the same bar:
1. Per-leg SL (checked first — hardest limit)
2. Combined / overall SL (portfolio-level)
3. Per-leg target
4. Overall target
5. Time exit

### Acceptance Criteria
- Short straddle (CE sell + PE sell), 4yr: **< 1.5 seconds**
- Trailing SL high-water mark never decreases once activated
- Combined SL fires correctly when sum of leg PnLs breaches threshold
- OCO: when SL fires on one leg, target on same leg is cancelled

---

## P4 — Re-entry + Momentum

**Goal:** Full re-entry state machine. RSI, EMA cross, range breakout, supertrend filters.

### Re-entry State Machine

```rust
pub enum ReEntryState {
    Idle,
    Cooling {
        bars_remaining: u32,
        attempt: u32,
    },
    WaitingForMomentum {
        attempt: u32,
    },
    Ready { attempt: u32 },       // cleared to re-enter next bar open
    Exhausted,                    // max_attempts reached
}

pub enum ReEntryMode {
    ReAsap,                       // re-enter at next bar open
    SameTime,                     // re-enter at same clock time next trading day
    AfterNBars { n: u32 },        // wait N bars then re-enter
    MomentumConfirm,              // wait for momentum filter to confirm
}
```

### Momentum Filters

```rust
pub enum MomentumFilter {
    Rsi {
        period: u32,
        bullish_above: f64,       // default 50.0
        bearish_below: f64,
    },
    EmaCross {
        fast: u32,                // default 9
        slow: u32,                // default 21
    },
    RangeBreakout {
        range_time: NaiveTime,    // e.g. 09:45
        side: BreakoutSide,       // High | Low | Either
    },
    Supertrend {
        period: u32,              // default 7
        multiplier: f64,          // default 3.0
    },
}
```

### Range Breakout Pre-computation
Range breakout needs the high/low of bars from market open to `range_time`. Pre-compute this in a single pass before the main simulation loop — O(N) setup cost, O(1) per bar lookup.

### Acceptance Criteria
- Re-entry fires correctly at right bar after cooldown
- `max_attempts` is respected — never re-enters beyond the limit
- Range breakout high/low computed correctly vs manual check on 3 sample days
- Momentum confirm does not re-enter when filter is against direction

---

## P5 — Full Metrics Suite

**Goal:** All 75 metrics. Greeks PnL attribution. Walk-forward. Monte Carlo.

### All 75 Metrics by Category

**Return (14):** `total_pnl_gross` `total_pnl_net` `cagr` `roi_pct` `expectancy` `profit_factor` `win_rate_pct` `avg_win` `avg_loss` `win_loss_ratio` `largest_win` `largest_loss` `gross_profit` `gross_loss`

**Risk (16):** `max_drawdown_inr` `max_drawdown_pct` `avg_drawdown` `sharpe_ratio` `sortino_ratio` `calmar_ratio` `omega_ratio` `var_95` `var_99` `cvar` `ulcer_index` `daily_volatility` `ann_volatility` `skewness` `kurtosis` `recovery_factor` `drawdown_duration_bars`

**Trade (14):** `total_trades` `avg_hold_bars` `max_hold_bars` `max_consec_wins` `max_consec_losses` `sl_hit_rate_pct` `target_hit_rate_pct` `time_exit_rate_pct` `reentry_count` `reentry_win_rate` `total_brokerage` `total_slippage` `total_stt` `net_cost_ratio`

**Options (14):** `premium_capture_pct` `total_theta_collected` `avg_theta_per_day` `avg_iv_at_entry` `avg_iv_at_exit` `iv_crush_pct` `delta_pnl` `gamma_pnl` `theta_pnl` `vega_pnl` `avg_net_delta` `dte_distribution` `breakeven_range` `max_profit_theoretical`

**Portfolio (7):** `strategy_correlation` `portfolio_sharpe` `peak_margin_used` `capital_efficiency` `net_portfolio_greeks` `avg_concurrent_trades` `diversification_benefit`

**Time (12):** `monthly_pnl_heatmap` `day_of_week_pnl` `expiry_day_performance` `best_month` `worst_month` `pct_profitable_months` `pct_profitable_weeks` `walk_forward_results` `monte_carlo_bands` `rolling_sharpe_12m` `equity_curve` `drawdown_curve`

### Key Formulas

```
Sharpe  = (ann_return - 0.065) / ann_volatility          # risk-free = 6.5% (India)
Sortino = (ann_return - 0.065) / downside_deviation
Calmar  = CAGR / max_drawdown_pct
VaR 95  = percentile(daily_pnl_series, 5)
CVaR    = mean(daily_pnl < VaR_95)
Ulcer   = sqrt( mean( ((equity - rolling_max) / rolling_max * 100)^2 ) )

Greeks PnL attribution (per trade):
  delta_pnl = sum_legs(delta_entry * spot_move * lot_size)
  gamma_pnl = sum_legs(0.5 * gamma_entry * spot_move^2 * lot_size)
  theta_pnl = sum_legs(theta_entry * days_held)
  vega_pnl  = sum_legs(vega_entry * (iv_exit - iv_entry) * lot_size)
  unexplained = actual_pnl - (delta + gamma + theta + vega)
```

### Walk-Forward Setup
```
Total period: 4 years
Window: 6 months in-sample, 2 months out-of-sample
Slide: 2 months
Result: ~12 IS/OOS pairs
Report: IS Sharpe, OOS Sharpe, degradation ratio (OOS/IS) per window
```

### Monte Carlo
```
N = 1000 simulations
Method: shuffle trade sequence randomly, recompute equity curve
Output: 5th / 25th / 50th / 75th / 95th percentile equity curves
Key stat: probability of positive return at median, P(MDD > X%)
```

---

## P6 — Phoenix NIF Bridge

**Goal:** Rust callable from Elixir. Oban job queue. Postgres + DuckDB schemas. PubSub progress.

### Rustler NIF Functions

```elixir
# lib/quantedge/nif.ex
defmodule QuantEdge.NIF do
  use Rustler, otp_app: :quantedge, crate: "quantedge_nif"

  def run_backtest(_strategy_json, _opts_json),      do: :erlang.nif_error(:not_loaded)
  def run_optimizer(_strategy_json, _param_grid),    do: :erlang.nif_error(:not_loaded)
  def run_portfolio(_strategies_json, _opts_json),   do: :erlang.nif_error(:not_loaded)
  def load_data_summary(_symbol, _date_from, _date_to), do: :erlang.nif_error(:not_loaded)
end
```

All NIFs run on `dirty_cpu` scheduler — never blocks BEAM.

### Oban Workers

```elixir
# lib/quantedge/workers/backtest_worker.ex
defmodule QuantEdge.Workers.BacktestWorker do
  use Oban.Worker, queue: :backtests, max_attempts: 1

  def perform(%Oban.Job{args: %{"run_id" => run_id}}) do
    run = Runs.get_run!(run_id)
    strategy_json = Jason.encode!(run.strategy.config)

    # Broadcast start
    Phoenix.PubSub.broadcast(QuantEdge.PubSub, "run:#{run_id}", {:status, :running})

    case QuantEdge.NIF.run_backtest(strategy_json, build_opts(run)) do
      {:ok, result_json} ->
        result = Jason.decode!(result_json)
        Runs.store_result(run_id, result)         # → DuckDB trades + equity curve
        Runs.update_status(run_id, :completed)
        Phoenix.PubSub.broadcast(QuantEdge.PubSub, "run:#{run_id}", {:completed, result})

      {:error, reason} ->
        Runs.update_status(run_id, :failed)
        {:error, reason}
    end
  end
end
```

### Postgres Tables

```sql
-- strategies
CREATE TABLE strategies (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name        TEXT NOT NULL,
  underlying  TEXT NOT NULL,
  config_toml TEXT NOT NULL,
  created_at  TIMESTAMPTZ DEFAULT now(),
  updated_at  TIMESTAMPTZ DEFAULT now()
);

-- backtest_runs
CREATE TABLE backtest_runs (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  strategy_id     UUID REFERENCES strategies(id),
  status          TEXT NOT NULL DEFAULT 'pending',   -- pending|running|completed|failed
  date_from       DATE NOT NULL,
  date_to         DATE NOT NULL,
  capital         NUMERIC NOT NULL,
  started_at      TIMESTAMPTZ,
  completed_at    TIMESTAMPTZ,
  result_summary  JSONB                               -- top-level metrics only
);

-- optimizer_runs
CREATE TABLE optimizer_runs (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  strategy_id     UUID REFERENCES strategies(id),
  param_grid      JSONB NOT NULL,
  status          TEXT NOT NULL DEFAULT 'pending',
  total_combos    INTEGER,
  completed_combos INTEGER DEFAULT 0
);
```

### DuckDB Tables

```sql
-- trades (all legs of all runs — high row count, columnar queries)
CREATE TABLE trades (
  run_id        VARCHAR,
  trade_id      INTEGER,
  entry_time    TIMESTAMP,
  exit_time     TIMESTAMP,
  exit_reason   VARCHAR,
  legs          JSON,
  pnl_gross     DOUBLE,
  pnl_net       DOUBLE,
  hold_bars     INTEGER,
  greeks_entry  JSON,
  greeks_exit   JSON
);

-- equity_curves
CREATE TABLE equity_curves (
  run_id        VARCHAR,
  date          DATE,
  equity        DOUBLE,
  drawdown_pct  DOUBLE
);

-- metrics (one row per metric per run — fast aggregation)
CREATE TABLE metrics (
  run_id        VARCHAR,
  metric_name   VARCHAR,
  metric_value  DOUBLE
);

-- optimizer_results
CREATE TABLE optimizer_results (
  optimizer_run_id VARCHAR,
  combo_index      INTEGER,
  params           JSON,
  metrics          JSON
);
```

---

## P7 — LiveView UI

**Goal:** Full web UI. Strategy builder, run manager, results viewer, optimizer dashboard.

### Routes

```elixir
scope "/", QuantEdgeWeb do
  pipe_through :browser

  live "/",                         DashboardLive
  live "/strategies",               StrategyListLive
  live "/strategies/new",           StrategyBuilderLive
  live "/strategies/:id/edit",      StrategyBuilderLive
  live "/strategies/:id/run",       RunConfigLive
  live "/runs",                     RunListLive
  live "/runs/:id",                 RunResultsLive
  live "/optimizer",                OptimizerLive
  live "/optimizer/:id",            OptimizerResultsLive
  live "/portfolio",                PortfolioBuilderLive
  live "/portfolio/:id/run",        PortfolioResultsLive
  live "/data",                     DataExplorerLive
end
```

### Results Viewer — Required Charts
1. **Equity curve** — cumulative PnL line chart with drawdown band shaded below
2. **Monthly heatmap** — grid: rows=months Jan-Dec, cols=years, cell color = PnL green/red
3. **Greeks attribution** — stacked bar: delta/gamma/theta/vega PnL per trade (or monthly)
4. **Monte Carlo fan** — 1000 equity curves, show 5/25/50/75/95 percentile bands
5. **Walk-forward table** — IS Sharpe | OOS Sharpe | Degradation per window

### Hero Stats Cards (top of results page)
`Total PnL` · `CAGR` · `Win Rate` · `Max Drawdown` · `Sharpe` · `Profit Factor` · `Total Trades` · `Premium Capture %`

### Real-Time Progress Pattern
```elixir
# In RunResultsLive
def mount(%{"id" => run_id}, _session, socket) do
  Phoenix.PubSub.subscribe(QuantEdge.PubSub, "run:#{run_id}")
  {:ok, assign(socket, run_id: run_id, progress: 0, status: :pending)}
end

def handle_info({:progress, pct}, socket),       do: {:noreply, assign(socket, progress: pct)}
def handle_info({:completed, result}, socket),   do: {:noreply, assign(socket, result: result, status: :completed)}
```

---

## P8 — Portfolio Engine

**Goal:** Multiple strategies running simultaneously, shared capital, margin awareness, correlation.

### Portfolio Config TOML
```toml
[portfolio]
name    = "My FNO Portfolio"
capital = 2000000.0           # total INR

[[allocations]]
strategy = "short_straddle_weekly"
capital_pct = 40.0            # 40% of total capital

[[allocations]]
strategy = "iron_condor_monthly"
capital_pct = 35.0

[[allocations]]
strategy = "short_strangle_nifty"
capital_pct = 25.0
```

### Margin Model (Simplified SPAN)
```
short_option_margin = max(
    3 * entry_premium * lot_size,
    index_margin_factor * spot * lot_size * 0.12
)
index_margin_factor = 0.12 for BankNifty/Nifty/Sensex

Portfolio margin check before entry:
  available_margin = allocated_capital - current_open_position_margin
  if required_margin > available_margin → skip trade, log as "margin_skip"
```

### Portfolio Metrics Formulas
```
portfolio_sharpe = mean(portfolio_daily_returns) / std(portfolio_daily_returns) * sqrt(252)
correlation[i,j] = pearson(strategy_i_daily_returns, strategy_j_daily_returns)
diversification_benefit = portfolio_sharpe - mean(individual_sharpes)
capital_efficiency = total_net_pnl / peak_margin_used
```

---

## P9 — Optimizer + Hardening

**Goal:** Param sweep UI, heatmap visualization, profiling, edge case validation.

### Param Grid Format
```toml
# config/optimizer/straddle_sweep.toml
[grid]
strike_offset_ce  = [0, 1, 2, 3, 4, 5]
strike_offset_pe  = [0, -1, -2, -3, -4, -5]
sl_value          = [50.0, 60.0, 70.0, 80.0, 100.0]
target_value      = [30.0, 40.0, 50.0]
# Total: 6 × 6 × 5 × 4 = 720 combinations
```

### Optimizer Execution
```rust
// Rayon parallel sweep — each combo is independent
param_grid
    .into_par_iter()
    .map(|params| {
        let strategy = base_strategy.apply_params(&params);
        let result = run(&strategy, &bars);
        (params, compute_metrics(&result))
    })
    .collect::<Vec<_>>()
```

### Heatmap Display
- X-axis: param A values
- Y-axis: param B values
- Cell color: Sharpe ratio (green = high, red = low, grey = insufficient trades < 20)
- Click cell → opens full results for that combo

### Edge Cases to Test Before Shipping
- [ ] Strategy with 0 trades (all filtered by momentum)
- [ ] Strategy entering on first bar of dataset
- [ ] Strategy entering on last bar of dataset (no exit bar available)
- [ ] Transition week Oct 28–Nov 1, 2024 — both expiry types
- [ ] Lot size change date — trade open before, closed after change
- [ ] Zero volume bar — slippage model with volume_based must not divide by zero
- [ ] IV = 0 row (rare but exists in data) — Greeks calculation must not NaN
- [ ] Max concurrent trades limit hit — second entry correctly skipped
- [ ] Re-entry max attempts reached — no further re-entries despite SL triggers

---

## Rust Crate Map

```
apps/quantedge_core/
└── crates/
    ├── data/        ← P1: BarStream, ExpiryCalendar, IvSurface, LotSizes
    ├── greeks/      ← P5: BlackScholes, Greeks, IvSolver (SIMD)
    ├── core/        ← P2-P4: Leg, Strategy, SlStateMachine, ReEntryState, Runner
    ├── metrics/     ← P2+P5: all 75 metrics, WalkForward, MonteCarlo
    ├── portfolio/   ← P8: PortfolioEngine, MarginModel, CorrelationMatrix
    ├── optimizer/   ← P9: ParamGrid, OptimizerSweep
    └── nif/         ← P6: Rustler wrapper over core + portfolio + optimizer
```

---

## Performance Gates (Must Pass Before Phase Closes)

| Phase | Gate | Target |
|---|---|---|
| P1 | `validate_data.py` exits 0, bar load 4yr 1 symbol | < 100ms |
| P2 | Single ATM short call, 4yr BankNifty full run | < 1s |
| P3 | Short straddle (2 legs), 4yr full run | < 1.5s |
| P5 | All 75 metrics computed from 10k trade result | < 200ms |
| P6 | NIF call overhead (empty backtest round-trip) | < 5ms |
| P8 | 3-strategy portfolio, 4yr | < 5s |
| P9 | 720-combo optimizer sweep, 4yr | < 3min |

---

## Dependencies Quick Reference

### Rust `Cargo.toml`
```toml
polars       = { version = "0.39", features = ["parquet", "lazy"] }
arrow2       = { version = "0.18", features = ["io_parquet", "io_parquet_async"] }
rayon        = "1.10"
rustler      = "0.32"
serde        = { version = "1", features = ["derive"] }
serde_json   = "1"
chrono       = { version = "0.4", features = ["serde"] }
ndarray      = "0.15"
statrs       = "0.16"
toml         = "0.8"
```

### `mix.exs` deps
```elixir
{:phoenix, "~> 1.7"},
{:phoenix_live_view, "~> 0.20"},
{:rustler, "~> 0.32"},
{:oban, "~> 2.17"},
{:ecto_sql, "~> 3.11"},
{:postgrex, ">= 0.0.0"},
{:duckdbex, "~> 0.3"},
{:jason, "~> 1.4"},
{:nimble_toml, "~> 1.0"},
```