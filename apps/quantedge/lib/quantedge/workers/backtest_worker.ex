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

    # 2. Build NIF inputs — lot size comes from historical config keyed on entry date
    opts = Jason.encode!(%{
      symbol: strategy.underlying,
      date_from: Date.to_iso8601(run.date_from),
      date_to: Date.to_iso8601(run.date_to),
      capital: Decimal.to_float(run.capital),
      lot_size: QuantEdge.LotSizes.get(strategy.underlying, run.date_from),
      data_dir: Application.get_env(:quantedge, :data_dir, "Data/parquet")
    })

    # 3. Execute NIF on dirty CPU scheduler
    case NIF.run_backtest(strategy.config_toml, opts) do
      {:ok, result_json} ->
        result = Jason.decode!(result_json)

        trades = result["trades"] || []
        equity = result["equity_curve"] || []
        metrics_map = result["metrics"] || %{}

        Logger.info(
          "NIF returned for #{run_id}: trades=#{length(trades)} equity=#{length(equity)} " <>
            "metrics=#{map_size(metrics_map)} top_keys=#{inspect(Map.keys(result))}"
        )

        if trades == [] do
          Logger.warning(
            "NIF returned 0 trades. Strategy=#{strategy.name} symbol=#{strategy.underlying} " <>
              "range=#{run.date_from}..#{run.date_to}. " <>
              "Result snippet: #{String.slice(result_json, 0, 500)}"
          )
        end

        # 4. Store detailed data in DuckDB
        Writer.insert_trades(run_id, trades)
        Writer.insert_equity_curve(run_id, equity)
        Writer.insert_metrics(run_id, metrics_map)

        # 5. Store summary in Postgres
        summary = extract_summary(metrics_map)
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
    # Also broadcast to the global listing channel
    case message do
      {:status, :running} ->
        Phoenix.PubSub.broadcast(QuantEdge.PubSub, "runs:updates", {:run_started, run_id})
      {:completed, summary} ->
        Phoenix.PubSub.broadcast(QuantEdge.PubSub, "runs:updates", {:run_completed, run_id, summary})
      {:error, reason} ->
        Phoenix.PubSub.broadcast(QuantEdge.PubSub, "runs:updates", {:run_failed, run_id, reason})
      _ -> :ok
    end
  end

  defp extract_summary(metrics) when is_map(metrics) do
    # Store all metrics from the Rust MetricsResult (44 fields)
    metrics
  end
end
