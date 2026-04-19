# Requirements: QuantEdge

**Defined:** 2026-04-19
**Core Value:** Ultra-fast, metric-complete backtesting that gives a single trader the full analytical picture — Greeks attribution, walk-forward validation, Monte Carlo confidence bands, and portfolio-level risk — without paying for subscription tools that deliver less.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Data Foundation

- [ ] **DATA-01**: CSV to Parquet conversion with weekly/monthly partitioning by symbol/year/month
- [ ] **DATA-02**: ExpiryCalendar resolves next expiry, DTE, expiry type for any (symbol, date)
- [ ] **DATA-03**: BarStream loads 4yr 1-symbol Parquet data in <100ms via memory-mapped reads
- [ ] **DATA-04**: IvSurface cubic spline interpolation across strike offset and DTE
- [ ] **DATA-05**: LotSizes config-driven lookup with historical changes (BankNifty 15→30, Nifty 50→75)
- [ ] **DATA-06**: Data validation script (6 checks: weekly cutoff, date gaps, duplicates, IV coverage ≥95%, spot continuity, lot size coverage)

### Simulation Engine

- [ ] **SIM-01**: Single-leg CE/PE backtest with ATM strike selection
- [ ] **SIM-02**: Entry at configured time, exit at configured time or on SL/target
- [ ] **SIM-03**: Multi-leg strategy support (N legs, combined PnL tracking)
- [ ] **SIM-04**: 4 strike selection modes: ATM offset, delta, premium, percent OTM
- [ ] **SIM-05**: Full 4yr single strategy backtest completes in <1 second
- [ ] **SIM-06**: Short straddle (2 legs) 4yr backtest completes in <1.5 seconds

### Stop Loss & Targets

- [ ] **SL-01**: 7 SL types (points, %premium, %margin, index points, delta breach, combined premium, none)
- [ ] **SL-02**: Per-leg and overall/combined SL with strict priority ordering
- [ ] **SL-03**: Trailing SL with lock and trail modes (high-water mark never decreases once activated)
- [ ] **SL-04**: Per-leg and overall targets with percent_of_premium type
- [ ] **SL-05**: OCO (one-cancels-other) between SL and target at leg level
- [ ] **SL-06**: Exit priority: per-leg SL → combined SL → per-leg target → overall target → time exit

### Re-entry & Momentum

- [ ] **RE-01**: Re-entry state machine with 4 modes (ASAP, same-time, after-N-bars, momentum-confirm)
- [ ] **RE-02**: Configurable max re-entry attempts per leg (no re-entry beyond limit)
- [ ] **RE-03**: Momentum filters: RSI, EMA cross, range breakout, supertrend
- [ ] **RE-04**: Range breakout pre-computed in O(N) setup, O(1) per bar lookup

### Metrics

- [ ] **MET-01**: 14 return metrics (total_pnl_gross, total_pnl_net, cagr, roi_pct, expectancy, profit_factor, win_rate_pct, avg_win, avg_loss, win_loss_ratio, largest_win, largest_loss, gross_profit, gross_loss)
- [ ] **MET-02**: 16 risk metrics (max_drawdown_inr, max_drawdown_pct, avg_drawdown, sharpe_ratio, sortino_ratio, calmar_ratio, omega_ratio, var_95, var_99, cvar, ulcer_index, daily_volatility, ann_volatility, skewness, kurtosis, recovery_factor)
- [ ] **MET-03**: 14 trade analytics (total_trades, avg_hold_bars, max_hold_bars, max_consec_wins, max_consec_losses, sl_hit_rate_pct, target_hit_rate_pct, time_exit_rate_pct, reentry_count, reentry_win_rate, total_brokerage, total_slippage, total_stt, net_cost_ratio)
- [ ] **MET-04**: 14 options-specific metrics (premium_capture_pct, total_theta_collected, avg_theta_per_day, avg_iv_at_entry, avg_iv_at_exit, iv_crush_pct, delta_pnl, gamma_pnl, theta_pnl, vega_pnl, avg_net_delta, dte_distribution, breakeven_range, max_profit_theoretical)
- [ ] **MET-05**: 7 portfolio metrics (strategy_correlation, portfolio_sharpe, peak_margin_used, capital_efficiency, net_portfolio_greeks, avg_concurrent_trades, diversification_benefit)
- [ ] **MET-06**: 12 time-based analytics (monthly_pnl_heatmap, day_of_week_pnl, expiry_day_performance, best_month, worst_month, pct_profitable_months, pct_profitable_weeks, walk_forward_results, monte_carlo_bands, rolling_sharpe_12m, equity_curve, drawdown_curve)
- [ ] **MET-07**: Walk-forward analysis: 12 windows (6mo IS / 2mo OOS), IS Sharpe, OOS Sharpe, degradation ratio per window
- [ ] **MET-08**: Monte Carlo: 1000 simulations, shuffled trade sequence, 5/25/50/75/95 percentile equity bands, probability of positive return

### Greeks

