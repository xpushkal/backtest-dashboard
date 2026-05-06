defmodule QuantEdge.Strategies do
  @moduledoc "Context for strategy CRUD operations."
  import Ecto.Query
  alias QuantEdge.Repo
  alias QuantEdge.Strategies.Strategy

  def list_strategies do
    Repo.all(from s in Strategy, order_by: [desc: s.updated_at])
  end

  def list_strategies_by_underlying(underlying) do
    Repo.all(from s in Strategy, where: s.underlying == ^underlying, order_by: s.name)
  end

  def get_strategy!(id), do: Repo.get!(Strategy, id)

  def create_strategy(attrs \\ %{}) do
    %Strategy{}
    |> Strategy.changeset(attrs)
    |> Repo.insert()
  end

  def update_strategy(%Strategy{} = strategy, attrs) do
    strategy
    |> Strategy.changeset(attrs)
    |> Repo.update()
  end

  def delete_strategy(%Strategy{} = strategy) do
    Repo.delete(strategy)
  end

  def change_strategy(%Strategy{} = strategy, attrs \\ %{}) do
    Strategy.changeset(strategy, attrs)
  end
end
