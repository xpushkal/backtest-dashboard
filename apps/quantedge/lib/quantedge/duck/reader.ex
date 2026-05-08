defmodule QuantEdge.Duck.Reader do
  @moduledoc """
  Read-only query helpers for DuckDB analytical tables.

  All reads use parameterized queries via the Writer GenServer.
  Each public function returns a list of column-keyed maps so the
  consumers (LiveViews) can read fields by name.
  """
  alias QuantEdge.Duck.Writer

  @trade_columns ~w(
    run_id trade_id entry_date exit_date entry_time exit_time
    option_type position_side
    entry_price exit_price entry_spot exit_spot
    lots lot_size
    pnl_gross pnl_net brokerage stt slippage_cost other_charges
    exit_reason bars_held reentry_attempt
  )

  @equity_columns ~w(run_id date equity drawdown_pct)

  @doc "Get all trades for a run, ordered by trade_id. Returns a list of maps."
  def get_trades(run_id) do
    Writer.query("SELECT * FROM trades WHERE run_id = ? ORDER BY trade_id", [run_id])
    |> rows_to_maps(@trade_columns)
  end

  @doc "Get equity curve for a run, ordered by date. Returns a list of maps."
  def get_equity_curve(run_id) do
    Writer.query(
      "SELECT * FROM equity_curves WHERE run_id = ? ORDER BY date",
      [run_id]
    )
    |> rows_to_maps(@equity_columns)
  end

  @doc "Get all metrics for a run as a string-keyed map."
  def get_metrics(run_id) do
    case Writer.query(
           "SELECT metric_name, metric_value FROM metrics WHERE run_id = ?",
           [run_id]
         ) do
      {:ok, rows} when is_list(rows) ->
        rows
        |> Enum.into(%{}, fn
          [name, value] -> {to_string(name), value}
          %{"metric_name" => name, "metric_value" => v} -> {to_string(name), v}
        end)

      _ ->
        %{}
    end
  end

  @doc "Monthly PnL aggregation for heatmap display."
  def get_monthly_pnl(run_id) do
    Writer.query(
      """
      SELECT
        CAST(EXTRACT(YEAR FROM CAST(exit_date AS DATE)) AS INTEGER) AS year,
        CAST(EXTRACT(MONTH FROM CAST(exit_date AS DATE)) AS INTEGER) AS month,
        SUM(pnl_net) AS pnl
      FROM trades
      WHERE run_id = ?
      GROUP BY year, month
      ORDER BY year, month
      """,
      [run_id]
    )
    |> rows_to_maps(~w(year month pnl))
  end

  @doc "Daily PnL aggregation."
  def get_daily_pnl(run_id) do
    Writer.query(
      """
      SELECT
        CAST(exit_date AS DATE) AS date,
        SUM(pnl_net) AS pnl,
        COUNT(*) AS trades
      FROM trades
      WHERE run_id = ?
      GROUP BY date
      ORDER BY date
      """,
      [run_id]
    )
    |> rows_to_maps(~w(date pnl trades))
  end

  @doc "Get optimizer results for an optimizer run."
  def get_optimizer_results(optimizer_run_id) do
    Writer.query(
      """
      SELECT combo_index, params, metrics
      FROM optimizer_results
      WHERE optimizer_run_id = ?
      ORDER BY combo_index
      """,
      [optimizer_run_id]
    )
    |> rows_to_maps(~w(combo_index params metrics))
  end

  # ─── Private ────────────────────────────────────────────────

  defp rows_to_maps({:ok, rows}, columns) when is_list(rows) do
    Enum.map(rows, fn
      row when is_list(row) ->
        columns
        |> Enum.zip(row)
        |> Enum.into(%{})

      row when is_map(row) ->
        row
    end)
  end

  defp rows_to_maps(_, _), do: []
end
