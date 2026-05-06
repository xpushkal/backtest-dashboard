# Roadmap: QuantEdge

## Overview

QuantEdge is built in 9 phases following a strict dependency chain: data foundation → simulation engine → extensions → bridge → UI → portfolio → optimizer. Each phase produces working, tested artifacts that the next phase builds upon. The Rust compute layer (Phases 1-5) is built first and stabilized before the Elixir orchestration layer (Phase 6) bridges them. The UI (Phase 7), portfolio (Phase 8), and optimizer (Phase 9) complete the full vision.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Data Foundation** — CSV→Parquet pipeline, ExpiryCalendar, BarStream, IvSurface, data validation
- [ ] **Phase 2: Single-Leg Engine** — CE/PE simulation, ATM strike, fixed SL/target, 20 core metrics, CLI
- [ ] **Phase 3: Multi-Leg + Advanced SL** — N-leg strategies, 7 SL types, trailing SL, OCO, strategy DSL
- [ ] **Phase 4: Re-entry + Momentum** — Re-entry state machine (4 modes), RSI/EMA/range-breakout/supertrend filters
- [ ] **Phase 5: Full Metrics Suite** — All 75+ metrics, Greeks engine, PnL attribution, walk-forward, Monte Carlo
- [ ] **Phase 6: Phoenix NIF Bridge** — Rustler NIFs, Oban workers, Postgres + DuckDB schemas, PubSub progress
- [ ] **Phase 7: LiveView UI** — Strategy builder, run manager, results viewer, optimizer dashboard, data explorer
- [ ] **Phase 8: Portfolio Engine** — Multi-strategy runner, simplified SPAN margin model, correlation matrix
- [ ] **Phase 9: Optimizer + Hardening** — Param grid sweep, heatmap visualization, profiling, edge case testing

## Phase Details

### Phase 1: Data Foundation
**Goal**: All data loaded, validated, and accessible as zero-copy bar streams in Rust. ExpiryCalendar correctly handles weekly→monthly transitions. IV surface interpolation working.
**Depends on**: Nothing (first phase)
**Requirements**: DATA-01, DATA-02, DATA-03, DATA-04, DATA-05, DATA-06
**UI hint**: no
**Success Criteria** (what must be TRUE):
  1. `scripts/csv_to_parquet.py` converts raw CSVs to partitioned Parquet files (weekly/monthly/year/month)
  2. `ExpiryCalendar` resolves correct expiry dates pre-cutoff (weekly), post-cutoff (monthly), and during transition week
  3. Full 4-year BankNifty bar load from Parquet completes in <100ms
  4. `IvSurface` interpolates IV across strike offsets and DTE via cubic spline
  5. `validate_data.py` exits 0 on clean data (6 validation checks pass)
  6. Lot size lookup returns correct values across BankNifty 15→30 and Nifty 50→75 transitions
**Plans**: 5 plans

Plans:
- [ ] 01-01: Rust workspace setup and crate structure (Cargo.toml, crate shells)
- [ ] 01-02: CSV to Parquet conversion script and config files (expiry_calendar.toml, lot_sizes.toml)
- [ ] 01-03: ExpiryCalendar and LotSizes Rust implementation with unit tests
- [ ] 01-04: BarStream memory-mapped Parquet reader and IvSurface interpolator
- [ ] 01-05: Data validation script and integration tests

### Phase 2: Single-Leg Engine
**Goal**: End-to-end backtest for one leg — CE or PE, ATM strike, fixed SL/target, 20 core metrics. CLI binary for testing.
**Depends on**: Phase 1
**Requirements**: SIM-01, SIM-02, SIM-05, MET-01 (partial: 20 core metrics), MET-03 (partial)
**UI hint**: no
**Success Criteria** (what must be TRUE):
  1. Single ATM short call, 4yr BankNifty backtest completes in <1 second
  2. SL state machine correctly transitions: Active → TrailActivated → Triggered
  3. Brokerage + STT correctly deducted from every trade
  4. All 20 core metrics match manual Excel calculation on 10-trade sample
  5. CLI `cargo run --bin backtest -- --strategy example.toml` produces correct results
  6. Equity curve snapshots captured per bar for downstream charting
