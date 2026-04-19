# Feature Research

**Domain:** FNO Options Backtesting Platform (Indian Markets)
**Researched:** 2026-04-19
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features any options backtesting platform must have. Missing these = unusable.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Single-leg CE/PE backtest | Fundamental capability; can't test any strategy without it | MEDIUM | ATM strike selection, basic entry/exit logic |
| Multi-leg strategies (straddle, strangle, iron condor) | 90%+ of FNO strategies are multi-leg | HIGH | Combined PnL, per-leg and overall SL, leg correlation |
| Stop loss (fixed types) | Risk management is non-negotiable | MEDIUM | Points, percent of premium, max loss — minimum 3 types |
| Date range selection | Must test across specific periods | LOW | User selects start/end dates |
| Brokerage + STT + slippage modeling | Results without costs are meaningless | MEDIUM | India-specific: STT on sell, GST on brokerage, stamp duty |
| Basic metrics (PnL, win rate, drawdown, Sharpe) | Minimum viable analytics | MEDIUM | ~20 core metrics expected by any serious trader |
| Equity curve visualization | Visual confirmation of strategy performance | LOW | Line chart with drawdown shading |
| Strategy save/load | Can't iterate without persistence | LOW | CRUD operations on strategy configs |
| Expiry handling (weekly/monthly) | India-specific: BankNifty/Nifty/Sensex transitions | HIGH | Must handle 2024 weekly→monthly transitions correctly |
| Lot size awareness | Position sizing must reflect NSE lot sizes | LOW | Config-driven lookup with historical changes |

### Differentiators (Competitive Advantage)

Features that set QuantEdge apart from AlgoTest/Stockmock.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Sub-second backtest speed (4yr) | Iterate 30-60× faster than competitors | HIGH | Rust kernel; zero-copy Parquet; single-threaded cache-optimized inner loop |
| All 75+ metrics | Complete analytical picture vs competitors' 15-25 | HIGH | Return, risk, trade, options, portfolio, time-based categories |
| Greeks PnL attribution (Δ Γ Θ V) | Understand WHY a strategy made/lost money | HIGH | Per-trade delta/gamma/theta/vega decomposition + unexplained residual |
| Walk-forward analysis | Out-of-sample validation prevents overfitting | MEDIUM | 12 windows (6mo IS / 2mo OOS); degradation ratio |
| Monte Carlo simulation | Confidence bands on equity curves; probability of ruin | MEDIUM | 1000 shuffled equity curves; 5/25/50/75/95 percentile |
| Trailing SL state machine | Sophisticated risk management (lock-in / trail modes) | HIGH | High-water mark tracking; activation threshold; never-decrease guarantee |
| 7 SL types | Covers every risk management approach | HIGH | Points, %premium, %margin, index points, delta breach, combined premium, none |
| Re-entry with 4 modes | Most competitors don't support re-entry at all | HIGH | ASAP, same-time, after-N-bars, momentum-confirm; max attempts |
| Momentum filters (RSI, EMA, range breakout, supertrend) | Entry/re-entry conditional on market state | MEDIUM | Pre-computed range high/low for O(1) lookup |
| Parameter optimizer with grid sweep | Systematic exploration of parameter space | HIGH | Rayon parallel; 1000+ combos in <3 minutes |
| Portfolio backtesting | Multiple strategies running together with shared capital | HIGH | Capital allocation, margin awareness, correlation matrix |
| TOML strategy DSL + GUI builder | Define strategies programmatically or visually | MEDIUM | Import/export; version control friendly |
| Optimizer heatmap visualization | Visual pattern recognition across param combinations | MEDIUM | 2D color grid; click-through to full results |
| Monthly PnL heatmap | At-a-glance performance calendar | LOW | Year×Month grid, color-coded |
| IV regime performance breakdown | Understand how strategy performs in different vol environments | MEDIUM | Segment trades by entry IV percentile |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Calendar spreads | Some traders want inter-expiry strategies | Cross-expiry bar stitching is enormously complex; data alignment issues; different expiry Greeks | Defer to V2; focus on same-expiry multi-leg |
| Full SPAN margin parsing | "Accurate" margin calculations | NSE SPAN files are complex, format changes frequently, adds significant maintenance burden | Simplified SPAN (3× premium + index factor) — directionally accurate |
| Live trading integration | "Backtest to live in one click" | Completely different system requirements; order management, risk checks, exchange connectivity | Keep backtesting pure; separate live system can import strategy configs |
| Unlimited historical data | "More data = better" | Diminishing returns past 4-5 years; market regime shifts make old data misleading | Focus on quality 4yr data with proper regime tagging |
| Real-time options chain | "See live prices alongside backtest" | Requires exchange data feed subscription; latency requirements unlike backtesting | Out of scope; use broker terminal for live data |

## Feature Dependencies

