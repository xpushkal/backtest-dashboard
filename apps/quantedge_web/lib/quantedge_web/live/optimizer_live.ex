defmodule QuantEdgeWeb.OptimizerLive do
  @moduledoc "Optimizer dashboard with parameter grid config and heatmap."
  use QuantEdgeWeb, :live_view

  @impl true
  def mount(_params, _session, socket) do
    {:ok,
     socket
     |> assign(:page_title, "Optimizer")
     |> assign(:active_nav, :optimizer)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>🔧 Optimizer</h1>
    </div>

    <.empty_state
      icon="🔧"
      title="Optimizer Dashboard"
      description="Configure parameter sweeps to find optimal strategy settings. Select a strategy and define parameter ranges to get started."
      action_label="View Strategies"
      action_href="/strategies"
    />
    """
  end

  defp empty_state(assigns), do: QuantEdgeWeb.UiComponents.empty_state(assigns)
end
