defmodule QuantEdge.Duck.Writer do
  @moduledoc """
  Serialized DuckDB writer GenServer.

  All writes go through this single GenServer to prevent
  concurrent WAL conflicts. Reads are also routed here
  for simplicity (DuckDB supports concurrent reads natively).
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
    GenServer.call(__MODULE__, {:insert_trades, run_id, trades}, 30_000)
  end

  @doc "Insert equity curve for a backtest run."
  def insert_equity_curve(run_id, curve) do
    GenServer.call(__MODULE__, {:insert_equity, run_id, curve}, 30_000)
  end

  @doc "Insert metrics (one row per metric) for a backtest run."
  def insert_metrics(run_id, metrics_map) do
    GenServer.call(__MODULE__, {:insert_metrics, run_id, metrics_map}, 30_000)
  end

  @doc "Insert optimizer results."
  def insert_optimizer_results(run_id, results) do
    GenServer.call(__MODULE__, {:insert_optimizer, run_id, results}, 30_000)
  end

  @doc "Execute a raw SQL query (read)."
  def query(sql) do
    GenServer.call(__MODULE__, {:query, sql}, 30_000)
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
    _result =
      Enum.with_index(trades)
      |> Enum.each(fn {trade, idx} ->
        sql = """
        INSERT INTO trades VALUES (
          '#{run_id}', #{idx},
          '#{trade["entry_time"]}', '#{trade["exit_time"]}',
          '#{trade["exit_reason"]}', '#{Jason.encode!(trade["legs"] || [])}',
          #{trade["pnl_gross"]}, #{trade["pnl_net"]},
          #{trade["bars_held"] || trade["hold_bars"] || 0},
          '#{Jason.encode!(trade["greeks_entry"] || %{})}',
          '#{Jason.encode!(trade["greeks_exit"] || %{})}'
        )
        """
        Duckdbex.query(state.conn, sql)
      end)

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:insert_equity, run_id, curve}, _from, state) do
    Enum.each(curve, fn point ->
      sql = """
      INSERT INTO equity_curves VALUES (
        '#{run_id}', '#{point["date"]}',
        #{point["equity"]}, #{point["drawdown_pct"] || 0.0}
      )
      """
      Duckdbex.query(state.conn, sql)
    end)

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:insert_metrics, run_id, metrics_map}, _from, state) when is_map(metrics_map) do
    Enum.each(metrics_map, fn {name, value} ->
      numeric_value = case value do
        v when is_number(v) -> v
        _ -> 0.0
      end

      sql = "INSERT INTO metrics VALUES ('#{run_id}', '#{name}', #{numeric_value})"
      Duckdbex.query(state.conn, sql)
    end)

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:insert_optimizer, run_id, results}, _from, state) do
    Enum.with_index(results)
    |> Enum.each(fn {result, idx} ->
      sql = """
      INSERT INTO optimizer_results VALUES (
        '#{run_id}', #{idx},
        '#{Jason.encode!(result["params"] || %{})}',
        '#{Jason.encode!(result["metrics"] || %{})}'
      )
      """
      Duckdbex.query(state.conn, sql)
    end)

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:query, sql}, _from, state) do
    result = Duckdbex.query(state.conn, sql)
    {:reply, result, state}
  end

  # ─── Table Creation ────────────────────────────────────────

  defp create_tables(conn) do
    tables = [
      """
      CREATE TABLE IF NOT EXISTS trades (
        run_id        VARCHAR,
        trade_id      INTEGER,
        entry_time    VARCHAR,
        exit_time     VARCHAR,
        exit_reason   VARCHAR,
        legs          VARCHAR,
        pnl_gross     DOUBLE,
        pnl_net       DOUBLE,
        hold_bars     INTEGER,
        greeks_entry  VARCHAR,
        greeks_exit   VARCHAR
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
