defmodule QuantEdgeWeb.RunLive.Index do
  @moduledoc "Backtest runs list with real-time status updates."
  use QuantEdgeWeb, :live_view

  @impl true
  def mount(_params, _session, socket) do
    {:ok,
     socket
     |> assign(:page_title, "Runs")
     |> assign(:active_nav, :runs)
     |> assign(:runs, [])}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>🚀 Backtest Runs</h1>
      <button class="btn btn-primary">+ New Run</button>
    </div>

    <div :if={@runs == []}>
      <.empty_state
        icon="🚀"
        title="No runs yet"
        description="Configure and launch your first backtest to see results here."
        action_label="View Strategies"
        action_href="/strategies"
      />
    </div>
    """
  end

  defp empty_state(assigns), do: QuantEdgeWeb.UiComponents.empty_state(assigns)
end
