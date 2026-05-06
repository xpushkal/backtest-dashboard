defmodule QuantEdgeWeb.StrategyLive.Show do
  @moduledoc "Strategy detail view."
  use QuantEdgeWeb, :live_view

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    {:ok,
     socket
     |> assign(:page_title, "Strategy Details")
     |> assign(:active_nav, :strategies)
     |> assign(:strategy_id, id)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>⚡ Strategy Details</h1>
      <a href="/strategies" class="btn btn-secondary">← Back</a>
    </div>
    <div class="card">
      <p class="text-muted">Strategy ID: <span class="text-mono">{@strategy_id}</span></p>
    </div>
    """
  end
end