- [ ] **GRK-01**: Black-Scholes option pricing for CE/PE
- [ ] **GRK-02**: Greeks computation (delta, gamma, theta, vega) per leg at entry and exit
- [ ] **GRK-03**: Greeks PnL attribution per trade: delta_pnl + gamma_pnl + theta_pnl + vega_pnl + unexplained
- [ ] **GRK-04**: Sharpe/Sortino use 6.5% risk-free rate (India)

### NIF Bridge

- [ ] **NIF-01**: Rustler NIF for run_backtest (dirty CPU scheduler, JSON in → JSON out)
- [ ] **NIF-02**: Rustler NIF for run_optimizer (parallel grid sweep via Rayon)
- [ ] **NIF-03**: Rustler NIF for run_portfolio (multi-strategy simultaneous)
- [ ] **NIF-04**: NIF call overhead <5ms (empty backtest round-trip)
- [ ] **NIF-05**: PubSub progress broadcasting during long-running NIF operations

### Storage

- [ ] **STR-01**: Postgres tables: strategies (UUID, name, underlying, config_toml), backtest_runs (UUID, strategy_id, status, dates, capital, result_summary JSONB), optimizer_runs (UUID, strategy_id, param_grid JSONB, status, combo counts)
- [ ] **STR-02**: DuckDB tables: trades (run_id, trade details, legs JSON, PnL, Greeks), equity_curves (run_id, date, equity, drawdown), metrics (run_id, metric_name, metric_value), optimizer_results (optimizer_run_id, combo_index, params, metrics)
- [ ] **STR-03**: Oban job queue: BacktestWorker, OptimizerWorker, PortfolioWorker with PubSub status updates
- [ ] **STR-04**: Single GenServer writer for DuckDB to prevent concurrent write conflicts

### Web UI

- [ ] **UI-01**: Strategy builder LiveView (create/edit multi-leg strategies via interactive GUI)
- [ ] **UI-02**: Strategy list with CRUD operations and TOML import/export
- [ ] **UI-03**: Run configuration (date range, capital, brokerage, slippage settings)
- [ ] **UI-04**: Results viewer with equity curve chart (cumulative PnL + drawdown band)
- [ ] **UI-05**: Monthly PnL heatmap (rows=months, cols=years, color=PnL green/red)
- [ ] **UI-06**: Greeks attribution chart (stacked bar: delta/gamma/theta/vega PnL)
- [ ] **UI-07**: Monte Carlo fan chart (1000 curves, 5/25/50/75/95 percentile bands)
- [ ] **UI-08**: Walk-forward results table (IS Sharpe, OOS Sharpe, degradation per window)
- [ ] **UI-09**: Hero stats cards (Total PnL, CAGR, Win Rate, Max DD, Sharpe, PF, Trades, Premium Capture %)
- [ ] **UI-10**: Real-time progress bar during backtest execution via PubSub
- [ ] **UI-11**: Data explorer showing loaded symbols, date ranges, bar counts, IV coverage
- [ ] **UI-12**: Optimizer dashboard with param grid config and 2D heatmap (click cell → full results)
- [ ] **UI-13**: Portfolio builder with capital allocation sliders and correlation matrix display
- [ ] **UI-14**: Dashboard landing page with recent runs and quick-launch

### Portfolio

- [ ] **PORT-01**: Multi-strategy portfolio backtest with capital allocation percentages (TOML config)
- [ ] **PORT-02**: Simplified SPAN margin model (max(3×premium×lots, index_factor×spot×lots×0.12))
- [ ] **PORT-03**: Margin check before entry — skip trade and log as "margin_skip" if insufficient
- [ ] **PORT-04**: Portfolio Sharpe, correlation matrix, diversification benefit, capital efficiency
- [ ] **PORT-05**: 3-strategy portfolio 4yr backtest completes in <5 seconds

### Optimizer

- [ ] **OPT-01**: Parameter grid sweep with TOML config defining param ranges
- [ ] **OPT-02**: Rayon parallel execution — each combo independent, work-stealing across CPU cores
- [ ] **OPT-03**: 720-combo sweep (6×6×5×4 grid) completes in <3 minutes for 4yr data
- [ ] **OPT-04**: Heatmap visualization (X/Y axes = 2 params, cell color = Sharpe, grey = <20 trades)
- [ ] **OPT-05**: Edge case hardening: 0 trades, first/last bar entry, transition week Oct-Nov 2024, lot size change mid-trade, zero volume bar, IV=0, max concurrent limit, re-entry exhaustion

### Strategy DSL

- [ ] **DSL-01**: TOML strategy definition with [strategy], [[legs]], [overall] sections
- [ ] **DSL-02**: GUI builder generates valid TOML and parses TOML back to GUI state
- [ ] **DSL-03**: TOML import/export for version control and strategy sharing
- [ ] **DSL-04**: Strategy validation: reject invalid configs with clear error messages

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Advanced Strategies

