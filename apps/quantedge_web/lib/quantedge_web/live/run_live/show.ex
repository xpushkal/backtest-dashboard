defmodule QuantEdgeWeb.RunLive.Show do
  @moduledoc "Backtest results viewer — equity curve, stats, trades, metrics."
  use QuantEdgeWeb, :live_view

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    {:ok,
     socket
     |> assign(:page_title, "Run Results")
     |> assign(:active_nav, :runs)
     |> assign(:run_id, id)
     |> assign(:active_tab, "Overview")}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>🚀 Run Results</h1>
      <a href="/runs" class="btn btn-secondary">← Back to Runs</a>
    </div>
    <div class="card">
      <p class="text-muted">Run ID: <span class="text-mono">{@run_id}</span></p>
      <p class="text-muted mt-4">Results will appear here after backtest execution.</p>
    </div>
    """
  end
end
