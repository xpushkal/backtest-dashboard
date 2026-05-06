defmodule QuantEdgeWeb.PortfolioLive do
  @moduledoc "Portfolio builder with capital allocation and correlation matrix."
  use QuantEdgeWeb, :live_view

  @impl true
  def mount(_params, _session, socket) do
    {:ok,
     socket
     |> assign(:page_title, "Portfolio")
     |> assign(:active_nav, :portfolio)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>📈 Portfolio Builder</h1>
    </div>

    <.empty_state
      icon="📈"
      title="Portfolio Builder"
      description="Combine multiple strategies with capital allocation to test portfolio-level performance and correlation."
      action_label="View Strategies"
      action_href="/strategies"
    />
    """
  end

  defp empty_state(assigns), do: QuantEdgeWeb.UiComponents.empty_state(assigns)
end
