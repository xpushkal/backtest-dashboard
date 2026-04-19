# QuantEdge

## What This Is

A personal-grade FNO options backtesting platform for Indian markets (BankNifty, Nifty, Sensex). Built on a Rust simulation kernel for sub-second performance and Phoenix/Elixir for orchestration and real-time web UI. Replaces AlgoTest/Stockmock with full metric coverage (75+), multi-leg strategy support, portfolio backtesting, and parameter optimization — all running locally against 4+ years of 1-minute OHLCV, OI, and IV data.

## Core Value

Ultra-fast, metric-complete backtesting that gives a single trader the full analytical picture — Greeks attribution, walk-forward validation, Monte Carlo confidence bands, and portfolio-level risk — without paying for subscription tools that deliver less.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

(None yet — ship to validate)

### Active

<!-- Current scope. Building toward these. -->

- [ ] CSV to Parquet data pipeline with weekly/monthly partitioning and expiry calendar
- [ ] Zero-copy memory-mapped Parquet reads for sub-100ms data loading (4yr, 1 symbol)
- [ ] IV surface cubic spline interpolation across strike/DTE
- [ ] Single-leg CE/PE simulation with ATM strike selection and fixed SL/target
- [ ] Multi-leg strategy support (straddle, strangle, iron condor, custom N-leg)
- [ ] Full SL state machine: 7 SL types + trailing SL (lock/trail modes) + OCO
- [ ] Re-entry state machine: 4 modes (ASAP, same-time, after-N-bars, momentum-confirm)
- [ ] Momentum filters: RSI, EMA cross, range breakout, supertrend
- [ ] All 75+ metrics: return, risk, trade analytics, options-specific, portfolio, time-based
- [ ] Greeks PnL attribution per trade (delta, gamma, theta, vega, unexplained)
- [ ] Walk-forward analysis (12 windows, 6mo IS / 2mo OOS)
- [ ] Monte Carlo simulation (1,000 shuffled equity curves, percentile bands)
- [ ] Strategy DSL in TOML + GUI builder in LiveView
- [ ] Rustler NIF bridge (dirty CPU scheduler, async)
- [ ] Oban background job queue for backtest/optimizer/portfolio runs
- [ ] Postgres for strategies/runs metadata + DuckDB for trades/equity/metrics analytics
- [ ] Phoenix LiveView UI: strategy builder, run manager, results viewer, optimizer dashboard, data explorer
- [ ] Real-time PubSub progress updates during backtest execution
- [ ] Multi-strategy portfolio engine with capital allocation, simplified SPAN margin model, correlation matrix
- [ ] Parameter optimizer with grid sweep (Rayon parallel), heatmap visualization
- [ ] Sub-1-second single strategy 4yr backtest, <3min for 1,000-combo optimizer sweep

### Out of Scope

<!-- Explicit boundaries. Includes reasoning to prevent re-adding. -->

- Calendar spread support — requires cross-expiry bar stitching, high complexity, V2 feature
- Full NSE SPAN file parsing — simplified SPAN approximation is sufficient for personal margin awareness
- OAuth/social login — personal tool, no multi-user auth needed
- Mobile app — web-first, desktop browser is the primary interface
- Real-time live trading integration — this is a backtesting tool, not an execution system
- Cloud deployment — runs locally on personal machine
- Multi-user/team features — single-user tool

## Context

- **Data**: 4+ years of 1-minute OHLCV, OI, and IV data for BankNifty, Nifty, Sensex already available as CSVs
- **Expiry transitions**: BankNifty weekly→monthly (2024-11-01), Nifty (2024-10-03), Sensex (2024-09-18) — must be handled correctly
- **Lot size changes**: BankNifty 15→30 (2024-11-20), Nifty 50→75 (2024-07-25) — config-driven lookup
- **Performance baseline**: AlgoTest takes 10-30s for a single 4yr backtest; target is <1s
- **Risk-free rate**: 6.5% (India) used for Sharpe/Sortino calculations
- **Detailed PRD**: Complete phase-by-phase implementation reference exists at `Docs/PRD.md` with Rust struct definitions, SQL schemas, and acceptance criteria

## Constraints

- **Tech stack**: Rust (simulation kernel) + Phoenix/Elixir (web + orchestration) — non-negotiable
- **Storage**: Postgres (transactional) + DuckDB (analytical) + Parquet (bar data) — three-tier storage
- **Performance**: Single strategy 4yr <1s, 10-strategy portfolio <5s, 1000-combo optimizer <3min
- **Data format**: CSV schema is fixed (timestamp, date, time, weekday, option_type, strike_label, strike_offset, moneyness, OHLCV, strike, oi, spot, iv)
- **Single user**: No authentication, no multi-tenancy, no deployment concerns

## Key Decisions

<!-- Decisions that constrain future work. Add throughout project lifecycle. -->

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust for simulation kernel | 10-50× faster than Python; sub-second on 4yr backtest | — Pending |
| Phoenix LiveView for UI | Real-time PubSub progress; Elixir concurrency for job orchestration | — Pending |
| DuckDB alongside Postgres | Columnar engine for time-series analytics (monthly heatmaps, equity curves) is 10-50× faster than Postgres | — Pending |
| Parquet with memory-mapped reads | Zero-copy deserialization; <100ms for 4yr single symbol load | — Pending |
| TOML for strategy DSL | Human-readable, well-structured for nested leg configs | — Pending |
| Simplified SPAN margin model | 3× premium + index factor; directionally accurate, avoids NSE SPAN file complexity | — Pending |
| All simulation logic in Rust | Clean boundary: Rust returns data, Elixir orchestrates + persists | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-19 after initialization*
