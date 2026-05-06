defmodule QuantEdgeWeb.PortfolioLive do
  @moduledoc "Portfolio builder with strategy selection, capital allocation, and correlation matrix."
  use QuantEdgeWeb, :live_view

  import QuantEdgeWeb.UiComponents

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      Phoenix.PubSub.subscribe(QuantEdge.PubSub, "portfolio:updates")
    end

    strategies = safe_list_strategies()

    {:ok,
     socket
     |> assign(:page_title, "Portfolio Builder")
     |> assign(:active_nav, :portfolio)
     |> assign(:strategies, strategies)
     |> assign(:selected, MapSet.new())
     |> assign(:allocations, %{})
     |> assign(:total_capital, 500_000)
     |> assign(:date_from, "2021-01-01")
     |> assign(:date_to, "2024-12-31")
     |> assign(:running, false)
     |> assign(:progress, 0.0)
     |> assign(:results, nil)
     |> assign(:active_tab, "Config")}
  end

  @impl true
  def handle_event("toggle_strategy", %{"id" => id}, socket) do
    selected =
      if MapSet.member?(socket.assigns.selected, id) do
        MapSet.delete(socket.assigns.selected, id)
      else
        MapSet.put(socket.assigns.selected, id)
      end

    # Auto-distribute allocations equally
    n = MapSet.size(selected)
    allocations =
      if n > 0 do
        pct = Float.round(100.0 / n, 1)
        selected |> Enum.into(%{}, &{&1, pct})
      else
        %{}
      end

    {:noreply,
     socket
     |> assign(:selected, selected)
     |> assign(:allocations, allocations)}
  end

  def handle_event("update_allocation", %{"id" => id, "pct" => pct_str}, socket) do
    pct = parse_float(pct_str, 0.0)
    allocations = Map.put(socket.assigns.allocations, id, pct)
    {:noreply, assign(socket, :allocations, allocations)}
  end

  def handle_event("update_capital", %{"capital" => val}, socket) do
    {:noreply, assign(socket, :total_capital, parse_int(val, 500_000))}
  end

  def handle_event("update_dates", params, socket) do
    {:noreply,
     socket
     |> assign(:date_from, params["date_from"] || socket.assigns.date_from)
     |> assign(:date_to, params["date_to"] || socket.assigns.date_to)}
  end

  def handle_event("run_portfolio", _params, socket) do
    if MapSet.size(socket.assigns.selected) < 2 do
      {:noreply, put_flash(socket, :error, "Select at least 2 strategies")}
    else
      {:noreply,
       socket
       |> assign(:running, true)
       |> assign(:progress, 0.0)
       |> put_flash(:info, "Portfolio backtest started!")}
    end
  end

  def handle_event("switch_tab", %{"tab" => tab}, socket) do
    {:noreply, assign(socket, :active_tab, tab)}
  end

  @impl true
  def handle_info({:portfolio_progress, _id, pct}, socket) do
    {:noreply, assign(socket, :progress, pct)}
  end

  def handle_info({:portfolio_completed, _id, results}, socket) do
    {:noreply,
     socket
     |> assign(:running, false)
     |> assign(:results, results)
     |> assign(:active_tab, "Results")
     |> put_flash(:info, "Portfolio backtest complete!")}
  end

  def handle_info(_msg, socket), do: {:noreply, socket}

  @impl true
  def render(assigns) do
    alloc_sum = assigns.allocations |> Map.values() |> Enum.sum() |> Float.round(1)
    assigns = assign(assigns, :alloc_sum, alloc_sum)

    ~H"""
    <div class="page-header">
      <h1>📈 Portfolio Builder</h1>
    </div>

    <.tab_bar tabs={["Config", "Results"]} active={@active_tab} />

    <%!-- Config Tab --%>
    <div :if={@active_tab == "Config"}>
      <div class="grid-2" style="grid-template-columns: 1fr 1fr; gap: 1.5rem;">
        <%!-- Left: Strategy Selection --%>
        <div class="card">
          <h3 class="mb-4">Select Strategies</h3>
          <div :if={@strategies == []}>
            <.empty_state icon="⚡" title="No strategies" description="Create strategies first." />
          </div>
          <div :for={strategy <- @strategies} class="flex-between mb-3" style="padding: 0.5rem; border-bottom: 1px solid var(--border-primary);">
            <div class="flex-gap-3" style="align-items: center;">
              <input
                type="checkbox"
                checked={MapSet.member?(@selected, strategy.id)}
                phx-click="toggle_strategy"
                phx-value-id={strategy.id}
                style="accent-color: var(--accent-cyan);"
              />
              <div>
                <span>{strategy.name}</span>
                <span class="ml-2"><.underlying_badge underlying={strategy.underlying} /></span>
              </div>
            </div>
          </div>
        </div>

        <%!-- Right: Allocation + Config --%>
        <div>
          <%!-- Capital & Dates --%>
          <div class="card mb-6">
            <h3 class="mb-4">Configuration</h3>
            <div class="input-group">
              <label class="input-label">Total Capital (₹)</label>
              <input type="number" value={@total_capital} class="input" phx-change="update_capital" name="capital" />
            </div>
            <div class="grid-2">
              <div class="input-group">
                <label class="input-label">Date From</label>
                <input type="date" value={@date_from} class="input" phx-change="update_dates" name="date_from" />
              </div>
              <div class="input-group">
                <label class="input-label">Date To</label>
                <input type="date" value={@date_to} class="input" phx-change="update_dates" name="date_to" />
              </div>
            </div>
          </div>

          <%!-- Allocation Sliders --%>
          <div :if={MapSet.size(@selected) > 0} class="card mb-6">
            <h3 class="mb-4">Capital Allocation</h3>
            <div :for={strategy <- selected_strategies(@strategies, @selected)} class="mb-4">
              <div class="flex-between mb-1">
                <span class="text-sm">{strategy.name}</span>
                <span class="text-mono text-sm">{Map.get(@allocations, strategy.id, 0)}%</span>
              </div>
              <input
                type="range"
                min="0" max="100" step="1"
                value={Map.get(@allocations, strategy.id, 0)}
                phx-change="update_allocation"
                phx-value-id={strategy.id}
                name="pct"
                style="width: 100%; accent-color: var(--accent-cyan);"
              />
            </div>

            <div class={"flex-between mt-4 #{if @alloc_sum != 100.0, do: "text-loss", else: "text-profit"}"}>
              <span class="text-sm">Total Allocation</span>
              <span class="text-mono">{@alloc_sum}%</span>
            </div>
          </div>

          <%!-- Run Button --%>
          <button
            class="btn btn-primary btn-lg w-full"
            phx-click="run_portfolio"
            disabled={@running || MapSet.size(@selected) < 2 || @alloc_sum != 100.0}
          >
            {if @running, do: "Running...", else: "🚀 Run Portfolio Backtest"}
          </button>

          <div :if={@running} class="mt-4">
            <.progress_bar percent={@progress} label="Running portfolio..." animated={true} />
          </div>
        </div>
      </div>
    </div>

    <%!-- Results Tab --%>
    <div :if={@active_tab == "Results"}>
      <div :if={@results == nil}>
        <.empty_state
          icon="📈"
          title="No portfolio results"
          description="Run a portfolio backtest to see combined results and correlation matrix."
        />
      </div>

      <div :if={@results} class="grid-4 mb-8">
        <.stat_card label="Portfolio Sharpe" value={@results["sharpe"] || "—"} />
        <.stat_card label="Portfolio PnL" value={@results["total_pnl"] || "—"} />
        <.stat_card label="Max Portfolio DD" value={@results["max_dd"] || "—"} />
        <.stat_card label="Diversification Benefit" value={@results["div_benefit"] || "—"} />
      </div>
    </div>
    """
  end

  # --- Helpers ---

  defp selected_strategies(strategies, selected) do
    Enum.filter(strategies, &MapSet.member?(selected, &1.id))
  end

  defp parse_float(nil, default), do: default
  defp parse_float("", default), do: default
  defp parse_float(str, default) do
    case Float.parse(str) do
      {v, _} -> v
      :error -> default
    end
  end

  defp parse_int(nil, default), do: default
  defp parse_int("", default), do: default
  defp parse_int(str, default) do
    case Integer.parse(str) do
      {v, _} -> v
      :error -> default
    end
  end

  defp safe_list_strategies do
    try do
      QuantEdge.Strategies.list_strategies()
    rescue
      _ -> []
    end
  end
end
