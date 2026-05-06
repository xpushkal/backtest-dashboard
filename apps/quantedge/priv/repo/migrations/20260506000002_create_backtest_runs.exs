defmodule QuantEdge.Repo.Migrations.CreateBacktestRuns do
  use Ecto.Migration

  def change do
    create table(:backtest_runs, primary_key: false) do
      add :id, :binary_id, primary_key: true
      add :strategy_id, references(:strategies, type: :binary_id, on_delete: :delete_all),
        null: false
      add :status, :string, null: false, default: "pending"
      add :date_from, :date, null: false
      add :date_to, :date, null: false
      add :capital, :decimal, null: false
      add :started_at, :utc_datetime
      add :completed_at, :utc_datetime
      add :result_summary, :map
      timestamps(type: :utc_datetime)
    end

    create index(:backtest_runs, [:strategy_id])
    create index(:backtest_runs, [:status])
  end
end
