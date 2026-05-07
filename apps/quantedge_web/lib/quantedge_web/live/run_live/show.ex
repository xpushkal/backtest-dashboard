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

    <%!-- Hero Stats Row 1: Core Returns --%>
    <div class="grid-4 mb-4">
      <.stat_card
        label="Total PnL (Net)"
        value={fmt_currency(@summary["total_pnl_net"])}
        trend={pnl_trend(@summary["total_pnl_net"])}
        class={pnl_border(@summary["total_pnl_net"])}
      />
      <.stat_card label="CAGR" value={fmt_pct(@summary["cagr"])} />
      <.stat_card label="ROI" value={fmt_pct(@summary["roi_pct"])} />
      <.stat_card label="Win Rate" value={fmt_pct(@summary["win_rate_pct"])} />
    </div>
    <%!-- Hero Stats Row 2: Risk --%>
    <div class="grid-4 mb-4">
      <.stat_card label="Max Drawdown" value={fmt_pct(@summary["max_drawdown_pct"])} class="stat-card-loss" />
      <.stat_card label="Sharpe Ratio" value={fmt_num(@summary["sharpe_ratio"])} />
      <.stat_card label="Sortino Ratio" value={fmt_num(@summary["sortino_ratio"])} />
      <.stat_card label="Calmar Ratio" value={fmt_num(@summary["calmar_ratio"])} />
    </div>
    <%!-- Hero Stats Row 3: Trade Analytics --%>
    <div class="grid-4 mb-8">
      <.stat_card label="Profit Factor" value={fmt_num(@summary["profit_factor"])} />
      <.stat_card label="Total Trades" value={fmt_int(@summary["total_trades"])} />
      <.stat_card label="Expectancy" value={fmt_currency(@summary["expectancy"])} />
      <.stat_card label="Avg Win/Loss" value={fmt_num(@summary["win_loss_ratio"])} />
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
    <.tab_bar tabs={["Overview", "Trades", "Metrics", "Analytics"]} active={@active_tab} />

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

    <%!-- Analytics Tab (07-06) --%>
    <div :if={@active_tab == "Analytics"}>
      <%!-- Monthly PnL Heatmap --%>
      <div class="card mb-8">
        <div class="card-header">
          <span class="card-title">Monthly PnL Heatmap</span>
        </div>
        <div id="monthly-heatmap" phx-hook="MonthlyHeatmap" style="min-height: 200px;">
          <p class="text-center text-muted" style="padding: 2rem;">Loading heatmap data...</p>
        </div>
      </div>

      <div class="grid-2 mb-8">
        <%!-- Monte Carlo Confidence Bands --%>
        <div class="card">
          <div class="card-header">
            <span class="card-title">Monte Carlo Simulation</span>
          </div>
          <div id="montecarlo-chart" phx-hook="MonteCarloChart" style="height: 320px; position: relative;">
            <canvas></canvas>
          </div>
        </div>

        <%!-- Greeks Attribution --%>
        <div class="card">
          <div class="card-header">
            <span class="card-title">Greeks Attribution</span>
          </div>
          <div id="greeks-chart" phx-hook="GreeksChart" style="height: 320px; position: relative;">
            <canvas></canvas>
          </div>
        </div>
      </div>

      <%!-- Walk-Forward Analysis --%>
      <div class="card mb-8">
        <div class="card-header">
          <span class="card-title">Walk-Forward Analysis</span>
        </div>
        <div id="walkforward-chart" phx-hook="WalkForwardChart" style="height: 300px; position: relative;">
          <canvas></canvas>
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
      QuantEdge.Duck.Reader.get_metrics(id)
    rescue
      _ -> %{}
    end
  end

  defp safe_get_trades(id) do
    try do
      QuantEdge.Duck.Reader.get_trades(id)
    rescue
      _ -> []
    end
  end

  defp safe_get_equity(id) do
    try do
      QuantEdge.Duck.Reader.get_equity_curve(id)
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
    if finite?(val) do
      sign = if val >= 0, do: "+", else: "-"
      "#{sign}₹#{abs(round(val))}"
    else
      "—"
    end
  end
  defp fmt_currency(_), do: "—"

  defp fmt_pct(nil), do: "—"
  defp fmt_pct(val) when is_number(val) do
    if finite?(val), do: "#{Float.round(val * 1.0, 2)}%", else: "—"
  end
  defp fmt_pct(_), do: "—"

  defp fmt_num(nil), do: "—"
  defp fmt_num(val) when is_number(val) do
    if finite?(val), do: "#{Float.round(val * 1.0, 2)}", else: "∞"
  end
  defp fmt_num(_), do: "—"

  defp fmt_int(nil), do: "—"
  defp fmt_int(val) when is_number(val) do
    if finite?(val), do: "#{round(val)}", else: "—"
  end
  defp fmt_int(_), do: "—"

  defp fmt_trade_pnl(nil), do: "—"
  defp fmt_trade_pnl(val) when is_number(val) do
    if finite?(val) do
      sign = if val >= 0, do: "+", else: ""
      "#{sign}#{Float.round(val * 1.0, 2)}"
    else
      "—"
    end
  end
  defp fmt_trade_pnl(_), do: "—"

  defp pnl_trend(val) when is_number(val), do: if(val >= 0, do: :up, else: :down)
  defp pnl_trend(_), do: nil

  defp pnl_border(val) when is_number(val), do: if(val >= 0, do: "stat-card-profit", else: "stat-card-loss")
  defp pnl_border(_), do: ""

  defp pnl_class(val) when is_number(val), do: if(val >= 0, do: "text-profit", else: "text-loss")
  defp pnl_class(_), do: "text-muted"

  # Float NaN/Infinity guard. Erlang/Elixir Float.round/2 raises on these.
  defp finite?(v) when is_integer(v), do: true
  defp finite?(v) when is_float(v) do
    v == v and v != :infinity and abs(v) < 1.0e308
  end
  defp finite?(_), do: false

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

  @return_keys ~w(total_pnl_gross total_pnl_net cagr roi_pct expectancy profit_factor
    win_rate_pct avg_win avg_loss win_loss_ratio largest_win largest_loss gross_profit gross_loss)
  @risk_keys ~w(max_drawdown_inr max_drawdown_pct avg_drawdown sharpe_ratio sortino_ratio
    calmar_ratio omega_ratio var_95 var_99 cvar ulcer_index daily_volatility ann_volatility
    skewness kurtosis recovery_factor drawdown_duration_days)
  @trade_keys ~w(total_trades avg_hold_bars max_hold_bars max_consec_wins max_consec_losses
    sl_hit_rate_pct target_hit_rate_pct time_exit_rate_pct reentry_count reentry_win_rate)
  @cost_keys ~w(total_brokerage total_slippage total_stt_cost net_cost_ratio)

  defp metric_category(key) do
    cond do
      key in @return_keys -> "📈 Return Metrics"
      key in @risk_keys -> "🛡️ Risk Metrics"
      key in @trade_keys -> "📊 Trade Analytics"
      key in @cost_keys -> "💰 Cost Breakdown"
      true -> "📋 Other"
    end
  end

  defp category_order("📈 Return Metrics"), do: 0
  defp category_order("🛡️ Risk Metrics"), do: 1
  defp category_order("📊 Trade Analytics"), do: 2
  defp category_order("💰 Cost Breakdown"), do: 3
  defp category_order(_), do: 4

  defp humanize_key(key) do
    key
    |> String.replace("_", " ")
    |> String.split(" ")
    |> Enum.map(&String.capitalize/1)
    |> Enum.join(" ")
  end

  defp format_metric_val(val) when is_float(val) do
    if finite?(val), do: val |> Float.round(4) |> to_string(), else: "∞"
  end
  defp format_metric_val(val), do: to_string(val)
end
