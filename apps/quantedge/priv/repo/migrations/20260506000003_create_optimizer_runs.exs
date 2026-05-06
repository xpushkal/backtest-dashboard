defmodule QuantEdge.Repo.Migrations.CreateOptimizerRuns do
  use Ecto.Migration

  def change do
    create table(:optimizer_runs, primary_key: false) do
      add :id, :binary_id, primary_key: true
      add :strategy_id, references(:strategies, type: :binary_id, on_delete: :delete_all),
        null: false
      add :param_grid, :map, null: false
      add :status, :string, null: false, default: "pending"
      add :total_combos, :integer
      add :completed_combos, :integer, default: 0
      timestamps(type: :utc_datetime)
    end

    create index(:optimizer_runs, [:strategy_id])
  end
end
