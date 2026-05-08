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

  # Chunk size for multi-row INSERTs. 22 cols × 500 rows = 11k params, well under DuckDB's limit.
  @chunk_size 500

  @impl true
  def handle_call({:insert_trades, run_id, trades}, _from, state) do
    cols = ~w(run_id trade_id entry_date exit_date entry_time exit_time
              option_type position_side entry_price exit_price entry_spot exit_spot
              lots lot_size pnl_gross pnl_net brokerage stt slippage_cost other_charges
              exit_reason bars_held reentry_attempt)
    col_count = length(cols)

    rows =
      trades
      |> Enum.with_index()
      |> Enum.map(fn {t, idx} ->
        [
          run_id, idx,
          to_string(t["entry_date"] || ""),
          to_string(t["exit_date"] || ""),
          to_string(t["entry_time"] || ""),
          to_string(t["exit_time"] || ""),
          to_string(t["option_type"] || ""),
          to_string(t["position_side"] || ""),
          safe_num(t["entry_price"]),
          safe_num(t["exit_price"]),
          safe_num(t["entry_spot"]),
          safe_num(t["exit_spot"]),
          safe_int(t["lots"]),
          safe_int(t["lot_size"]),
          safe_num(t["pnl_gross"]),
          safe_num(t["pnl_net"]),
          safe_num(t["brokerage"]),
          safe_num(t["stt"]),
          safe_num(t["slippage_cost"]),
          safe_num(t["other_charges"]),
          to_string(t["exit_reason"] || ""),
          safe_int(t["bars_held"]),
          safe_int(t["reentry_attempt"])
        ]
      end)

    inserted = batch_insert(state.conn, "trades", cols, col_count, rows, "trade")
    if inserted < length(rows) do
      Logger.warning("DuckDB: only #{inserted}/#{length(rows)} trades inserted for run #{run_id}")
    end

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:insert_equity, run_id, curve}, _from, state) do
    cols = ~w(run_id date equity drawdown_pct)

    rows =
      Enum.map(curve, fn point ->
        [
          run_id,
          to_string(point["date"] || point[:date] || ""),
          safe_num(point["equity"] || point[:equity]),
          safe_num(point["drawdown_pct"] || point[:drawdown_pct] || 0.0)
        ]
      end)

    batch_insert(state.conn, "equity_curves", cols, 4, rows, "equity")
    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:insert_metrics, run_id, metrics_map}, _from, state) when is_map(metrics_map) do
    cols = ~w(run_id metric_name metric_value)

    rows =
      Enum.map(metrics_map, fn {name, value} ->
        [run_id, to_string(name), safe_num(value)]
      end)

    batch_insert(state.conn, "metrics", cols, 3, rows, "metric")
    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:insert_optimizer, run_id, results}, _from, state) do
    cols = ~w(optimizer_run_id combo_index params metrics)

    rows =
      results
      |> Enum.with_index()
      |> Enum.map(fn {result, idx} ->
        [
          run_id,
          idx,
          Jason.encode!(result["params"] || %{}),
          Jason.encode!(Map.drop(result, ["params", "combo_index"]))
        ]
      end)

    batch_insert(state.conn, "optimizer_results", cols, 4, rows, "optimizer result")
    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:query, sql, params}, _from, state) do
    raw =
      case params do
        [] -> Duckdbex.query(state.conn, sql)
        _ -> Duckdbex.query(state.conn, sql, params)
      end

    result =
      case raw do
        {:ok, ref} when is_reference(ref) ->
          {:ok, Duckdbex.fetch_all(ref)}

        other ->
          other
      end

    {:reply, result, state}
  end

  # ─── Helpers ────────────────────────────────────────────────

  # Chunk rows and insert each chunk with a single multi-row INSERT,
  # all wrapped in a transaction. Single-row inserts in DuckDB are
  # ~100x slower than batched ones because each is its own commit.
  defp batch_insert(_conn, _table, _cols, _col_count, [], _label), do: 0
  defp batch_insert(conn, table, cols, col_count, rows, label) do
    col_list = Enum.join(cols, ", ")
    {:ok, _} = Duckdbex.query(conn, "BEGIN TRANSACTION")

    inserted =
      rows
      |> Enum.chunk_every(@chunk_size)
      |> Enum.reduce(0, fn chunk, acc ->
        row_count = length(chunk)
        row_placeholder = "(" <> Enum.map_join(1..col_count, ", ", fn _ -> "?" end) <> ")"
        values_clause = Enum.map_join(1..row_count, ", ", fn _ -> row_placeholder end)
        sql = "INSERT INTO #{table} (#{col_list}) VALUES #{values_clause}"
        params = List.flatten(chunk)

        case Duckdbex.query(conn, sql, params) do
          {:ok, _} ->
            acc + row_count

          {:error, reason} ->
            Logger.error("DuckDB #{label} batch insert failed (#{row_count} rows): #{inspect(reason)}")
            acc
        end
      end)

    {:ok, _} = Duckdbex.query(conn, "COMMIT")
    inserted
  end

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
        other_charges   DOUBLE,
        exit_reason     VARCHAR,
        bars_held       INTEGER,
        reentry_attempt INTEGER
      )
      """,
      "ALTER TABLE trades ADD COLUMN IF NOT EXISTS other_charges DOUBLE",
      "ALTER TABLE trades ADD COLUMN IF NOT EXISTS entry_date VARCHAR",
      "ALTER TABLE trades ADD COLUMN IF NOT EXISTS exit_date VARCHAR",
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
