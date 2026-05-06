defmodule QuantEdge.Workers.BacktestWorker do
  @moduledoc """
  Oban worker for executing backtests.

  Flow: Receive run_id → load strategy → call NIF → store in DuckDB + Postgres → broadcast.
  """
  use Oban.Worker, queue: :backtests, max_attempts: 1

  alias QuantEdge.{NIF, Runs}
  alias QuantEdge.Duck.Writer
  require Logger

  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"run_id" => run_id}}) do
    run = Runs.get_run!(run_id)
    strategy = run.strategy

    # 1. Broadcast: running
    Runs.update_status(run_id, :running)
    broadcast(run_id, {:status, :running})
    Logger.info("Backtest started: #{run_id}")

    # 2. Build NIF inputs
    opts = Jason.encode!(%{
      symbol: strategy.underlying,
      date_from: Date.to_iso8601(run.date_from),
      date_to: Date.to_iso8601(run.date_to),
      capital: Decimal.to_float(run.capital),
      lot_size: get_lot_size(strategy.underlying),
      data_dir: Application.get_env(:quantedge, :data_dir, "Data/parquet")
    })

    # 3. Execute NIF on dirty CPU scheduler
    case NIF.run_backtest(strategy.config_toml, opts) do
      {:ok, result_json} ->
        result = Jason.decode!(result_json)

        # 4. Store detailed data in DuckDB
        Writer.insert_trades(run_id, result["trades"] || [])
        Writer.insert_equity_curve(run_id, result["equity_curve"] || [])
        Writer.insert_metrics(run_id, result["metrics"] || %{})

        # 5. Store summary in Postgres
        summary = extract_summary(result["metrics"] || %{})
        Runs.store_result(run_id, summary)

        # 6. Broadcast completion
        broadcast(run_id, {:completed, summary})
        Logger.info("Backtest completed: #{run_id} — PnL: #{summary["total_pnl_net"]}")
        :ok

      {:error, reason} ->
        Runs.update_status(run_id, :failed)
        broadcast(run_id, {:error, reason})
        Logger.error("Backtest failed: #{run_id} — #{reason}")
        {:error, reason}
    end
  end

  defp broadcast(run_id, message) do
    Phoenix.PubSub.broadcast(QuantEdge.PubSub, "run:#{run_id}", message)
  end

  defp extract_summary(metrics) when is_map(metrics) do
    Map.take(metrics, [
      "total_pnl_net", "cagr", "win_rate_pct", "max_drawdown_pct",
      "sharpe_ratio", "profit_factor", "total_trades", "premium_capture_pct"
    ])
  end

  defp get_lot_size("BANKNIFTY"), do: 15
  defp get_lot_size("NIFTY"), do: 25
  defp get_lot_size("SENSEX"), do: 10
  defp get_lot_size(_), do: 1
end
