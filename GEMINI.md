<!-- GSD:project-start source:PROJECT.md -->
## Project

**QuantEdge**

A personal-grade FNO options backtesting platform for Indian markets (BankNifty, Nifty, Sensex). Built on a Rust simulation kernel for sub-second performance and Phoenix/Elixir for orchestration and real-time web UI. Replaces AlgoTest/Stockmock with full metric coverage (75+), multi-leg strategy support, portfolio backtesting, and parameter optimization — all running locally against 4+ years of 1-minute OHLCV, OI, and IV data.

**Core Value:** Ultra-fast, metric-complete backtesting that gives a single trader the full analytical picture — Greeks attribution, walk-forward validation, Monte Carlo confidence bands, and portfolio-level risk — without paying for subscription tools that deliver less.

### Constraints

- **Tech stack**: Rust (simulation kernel) + Phoenix/Elixir (web + orchestration) — non-negotiable
- **Storage**: Postgres (transactional) + DuckDB (analytical) + Parquet (bar data) — three-tier storage
- **Performance**: Single strategy 4yr <1s, 10-strategy portfolio <5s, 1000-combo optimizer <3min
- **Data format**: CSV schema is fixed (timestamp, date, time, weekday, option_type, strike_label, strike_offset, moneyness, OHLCV, strike, oi, spot, iv)
- **Single user**: No authentication, no multi-tenancy, no deployment concerns
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## Recommended Stack
### Core Technologies
| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust (stable) | 1.82+ | Simulation kernel, data processing, metrics | 10-50× faster than Python for numeric simulation; zero-cost abstractions; memory safety without GC pauses |
| Elixir | 1.17+ | Web application, job orchestration, PubSub | BEAM concurrency model ideal for real-time progress updates; fault tolerance via supervisors; LiveView eliminates JS frontend |
| Erlang/OTP | 27+ | Runtime for Elixir | Latest stable with improved JIT; required for Phoenix 1.7+ |
| Phoenix Framework | 1.7+ | Web framework | Industry standard for Elixir web apps; LiveView 1.0 stable for real-time UI |
| Phoenix LiveView | 1.0+ | Real-time UI without JavaScript | Server-rendered real-time updates; eliminates separate frontend build; PubSub integration native |
| Postgres | 16+ | Transactional storage (strategies, runs, metadata) | ACID compliance for strategy configs; JSONB for flexible result storage; Oban job queue backing |
| DuckDB | 0.10+ | Analytical queries (trades, equity curves, metrics) | 10-50× faster than Postgres for columnar time-series aggregations; embedded, no separate server |
### Supporting Libraries — Rust
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `polars` | 0.53+ | Columnar dataframe, Parquet I/O | Data loading, CSV-to-Parquet conversion, data validation |
| `rayon` | 1.10+ | Data parallelism (work-stealing thread pool) | Optimizer sweep — each strategy combo runs on separate core |
| `rustler` | 0.37+ | Elixir NIF bridge | All Rust↔Elixir communication; dirty CPU schedulers for long backtests |
| `serde` + `serde_json` | 1.x / 1.x | Serialization/deserialization | Strategy config parsing, NIF result encoding |
| `chrono` | 0.4+ | Date/time handling | Bar timestamps, expiry dates, DTE calculations |
| `ndarray` | 0.16+ | Matrix operations | Greeks batch computation, correlation matrix |
| `statrs` | 0.17+ | Statistical distributions | Monte Carlo simulations, VaR/CVaR calculations |
| `toml` | 0.8+ | TOML parsing | Strategy DSL file parsing |
### Supporting Libraries — Elixir
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `phoenix_live_view` | 1.0+ | Real-time server-side UI | All interactive pages — strategy builder, results viewer, optimizer |
| `oban` | 2.17+ | Background job queue (Postgres-backed) | Backtest execution, optimizer sweeps, portfolio runs |
| `rustler` (hex) | 0.35+ | NIF compilation + loading | Compiling and linking Rust crate at mix compile time |
| `ecto` + `ecto_sql` | 3.11+ | Postgres ORM | Strategy CRUD, run management, migrations |
| `duckdbex` | 0.4+ | DuckDB Elixir bindings | Trade log queries, equity curve storage, metrics aggregation |
| `jason` | 1.4+ | JSON encoding/decoding | API responses, NIF data interchange |
| `nimble_toml` | 1.0+ | TOML parsing | Strategy DSL parsing on Elixir side |
### Development Tools
| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo fmt` + `cargo clippy` | Rust code quality | Run before every commit; clippy catches common Rust pitfalls |
| `mix format` | Elixir code formatting | Standard Elixir formatter |
| `cargo bench` (criterion) | Rust benchmarking | Performance regression testing for simulation kernel |
| `mix test` | Elixir testing | ExUnit for context/worker tests |
| Python 3.11+ (`polars`, `pyarrow`) | Data conversion scripts | One-time CSV→Parquet conversion only |
## Installation
# Rust
# Elixir (via asdf or mise)
# Phoenix
# Postgres
# Python (data conversion only)
## Alternatives Considered
| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Polars (Rust) | Arrow2 standalone | Only if you need raw Arrow IPC and don't need DataFrame operations; Polars wraps Arrow internally |
| DuckDB embedded | ClickHouse | If you need distributed analytical queries at massive scale; overkill for single-user |
| Phoenix LiveView | React/Next.js frontend | If you need complex client-side interactions (drag-and-drop, rich canvas); LiveView handles 95% of this use case |
| Oban (Postgres-backed) | RabbitMQ/Redis queue | If you need multi-node distributed job processing; Oban is simpler for single-node |
| Rustler NIF | Separate Rust microservice (Axum) | If backtests exceed 10+ seconds and you want process isolation; NIF dirty schedulers handle <10s well |
## What NOT to Use
| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `arrow2` as standalone (v0.18) | Maintenance has slowed; Polars manages its own Arrow internals | `polars` with `parquet` feature flag |
| Python for simulation kernel | 10-50× slower; 4yr backtest: 30-60s vs <1s in Rust | Rust `quantedge-core` crate |
| SQLite for analytics | Row-oriented; terrible for columnar aggregations like monthly heatmaps | DuckDB (columnar, embedded) |
| GenServer for heavy compute | Blocks BEAM schedulers; causes latency spikes for all LiveView users | Rustler dirty CPU NIFs |
| `Ecto` for DuckDB | Ecto is designed for Postgres; DuckDB needs raw SQL via `duckdbex` | Direct `duckdbex` calls |
## Version Compatibility
| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `rustler` hex 0.35+ | `rustler` crate 0.37+ | Hex and crate versions differ; ensure both updated |
| `polars` 0.53 | Rust 1.80+ | Polars requires recent stable Rust |
| `phoenix_live_view` 1.0 | Phoenix 1.7+ | LiveView 1.0 requires Phoenix 1.7 minimum |
| `oban` 2.17 | Ecto 3.10+, Postgres 12+ | Oban migrations require Postgres |
| `duckdbex` 0.4 | DuckDB 0.10+ | Embedded DuckDB; no separate server install |
## Sources
- crates.io — verified polars 0.53, rustler 0.37, rayon 1.10
- hex.pm — verified phoenix_live_view 1.0, oban 2.17, duckdbex 0.4, rustler 0.35
- Elixir Forum — Rust+Elixir "Endurance Stack" architecture patterns
- Official Polars docs — feature flags, Parquet I/O configuration
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->
## Project Skills

No project skills found. Add skills to any of: `.agent/skills/`, `.agents/skills/`, `.cursor/skills/`, or `.github/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
