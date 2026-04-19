# Project Research Summary

**Project:** QuantEdge
**Domain:** FNO Options Backtesting Platform (Indian Markets)
**Researched:** 2026-04-19
**Confidence:** HIGH

## Executive Summary

QuantEdge is a personal-grade FNO options backtesting platform targeting Indian markets (BankNifty, Nifty, Sensex). The Rust + Phoenix/Elixir architecture is well-validated for this domain — Rust provides the 10-50× performance advantage needed for sub-second backtesting, while Elixir/Phoenix handles orchestration, real-time UI, and job management. This is commonly called the "Endurance Stack" in the trading systems community.

The recommended approach is a clean 4-layer architecture: Presentation (LiveView) → Orchestration (Elixir/Oban/PubSub) → Compute (Rust crates) → Storage (Postgres + DuckDB + Parquet). The critical boundary is the Rustler NIF bridge — all simulation math stays in Rust, Elixir handles I/O and persistence only. This sharp boundary prevents the most common architectural mistake in hybrid systems.

Key risks are domain-specific: look-ahead bias in simulation, incorrect expiry transition handling (BankNifty weekly→monthly 2024-11-01), SL/target timing ambiguity within bars, and DuckDB concurrent write conflicts. All are preventable with the mitigations identified in research.

## Key Findings

### Recommended Stack

**Core technologies:**
- Rust 1.82+ (stable): Simulation kernel — sub-second 4yr backtests via zero-cost abstractions and cache-friendly inner loops
- Elixir 1.17+ / OTP 27+: Orchestration layer — BEAM concurrency for real-time progress, fault tolerance, background jobs
- Phoenix 1.7+ / LiveView 1.0+: Real-time web UI — eliminates separate JS frontend, native PubSub integration
- Postgres 16+: Transactional storage — strategies, runs, Oban job queue backing
- DuckDB 0.10+: Analytical queries — 10-50× faster than Postgres for time-series aggregations
- Parquet: Bar data storage — columnar, zero-copy memory-mapped reads

**Key library versions verified:** polars 0.53, rustler crate 0.37, rustler hex 0.35, oban 2.17, duckdbex 0.4, phoenix_live_view 1.0.

### Expected Features

**Must have (table stakes):**
- Single and multi-leg strategy backtesting with configurable SL/target
- Expiry calendar handling (weekly/monthly transitions)
- Brokerage + STT + slippage modeling (India-specific costs)
- Basic metrics (PnL, win rate, drawdown, Sharpe)
- Equity curve visualization
- Strategy save/load persistence

**Should have (competitive — QuantEdge differentiators):**
- Sub-second backtest speed (<1s for 4yr single strategy)
- All 75+ metrics including Greeks PnL attribution
- Walk-forward analysis and Monte Carlo simulation
- 7 SL types + trailing SL state machine
- Re-entry with 4 modes + momentum filters
- Parameter optimizer with grid sweep (1000+ combos in <3min)
- Portfolio backtesting with margin awareness

**Defer (v2+):**
- Calendar spread support (cross-expiry complexity)
- Full NSE SPAN margin parsing
- Live trading integration

### Architecture Approach

Four-layer system with clean boundaries: Presentation (LiveView renders server-side HTML via WebSocket), Orchestration (Oban workers invoke NIFs, PubSub broadcasts progress), Compute (Rust workspace with 7 crates — data, core, greeks, metrics, portfolio, optimizer, nif), Storage (three-tier: Postgres for CRUD, DuckDB for analytics, Parquet for bar data).

**Major components:**
1. quantedge-core — Simulation engine: Leg, Strategy, SL state machine, re-entry, runner
2. quantedge-data — Data layer: BarStream, ExpiryCalendar, IvSurface, memory-mapped Parquet
3. quantedge-metrics — Analytics: 75+ metrics, walk-forward, Monte Carlo
4. quantedge-nif — NIF bridge: JSON gateway between Elixir and Rust (dirty CPU scheduler)
5. quantedge_web — Phoenix app: LiveView UI, Oban workers, Ecto contexts

### Critical Pitfalls

1. **Look-ahead bias** — Use t-1 data for signals, t open for entry; unit test with known edge cases (Phase 2)
2. **Expiry calendar errors** — ExpiryCalendar as single source of truth; test transition weeks explicitly (Phase 1)
3. **SL/target timing within bar** — Strict priority order; worst-case assumption for ambiguous bars (Phase 2)
4. **NIF memory lifecycle** — Load data within NIF call; never return references to mmap'd data (Phase 6)
5. **Overfitting via optimizer** — Walk-forward OOS/IS ratio >0.5; Monte Carlo confidence bands (Phase 5+9)