- **ADV-01**: Calendar spread support (cross-expiry bar stitching)
- **ADV-02**: Ratio spreads (unequal lot sizes across legs)
- **ADV-03**: Intraday position scaling (add/remove lots during trade)

### Data & Analytics

- **ANA-01**: Full NSE SPAN margin file parsing for exact margin calculations
- **ANA-02**: Market regime detection (automated ML-based classification)
- **ANA-03**: IV regime performance breakdown (segment by IV percentile)
- **ANA-04**: Automated data update pipeline (scheduled CSV fetch + conversion)

### Platform

- **PLT-01**: Strategy sharing/export format for community
- **PLT-02**: Live options chain visualization
- **PLT-03**: Paper trading integration
- **PLT-04**: Multi-user support with authentication

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Live trading execution | Fundamentally different system requirements; keep backtesting pure |
| Cloud deployment | Personal tool; runs locally on own machine |
| Real-time market data feed | Requires exchange subscription; use broker terminal for live data |
| Mobile app | Desktop browser is primary interface; responsive web sufficient |
| Multi-user/team features | Single-user personal tool |
| Calendar spreads (v1) | Cross-expiry bar stitching adds enormous complexity; defer to v2 |
| Full SPAN file parsing (v1) | Simplified model sufficient for personal margin awareness |
| Options chain visualization | Would require live data feed; out of scope for backtesting tool |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DATA-01 | Phase 1 | Pending |
| DATA-02 | Phase 1 | Pending |
| DATA-03 | Phase 1 | Pending |
| DATA-04 | Phase 1 | Pending |
| DATA-05 | Phase 1 | Pending |
| DATA-06 | Phase 1 | Pending |
| SIM-01 | Phase 2 | Pending |
| SIM-02 | Phase 2 | Pending |
| SIM-03 | Phase 3 | Pending |
| SIM-04 | Phase 3 | Pending |
| SIM-05 | Phase 2 | Pending |
| SIM-06 | Phase 3 | Pending |
| SL-01 | Phase 3 | Pending |
| SL-02 | Phase 3 | Pending |
| SL-03 | Phase 3 | Pending |
| SL-04 | Phase 3 | Pending |
| SL-05 | Phase 3 | Pending |
| SL-06 | Phase 3 | Pending |
| RE-01 | Phase 4 | Pending |
| RE-02 | Phase 4 | Pending |
| RE-03 | Phase 4 | Pending |
| RE-04 | Phase 4 | Pending |
| MET-01 | Phase 2+5 | Pending |
| MET-02 | Phase 5 | Pending |
| MET-03 | Phase 2+5 | Pending |
| MET-04 | Phase 5 | Pending |
| MET-05 | Phase 8 | Pending |
| MET-06 | Phase 5 | Pending |
| MET-07 | Phase 5 | Pending |
| MET-08 | Phase 5 | Pending |
| GRK-01 | Phase 5 | Pending |
| GRK-02 | Phase 5 | Pending |
| GRK-03 | Phase 5 | Pending |
| GRK-04 | Phase 5 | Pending |
| NIF-01 | Phase 6 | Pending |
| NIF-02 | Phase 6 | Pending |
| NIF-03 | Phase 6 | Pending |
| NIF-04 | Phase 6 | Pending |
| NIF-05 | Phase 6 | Pending |
| STR-01 | Phase 6 | Pending |
| STR-02 | Phase 6 | Pending |
| STR-03 | Phase 6 | Pending |
| STR-04 | Phase 6 | Pending |
| UI-01 | Phase 7 | Pending |
| UI-02 | Phase 7 | Pending |
| UI-03 | Phase 7 | Pending |
| UI-04 | Phase 7 | Pending |
| UI-05 | Phase 7 | Pending |
| UI-06 | Phase 7 | Pending |
| UI-07 | Phase 7 | Pending |
| UI-08 | Phase 7 | Pending |
| UI-09 | Phase 7 | Pending |
| UI-10 | Phase 7 | Pending |
| UI-11 | Phase 7 | Pending |
| UI-12 | Phase 7 | Pending |
| UI-13 | Phase 7 | Pending |
| UI-14 | Phase 7 | Pending |
| PORT-01 | Phase 8 | Pending |
| PORT-02 | Phase 8 | Pending |
| PORT-03 | Phase 8 | Pending |
| PORT-04 | Phase 8 | Pending |
| PORT-05 | Phase 8 | Pending |
| OPT-01 | Phase 9 | Pending |
| OPT-02 | Phase 9 | Pending |
| OPT-03 | Phase 9 | Pending |
| OPT-04 | Phase 9 | Pending |
| OPT-05 | Phase 9 | Pending |
| DSL-01 | Phase 3 | Pending |
| DSL-02 | Phase 7 | Pending |
| DSL-03 | Phase 7 | Pending |
| DSL-04 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 67 total
- Mapped to phases: 67
- Unmapped: 0 ✓

---
*Requirements defined: 2026-04-19*
*Last updated: 2026-04-19 after initial definition*
