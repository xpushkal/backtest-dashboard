defmodule QuantEdge.Runs do
  @moduledoc "Context for backtest and optimizer run management."
  import Ecto.Query
  alias QuantEdge.Repo
  alias QuantEdge.Runs.{BacktestRun, OptimizerRun}

  # ─── Backtest Runs ─────────────────────────────────────────

  def list_runs do
    Repo.all(from r in BacktestRun, order_by: [desc: r.inserted_at], preload: :strategy)
  end

  def list_runs_for_strategy(strategy_id) do
    Repo.all(
      from r in BacktestRun,
        where: r.strategy_id == ^strategy_id,
        order_by: [desc: r.inserted_at]
    )
  end

  def get_run!(id) do
    Repo.get!(BacktestRun, id) |> Repo.preload(:strategy)
  end

  def create_run(attrs \\ %{}) do
    %BacktestRun{}
    |> BacktestRun.changeset(attrs)
    |> Repo.insert()
  end

  def update_status(run_id, status) when is_atom(status) do
    update_status(run_id, Atom.to_string(status))
  end

  def update_status(run_id, status) when is_binary(status) do
    run = Repo.get!(BacktestRun, run_id)
    attrs = %{status: status}
    attrs = if status == "running", do: Map.put(attrs, :started_at, DateTime.utc_now()), else: attrs

    run
    |> BacktestRun.changeset(attrs)
    |> Repo.update()
  end

  def store_result(run_id, result_map) do
    run = Repo.get!(BacktestRun, run_id)

    run
    |> BacktestRun.changeset(%{
      status: "completed",
      completed_at: DateTime.utc_now(),
      result_summary: result_map
    })
    |> Repo.update()
  end

  # ─── Optimizer Runs ────────────────────────────────────────

  def get_optimizer_run!(id) do
    Repo.get!(OptimizerRun, id) |> Repo.preload(:strategy)
  end

  def create_optimizer_run(attrs) do
    %OptimizerRun{}
    |> OptimizerRun.changeset(attrs)
    |> Repo.insert()
  end

  def update_optimizer_status(run_id, status, completed_combos) do
    run = Repo.get!(OptimizerRun, run_id)

    run
    |> OptimizerRun.changeset(%{
      status: Atom.to_string(status),
      completed_combos: completed_combos
    })
    |> Repo.update()
  end

  # ─── Job Enqueueing ────────────────────────────────────────

  def enqueue_backtest(run_id) do
    %{run_id: run_id}
    |> QuantEdge.Workers.BacktestWorker.new()
    |> Oban.insert()
  end

  def enqueue_optimizer(optimizer_run_id) do
    %{optimizer_run_id: optimizer_run_id}
    |> QuantEdge.Workers.OptimizerWorker.new()
    |> Oban.insert()
  end
end
