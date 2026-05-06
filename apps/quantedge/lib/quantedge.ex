defmodule QuantEdge do
  @moduledoc """
  QuantEdge — Personal-grade FNO options backtesting platform.

  Business logic application providing:
  - Strategy CRUD (Postgres)
  - Backtest execution (Rust NIF → Oban workers)
  - Analytical storage (DuckDB)
  - Real-time progress (PubSub)
  """
end
