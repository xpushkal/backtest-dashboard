defmodule QuantEdgeWeb.RunLive.Show do
  @moduledoc "Backtest results viewer — equity curve, hero stats, trade log, metrics."
  use QuantEdgeWeb, :live_view

  import QuantEdgeWeb.UiComponents

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    run = safe_get_run(id)
    metrics = safe_get_metrics(id)
    trades = safe_get_trades(id)
    equity = safe_get_equity(id)

    {:ok,
     socket
     |> assign(:page_title, "Run Results")
     |> assign(:active_nav, :runs)
     |> assign(:run_id, id)
     |> assign(:run, run)
     |> assign(:metrics, metrics)
     |> assign(:trades, trades)
     |> assign(:equity, equity)
     |> assign(:active_tab, "Overview")
     |> assign(:trade_page, 0)
     |> push_chart_data(equity)}
  end

  @impl true
  def handle_event("switch_tab", %{"tab" => tab}, socket) do
    {:noreply, assign(socket, :active_tab, tab)}
  end

  def handle_event("next_page", _params, socket) do
    {:noreply, assign(socket, :trade_page, socket.assigns.trade_page + 1)}
  end

  def handle_event("prev_page", _params, socket) do
    {:noreply, assign(socket, :trade_page, max(0, socket.assigns.trade_page - 1))}
  end

  @impl true
  def render(assigns) do
    summary = assigns.run.result_summary || %{}
    assigns = assign(assigns, :summary, summary)

    ~H"""
    <div class="page-header">
      <div>
        <h1>🚀 {run_name(@run)}</h1>
        <p class="text-sm text-muted mt-2">
          <.underlying_badge underlying={run_underlying(@run)} />
          <span class="ml-2">{fmt_date(@run.date_from)} — {fmt_date(@run.date_to)}</span>
          <span class="ml-2">·</span>
          <.status_badge status={@run.status} />
        </p>
      </div>
      <a href="/runs" class="btn btn-secondary">← Back to Runs</a>
    </div>

    <%!-- Hero Stats --%>
    <div class="grid-4 mb-8">
      <.stat_card
        label="Total PnL"
        value={fmt_currency(@summary["total_pnl"])}
        trend={pnl_trend(@summary["total_pnl"])}
        class={pnl_border(@summary["total_pnl"])}
      />
      <.stat_card label="CAGR" value={fmt_pct(@summary["cagr"])} />
      <.stat_card label="Win Rate" value={fmt_pct(@summary["win_rate_pct"])} />
      <.stat_card
        label="Max Drawdown"
        value={fmt_pct(@summary["max_drawdown_pct"])}
        class="stat-card-loss"
      />
    </div>
    <div class="grid-4 mb-8">
      <.stat_card label="Sharpe Ratio" value={fmt_num(@summary["sharpe_ratio"])} />
      <.stat_card label="Profit Factor" value={fmt_num(@summary["profit_factor"])} />
      <.stat_card label="Total Trades" value={fmt_int(@summary["total_trades"])} />
      <.stat_card label="Premium Capture" value={fmt_pct(@summary["premium_capture_pct"])} />
    </div>

    <%!-- Equity Curve Chart --%>
    <div class="card mb-8">
      <div class="card-header">
        <span class="card-title">Equity Curve</span>
      </div>
      <div id="equity-chart" phx-hook="EquityChart" style="height: 360px; position: relative;">
        <canvas id="equity-canvas"></canvas>
      </div>
      <div :if={@equity == []} class="text-center text-muted" style="padding: 4rem;">
        No equity data available for this run.
      </div>
    </div>

    <%!-- Tabs --%>
    <.tab_bar tabs={["Overview", "Trades", "Metrics"]} active={@active_tab} />

    <%!-- Overview Tab --%>
    <div :if={@active_tab == "Overview"} class="card">
      <h3 class="mb-4">Run Configuration</h3>
      <div class="grid-3 mb-6">
        <div>
          <span class="text-sm text-muted">Capital</span>
          <p class="text-mono">₹{@run.capital || "—"}</p>
        </div>
        <div>
          <span class="text-sm text-muted">Date Range</span>
          <p>{fmt_date(@run.date_from)} — {fmt_date(@run.date_to)}</p>
        </div>
        <div>
          <span class="text-sm text-muted">Status</span>
          <p><.status_badge status={@run.status} /></p>
        </div>
      </div>

      <h3 class="mb-4 mt-6">Strategy TOML</h3>
      <div class="card" style="background: var(--bg-tertiary);">
        <pre class="text-mono text-sm" style="white-space: pre-wrap; color: var(--accent-cyan);">{strategy_toml(@run)}</pre>
      </div>
    </div>

    <%!-- Trades Tab --%>
    <div :if={@active_tab == "Trades"} class="card">
      <div class="card-header">
        <span class="card-title">Trade Log ({length(@trades)} trades)</span>
      </div>

      <div :if={@trades == []} class="text-center text-muted" style="padding: 2rem;">
        No trade data available.
      </div>

      <table :if={@trades != []} class="data-table">
        <thead>
          <tr>
            <th>#</th>
            <th>Entry Time</th>
            <th>Exit Time</th>
            <th>Exit Reason</th>
            <th class="col-number">PnL Gross</th>
            <th class="col-number">PnL Net</th>
            <th class="col-number">Bars Held</th>
          </tr>
        </thead>
        <tbody>
          <tr :for={{trade, idx} <- paged_trades(@trades, @trade_page)}>
            <td class="text-muted">{idx + 1 + @trade_page * 50}</td>
            <td class="text-sm">{trade["entry_time"] || "—"}</td>
            <td class="text-sm">{trade["exit_time"] || "—"}</td>
            <td><span class="badge badge-info">{trade["exit_reason"] || "—"}</span></td>
            <td class={"col-number text-mono #{pnl_class(trade["pnl_gross"])}"}>
              {fmt_trade_pnl(trade["pnl_gross"])}
            </td>
            <td class={"col-number text-mono #{pnl_class(trade["pnl_net"])}"}>
              {fmt_trade_pnl(trade["pnl_net"])}
            </td>
            <td class="col-number text-mono">{trade["bars_held"] || "—"}</td>
          </tr>
        </tbody>
      </table>

      <div :if={length(@trades) > 50} class="flex-between mt-4">
        <button :if={@trade_page > 0} class="btn btn-sm btn-secondary" phx-click="prev_page">← Previous</button>
        <span class="text-sm text-muted">Page {@trade_page + 1} of {ceil(length(@trades) / 50)}</span>
        <button :if={(@trade_page + 1) * 50 < length(@trades)} class="btn btn-sm btn-secondary" phx-click="next_page">Next →</button>
      </div>
    </div>

    <%!-- Metrics Tab --%>
    <div :if={@active_tab == "Metrics"} class="card">
      <div class="card-header">
        <span class="card-title">All Metrics ({map_size(@metrics)} metrics)</span>
      </div>

      <div :if={@metrics == %{}} class="text-center text-muted" style="padding: 2rem;">
        No metrics data available.
      </div>

      <div :if={@metrics != %{}} class="grid-2">
        <div :for={{category, items} <- grouped_metrics(@metrics)} class="mb-6">
          <h4 class="mb-3" style="color: var(--accent-cyan);">{category}</h4>
          <div :for={{key, val} <- items} class="flex-between mb-2" style="padding: 0.25rem 0; border-bottom: 1px solid var(--border-primary);">
            <span class="text-sm">{humanize_key(key)}</span>
            <span class="text-mono text-sm">{format_metric_val(val)}</span>
          </div>
        </div>
      </div>
    </div>
    """
  end

  # --- Helpers ---

  defp push_chart_data(socket, []), do: socket
  defp push_chart_data(socket, equity) do
    chart_data = %{
      labels: Enum.map(equity, & &1["date"]),
      equity: Enum.map(equity, & &1["equity"]),
      drawdown: Enum.map(equity, & &1["drawdown_pct"])
    }
    push_event(socket, "equity_data", chart_data)
  end

  defp safe_get_run(id) do
    try do
      QuantEdge.Runs.get_run!(id)
    rescue
      _ -> %{id: id, status: "unknown", date_from: nil, date_to: nil, capital: nil,
             result_summary: %{}, strategy: nil, inserted_at: nil}
    end
  end

  defp safe_get_metrics(id) do
    try do
      {:ok, metrics} = QuantEdge.Duck.Reader.get_metrics(id)
      metrics
    rescue
      _ -> %{}
    end
  end

  defp safe_get_trades(id) do
    try do
      {:ok, trades} = QuantEdge.Duck.Reader.get_trades(id)
      trades
    rescue
      _ -> []
    end
  end

  defp safe_get_equity(id) do
    try do
      {:ok, equity} = QuantEdge.Duck.Reader.get_equity_curve(id)
      equity
    rescue
      _ -> []
    end
  end

  defp run_name(run) do
    case run do
      %{strategy: %{name: name}} -> name
      _ -> "Run Results"
    end
  end

  defp run_underlying(run) do
    case run do
      %{strategy: %{underlying: u}} -> u
      _ -> "BANKNIFTY"
    end
  end

  defp strategy_toml(run) do
    case run do
      %{strategy: %{config_toml: toml}} when is_binary(toml) -> toml
      _ -> "# No TOML configuration available"
    end
  end

  defp fmt_date(nil), do: "—"
  defp fmt_date(date), do: Calendar.strftime(date, "%d %b %Y")

  defp fmt_currency(nil), do: "—"
  defp fmt_currency(val) when is_number(val) do
    sign = if val >= 0, do: "+", else: "-"
    "#{sign}₹#{abs(round(val))}"
  end

  defp fmt_pct(nil), do: "—"
  defp fmt_pct(val) when is_number(val), do: "#{Float.round(val * 1.0, 2)}%"

  defp fmt_num(nil), do: "—"
  defp fmt_num(val) when is_number(val), do: "#{Float.round(val * 1.0, 2)}"

  defp fmt_int(nil), do: "—"
  defp fmt_int(val) when is_number(val), do: "#{round(val)}"

  defp fmt_trade_pnl(nil), do: "—"
  defp fmt_trade_pnl(val) when is_number(val) do
    sign = if val >= 0, do: "+", else: ""
    "#{sign}#{Float.round(val * 1.0, 2)}"
  end

  defp pnl_trend(nil), do: nil
  defp pnl_trend(val) when val >= 0, do: :up
  defp pnl_trend(_), do: :down

  defp pnl_border(nil), do: ""
  defp pnl_border(val) when val >= 0, do: "stat-card-profit"
  defp pnl_border(_), do: "stat-card-loss"

  defp pnl_class(nil), do: "text-muted"
  defp pnl_class(val) when val >= 0, do: "text-profit"
  defp pnl_class(_), do: "text-loss"

  defp paged_trades(trades, page) do
    trades
    |> Enum.drop(page * 50)
    |> Enum.take(50)
    |> Enum.with_index()
  end

  defp grouped_metrics(metrics) when is_map(metrics) do
    metrics
    |> Enum.group_by(fn {key, _val} -> metric_category(key) end)
    |> Enum.sort_by(fn {cat, _} -> category_order(cat) end)
  end

  defp metric_category(key) do
    cond do
      String.contains?(key, ["pnl", "return", "cagr", "profit"]) -> "Returns"
      String.contains?(key, ["drawdown", "sharpe", "sortino", "var", "cvar"]) -> "Risk"
      String.contains?(key, ["trade", "win", "loss", "avg", "max_consecutive"]) -> "Trade Stats"
      String.contains?(key, ["premium", "theta", "delta", "gamma", "vega", "greeks"]) -> "Options"
      String.contains?(key, ["monthly", "weekly", "daily", "annual"]) -> "Time-Based"
      true -> "Other"
    end
  end

  defp category_order("Returns"), do: 0
  defp category_order("Risk"), do: 1
  defp category_order("Trade Stats"), do: 2
  defp category_order("Options"), do: 3
  defp category_order("Time-Based"), do: 4
  defp category_order(_), do: 5

  defp humanize_key(key) do
    key
    |> String.replace("_", " ")
    |> String.split(" ")
    |> Enum.map(&String.capitalize/1)
    |> Enum.join(" ")
  end

  defp format_metric_val(val) when is_float(val), do: Float.round(val, 4) |> to_string()
  defp format_metric_val(val), do: to_string(val)
end
