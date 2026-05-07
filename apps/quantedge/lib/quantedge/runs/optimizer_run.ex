defmodule QuantEdge.Runs.OptimizerRun do
  @moduledoc "Ecto schema for optimizer run tracking."
  use Ecto.Schema
  import Ecto.Changeset

  @primary_key {:id, :binary_id, autogenerate: true}
  @foreign_key_type :binary_id

  schema "optimizer_runs" do
    # Stored as JSONB. Always a list of param-range maps:
    #   [%{"name" => "sl_value", "min" => 20, "max" => 50, "step" => 5}, ...]
    field :param_grid, {:array, :map}
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
    |> validate_change(:param_grid, fn :param_grid, value ->
      cond do
        not is_list(value) -> [param_grid: "must be a list"]
        Enum.empty?(value) -> [param_grid: "cannot be empty"]
        not Enum.all?(value, &is_map/1) -> [param_grid: "each entry must be a map"]
        true -> []
      end
    end)
    |> foreign_key_constraint(:strategy_id)
  end
end
