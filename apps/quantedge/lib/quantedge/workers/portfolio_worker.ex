defmodule QuantEdge.Workers.PortfolioWorker do
  @moduledoc """
  Oban worker for portfolio backtests (multi-strategy).

  Accepts a portfolio configuration with N strategies, calls the NIF,
  stores results in DuckDB, and broadcasts progress via PubSub.
  """
  use Oban.Worker, queue: :portfolios, max_attempts: 1

  alias QuantEdge.NIF
  require Logger

  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"portfolio_run_id" => run_id, "portfolio_json" => portfolio_json}}) do
    broadcast(run_id, {:portfolio_started, run_id})
    Logger.info("Portfolio backtest started: #{run_id}")

    case NIF.run_portfolio(portfolio_json, "{}") do
      {:ok, result_json} ->
        result = Jason.decode!(result_json)

        # Store detailed results in DuckDB (best-effort).
        try do
          store_portfolio_results(run_id, result)
        rescue
          e -> Logger.warning("Failed to store DuckDB results: #{Exception.message(e)}")
        end

        broadcast(run_id, {:portfolio_completed, run_id, result})
        Logger.info("Portfolio completed: #{run_id} — #{result["total_trades"]} total trades")
        :ok

      {:error, reason} ->
        broadcast(run_id, {:portfolio_failed, run_id, reason})
        Logger.error("Portfolio failed: #{run_id} — #{reason}")
        {:error, reason}
    end
  end

  # Legacy arg name fallback.
  def perform(%Oban.Job{args: %{"portfolio_run_id" => run_id, "strategies_json" => strategies_json}}) do
    perform(%Oban.Job{args: %{"portfolio_run_id" => run_id, "portfolio_json" => strategies_json}})
  end

  defp store_portfolio_results(run_id, result) do
    # Store combined equity curve
    if equity = result["combined_equity"] do
      rows = Enum.map(equity, fn ep ->
        %{
          run_id: run_id,
          date: ep["date"],
          equity: ep["equity"]
        }
      end)

      try do
        apply(QuantEdge.Duck.Writer, :insert_equity_curve, [run_id, rows])
      rescue
        _ -> :ok
      end
    end

    # Store per-strategy trades
    for strategy <- result["strategies"] || [] do
      trades = Enum.map(strategy["trades"] || [], fn trade ->
        Map.merge(trade, %{
          "run_id" => run_id,
          "strategy_name" => strategy["name"]
        })
      end)

      if trades != [] do
        try do
          apply(QuantEdge.Duck.Writer, :insert_trades, [run_id, trades])
        rescue
          _ -> :ok
        end
      end
    end
  end

  defp broadcast(_run_id, message) do
    Phoenix.PubSub.broadcast(QuantEdge.PubSub, "portfolio:updates", message)
    case message do
      {_, run_id, _} -> Phoenix.PubSub.broadcast(QuantEdge.PubSub, "portfolio:#{run_id}", message)
      {_, run_id} -> Phoenix.PubSub.broadcast(QuantEdge.PubSub, "portfolio:#{run_id}", message)
      _ -> :ok
    end
  end
end
