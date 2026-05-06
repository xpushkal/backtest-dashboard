defmodule QuantEdgeWeb.DataExplorerLive do
  @moduledoc "Data explorer showing loaded symbols, date ranges, bar counts."
  use QuantEdgeWeb, :live_view

  @impl true
  def mount(_params, _session, socket) do
    {:ok,
     socket
     |> assign(:page_title, "Data Explorer")
     |> assign(:active_nav, :data)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>💾 Data Explorer</h1>
    </div>

    <.empty_state
      icon="💾"
      title="Data Explorer"
      description="View loaded market data coverage, IV statistics, and Parquet file details for BankNifty, Nifty, and Sensex."
    />
    """
  end

  defp empty_state(assigns), do: QuantEdgeWeb.UiComponents.empty_state(assigns)
end
