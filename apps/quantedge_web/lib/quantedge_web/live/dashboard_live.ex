defmodule QuantEdgeWeb.DashboardLive do
  @moduledoc "Dashboard landing page — system overview, recent runs, quick actions."
  use QuantEdgeWeb, :live_view

  import QuantEdgeWeb.UiComponents

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      Phoenix.PubSub.subscribe(QuantEdge.PubSub, "runs:updates")
    end

    strategies = safe_list_strategies()
    runs = safe_list_recent_runs()
    best_sharpe = find_best_sharpe(runs)
    total_pnl = compute_total_pnl(runs)

    {:ok,
     socket
     |> assign(:page_title, "Dashboard")
     |> assign(:active_nav, :dashboard)
     |> assign(:strategies, strategies)
     |> assign(:runs, runs)
     |> assign(:strategy_count, length(strategies))
     |> assign(:run_count, length(runs))
     |> assign(:best_sharpe, best_sharpe)
     |> assign(:total_pnl, total_pnl)}
  end

  @impl true
  def handle_info({:run_completed, _run_id, _summary}, socket) do
    runs = safe_list_recent_runs()
    {:noreply,
     socket
     |> assign(:runs, runs)
     |> assign(:run_count, length(runs))
     |> assign(:best_sharpe, find_best_sharpe(runs))
     |> assign(:total_pnl, compute_total_pnl(runs))}
  end

  def handle_info({:run_started, _run_id}, socket) do
    {:noreply, assign(socket, :runs, safe_list_recent_runs())}
  end

  def handle_info(_msg, socket), do: {:noreply, socket}

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>📊 Dashboard</h1>
      <div class="flex-gap-3">
        <a href="/strategies/new" class="btn btn-primary">+ New Strategy</a>
        <a href="/data" class="btn btn-secondary">💾 Data Explorer</a>
      </div>
    </div>

    <%!-- Hero Stats Row --%>
    <div class="grid-4 mb-8">
      <.stat_card label="Strategies" value={to_string(@strategy_count)} subtitle="saved configs" />
      <.stat_card label="Total Runs" value={to_string(@run_count)} subtitle="backtests executed" />
      <.stat_card label="Best Sharpe" value={@best_sharpe} />
      <.stat_card label="Total PnL" value={@total_pnl} />
    </div>

    <%!-- Quick Actions --%>
    <div class="grid-3 mb-8">
      <a href="/strategies/new" class="card" style="text-decoration:none;text-align:center;">
        <div style="font-size:2rem;margin-bottom:0.5rem;">⚡</div>
        <h4>New Strategy</h4>
        <p class="text-sm text-muted mt-2">Create a multi-leg options strategy</p>
      </a>
      <a href="/runs" class="card" style="text-decoration:none;text-align:center;">
        <div style="font-size:2rem;margin-bottom:0.5rem;">🚀</div>
        <h4>Run Backtest</h4>
        <p class="text-sm text-muted mt-2">Execute a strategy against historical data</p>
      </a>
      <a href="/optimizer" class="card" style="text-decoration:none;text-align:center;">
        <div style="font-size:2rem;margin-bottom:0.5rem;">🔧</div>
        <h4>Optimize</h4>
        <p class="text-sm text-muted mt-2">Sweep parameters for best configuration</p>
      </a>
    </div>

    <%!-- Recent Runs --%>
    <div class="card">
      <div class="card-header">
        <span class="card-title">Recent Runs</span>
        <a href="/runs" class="btn btn-sm btn-secondary">View All →</a>
      </div>

      <div :if={@runs == []}>
        <.empty_state
          icon="🚀"
          title="No runs yet"
          description="Create a strategy and run your first backtest to see results here."
          action_label="Create Strategy"
          action_href="/strategies/new"
        />
      </div>

      <table :if={@runs != []} class="data-table">
        <thead>
          <tr>
            <th>Strategy</th>
            <th>Underlying</th>
            <th>Date Range</th>
            <th>Status</th>
            <th class="col-number">PnL</th>
            <th class="col-number">Sharpe</th>
            <th>Started</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr :for={run <- @runs}>
            <td>{run.strategy_name || "—"}</td>
            <td><.underlying_badge underlying={run.underlying || "BANKNIFTY"} /></td>
            <td class="text-sm">{format_date(run.date_from)} — {format_date(run.date_to)}</td>
            <td><.status_badge status={run.status} /></td>
            <td class="col-number">
              <span :if={run.result_summary["total_pnl"]} class={"text-mono #{if run.result_summary["total_pnl"] >= 0, do: "text-profit", else: "text-loss"}"}>
                ₹{format_num(run.result_summary["total_pnl"])}
              </span>
              <span :if={!run.result_summary["total_pnl"]} class="text-muted">—</span>
            </td>
            <td class="col-number text-mono">
              {run.result_summary["sharpe_ratio"] || "—"}
            </td>
            <td class="text-sm text-muted">{format_datetime(run.inserted_at)}</td>
            <td>
              <a href={"/runs/#{run.id}"} class="btn btn-sm btn-secondary">View</a>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    """
  end

  # --- Private helpers ---

  defp safe_list_strategies do
    try do
      QuantEdge.Strategies.list_strategies()
    rescue
      _ -> []
    end
  end

  defp safe_list_recent_runs do
    try do
      QuantEdge.Runs.list_recent_runs(10)
    rescue
      _ -> []
    end
  end

  defp find_best_sharpe([]), do: "—"
  defp find_best_sharpe(runs) do
    runs
    |> Enum.map(& &1.result_summary["sharpe_ratio"])
    |> Enum.reject(&is_nil/1)
    |> case do
      [] -> "—"
      values -> values |> Enum.max() |> Float.round(2) |> to_string()
    end
  end

  defp compute_total_pnl([]), do: "—"
  defp compute_total_pnl(runs) do
    runs
    |> Enum.map(& &1.result_summary["total_pnl"])
    |> Enum.reject(&is_nil/1)
    |> case do
      [] -> "—"
      values ->
        total = Enum.sum(values)
        sign = if total >= 0, do: "+", else: ""
        "#{sign}₹#{round(total)}"
    end
  end

  defp format_date(nil), do: "—"
  defp format_date(date), do: Calendar.strftime(date, "%d %b %Y")

  defp format_datetime(nil), do: "—"
  defp format_datetime(dt), do: Calendar.strftime(dt, "%d %b, %H:%M")

  defp format_num(nil), do: "—"
  defp format_num(num) when is_number(num), do: "#{round(num)}"
end
