defmodule QuantEdge.Workers.PortfolioWorker do
  @moduledoc """
  Oban worker for portfolio backtests (multi-strategy).
  """
  use Oban.Worker, queue: :portfolios, max_attempts: 1

  alias QuantEdge.NIF
  require Logger

  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"portfolio_run_id" => run_id, "strategies_json" => strategies_json}}) do
    broadcast(run_id, {:status, :running})
    Logger.info("Portfolio backtest started: #{run_id}")

    case NIF.run_portfolio(strategies_json, "{}") do
      {:ok, result_json} ->
        result = Jason.decode!(result_json)
        broadcast(run_id, {:completed, result})
        Logger.info("Portfolio completed: #{run_id}")
        :ok

      {:error, reason} ->
        broadcast(run_id, {:error, reason})
        Logger.error("Portfolio failed: #{run_id} — #{reason}")
        {:error, reason}
    end
  end

  defp broadcast(run_id, message) do
    Phoenix.PubSub.broadcast(QuantEdge.PubSub, "portfolio:#{run_id}", message)
  end
end
