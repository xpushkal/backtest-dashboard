defmodule QuantEdge.Duck.Writer do
  @moduledoc """
  Serialized DuckDB writer GenServer.

  All writes go through this single GenServer to prevent
  concurrent WAL conflicts. Reads are also routed here
  for simplicity (DuckDB supports concurrent reads natively).

  All inserts use parameterized queries — string interpolation
  of trade payloads (which may contain apostrophes, NaN, or
  Infinity) is unsafe and is no longer used here.
  """
  use GenServer
  require Logger

  @db_dir "priv/duckdb"
  @db_file "quantedge.duckdb"

  # ─── Client API ────────────────────────────────────────────

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @doc "Insert trades for a backtest run."
  def insert_trades(run_id, trades) do
    GenServer.call(__MODULE__, {:insert_trades, run_id, trades}, 60_000)
  end

  @doc "Insert equity curve for a backtest run."
  def insert_equity_curve(run_id, curve) do
    GenServer.call(__MODULE__, {:insert_equity, run_id, curve}, 60_000)
  end

  @doc "Insert metrics (one row per metric) for a backtest run."
  def insert_metrics(run_id, metrics_map) do
    GenServer.call(__MODULE__, {:insert_metrics, run_id, metrics_map}, 30_000)
  end

  @doc "Insert optimizer results."
  def insert_optimizer_results(run_id, results) do
    GenServer.call(__MODULE__, {:insert_optimizer, run_id, results}, 60_000)
  end

  @doc "Execute a parameterized SQL query (read)."
  def query(sql, params \\ []) do
    GenServer.call(__MODULE__, {:query, sql, params}, 30_000)
  end

  # ─── Server Callbacks ─────────────────────────────────────

  @impl true
  def init(_opts) do
    db_path = Path.join(@db_dir, @db_file)
    File.mkdir_p!(@db_dir)

    case Duckdbex.open(db_path) do
      {:ok, db} ->
        case Duckdbex.connection(db) do
          {:ok, conn} ->
            create_tables(conn)
            Logger.info("DuckDB writer started: #{db_path}")
            {:ok, %{db: db, conn: conn}}

          {:error, reason} ->
            {:stop, {:connection_failed, reason}}
        end

      {:error, reason} ->
        {:stop, {:open_failed, reason}}
    end
  end

  @impl true
  def handle_call({:insert_trades, run_id, trades}, _from, state) do
    sql = """
    INSERT INTO trades (
      run_id, trade_id,
      entry_date, exit_date, entry_time, exit_time,
      option_type, position_side,
      entry_price, exit_price, entry_spot, exit_spot,
      lots, lot_size,
      pnl_gross, pnl_net, brokerage, stt, slippage_cost,
      exit_reason, bars_held, reentry_attempt
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    """

    inserted =
      trades
      |> Enum.with_index()
      |> Enum.reduce(0, fn {trade, idx}, acc ->
        params = [
          run_id,
          idx,
          to_string(trade["entry_date"] || ""),
          to_string(trade["exit_date"] || ""),
          to_string(trade["entry_time"] || ""),
          to_string(trade["exit_time"] || ""),
          to_string(trade["option_type"] || ""),
          to_string(trade["position_side"] || ""),
          safe_num(trade["entry_price"]),
          safe_num(trade["exit_price"]),
          safe_num(trade["entry_spot"]),
          safe_num(trade["exit_spot"]),
          safe_int(trade["lots"]),
          safe_int(trade["lot_size"]),
          safe_num(trade["pnl_gross"]),
          safe_num(trade["pnl_net"]),
          safe_num(trade["brokerage"]),
          safe_num(trade["stt"]),
          safe_num(trade["slippage_cost"]),
          to_string(trade["exit_reason"] || ""),
          safe_int(trade["bars_held"]),
          safe_int(trade["reentry_attempt"])
        ]

        case Duckdbex.query(state.conn, sql, params) do
          {:ok, _} ->
            acc + 1

          {:error, reason} ->
            Logger.error("DuckDB trade insert failed (run=#{run_id} idx=#{idx}): #{inspect(reason)}")
            acc
        end
      end)

    if inserted < length(trades) do
      Logger.warning("DuckDB: only #{inserted}/#{length(trades)} trades inserted for run #{run_id}")
    end

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:insert_equity, run_id, curve}, _from, state) do
    sql = """
    INSERT INTO equity_curves (run_id, date, equity, drawdown_pct)
    VALUES (?, ?, ?, ?)
    """

    Enum.each(curve, fn point ->
      params = [
        run_id,
        to_string(point["date"] || point[:date] || ""),
        safe_num(point["equity"] || point[:equity]),
        safe_num(point["drawdown_pct"] || point[:drawdown_pct] || 0.0)
      ]

      case Duckdbex.query(state.conn, sql, params) do
        {:ok, _} -> :ok
        {:error, reason} -> Logger.warning("Equity insert failed: #{inspect(reason)}")
      end
    end)

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:insert_metrics, run_id, metrics_map}, _from, state) when is_map(metrics_map) do
    sql = "INSERT INTO metrics (run_id, metric_name, metric_value) VALUES (?, ?, ?)"

    Enum.each(metrics_map, fn {name, value} ->
      params = [run_id, to_string(name), safe_num(value)]

      case Duckdbex.query(state.conn, sql, params) do
        {:ok, _} -> :ok
        {:error, reason} -> Logger.warning("Metric insert failed (#{name}): #{inspect(reason)}")
      end
    end)

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:insert_optimizer, run_id, results}, _from, state) do
    sql = """
    INSERT INTO optimizer_results (optimizer_run_id, combo_index, params, metrics)
    VALUES (?, ?, ?, ?)
    """

    Enum.with_index(results)
    |> Enum.each(fn {result, idx} ->
      params = [
        run_id,
        idx,
        Jason.encode!(result["params"] || %{}),
        Jason.encode!(Map.drop(result, ["params", "combo_index"]))
      ]

      case Duckdbex.query(state.conn, sql, params) do
        {:ok, _} -> :ok
        {:error, reason} -> Logger.warning("Optimizer result insert failed: #{inspect(reason)}")
      end
    end)

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:query, sql, params}, _from, state) do
    result =
      case params do
        [] -> Duckdbex.query(state.conn, sql)
        _ -> Duckdbex.query(state.conn, sql, params)
      end

    {:reply, result, state}
  end

  # ─── Helpers ────────────────────────────────────────────────

  # DuckDB rejects NaN/Infinity. Coerce them to nil so the row inserts as NULL.
  defp safe_num(nil), do: nil
  defp safe_num(v) when is_integer(v), do: v * 1.0
  defp safe_num(v) when is_float(v) do
    cond do
      v != v -> nil          # NaN
      v == :infinity -> nil  # not actually possible from Jason but defensive
      abs(v) > 1.0e308 -> nil
      true -> v
    end
  end
  defp safe_num(v) when is_binary(v) do
    case Float.parse(v) do
      {f, _} -> safe_num(f)
      :error -> nil
    end
  end
  defp safe_num(_), do: nil

  defp safe_int(nil), do: 0
  defp safe_int(v) when is_integer(v), do: v
  defp safe_int(v) when is_float(v), do: trunc(v)
  defp safe_int(v) when is_binary(v) do
    case Integer.parse(v) do
      {i, _} -> i
      :error -> 0
    end
  end
  defp safe_int(_), do: 0

  # ─── Table Creation ────────────────────────────────────────

  defp create_tables(conn) do
    tables = [
      """
      CREATE TABLE IF NOT EXISTS trades (
        run_id          VARCHAR,
        trade_id        INTEGER,
        entry_date      VARCHAR,
        exit_date       VARCHAR,
        entry_time      VARCHAR,
        exit_time       VARCHAR,
        option_type     VARCHAR,
        position_side   VARCHAR,
        entry_price     DOUBLE,
        exit_price      DOUBLE,
        entry_spot      DOUBLE,
        exit_spot       DOUBLE,
        lots            INTEGER,
        lot_size        INTEGER,
        pnl_gross       DOUBLE,
        pnl_net         DOUBLE,
        brokerage       DOUBLE,
        stt             DOUBLE,
        slippage_cost   DOUBLE,
        exit_reason     VARCHAR,
        bars_held       INTEGER,
        reentry_attempt INTEGER
      )
      """,
      """
      CREATE TABLE IF NOT EXISTS equity_curves (
        run_id        VARCHAR,
        date          VARCHAR,
        equity        DOUBLE,
        drawdown_pct  DOUBLE
      )
      """,
      """
      CREATE TABLE IF NOT EXISTS metrics (
        run_id        VARCHAR,
        metric_name   VARCHAR,
        metric_value  DOUBLE
      )
      """,
      """
      CREATE TABLE IF NOT EXISTS optimizer_results (
        optimizer_run_id VARCHAR,
        combo_index      INTEGER,
        params           VARCHAR,
        metrics          VARCHAR
      )
      """
    ]

    Enum.each(tables, fn sql ->
      case Duckdbex.query(conn, sql) do
        {:ok, _} -> :ok
        {:error, reason} -> Logger.error("DuckDB table creation failed: #{inspect(reason)}")
      end
    end)
  end
end