**Plans**: 5 plans

Plans:
- [ ] 02-01: Strategy config parsing (TOML → StrategyConfig struct)
- [ ] 02-02: Leg, Position, and basic SL state machine implementation
- [ ] 02-03: Simulation runner (entry/update/exit loop) with execution engine (slippage, brokerage, STT)
- [ ] 02-04: Core metrics engine (20 metrics) and equity curve snapshot capture
- [ ] 02-05: CLI binary, integration tests, and performance benchmarking

### Phase 3: Multi-Leg + Advanced SL
**Goal**: Full multi-leg strategies (straddle, strangle, iron condor). All 7 SL types. Trailing SL complete state machine. OCO. Strategy DSL validation.
**Depends on**: Phase 2
**Requirements**: SIM-03, SIM-04, SIM-06, SL-01, SL-02, SL-03, SL-04, SL-05, SL-06, DSL-01, DSL-04
**UI hint**: no
**Success Criteria** (what must be TRUE):
  1. Short straddle (CE sell + PE sell) 4yr BankNifty completes in <1.5 seconds
  2. All 7 SL types trigger at correct price levels (unit tested per type)
  3. Trailing SL high-water mark never decreases once activated
  4. Combined/overall SL fires when sum of leg PnLs breaches threshold
  5. OCO correctly cancels target when SL fires on same leg (and vice versa)
  6. Exit priority chain enforced: per-leg SL → combined SL → per-leg target → overall target → time exit
  7. Delta, premium, and percent-OTM strike modes produce correct strike selection
  8. Invalid TOML strategy configs rejected with clear error messages
**Plans**: 6 plans

Plans:
- [ ] 03-01: Multi-leg Strategy struct, CombinedSlMonitor, portfolio snapshot tracking
- [ ] 03-02: All 7 SL type implementations with unit tests
- [ ] 03-03: Trailing SL (lock + trail modes) with high-water mark tracking
- [ ] 03-04: OCO logic and exit priority chain enforcement
- [ ] 03-05: Advanced strike selection modes (delta, premium, percent OTM)
- [ ] 03-06: Strategy DSL validation and error reporting

### Phase 4: Re-entry + Momentum
**Goal**: Full re-entry state machine with 4 modes. All momentum filters implemented. Range breakout pre-computation optimized.
**Depends on**: Phase 3
**Requirements**: RE-01, RE-02, RE-03, RE-04
**UI hint**: no
**Success Criteria** (what must be TRUE):
  1. Re-entry fires at correct bar after cooldown period (after_n_bars mode)
  2. max_attempts respected — no re-entry beyond limit; state transitions to Exhausted
  3. Range breakout high/low computed correctly vs manual check on 3 sample days
  4. Momentum-confirm mode does not re-enter when filter is against direction
  5. RSI, EMA cross, supertrend filters tested with known signal sequences
**Plans**: 4 plans

Plans:
- [ ] 04-01: Re-entry state machine (Idle → Cooling → WaitingForMomentum → Ready → Exhausted)
- [ ] 04-02: ASAP, same-time, and after-N-bars re-entry modes
- [ ] 04-03: Momentum filters (RSI, EMA cross, range breakout, supertrend) with pre-computation
- [ ] 04-04: Momentum-confirm re-entry mode and integration tests

