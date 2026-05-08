defmodule QuantEdgeWeb.RunLive.Show do
  @moduledoc "Backtest results viewer — equity curve, hero stats, trade log, metrics."
  use QuantEdgeWeb, :live_view

  import QuantEdgeWeb.UiComponents

  require Logger

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    run = safe_get_run(id)
    metrics = safe_get_metrics(id)
    trades = safe_get_trades(id)
    equity = safe_get_equity(id) |> downsample(1500)

    Logger.info(
      "RunLive.Show mount run_id=#{inspect(id)} status=#{inspect(Map.get(run, :status))} " <>
        "metrics=#{map_size(metrics)} trades=#{length(trades)} equity=#{length(equity)}"
    )

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

  def handle_event("request_chart_data", %{"chart" => chart}, socket) do
    {:noreply, push_chart(socket, chart)}
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
        <h1> {run_name(@run)}</h1>
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
            <th>Entry</th>
            <th>Exit</th>
            <th>Side</th>
            <th>Type</th>
            <th class="col-number">Entry Px</th>
            <th class="col-number">Exit Px</th>
            <th class="col-number">Entry Spot</th>
            <th class="col-number">Exit Spot</th>
            <th class="col-number">Lots</th>
            <th>Exit Reason</th>
            <th class="col-number">PnL Gross</th>
            <th class="col-number">Costs</th>
            <th class="col-number">PnL Net</th>
            <th class="col-number">Bars</th>
          </tr>
        </thead>
        <tbody>
          <tr :for={{trade, idx} <- paged_trades(@trades, @trade_page)}>
            <td class="text-muted">{idx + 1 + @trade_page * 50}</td>
            <td class="text-sm">{trade["entry_time"] || trade["entry_date"] || "—"}</td>
            <td class="text-sm">{trade["exit_time"] || trade["exit_date"] || "—"}</td>
            <td><span class="badge badge-info">{trade["position_side"] || "—"}</span></td>
            <td class="text-mono">{trade["option_type"] || "—"}</td>
            <td class="col-number text-mono">{fmt_num(trade["entry_price"])}</td>
            <td class="col-number text-mono">{fmt_num(trade["exit_price"])}</td>
            <td class="col-number text-mono">{fmt_num(trade["entry_spot"])}</td>
            <td class="col-number text-mono">{fmt_num(trade["exit_spot"])}</td>
            <td class="col-number text-mono">{trade["lots"] || "—"}</td>
            <td><span class="badge badge-info">{trade["exit_reason"] || "—"}</span></td>
            <td class={"col-number text-mono #{pnl_class(trade["pnl_gross"])}"}>
              {fmt_trade_pnl(trade["pnl_gross"])}
            </td>
            <td class="col-number text-mono text-muted">{fmt_trade_pnl(trade_costs(trade))}</td>
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

    <%!-- Analytics Tab --%>
    <div :if={@active_tab == "Analytics"}>
      <%!-- Monthly PnL Heatmap --%>
      <div class="card mb-8">
        <div class="card-header">
          <span class="card-title">Monthly PnL Heatmap</span>
        </div>
        <div id="monthly-heatmap" phx-hook="MonthlyHeatmap" phx-update="ignore" style="min-height: 200px;">
          <p class="text-center text-muted" style="padding: 2rem;">Loading heatmap data...</p>
        </div>
      </div>

      <%!-- Daily PnL Distribution --%>
      <div class="card mb-8">
        <div class="card-header">
          <span class="card-title">Daily PnL Distribution</span>
        </div>
        <div id="daily-pnl-chart" phx-hook="DailyPnLChart" phx-update="ignore" style="height: 300px; position: relative;">
          <canvas></canvas>
        </div>
      </div>

      <%!-- Drawdown Curve --%>
      <div class="card mb-8">
        <div class="card-header">
          <span class="card-title">Drawdown Curve</span>
        </div>
        <div id="drawdown-chart" phx-hook="DrawdownChart" phx-update="ignore" style="height: 280px; position: relative;">
          <canvas></canvas>
        </div>
      </div>

      <div class="grid-2 mb-8">
        <%!-- Monte Carlo Confidence Bands --%>
        <div class="card">
          <div class="card-header">
            <span class="card-title">Monte Carlo Simulation (1000 paths)</span>
          </div>
          <div id="montecarlo-chart" phx-hook="MonteCarloChart" phx-update="ignore" style="height: 320px; position: relative;">
            <canvas></canvas>
          </div>
        </div>

        <%!-- Returns Histogram --%>
        <div class="card">
          <div class="card-header">
            <span class="card-title">Trade PnL Distribution</span>
          </div>
          <div id="returns-histogram" phx-hook="ReturnsHistogram" phx-update="ignore" style="height: 320px; position: relative;">
            <canvas></canvas>
          </div>
        </div>
      </div>

      <%!-- Greeks Attribution (only if options data exists) --%>
      <div :if={has_greeks?(@metrics)} class="card mb-8">
        <div class="card-header">
          <span class="card-title">Greeks PnL Attribution</span>
        </div>
        <div id="greeks-chart" phx-hook="GreeksChart" phx-update="ignore" style="height: 320px; position: relative;">
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

  defp push_chart(socket, "equity"),    do: push_chart_data(socket, socket.assigns.equity)
  defp push_chart(socket, "heatmap"),   do: push_heatmap(socket, socket.assigns.run_id)
  defp push_chart(socket, "daily_pnl"), do: push_daily_pnl(socket, socket.assigns.run_id)
  defp push_chart(socket, "drawdown"),  do: push_drawdown(socket, socket.assigns.equity)
  defp push_chart(socket, "montecarlo"), do: push_montecarlo(socket, socket.assigns.equity)
  defp push_chart(socket, "histogram"), do: push_returns_histogram(socket, socket.assigns.trades)
  defp push_chart(socket, "greeks"),    do: push_greeks(socket, socket.assigns.metrics)
  defp push_chart(socket, _),           do: socket

  defp push_heatmap(socket, run_id) do
    months =
      try do
        QuantEdge.Duck.Reader.get_monthly_pnl(run_id)
      rescue
        _ -> []
      end

    payload = %{
      months:
        Enum.map(months, fn m ->
          %{
            label: "#{m["year"]}-#{String.pad_leading(to_string(m["month"]), 2, "0")}",
            pnl: m["pnl"] || 0.0
          }
        end)
    }

    push_event(socket, "heatmap_data", payload)
  end

  defp push_daily_pnl(socket, run_id) do
    daily =
      try do
        QuantEdge.Duck.Reader.get_daily_pnl(run_id)
      rescue
        _ -> []
      end

    payload = %{
      labels: Enum.map(daily, &to_string(&1["date"])),
      pnl: Enum.map(daily, & &1["pnl"]),
      trades: Enum.map(daily, & &1["trades"])
    }

    push_event(socket, "daily_pnl_data", payload)
  end

  defp push_drawdown(socket, []), do: socket
  defp push_drawdown(socket, equity) do
    payload = %{
      labels: Enum.map(equity, & &1["date"]),
      drawdown: Enum.map(equity, & &1["drawdown_pct"])
    }
    push_event(socket, "drawdown_data", payload)
  end

  defp push_montecarlo(socket, equity) when length(equity) < 2, do: socket
  defp push_montecarlo(socket, equity) do
    # Downsample for the actual line + labels so the chart isn't dense.
    sampled = mc_downsample(equity, 200)
    values = Enum.map(sampled, & &1["equity"])
    labels = Enum.map(sampled, & &1["date"])

    returns =
      values
      |> Enum.chunk_every(2, 1, :discard)
      |> Enum.map(fn [a, b] -> if a in [nil, 0, 0.0], do: 0.0, else: (b - a) / a end)

    # Deterministic per run_id so the chart doesn't shift on reload.
    seed_rand(socket.assigns.run_id)
    {p5, median, p95} = monte_carlo_paths(returns, hd(values), 500)

    payload = %{
      labels: labels,
      actual: values,
      p5: p5,
      median: median,
      p95: p95
    }

    push_event(socket, "montecarlo_data", payload)
  end

  defp mc_downsample(list, max_points) when length(list) <= max_points, do: list
  defp mc_downsample(list, max_points) do
    n = length(list)
    step = n / max_points

    list
    |> Enum.with_index()
    |> Enum.filter(fn {_, i} -> rem(i, max(1, trunc(step))) == 0 end)
    |> Enum.map(&elem(&1, 0))
  end

  # Equity-curve downsampler used in mount/3 to keep the wire payload small.
  defp downsample(list, max_points) when length(list) <= max_points, do: list
  defp downsample(list, max_points), do: mc_downsample(list, max_points)

  defp seed_rand(run_id) do
    h = :erlang.phash2(run_id)
    :rand.seed(:exsss, {h, h + 1, h + 2})
  end

  defp monte_carlo_paths([], start, _n), do: {[start], [start], [start]}
  defp monte_carlo_paths(returns, start, n_paths) do
    returns_vec = List.to_tuple(returns)
    rsize = tuple_size(returns_vec)
    init_col = List.duplicate(start, n_paths)

    # Simulate column-by-column: each "column" holds the equity of every path
    # at that timestep. Avoids the O(N²) Enum.at-based transpose.
    {_, columns_rev} =
      Enum.reduce(returns, {init_col, [init_col]}, fn _, {prev, acc} ->
        next =
          Enum.map(prev, fn eq ->
            r = elem(returns_vec, :rand.uniform(rsize) - 1)
            eq * (1.0 + r)
          end)

        {next, [next | acc]}
      end)

    columns = Enum.reverse(columns_rev)

    p5 = Enum.map(columns, &percentile(&1, 0.05))
    median = Enum.map(columns, &percentile(&1, 0.50))
    p95 = Enum.map(columns, &percentile(&1, 0.95))

    {p5, median, p95}
  end

  defp percentile(values, p) do
    sorted = Enum.sort(values)
    n = length(sorted)
    idx = max(0, min(n - 1, trunc(p * (n - 1))))
    Enum.at(sorted, idx)
  end

  defp push_returns_histogram(socket, []), do: socket
  defp push_returns_histogram(socket, trades) do
    pnls =
      trades
      |> Enum.map(& &1["pnl_net"])
      |> Enum.filter(&is_number/1)

    if pnls == [] do
      socket
    else
      {min_v, max_v} = Enum.min_max(pnls)
      bin_count = 20
      span = max(max_v - min_v, 1.0)
      bin_w = span / bin_count

      bins =
        Enum.reduce(pnls, %{}, fn v, acc ->
          idx = min(bin_count - 1, max(0, trunc((v - min_v) / bin_w)))
          Map.update(acc, idx, 1, &(&1 + 1))
        end)

      labels =
        for i <- 0..(bin_count - 1) do
          lo = min_v + i * bin_w
          "#{round(lo)}"
        end

      counts = for i <- 0..(bin_count - 1), do: Map.get(bins, i, 0)

      push_event(socket, "histogram_data", %{labels: labels, counts: counts})
    end
  end

  defp push_greeks(socket, metrics) when is_map(metrics) do
    if has_greeks?(metrics) do
      payload = %{
        labels: ["Delta", "Theta", "Vega", "Gamma"],
        delta: [metrics["total_delta_pnl"] || 0.0],
        theta: [metrics["total_theta_pnl"] || 0.0],
        vega: [metrics["total_vega_pnl"] || 0.0],
        gamma: [metrics["total_gamma_pnl"] || 0.0]
      }
      push_event(socket, "greeks_data", payload)
    else
      socket
    end
  end
  defp push_greeks(socket, _), do: socket

  defp has_greeks?(metrics) when is_map(metrics) do
    Enum.any?(~w(total_delta_pnl total_theta_pnl total_vega_pnl total_gamma_pnl), fn k ->
      v = metrics[k]
      is_number(v) and v != 0
    end)
  end
  defp has_greeks?(_), do: false

  defp trade_costs(trade) do
    [trade["brokerage"], trade["stt"], trade["slippage_cost"], trade["other_charges"]]
    |> Enum.map(fn
      v when is_number(v) -> v
      _ -> 0.0
    end)
    |> Enum.sum()
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
    win_rate_pct avg_win avg_loss win_loss_ratio largest_win largest_loss gross_profit gross_loss
    payoff_ratio avg_trade_pnl median_trade_pnl best_month worst_month avg_monthly_return)
  @risk_keys ~w(max_drawdown_inr max_drawdown_pct avg_drawdown sharpe_ratio sortino_ratio
    calmar_ratio omega_ratio var_95 var_99 cvar ulcer_index daily_volatility ann_volatility
    skewness kurtosis recovery_factor drawdown_duration_days max_drawdown_duration_days
    information_ratio treynor_ratio downside_deviation tail_ratio gain_to_pain_ratio)
  @trade_keys ~w(total_trades winning_trades losing_trades avg_hold_bars max_hold_bars
    min_hold_bars max_consec_wins max_consec_losses sl_hit_rate_pct target_hit_rate_pct
    time_exit_rate_pct reentry_count reentry_win_rate avg_bars_in_winners avg_bars_in_losers
    long_count short_count long_win_rate short_win_rate)
  @cost_keys ~w(total_brokerage total_slippage total_stt_cost total_other_charges
    net_cost_ratio cost_per_trade gross_to_net_ratio)
  @options_keys ~w(premium_capture_pct total_theta_collected avg_theta_per_day
    avg_iv_at_entry avg_iv_at_exit iv_crush_pct avg_net_delta avg_dte min_dte max_dte
    pct_below_3 pct_3_to_7 pct_above_7)
  @greeks_keys ~w(total_delta_pnl total_gamma_pnl total_theta_pnl total_vega_pnl)

  defp metric_category(key) do
    cond do
      key in @return_keys -> " Return Metrics"
      key in @risk_keys -> " Risk Metrics"
      key in @trade_keys -> " Trade Analytics"
      key in @cost_keys -> " Cost Breakdown"
      key in @options_keys -> " Options Analytics"
      key in @greeks_keys -> " Greeks PnL"
      true -> " Other"
    end
  end

  defp category_order(" Return Metrics"), do: 0
  defp category_order(" Risk Metrics"), do: 1
  defp category_order(" Trade Analytics"), do: 2
  defp category_order(" Cost Breakdown"), do: 3
  defp category_order(" Options Analytics"), do: 4
  defp category_order(" Greeks PnL"), do: 5
  defp category_order(_), do: 6

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
