defmodule QuantEdge.Runs.BacktestRun do
  @moduledoc "Ecto schema for backtest run tracking."
  use Ecto.Schema
  import Ecto.Changeset

  @primary_key {:id, :binary_id, autogenerate: true}
  @foreign_key_type :binary_id

  schema "backtest_runs" do
    field :status, :string, default: "pending"
    field :date_from, :date
    field :date_to, :date
    field :capital, :decimal
    field :started_at, :utc_datetime
    field :completed_at, :utc_datetime
    field :result_summary, :map
    belongs_to :strategy, QuantEdge.Strategies.Strategy
    timestamps(type: :utc_datetime)
  end

  @valid_statuses ~w(pending running completed failed)

  def changeset(run, attrs) do
    run
    |> cast(attrs, [:strategy_id, :status, :date_from, :date_to, :capital,
                     :started_at, :completed_at, :result_summary])
    |> validate_required([:strategy_id, :date_from, :date_to, :capital])
    |> validate_inclusion(:status, @valid_statuses)
    |> foreign_key_constraint(:strategy_id)
  end
end
