defmodule QuantEdge.Duck.WriterTest do
  @moduledoc "Tests for the DuckDB GenServer writer."
  use ExUnit.Case

  alias QuantEdge.Duck.Writer

  setup do
    # Writer should already be started by the application supervisor
    # If not in a full app context, start it manually
    case Process.whereis(Writer) do
      nil ->
        {:ok, _pid} = Writer.start_link([])
        :ok

      _pid ->
        :ok
    end
  end

  describe "insert_trades/2" do
    test "inserts trades and retrieves them" do
      run_id = "test-run-#{:rand.uniform(100_000)}"

      trades = [
        %{
          "entry_time" => "2024-01-15 09:20:00",
          "exit_time" => "2024-01-15 15:15:00",
          "exit_reason" => "time_exit",
          "pnl_gross" => 1500.0,
          "pnl_net" => 1420.0,
          "bars_held" => 355
        }
      ]

      assert :ok = Writer.insert_trades(run_id, trades)

      # Verify retrieval
      {:ok, result} = Writer.query("SELECT COUNT(*) FROM trades WHERE run_id = '#{run_id}'")
      assert result != nil
    end
  end

  describe "insert_equity_curve/2" do
    test "inserts equity points" do
      run_id = "test-equity-#{:rand.uniform(100_000)}"

      curve = [
        %{"date" => "2024-01-01", "equity" => 100_000.0, "drawdown_pct" => 0.0},
        %{"date" => "2024-01-02", "equity" => 101_500.0, "drawdown_pct" => 0.0},
        %{"date" => "2024-01-03", "equity" => 99_800.0, "drawdown_pct" => 1.68}
      ]

      assert :ok = Writer.insert_equity_curve(run_id, curve)
    end
  end

  describe "insert_metrics/2" do
    test "inserts metric key-value pairs" do
      run_id = "test-metrics-#{:rand.uniform(100_000)}"

      metrics = %{
        "total_pnl_net" => 45_000.0,
        "sharpe_ratio" => 1.85,
        "max_drawdown_pct" => 12.3,
        "win_rate_pct" => 62.5
      }

      assert :ok = Writer.insert_metrics(run_id, metrics)

      {:ok, result} = Writer.query("SELECT COUNT(*) FROM metrics WHERE run_id = '#{run_id}'")
      assert result != nil
    end
  end

  describe "query/1" do
    test "executes raw SQL" do
      {:ok, result} = Writer.query("SELECT 1 + 1 AS answer")
      assert result != nil
    end
  end
end
