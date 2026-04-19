# Pitfalls Research

**Domain:** FNO Options Backtesting Platform (Indian Markets)
**Researched:** 2026-04-19
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: Look-Ahead Bias in Simulation

**What goes wrong:**
Backtest uses data not available at the decision point — e.g., using the closing price of a candle to trigger an entry on that same candle, or using future IV to compute Greeks.

**Why it happens:**
When iterating over bars, it's easy to accidentally use `bar[i].close` to make a decision that should have been made with `bar[i-1]` data. Also happens when computing indicators that include current bar.

**How to avoid:**
- Entry signals use only data from bars before the current bar (t-1 or earlier)
- Entry price uses the OPEN of the bar AFTER the signal, not the close of the signal bar
- IV surface interpolation only uses IV data points up to current timestamp
- Unit test with known edge cases where look-ahead would give different results

**Warning signs:**
- Results are "too good to be true" — unnaturally smooth equity curve
- Backtest results don't match even approximately when paper traded

**Phase to address:** Phase 2 (Single-leg engine — establish correct simulation loop pattern)

---

### Pitfall 2: Incorrect Expiry Calendar Handling

**What goes wrong:**
BankNifty weekly→monthly transition (2024-11-01), Nifty (2024-10-03), Sensex (2024-09-18) — if handled incorrectly, strategies either miss trades during transition weeks or trade non-existent expiries.

**Why it happens:**
Hard-coding expiry logic or assuming a single expiry type. The transition week has an overlap window where both weekly and monthly data exist.

**How to avoid:**
- ExpiryCalendar is the ONLY code that resolves expiry dates — all other code calls it
- Config-driven transition definitions in `expiry_calendar.toml`
- Unit tests specifically for: pre-transition week, transition week, post-transition, overlap window
- Store actual `NaiveDate` expiry on every open leg — never store expiry type string

**Warning signs:**
- Trades during Oct-Nov 2024 show unexpected behavior or missing trades
- Expiry-day performance metrics look anomalous around transition dates

**Phase to address:** Phase 1 (Data foundation — ExpiryCalendar struct)

---

### Pitfall 3: SL/Target Timing Within a Bar

**What goes wrong:**
When using 1-minute bars, a bar can trigger both SL and target (high hits one, low hits the other). Without careful intra-bar logic, you can't know which was hit first.

**Why it happens:**
OHLCV bars don't tell you the order of price movement within the bar. With 1-minute bars this is less severe than with daily bars, but still occurs during volatile moves.

**How to avoid:**
- Define a strict priority: SL always checked before target on the same bar
- Use the HIGH and LOW rather than just CLOSE for SL/target checks
- For same-bar SL+target ambiguity: assume worst case (SL hit first for sells, target for buys)
- Document the assumption clearly in code

**Warning signs:**
- Trades with 0 hold bars (entered and exited on same bar) showing inconsistent PnL signs
- Strategy performance changes dramatically when switching SL/target priority

**Phase to address:** Phase 2 (SL state machine initial implementation)

---

### Pitfall 4: Incorrect Premium Percentage Calculations

**What goes wrong:**
"100% SL on premium" should mean the option price doubled from entry (seller loses 100% of collected premium), but implementations often get the direction or base wrong.

**Why it happens:**
Confusion between: (a) loss as % of entry premium, (b) current price as % of entry premium, (c) absolute point move.

**How to avoid:**
- Define clearly: `percent_of_premium SL` = MTM loss exceeds X% of entry premium
- For a SELL at 200: 100% SL triggers when loss = 200 INR per unit (option price = 400)
- For a BUY at 200: 100% SL triggers when loss = 200 INR per unit (option price = 0)
- Unit test with buy AND sell positions, verifying exact trigger prices

**Warning signs:**
- SL triggers at wrong price levels in manual spot checks
- Buy and sell strategies show symmetric behavior when they shouldn't

**Phase to address:** Phase 2 (SL types implementation)

---

### Pitfall 5: Memory-Mapped File Lifecycle in NIFs

**What goes wrong:**
Rust memory-maps Parquet files and creates references to them. If the memory map is dropped while a NIF is still running (e.g., Elixir garbage collects the resource), the process crashes with a segfault.

**Why it happens:**
Rust ownership model protects within Rust, but NIF resource lifecycle is managed by the BEAM VM's garbage collector, which doesn't know about Rust lifetimes.

**How to avoid:**
- Load data WITHIN the NIF call (load → compute → return → data dropped naturally)
- If caching data across NIF calls, use Rustler ResourceArc with proper Drop implementation
- Never return references to memory-mapped data to Elixir — only return owned data (String, Vec)
- Test with concurrent NIF calls to verify no use-after-free

**Warning signs:**
- Intermittent segfaults during high-concurrency usage
- Crashes that appear random and unreproducible

**Phase to address:** Phase 6 (NIF bridge implementation)

---

### Pitfall 6: DuckDB Concurrent Write Conflicts

**What goes wrong:**
Multiple Oban workers try to write to the same DuckDB database simultaneously. DuckDB is designed for single-writer; concurrent writes can cause lock contention or corruption.

**Why it happens:**
DuckDB is an embedded analytical DB, not a concurrent OLTP system. Multiple optimizer combos completing simultaneously all try to INSERT.

**How to avoid:**
- Use a single GenServer as a DuckDB writer process — all writes go through it
- Batch inserts: collect results in memory, write in one INSERT per run completion
- Reads can be concurrent (DuckDB handles concurrent readers fine)
- Alternative: use Postgres for writes, DuckDB for reads only (materialized views)