```
[Data Foundation (P1)]
    └──requires──> [Expiry Calendar + Lot Sizes]
                       └──enables──> [Single-Leg Engine (P2)]
                                         └──enables──> [Multi-Leg + SL (P3)]
                                                           └──enables──> [Re-entry + Momentum (P4)]

[Greeks Engine (P5)]
    └──requires──> [Single-Leg Engine (P2)]
    └──requires──> [IV Surface from Data Foundation (P1)]
    └──enhances──> [SL State Machine (P3)] via delta-breach SL

[Full Metrics (P5)]
    └──requires──> [Multi-Leg Engine (P3)]
    └──requires──> [Greeks Engine (P5)]

[NIF Bridge (P6)]
    └──requires──> [Rust Core (P2-P5)]

[LiveView UI (P7)]
    └──requires──> [NIF Bridge (P6)]
    └──requires──> [Postgres + DuckDB schemas (P6)]

[Portfolio Engine (P8)]
    └──requires──> [Multi-Leg Engine (P3)]
    └──requires──> [Full Metrics (P5)]

[Optimizer (P9)]
    └──requires──> [Full Metrics (P5)]
    └──requires──> [NIF Bridge (P6)]
    └──enhances──> [LiveView UI (P7)] via optimizer dashboard
```

### Dependency Notes

- **Greeks PnL requires IV Surface:** Delta/gamma/theta/vega calculation needs the IV surface from P1 data foundation
- **Delta-breach SL enhances P3:** Stubbed in P3 (returns false), goes live when Greeks engine lands in P5
- **Portfolio requires correlation matrix:** Which requires equity curves from individual strategy runs (P5 metrics)
- **NIF bridge must wrap all of P2-P5:** Can't do P6 until the Rust API is stable

## MVP Definition

### Launch With (v1 — all 9 phases)

- [ ] Data pipeline (CSV→Parquet, expiry calendar, IV surface) — foundation for everything
- [ ] Full simulation engine (single-leg, multi-leg, SL state machine, trailing, re-entry, momentum) — core backtesting
- [ ] All 75+ metrics including Greeks attribution, walk-forward, Monte Carlo — analytical depth
- [ ] Phoenix LiveView UI (strategy builder, run manager, results viewer) — usable interface
- [ ] Portfolio engine with margin model — multi-strategy awareness
- [ ] Optimizer with grid sweep and heatmap — parameter exploration

### Future Consideration (v2+)

- [ ] Calendar spread support — when cross-expiry data stitching is feasible
- [ ] Full NSE SPAN margin parsing — when simplified model proves insufficient
- [ ] Market regime detection (automated) — ML-based regime classification
- [ ] Strategy sharing/export format — if others adopt the platform
- [ ] Option chain visualization — live data feed integration

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Sub-second backtest | HIGH | HIGH | P1 |
| Multi-leg strategies | HIGH | HIGH | P1 |
| 75+ metrics | HIGH | HIGH | P1 |
| Greeks PnL attribution | HIGH | HIGH | P1 |
| Walk-forward / Monte Carlo | HIGH | MEDIUM | P1 |
| Trailing SL state machine | HIGH | HIGH | P1 |
| Re-entry with momentum | MEDIUM | HIGH | P1 |
| Parameter optimizer | HIGH | MEDIUM | P1 |
| Portfolio backtesting | HIGH | HIGH | P1 |
| LiveView UI | HIGH | HIGH | P1 |
| Optimizer heatmap | MEDIUM | LOW | P1 |
| Calendar spreads | MEDIUM | HIGH | P3 |
| Full SPAN margin | LOW | HIGH | P3 |

## Competitor Feature Analysis

| Feature | AlgoTest | Stockmock | QuantEdge |
|---------|----------|-----------|-----------|
| Speed (4yr backtest) | 10-30s | 15-45s | **<1s** |
| Metrics count | ~15-20 | ~20-25 | **75+** |
| Greeks PnL attribution | No | No | **Yes (Δ Γ Θ V)** |
| Walk-forward analysis | No | No | **Yes (12 windows)** |
| Monte Carlo | No | No | **Yes (1000 sims)** |
| Portfolio backtesting | No | No | **Yes** |
| Optimizer sweep | No | Limited | **Yes (1000+ combos)** |
| Per-leg trailing SL | Limited | Limited | **Full state machine** |
| Re-entry modes | Basic | No | **4 modes** |
| Strategy DSL | GUI only | GUI only | **TOML + GUI** |
| Self-hosted / no subscription | No | No | **Yes** |

## Sources

- AlgoTest feature analysis — algotest.in product pages
- Stockmock feature analysis — stockmock.com product pages
- NSE options data requirements — nseindia.com documentation
- Options backtesting best practices — QuantConnect, Tastytrade, CBOE research
- Indian FNO trading community feedback — TradingQ&A, ValuePickr forums

---
*Feature research for: FNO Options Backtesting Platform*
*Researched: 2026-04-19*
