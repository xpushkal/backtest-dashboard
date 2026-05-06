defmodule QuantEdge.Runs.OptimizerRun do
  @moduledoc "Ecto schema for optimizer run tracking."
  use Ecto.Schema
  import Ecto.Changeset

  @primary_key {:id, :binary_id, autogenerate: true}
  @foreign_key_type :binary_id

  schema "optimizer_runs" do
    field :param_grid, :map
    field :status, :string, default: "pending"
    field :total_combos, :integer
    field :completed_combos, :integer, default: 0
    belongs_to :strategy, QuantEdge.Strategies.Strategy
    timestamps(type: :utc_datetime)
  end

  def changeset(run, attrs) do
    run
    |> cast(attrs, [:strategy_id, :param_grid, :status, :total_combos, :completed_combos])
    |> validate_required([:strategy_id, :param_grid])
    |> validate_inclusion(:status, ~w(pending running completed failed))
    |> foreign_key_constraint(:strategy_id)
  end
end
