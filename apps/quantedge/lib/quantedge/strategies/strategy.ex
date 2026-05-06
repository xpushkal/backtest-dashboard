defmodule QuantEdge.Strategies.Strategy do
  @moduledoc "Ecto schema for backtesting strategies."
  use Ecto.Schema
  import Ecto.Changeset

  @primary_key {:id, :binary_id, autogenerate: true}
  @foreign_key_type :binary_id

  schema "strategies" do
    field :name, :string
    field :underlying, :string
    field :config_toml, :string
    has_many :backtest_runs, QuantEdge.Runs.BacktestRun
    timestamps(type: :utc_datetime)
  end

  def changeset(strategy, attrs) do
    strategy
    |> cast(attrs, [:name, :underlying, :config_toml])
    |> validate_required([:name, :underlying, :config_toml])
    |> unique_constraint(:name)
    |> validate_inclusion(:underlying, ~w(BANKNIFTY NIFTY SENSEX))
  end
end