### Phase 5: Full Metrics Suite
**Goal**: All 75+ metrics computed. Greeks engine with Black-Scholes. PnL attribution (Δ Γ Θ V). Walk-forward analysis. Monte Carlo simulation.
**Depends on**: Phase 3 (multi-leg results needed), Phase 1 (IV surface for Greeks)
**Requirements**: MET-01, MET-02, MET-03, MET-04, MET-06, MET-07, MET-08, GRK-01, GRK-02, GRK-03, GRK-04
**UI hint**: no
**Success Criteria** (what must be TRUE):
  1. All 75+ metrics computed from a 10k-trade result set in <200ms
  2. Black-Scholes prices match reference implementation within 0.01%
  3. Greeks PnL attribution: delta_pnl + gamma_pnl + theta_pnl + vega_pnl + unexplained ≈ actual_pnl (within 5%)
  4. Walk-forward: 12 windows produced, IS and OOS periods are strictly non-overlapping
  5. Monte Carlo: 1000 simulations, percentile bands computed, probability of positive return calculated
  6. Sharpe ratio uses 6.5% risk-free rate (India)
**Plans**: 6 plans

Plans:
- [ ] 05-01: Greeks engine — Black-Scholes pricer, delta/gamma/theta/vega computation
- [ ] 05-02: Greeks PnL attribution per trade (decomposition + unexplained residual)
- [ ] 05-03: Complete risk metrics (Sortino, Calmar, Omega, VaR, CVaR, Ulcer index)
- [ ] 05-04: Options-specific metrics (premium capture, theta collected, IV crush, DTE distribution)
- [ ] 05-05: Walk-forward analysis engine (windowed IS/OOS with degradation ratio)
- [ ] 05-06: Monte Carlo simulation engine (trade shuffle, percentile equity bands)

### Phase 6: Phoenix NIF Bridge
**Goal**: Rust callable from Elixir via Rustler. Oban job queue. Postgres + DuckDB schemas. PubSub real-time progress.
**Depends on**: Phase 5 (stable Rust API surface)
**Requirements**: NIF-01, NIF-02, NIF-03, NIF-04, NIF-05, STR-01, STR-02, STR-03, STR-04
**UI hint**: no
**Success Criteria** (what must be TRUE):
  1. NIF round-trip overhead <5ms (empty backtest call)
  2. run_backtest NIF executes on dirty CPU scheduler without blocking BEAM
  3. Postgres migrations create strategies, backtest_runs, optimizer_runs tables
  4. DuckDB tables created — trades, equity_curves, metrics, optimizer_results
  5. Oban BacktestWorker enqueues, executes NIF, stores results, broadcasts completion via PubSub
  6. DuckDB writes serialized through single GenServer (no concurrent write conflicts)
  7. PubSub broadcasts progress updates receivable by LiveView processes
**Plans**: 6 plans

Plans:
- [x] 06-01: Phoenix umbrella app scaffold and Elixir project structure
- [x] 06-02: Rustler NIF crate (run_backtest, run_optimizer, run_portfolio) with dirty_cpu scheduling
- [x] 06-03: Postgres Ecto schemas and migrations (strategies, runs)
- [x] 06-04: DuckDB schema, GenServer writer, batch insert helpers
- [x] 06-05: Oban workers (BacktestWorker, OptimizerWorker, PortfolioWorker) with PubSub integration
- [x] 06-06: Integration tests — NIF calls, Oban lifecycle, PubSub delivery

### Phase 7: LiveView UI
**Goal**: Full web UI — strategy builder, run manager, full results viewer with charts, optimizer dashboard, data explorer. All real-time via LiveView.
**Depends on**: Phase 6
**Requirements**: UI-01, UI-02, UI-03, UI-04, UI-05, UI-06, UI-07, UI-08, UI-09, UI-10, UI-11, UI-12, UI-13, UI-14, DSL-02, DSL-03
**UI hint**: yes
**Success Criteria** (what must be TRUE):
  1. User can create a multi-leg strategy via the GUI builder and save it
  2. User can run a backtest and see real-time progress bar updating via PubSub
  3. Results page shows equity curve, monthly heatmap, Greeks attribution, hero stats cards
  4. Monte Carlo fan chart and walk-forward table render correctly
  5. Optimizer dashboard accepts param grid config and displays 2D Sharpe heatmap
  6. Data explorer shows loaded symbols, date ranges, bar counts
  7. Strategy TOML import/export works round-trip (import → edit → export produces valid TOML)
  8. Portfolio builder allows capital allocation and displays correlation matrix