## Implications for Roadmap

Based on research, the PRD's 9-phase structure aligns well with dependency analysis:

### Phase 1: Data Foundation
**Rationale:** Everything depends on correct data loading and expiry handling — must be first
**Delivers:** CSV→Parquet pipeline, ExpiryCalendar, BarStream, IvSurface, data validation
**Avoids:** Pitfall 2 (expiry calendar), Pitfall 8 (lot size changes)

### Phase 2: Single-Leg Engine
**Rationale:** Core simulation loop establishes patterns used by all subsequent phases
**Delivers:** Leg, Position, SL state machine, basic metrics (20), CLI binary
**Avoids:** Pitfall 1 (look-ahead bias), Pitfall 3 (SL timing), Pitfall 4 (premium %)

### Phase 3: Multi-Leg + Advanced SL
**Rationale:** Depends on single-leg; extends to N-leg strategies
**Delivers:** Strategy (N legs), all 7 SL types, trailing SL, OCO, combined SL
**Uses:** Core simulation patterns from Phase 2

### Phase 4: Re-entry + Momentum
**Rationale:** Depends on SL triggers from Phase 3
**Delivers:** Re-entry state machine (4 modes), momentum filters (RSI, EMA, range breakout, supertrend)

### Phase 5: Full Metrics Suite
**Rationale:** Depends on multi-leg engine + Greeks; enables analytical differentiators
**Delivers:** All 75+ metrics, Greeks PnL attribution, walk-forward, Monte Carlo
**Avoids:** Pitfall 7 (overfitting — walk-forward/Monte Carlo as countermeasure)

### Phase 6: Phoenix NIF Bridge
**Rationale:** Rust API must be stable before bridging; depends on P2-P5
**Delivers:** Rustler NIFs, Oban workers, Postgres + DuckDB schemas, PubSub
**Avoids:** Pitfall 5 (NIF memory), Pitfall 6 (DuckDB concurrency)

### Phase 7: LiveView UI
**Rationale:** Depends on NIF bridge and DB schemas
**Delivers:** Strategy builder, run manager, results viewer, optimizer dashboard, data explorer

### Phase 8: Portfolio Engine
**Rationale:** Depends on multi-leg engine and full metrics
**Delivers:** Multi-strategy runner, margin model, correlation matrix

### Phase 9: Optimizer + Hardening
**Rationale:** Final phase; depends on all compute + UI
**Delivers:** Param sweep, heatmap, performance profiling, edge case testing

### Phase Ordering Rationale

- Data → Core → Extensions → Bridge → UI is the natural dependency chain
- Greeks engine must precede full metrics (needed for PnL attribution)
- NIF bridge must come after Rust API stabilizes (P2-P5) to avoid churn
- UI and Portfolio/Optimizer can be somewhat parallel but are ordered for clean dependencies

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1:** Parquet partitioning strategy for weekly/monthly split; Polars memory-mapping API
- **Phase 5:** Black-Scholes implementation details; SIMD vectorization for Greeks
- **Phase 6:** Rustler dirty scheduler patterns; DuckDB batch insert optimization

Phases with standard patterns (skip research-phase):
- **Phase 2:** Standard simulation loop; well-documented Rust patterns
- **Phase 7:** Standard Phoenix LiveView; well-documented framework

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions verified on crates.io/hex.pm; Rust+Elixir well-proven for trading |
| Features | HIGH | Comprehensive PRD exists; competitor analysis clear |
| Architecture | HIGH | Clean 4-layer design; NIF boundary well-understood |
| Pitfalls | HIGH | Domain-specific pitfalls well-documented in trading community |

**Overall confidence:** HIGH

### Gaps to Address

- DuckDB concurrent write pattern needs prototyping during Phase 6 planning
- Polars memory-mapped Parquet API specifics may have changed in 0.53 — verify during Phase 1 planning
- SIMD availability for Greeks batch computation depends on target CPU — verify during Phase 5

## Sources

### Primary (HIGH confidence)
- crates.io — polars 0.53, rustler 0.37, rayon 1.10, statrs 0.17
- hex.pm — phoenix_live_view 1.0, oban 2.17, duckdbex 0.4
- Elixir Forum — Rust+Elixir "Endurance Stack" architecture

### Secondary (MEDIUM confidence)
- Trading community forums — backtesting bias prevention strategies
- Polars documentation — Parquet I/O and lazy evaluation patterns

### Tertiary (LOW confidence)
- SIMD availability for ndarray operations — needs runtime verification

---
*Research completed: 2026-04-19*
*Ready for roadmap: yes*
