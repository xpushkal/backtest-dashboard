defmodule QuantEdge.Duck.Reader do
  @moduledoc """
  Read-only query helpers for DuckDB analytical tables.

  All queries route through the Writer GenServer for connection management.
  """
  alias QuantEdge.Duck.Writer

  @doc "Get all trades for a run, ordered by trade_id."
  def get_trades(run_id) do
    Writer.query("SELECT * FROM trades WHERE run_id = '#{run_id}' ORDER BY trade_id")
  end

  @doc "Get equity curve for a run, ordered by date."
  def get_equity_curve(run_id) do
    Writer.query("SELECT * FROM equity_curves WHERE run_id = '#{run_id}' ORDER BY date")
  end

  @doc "Get all metrics for a run as {metric_name, metric_value} rows."
  def get_metrics(run_id) do
    Writer.query("SELECT metric_name, metric_value FROM metrics WHERE run_id = '#{run_id}'")
  end

  @doc "Monthly PnL aggregation for heatmap display."
  def get_monthly_pnl(run_id) do
    Writer.query("""
    SELECT
      CAST(EXTRACT(YEAR FROM CAST(exit_time AS TIMESTAMP)) AS INTEGER) AS year,
      CAST(EXTRACT(MONTH FROM CAST(exit_time AS TIMESTAMP)) AS INTEGER) AS month,
      SUM(pnl_net) AS pnl
    FROM trades
    WHERE run_id = '#{run_id}'
    GROUP BY year, month
    ORDER BY year, month
    """)
  end

  @doc "Daily PnL aggregation."
  def get_daily_pnl(run_id) do
    Writer.query("""
    SELECT
      CAST(exit_time AS DATE) AS date,
      SUM(pnl_net) AS pnl,
      COUNT(*) AS trades
    FROM trades
    WHERE run_id = '#{run_id}'
    GROUP BY date
    ORDER BY date
    """)
  end

  @doc "Get optimizer results for an optimizer run."
  def get_optimizer_results(optimizer_run_id) do
    Writer.query("""
    SELECT combo_index, params, metrics
    FROM optimizer_results
    WHERE optimizer_run_id = '#{optimizer_run_id}'
    ORDER BY combo_index
    """)
  end
end