**Warning signs:**
- "database is locked" errors during optimizer runs
- Missing rows in DuckDB tables after concurrent operations

**Phase to address:** Phase 6 (DuckDB schema and access patterns)

---

### Pitfall 7: Overfitting to Historical Data

**What goes wrong:**
Optimizer finds parameters that achieve 500% CAGR on historical data but fail in forward testing. Walk-forward and Monte Carlo are supposed to catch this, but can be misimplemented.

**Why it happens:**
With 720+ parameter combinations and 4 years of data, there will always be some combination that happens to fit perfectly. Without proper out-of-sample validation, you can't distinguish signal from noise.

**How to avoid:**
- Walk-forward analysis: 6mo IS / 2mo OOS / 2mo slide — never optimize on OOS period
- OOS Sharpe degradation ratio: flag if OOS/IS < 0.5
- Monte Carlo: shuffling trade order destroys time-dependency — if equity curve shape survives shuffling, the strategy has genuine edge
- Display IS vs OOS metrics prominently in optimizer results

**Warning signs:**
- Optimizer's best Sharpe is >3.0 (unrealistically high)
- Walk-forward OOS degradation ratio is consistently <0.3
- Monte Carlo 5th percentile equity curve is negative

**Phase to address:** Phase 5 (Walk-forward and Monte Carlo) + Phase 9 (Optimizer hardening)

---

### Pitfall 8: Lot Size Change Mid-Trade

**What goes wrong:**
BankNifty lot size changed from 15 to 30 on 2024-11-20. A trade opened with lot_size=15 before the change and closed after must use the original lot size, not the new one.

**Why it happens:**
Lot size lookup at exit time instead of entry time.

**How to avoid:**
- Store `lot_size` on the Position struct at ENTRY time — never re-lookup
- `lot_sizes.toml` covers every (symbol, date) pair in the dataset
- Unit test: open position on 2024-11-19 (lot=15), close on 2024-11-21 (lot should still be 15)

**Warning signs:**
- PnL calculations around lot size change dates are off by 2×
- Position sizing suddenly changes mid-trade

**Phase to address:** Phase 1 (LotSizes config) + Phase 2 (Position struct)

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip IV validation | Faster data loading | NaN Greeks, wrong PnL attribution | Never — validate IV ≥95% coverage at data load |
| Hard-code expiry dates | Quick to implement | Breaks when NSE changes expiry rules | Never — use config-driven ExpiryCalendar |
| Stub delta-breach SL | Unblocks P3 without Greeks | Must remember to wire it up in P5 | Acceptable during P3, must resolve in P5 |
| Single DuckDB file | Simpler implementation | Performance degrades with 100k+ trades | OK for v1; consider partitioning in v2 |
| Skip slippage model | Faster iteration on strategy logic | Overly optimistic results | Never — even fixed_pts=1.0 is better than nothing |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| String allocation in inner loop | Backtest >2s for 4yr | Pre-allocate all strings; use enums for OptionType/ExitReason | Immediately — this is the #1 Rust perf killer |
| Recomputing expiry on every bar | Expiry lookup shows up in profile | Cache expiry per trading day; only recompute at day boundary | At scale with multiple indicators per bar |
| Loading all Parquet files into memory | OOM with 3 symbols × 4 years | Memory-map files; load only requested date range | ~8GB+ of bar data |
| JSON serialization for large results | NIF return takes >100ms | Filter results before serialization; only send summary metrics | When optimizer returns 1000+ combo results |

## "Looks Done But Isn't" Checklist

- [ ] **SL state machine:** Often missing high-water mark persistence across bars — verify trailing SL never decreases
- [ ] **Re-entry:** Often missing max_attempts enforcement — verify re-entry stops after N attempts
- [ ] **Brokerage:** Often missing STT/GST/stamp duty — verify full India cost stack included
- [ ] **Expiry filter:** Often missing overlap window handling — verify Oct-Nov 2024 transition works
- [ ] **Metrics:** Often missing risk-free rate in Sharpe — verify using 6.5% (India) not 0%
- [ ] **Walk-forward:** Often using IS data in OOS window — verify strict temporal separation
- [ ] **Overall SL:** Often checking after per-leg SL — verify priority order is correct
- [ ] **OCO:** Often missing target cancellation on SL trigger — verify one-cancels-other works

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Look-ahead bias | Phase 2 | Unit test: signal uses t-1 data, entry at t open |
| Expiry calendar errors | Phase 1 | Unit test: pre/post transition, overlap window |
| SL/target timing | Phase 2 | Unit test: same-bar trigger, verify priority |
| Premium % calculation | Phase 2 | Unit test: buy vs sell, exact trigger prices |
| NIF memory lifecycle | Phase 6 | Stress test: concurrent NIF calls, no segfaults |
| DuckDB concurrency | Phase 6 | Load test: 10 concurrent writes |
| Overfitting | Phase 5 + 9 | Walk-forward OOS/IS ratio check |
| Lot size change | Phase 1 + 2 | Unit test: position across lot size change date |

## Sources

- QuantConnect community — backtesting bias documentation
- Corporate Finance Institute — look-ahead bias definition and examples
- DuckDB documentation — concurrency model and limitations
- Rustler GitHub issues — NIF resource lifecycle discussions
- NSE circulars — lot size changes, expiry transition announcements
- Indian trading forums — common FNO backtesting mistakes

---
*Pitfalls research for: FNO Options Backtesting Platform*
*Researched: 2026-04-19*
