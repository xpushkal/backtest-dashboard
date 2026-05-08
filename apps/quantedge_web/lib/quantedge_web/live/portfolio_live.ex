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

    # Auto-distribute allocations equally, giving remainder to the last strategy
    n = MapSet.size(selected)
    allocations =
      if n > 0 do
        base_pct = Float.round(100.0 / n, 1)
        ids = MapSet.to_list(selected)
        {leading, [last_id]} = Enum.split(ids, n - 1)
        remainder = Float.round(100.0 - base_pct * (n - 1), 1)
        leading
        |> Enum.into(%{}, &{&1, base_pct})
        |> Map.put(last_id, remainder)
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
    cond do
      MapSet.size(socket.assigns.selected) < 2 ->
        {:noreply, put_flash(socket, :error, "Select at least 2 strategies")}

      abs((socket.assigns.allocations |> Map.values() |> Enum.sum()) - 100.0) > 0.15 ->
        {:noreply, put_flash(socket, :error, "Allocations must sum to 100%")}

      true ->
        strategies = selected_strategies(socket.assigns.strategies, socket.assigns.selected)
        portfolio_id = Ecto.UUID.generate()

        portfolio_config = build_portfolio_config(
          portfolio_id,
          strategies,
          socket.assigns.allocations,
          socket.assigns.total_capital,
          socket.assigns.date_from,
          socket.assigns.date_to
        )

        json = Jason.encode!(portfolio_config)

        case QuantEdge.Runs.enqueue_portfolio(portfolio_id, json) do
          {:ok, _job} ->
            if connected?(socket) do
              Phoenix.PubSub.subscribe(QuantEdge.PubSub, "portfolio:#{portfolio_id}")
            end

            {:noreply,
             socket
             |> assign(:running, true)
             |> assign(:progress, 0.0)
             |> assign(:portfolio_id, portfolio_id)
             |> put_flash(:info, "Portfolio backtest queued (#{length(strategies)} strategies)")}

          {:error, reason} ->
            {:noreply, put_flash(socket, :error, "Failed to queue: #{inspect(reason)}")}
        end
    end
  end

  def handle_event("switch_tab", %{"tab" => tab}, socket) do
    {:noreply, assign(socket, :active_tab, tab)}
  end

  defp build_portfolio_config(id, strategies, allocations, total_capital, date_from, date_to) do
    %{
      "name" => "Portfolio #{String.slice(id, 0..7)}",
      "total_capital" => total_capital * 1.0,
      "date_from" => date_from,
      "date_to" => date_to,
      "data_dir" => Application.get_env(:quantedge, :data_dir, "Data/parquet"),
      "strategies" =>
        Enum.map(strategies, fn s ->
          %{
            "name" => s.name,
            "underlying" => s.underlying,
            "allocation_pct" => Map.get(allocations, s.id, 0.0) * 1.0,
            "lot_size" => QuantEdge.LotSizes.get(s.underlying, Date.from_iso8601!(date_from)),
            "toml" => s.config_toml || ""
          }
        end)
    }
  end

  @impl true
  def handle_info({:portfolio_started, _id}, socket) do
    {:noreply, socket |> assign(:running, true) |> assign(:progress, 5.0)}
  end

  def handle_info({:portfolio_completed, _id, result}, socket) do
    pm = result["portfolio_metrics"] || %{}

    flat = %{
      "sharpe" => fmt_pm(pm["sharpe_ratio"]),
      "total_pnl" => fmt_pm_currency(pm["total_pnl"]),
      "max_dd" => fmt_pm_pct(pm["max_drawdown_pct"]),
      "div_benefit" => fmt_pm(pm["diversification_benefit"]),
      "raw" => result
    }

    {:noreply,
     socket
     |> assign(:running, false)
     |> assign(:progress, 100.0)
     |> assign(:results, flat)
     |> assign(:active_tab, "Results")
     |> put_flash(:info, "Portfolio backtest complete!")}
  end

  def handle_info({:portfolio_failed, _id, reason}, socket) do
    {:noreply,
     socket
     |> assign(:running, false)
     |> put_flash(:error, "Portfolio failed: #{reason}")}
  end

  def handle_info(_msg, socket), do: {:noreply, socket}

  defp fmt_pm(nil), do: "—"
  defp fmt_pm(v) when is_number(v) do
    cond do
      v != v -> "—"
      abs(v) > 1.0e308 -> "∞"
      true -> Float.round(v * 1.0, 2) |> to_string()
    end
  end
  defp fmt_pm(_), do: "—"

  defp fmt_pm_currency(nil), do: "—"
  defp fmt_pm_currency(v) when is_number(v) do
    if v != v or abs(v) > 1.0e308 do
      "—"
    else
      sign = if v >= 0, do: "+", else: "-"
      "#{sign}₹#{abs(round(v))}"
    end
  end
  defp fmt_pm_currency(_), do: "—"

  defp fmt_pm_pct(nil), do: "—"
  defp fmt_pm_pct(v) when is_number(v) do
    if v != v or abs(v) > 1.0e308, do: "—", else: "#{Float.round(v * 1.0, 2)}%"
  end
  defp fmt_pm_pct(_), do: "—"

  @impl true
  def render(assigns) do
    alloc_sum = assigns.allocations |> Map.values() |> Enum.sum() |> then(&Float.round(&1 * 1.0, 1))
    assigns = assign(assigns, :alloc_sum, alloc_sum)

    ~H"""
    <div class="page-header">
      <h1> Portfolio Builder</h1>
    </div>

    <.tab_bar tabs={["Config", "Results"]} active={@active_tab} />

    <%!-- Config Tab --%>
    <div :if={@active_tab == "Config"}>
      <div class="grid-2" style="grid-template-columns: 1fr 1fr; gap: 1.5rem;">
        <%!-- Left: Strategy Selection --%>
        <div class="card">
          <h3 class="mb-4">Select Strategies</h3>
          <div :if={@strategies == []}>
            <.empty_state icon="" title="No strategies" description="Create strategies first." />
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

            <div class={"flex-between mt-4 #{if abs(@alloc_sum - 100.0) > 0.15, do: "text-loss", else: "text-profit"}"}>
              <span class="text-sm">Total Allocation</span>
              <span class="text-mono">{@alloc_sum}%</span>
            </div>
          </div>

          <%!-- Run Button --%>
          <button
            class="btn btn-primary btn-lg w-full"
            phx-click="run_portfolio"
            disabled={@running || MapSet.size(@selected) < 2 || abs(@alloc_sum - 100.0) > 0.15}
          >
            {if @running, do: "Running...", else: " Run Portfolio Backtest"}
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
          icon=""
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
