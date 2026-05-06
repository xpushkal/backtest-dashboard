defmodule QuantEdge.Runs.BacktestRunTest do
  @moduledoc "Tests for BacktestRun schema."
  use ExUnit.Case

  alias QuantEdge.Runs.BacktestRun

  describe "changeset/2" do
    test "valid changeset" do
      attrs = %{
        strategy_id: Ecto.UUID.generate(),
        date_from: ~D[2021-01-01],
        date_to: ~D[2024-12-31],
        capital: Decimal.new("100000")
      }

      changeset = BacktestRun.changeset(%BacktestRun{}, attrs)
      assert changeset.valid?
    end

    test "defaults status to pending" do
      attrs = %{
        strategy_id: Ecto.UUID.generate(),
        date_from: ~D[2021-01-01],
        date_to: ~D[2024-12-31],
        capital: Decimal.new("100000")
      }

      changeset = BacktestRun.changeset(%BacktestRun{}, attrs)
      assert Ecto.Changeset.get_field(changeset, :status) == "pending"
    end

    test "invalid status rejected" do
      attrs = %{
        strategy_id: Ecto.UUID.generate(),
        date_from: ~D[2021-01-01],
        date_to: ~D[2024-12-31],
        capital: Decimal.new("100000"),
        status: "invalid_status"
      }

      changeset = BacktestRun.changeset(%BacktestRun{}, attrs)
      refute changeset.valid?
    end

    test "valid statuses accepted" do
      for status <- ~w(pending running completed failed) do
        attrs = %{
          strategy_id: Ecto.UUID.generate(),
          date_from: ~D[2021-01-01],
          date_to: ~D[2024-12-31],
          capital: Decimal.new("100000"),
          status: status
        }

        changeset = BacktestRun.changeset(%BacktestRun{}, attrs)
        assert changeset.valid?, "Expected status '#{status}' to be valid"
      end
    end

    test "requires strategy_id, date_from, date_to, capital" do
      changeset = BacktestRun.changeset(%BacktestRun{}, %{})
      refute changeset.valid?

      errors = errors_on(changeset)
      assert Map.has_key?(errors, :strategy_id)
      assert Map.has_key?(errors, :date_from)
      assert Map.has_key?(errors, :date_to)
      assert Map.has_key?(errors, :capital)
    end
  end

  defp errors_on(changeset) do
    Ecto.Changeset.traverse_errors(changeset, fn {msg, opts} ->
      Regex.replace(~r"%{(\w+)}", msg, fn _, key ->
        opts |> Keyword.get(String.to_existing_atom(key), key) |> to_string()
      end)
    end)
  end
end
