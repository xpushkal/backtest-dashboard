defmodule QuantEdge.Strategies.StrategyTest do
  @moduledoc "Tests for Strategy schema and context."
  use ExUnit.Case

  alias QuantEdge.Strategies.Strategy

  describe "changeset/2" do
    test "valid changeset with all required fields" do
      attrs = %{
        name: "Short Straddle BN",
        underlying: "BANKNIFTY",
        config_toml: "[strategy]\nname = \"test\""
      }

      changeset = Strategy.changeset(%Strategy{}, attrs)
      assert changeset.valid?
    end

    test "invalid without name" do
      attrs = %{underlying: "BANKNIFTY", config_toml: "test"}
      changeset = Strategy.changeset(%Strategy{}, attrs)
      refute changeset.valid?
      assert %{name: ["can't be blank"]} = errors_on(changeset)
    end

    test "invalid underlying" do
      attrs = %{name: "test", underlying: "INVALID", config_toml: "test"}
      changeset = Strategy.changeset(%Strategy{}, attrs)
      refute changeset.valid?
      assert %{underlying: ["is invalid"]} = errors_on(changeset)
    end

    test "valid underlyings" do
      for underlying <- ~w(BANKNIFTY NIFTY SENSEX) do
        attrs = %{name: "test-#{underlying}", underlying: underlying, config_toml: "test"}
        changeset = Strategy.changeset(%Strategy{}, attrs)
        assert changeset.valid?, "Expected #{underlying} to be valid"
      end
    end
  end

  # Helper to extract errors from changeset
  defp errors_on(changeset) do
    Ecto.Changeset.traverse_errors(changeset, fn {msg, opts} ->
      Regex.replace(~r"%{(\w+)}", msg, fn _, key ->
        opts |> Keyword.get(String.to_existing_atom(key), key) |> to_string()
      end)
    end)
  end
end