**Plans**: 8 plans

Plans:
- [x] 07-01: Phoenix layout, navigation, design system (CSS, components)
- [x] 07-02: Dashboard landing page with recent runs
- [x] 07-03: Strategy builder LiveView (leg editor, TOML generation, import/export)
- [x] 07-04: Run configuration and execution with real-time progress
- [x] 07-05: Results viewer — equity curve, hero stats, metrics tables
- [x] 07-06: Results viewer — monthly heatmap, Greeks attribution, Monte Carlo, walk-forward
- [x] 07-07: Data explorer LiveView
- [x] 07-08: Optimizer dashboard and portfolio builder LiveViews

### Phase 8: Portfolio Engine
**Goal**: Multiple strategies running simultaneously with shared capital, margin awareness, correlation analysis, portfolio-level metrics.
**Depends on**: Phase 5 (metrics), Phase 6 (NIF + storage)
**Requirements**: PORT-01, PORT-02, PORT-03, PORT-04, PORT-05, MET-05
**UI hint**: no
**Success Criteria** (what must be TRUE):
  1. 3-strategy portfolio backtest completes in <5 seconds (4yr data)
  2. Simplified SPAN margin correctly computed: max(3×premium×lots, index_factor×spot×lots×0.12)
  3. Trades skipped and logged as "margin_skip" when margin insufficient
  4. Portfolio Sharpe, correlation matrix, diversification benefit computed correctly
  5. Capital allocation percentages respected per strategy
**Plans**: 4 plans

Plans:
- [x] 08-01: Portfolio config (TOML) and PortfolioEngine orchestrator
- [x] 08-02: Simplified SPAN MarginModel with margin check at entry
- [x] 08-03: Combined equity curve, portfolio metrics, CorrelationMatrix
- [x] 08-04: NIF wrapper (run_portfolio), Oban worker, integration tests

### Phase 9: Optimizer + Hardening
**Goal**: Parameter sweep with Rayon parallelism, heatmap visualization, performance profiling, comprehensive edge case testing.
**Depends on**: Phase 5 (metrics), Phase 6 (NIF + storage), Phase 7 (UI for heatmap)
**Requirements**: OPT-01, OPT-02, OPT-03, OPT-04, OPT-05
**UI hint**: yes
**Success Criteria** (what must be TRUE):
  1. 720-combo optimizer sweep completes in <3 minutes (4yr BankNifty)
  2. Rayon parallel sweep uses all available CPU cores
  3. Heatmap displays Sharpe by 2 selected params with click-through to full results
  4. Edge cases pass: 0 trades, first/last bar, transition week, lot size change, zero volume, IV=0, max concurrent, re-entry exhaustion
  5. Performance profiling shows no regressions from baseline benchmarks
**Plans**: 5 plans

Plans:
- [ ] 09-01: ParamGrid TOML parser and OptimizerSweep with Rayon parallelism
- [ ] 09-02: Optimizer result storage (DuckDB) and NIF wrapper
- [ ] 09-03: Heatmap LiveView component (2D Sharpe grid, click-through)
- [ ] 09-04: Edge case test suite (9 edge cases from PRD)
- [ ] 09-05: Performance profiling, benchmark suite, regression testing

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Data Foundation | 0/5 | Not started | - |
| 2. Single-Leg Engine | 0/5 | Not started | - |
| 3. Multi-Leg + Advanced SL | 0/6 | Not started | - |
| 4. Re-entry + Momentum | 0/4 | Not started | - |
| 5. Full Metrics Suite | 0/6 | Not started | - |
| 6. Phoenix NIF Bridge | 0/6 | Not started | - |
| 7. LiveView UI | 0/8 | Not started | - |
| 8. Portfolio Engine | 0/4 | Not started | - |
| 9. Optimizer + Hardening | 0/5 | Not started | - |
