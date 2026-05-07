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

    broadcast(run_id, {:optimizer_status, run_id, :running})
    Logger.info("Optimizer started: #{run_id}")

    opts = Jason.encode!(%{
      date_from: optional_date(run, :date_from, "2021-01-01"),
      date_to: optional_date(run, :date_to, "2024-12-31"),
      capital: optional_capital(run),
      lot_size: QuantEdge.LotSizes.current(strategy.underlying),
      data_dir: Application.get_env(:quantedge, :data_dir, "Data/parquet")
    })

    param_grid_json = Jason.encode!(normalize_grid(run.param_grid))

    case NIF.run_optimizer(strategy.config_toml, param_grid_json, opts) do
      {:ok, results_json} ->
        results = Jason.decode!(results_json)
        Writer.insert_optimizer_results(run_id, results)
        Runs.update_optimizer_status(run_id, :completed, length(results))
        broadcast(run_id, {:optimizer_completed, run_id, results})
        Logger.info("Optimizer completed: #{run_id} — #{length(results)} combos")
        :ok

      {:error, reason} ->
        Runs.update_optimizer_status(run_id, :failed, 0)
        broadcast(run_id, {:optimizer_error, run_id, reason})
        Logger.error("Optimizer failed: #{run_id} — #{reason}")
        {:error, reason}
    end
  end

  # Broadcast on the global topic that OptimizerLive subscribes to,
  # plus the per-run topic for any future detail screens.
  defp broadcast(run_id, message) do
    Phoenix.PubSub.broadcast(QuantEdge.PubSub, "optimizer:updates", message)
    Phoenix.PubSub.broadcast(QuantEdge.PubSub, "optimizer:#{run_id}", message)
  end

  defp optional_date(run, key, default) do
    case Map.get(run, key) do
      %Date{} = d -> Date.to_iso8601(d)
      str when is_binary(str) -> str
      _ -> default
    end
  end

  defp optional_capital(run) do
    case Map.get(run, :capital) do
      %Decimal{} = d -> Decimal.to_float(d)
      n when is_number(n) -> n / 1
      _ -> 500_000.0
    end
  end

  # The UI builds param_grid as a list of maps. The Rust side accepts either
  # a list-of-objects (preferred) or an object — pass the list through verbatim.
  defp normalize_grid(grid) when is_list(grid), do: grid
  defp normalize_grid(grid) when is_map(grid), do: grid
  defp normalize_grid(_), do: []
end
