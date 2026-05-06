defmodule QuantEdge.Repo.Migrations.CreateStrategies do
  use Ecto.Migration

  def change do
    create table(:strategies, primary_key: false) do
      add :id, :binary_id, primary_key: true
      add :name, :string, null: false
      add :underlying, :string, null: false
      add :config_toml, :text, null: false
      timestamps(type: :utc_datetime)
    end

    create unique_index(:strategies, [:name])
    create index(:strategies, [:underlying])
  end
end
