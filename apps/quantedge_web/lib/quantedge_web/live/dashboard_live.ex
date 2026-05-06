defmodule QuantEdgeWeb.DashboardLive do
  @moduledoc "Dashboard landing page — system overview, recent runs, quick actions."
  use QuantEdgeWeb, :live_view

  @impl true
  def mount(_params, _session, socket) do
    {:ok,
     socket
     |> assign(:page_title, "Dashboard")
     |> assign(:active_nav, :dashboard)
     |> assign(:strategy_count, 0)
     |> assign(:run_count, 0)
     |> assign(:best_sharpe, "—")
     |> assign(:total_pnl, "—")}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>📊 Dashboard</h1>
      <a href="/strategies/new" class="btn btn-primary">+ New Strategy</a>
    </div>

    <div class="grid-4 mb-8">
      <.stat_card label="Strategies" value={to_string(@strategy_count)} subtitle="saved" />
      <.stat_card label="Total Runs" value={to_string(@run_count)} subtitle="backtests" />
      <.stat_card label="Best Sharpe" value={@best_sharpe} />
      <.stat_card label="Total PnL" value={@total_pnl} />
    </div>

    <div class="card">
      <div class="card-header">
        <h3 class="card-title">Recent Runs</h3>
      </div>
      <.empty_state
        icon="🚀"
        title="No runs yet"
        description="Create a strategy and run your first backtest to see results here."
        action_label="Create Strategy"
        action_href="/strategies/new"
      />
    </div>
    """
  end

  defp stat_card(assigns) do
    QuantEdgeWeb.UiComponents.stat_card(assigns)
  end

  defp empty_state(assigns) do
    QuantEdgeWeb.UiComponents.empty_state(assigns)
  end
end
