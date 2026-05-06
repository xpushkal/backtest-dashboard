defmodule QuantEdge.NIFTest do
  @moduledoc "Tests for the Rust NIF bridge."
  use ExUnit.Case, async: true

  alias QuantEdge.NIF

  describe "run_backtest/2" do
    test "returns error tuple with invalid TOML" do
      assert {:error, reason} = NIF.run_backtest("not valid toml", "{}")
      assert is_binary(reason)
      assert reason =~ "Invalid"
    end

    test "returns error tuple with invalid opts JSON" do
      valid_toml = """
      [strategy]
      name = "test"
      underlying = "BANKNIFTY"
      capital = 100000
      entry_time = "09:20"
      exit_time = "15:15"
      lot_size = 15

      [[legs]]
      option_type = "CE"
      position = "sell"
      lots = 1
      expiry = "weekly"
      strike_mode = "atm_offset"
      strike_offset = 0
      sl_type = "percent_of_premium"
      sl_value = 30.0
      """

      assert {:error, reason} = NIF.run_backtest(valid_toml, "not valid json")
      assert reason =~ "Invalid opts"
    end
  end

  describe "run_optimizer/2" do
    test "returns not implemented error" do
      assert {:error, "Optimizer not yet implemented"} =
               NIF.run_optimizer("", "{}")
    end
  end

  describe "run_portfolio/2" do
    test "returns not implemented error" do
      assert {:error, "Portfolio not yet implemented"} =
               NIF.run_portfolio("", "{}")
    end
  end

  describe "NIF overhead" do
    test "NIF call roundtrip completes in <5ms" do
      # Measure overhead of NIF call (error path, no actual computation)
      {time_us, _result} =
        :timer.tc(fn ->
          NIF.run_backtest("invalid", "{}")
        end)

      # Must complete in under 5ms (5000 microseconds)
      assert time_us < 5_000,
             "NIF overhead was #{time_us}μs, expected <5000μs"
    end
  end
end
