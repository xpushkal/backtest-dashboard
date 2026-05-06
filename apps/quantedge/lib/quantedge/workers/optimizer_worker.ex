defmodule QuantEdge.Workers.OptimizerWorker do
  @moduledoc """
  Oban worker for optimizer grid sweeps.

  Calls the Rust NIF which uses Rayon for parallel parameter combinations.
  """
  use Oban.Worker, queue: :optimizers, max_attempts: 1

  alias QuantEdge.{NIF, Runs}
  alias QuantEdge.Duck.Writer
  require Logger

  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"optimizer_run_id" => run_id}}) do
    run = Runs.get_optimizer_run!(run_id)
    strategy = run.strategy

    broadcast(run_id, {:status, :running})
    Logger.info("Optimizer started: #{run_id}")

    case NIF.run_optimizer(strategy.config_toml, Jason.encode!(run.param_grid)) do
      {:ok, results_json} ->
        results = Jason.decode!(results_json)
        Writer.insert_optimizer_results(run_id, results)
        Runs.update_optimizer_status(run_id, :completed, length(results))
        broadcast(run_id, {:completed, length(results)})
        Logger.info("Optimizer completed: #{run_id} — #{length(results)} combos")
        :ok

      {:error, reason} ->
        Runs.update_optimizer_status(run_id, :failed, 0)
        broadcast(run_id, {:error, reason})
        Logger.error("Optimizer failed: #{run_id} — #{reason}")
        {:error, reason}
    end
  end

  defp broadcast(run_id, message) do
    Phoenix.PubSub.broadcast(QuantEdge.PubSub, "optimizer:#{run_id}", message)
  end
end
