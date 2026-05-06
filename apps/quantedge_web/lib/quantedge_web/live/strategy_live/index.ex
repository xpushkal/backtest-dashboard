defmodule QuantEdgeWeb.StrategyLive.Index do
  @moduledoc "Strategy list and builder."
  use QuantEdgeWeb, :live_view

  @impl true
  def mount(_params, _session, socket) do
    {:ok,
     socket
     |> assign(:page_title, "Strategies")
     |> assign(:active_nav, :strategies)
     |> assign(:strategies, [])}
  end

  @impl true
  def handle_params(_params, _url, socket) do
    {:noreply, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>⚡ Strategies</h1>
      <a href="/strategies/new" class="btn btn-primary">+ New Strategy</a>
    </div>

    <div :if={@strategies == []} >
      <.empty_state
        icon="⚡"
        title="No strategies yet"
        description="Create your first multi-leg options strategy to start backtesting."
        action_label="Create Strategy"
        action_href="/strategies/new"
      />
    </div>
    """
  end

  defp empty_state(assigns), do: QuantEdgeWeb.UiComponents.empty_state(assigns)
end
