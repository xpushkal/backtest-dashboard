defmodule QuantEdgeWeb.StrategyLive.Show do
  @moduledoc "Strategy detail view — config, TOML, legs breakdown, run history, and quick-run."
  use QuantEdgeWeb, :live_view

  import QuantEdgeWeb.UiComponents

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    strategy = safe_get_strategy(id)
    runs = safe_list_runs(id)
    legs = parse_legs(strategy.config_toml)

    {:ok,
     socket
     |> assign(:page_title, "#{strategy.name}")
     |> assign(:active_nav, :strategies)
     |> assign(:strategy, strategy)
     |> assign(:runs, runs)
     |> assign(:legs, legs)
     |> assign(:show_run_modal, false)
     |> assign(:run_form, %{
       "date_from" => "2022-01-01",
       "date_to" => "2025-12-31",
       "capital" => "100000"
     })}
  end

  @impl true
  def handle_event("show_run_modal", _params, socket) do
    {:noreply, assign(socket, :show_run_modal, true)}
  end

  def handle_event("hide_run_modal", _params, socket) do
    {:noreply, assign(socket, :show_run_modal, false)}
  end

  def handle_event("update_run_form", params, socket) do
    form = Map.merge(socket.assigns.run_form, Map.drop(params, ["_target"]))
    {:noreply, assign(socket, :run_form, form)}
  end

  def handle_event("quick_run", params, socket) do
    form = Map.merge(socket.assigns.run_form, Map.drop(params, ["_target"]))
    strategy = socket.assigns.strategy

    attrs = %{
      strategy_id: strategy.id,
      date_from: Date.from_iso8601!(form["date_from"]),
      date_to: Date.from_iso8601!(form["date_to"]),
      capital: Decimal.new(form["capital"] || "100000")
    }

    with {:ok, run} <- QuantEdge.Runs.create_run(attrs),
         {:ok, _job} <- QuantEdge.Runs.enqueue_backtest(run.id) do
      {:noreply,
       socket
       |> assign(:show_run_modal, false)
       |> assign(:runs, safe_list_runs(strategy.id))
       |> put_flash(:info, "Backtest queued!")}
    else
      _ -> {:noreply, put_flash(socket, :error, "Failed to start run")}
    end
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <div>
        <h1> {@strategy.name}</h1>
        <p class="text-sm text-muted mt-2">
          <.underlying_badge underlying={@strategy.underlying} />
          <span class="ml-2">Options</span>
          <span class="ml-2">· {@strategy.underlying}</span>
          <span class="ml-2">· {length(@legs)} leg(s)</span>
        </p>
      </div>
      <div class="flex-gap-2">
        <button class="btn btn-primary" phx-click="show_run_modal">▶ Run Backtest</button>
        <a href={"/strategies/#{@strategy.id}/edit"} class="btn btn-secondary"> Edit</a>
        <a href="/strategies" class="btn btn-secondary">← Back</a>
      </div>
    </div>

    <%!-- Legs Breakdown --%>
    <div class="grid-2 mb-8" style="gap: 1.5rem;">
      <div class="card">
        <h3 class="mb-4"> Legs Configuration</h3>
        <div :if={@legs == []} class="text-muted">No legs configured.</div>
        <div :for={{leg, idx} <- Enum.with_index(@legs)} class="mb-4" style="padding: 0.75rem; border: 1px solid var(--border-primary); border-radius: 8px; background: var(--bg-tertiary);">
          <div class="flex-between mb-2">
            <span class="badge badge-info">Leg {idx + 1}</span>
            <span class={"badge #{if leg.position == "sell", do: "badge-danger", else: "badge-success"}"}>
              {String.upcase(leg.position)} {leg.option_type}
            </span>
          </div>
          <div class="grid-4" style="gap: 0.75rem;">
            <div>
              <span class="text-sm text-muted">Strike</span>
              <p class="text-mono">{leg.strike}</p>
            </div>
            <div>
              <span class="text-sm text-muted">Lots</span>
              <p class="text-mono">{leg.lots}</p>
            </div>
            <div>
              <span class="text-sm text-muted">SL</span>
              <p class="text-mono">{leg.sl}</p>
            </div>
            <div>
              <span class="text-sm text-muted">Expiry</span>
              <p class="text-mono">{leg.expiry}</p>
            </div>
          </div>
        </div>
      </div>

      <%!-- TOML Config --%>
      <div class="card">
        <h3 class="mb-4"> TOML Configuration</h3>
        <div style="background: var(--bg-tertiary); border-radius: 8px; padding: 1rem; max-height: 400px; overflow-y: auto;">
          <pre class="text-mono text-sm" style="white-space: pre-wrap; color: var(--accent-cyan);">{@strategy.config_toml || "# No configuration"}</pre>
        </div>
      </div>
    </div>

    <%!-- Run History --%>
    <div class="card mb-8">
      <div class="card-header">
        <span class="card-title"> Run History ({length(@runs)} runs)</span>
      </div>
      <div :if={@runs == []} class="text-center text-muted" style="padding: 2rem;">
        No runs yet. Click "Run Backtest" above to start.
      </div>
      <table :if={@runs != []} class="data-table">
        <thead>
          <tr>
            <th>Date Range</th>
            <th>Capital</th>
            <th>Status</th>
            <th class="col-number">PnL</th>
            <th class="col-number">Sharpe</th>
            <th class="col-number">Win Rate</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr :for={run <- @runs}>
            <td class="text-sm">{fmt_date(run.date_from)} — {fmt_date(run.date_to)}</td>
            <td class="text-mono">₹{run.capital || "—"}</td>
            <td><.status_badge status={run.status} /></td>
            <td class="col-number">
              <span :if={run.result_summary["total_pnl_net"]} class={"text-mono #{if run.result_summary["total_pnl_net"] >= 0, do: "text-profit", else: "text-loss"}"}>
                ₹{round(run.result_summary["total_pnl_net"])}
              </span>
              <span :if={!run.result_summary["total_pnl_net"]} class="text-muted">—</span>
            </td>
            <td class="col-number text-mono">{fmt_num(run.result_summary["sharpe_ratio"])}</td>
            <td class="col-number text-mono">{fmt_pct(run.result_summary["win_rate_pct"])}</td>
            <td>
              <a :if={run.status == "completed"} href={"/runs/#{run.id}"} class="btn btn-sm btn-primary">View</a>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <%!-- Quick Run Modal --%>
    <div :if={@show_run_modal} class="modal-overlay" phx-click="hide_run_modal">
      <div class="modal-content" phx-click-away="hide_run_modal" style="max-width: 480px;">
        <div class="modal-header">
          <h3 class="modal-title">▶ Run {@strategy.name}</h3>
          <button class="modal-close" phx-click="hide_run_modal">×</button>
        </div>
        <form phx-submit="quick_run" phx-change="update_run_form">
          <div class="grid-2">
            <div class="input-group">
              <label class="input-label">Date From</label>
              <input type="date" name="date_from" value={@run_form["date_from"]} class="input" required />
            </div>
            <div class="input-group">
              <label class="input-label">Date To</label>
              <input type="date" name="date_to" value={@run_form["date_to"]} class="input" required />
            </div>
          </div>
          <div class="input-group">
            <label class="input-label">Capital (₹)</label>
            <input type="number" name="capital" value={@run_form["capital"]} class="input" required />
          </div>
          <div class="modal-footer">
            <button type="button" class="btn btn-secondary" phx-click="hide_run_modal">Cancel</button>
            <button type="submit" class="btn btn-primary"> Run Backtest</button>
          </div>
        </form>
      </div>
    </div>
    """
  end

  # --- Helpers ---

  defp safe_get_strategy(id) do
    try do
      QuantEdge.Strategies.get_strategy!(id)
    rescue
      _ -> %{id: id, name: "Unknown", underlying: "NIFTY", config_toml: nil, updated_at: nil}
    end
  end

  defp safe_list_runs(strategy_id) do
    try do
      QuantEdge.Runs.list_runs_for_strategy(strategy_id)
    rescue
      _ -> []
    end
  end

  defp parse_legs(nil), do: []
  defp parse_legs(toml) do
    toml
    |> String.split("[[legs]]")
    |> Enum.drop(1)
    |> Enum.map(fn leg_str ->
      %{
        option_type: extract_val(leg_str, "option_type", "CE"),
        position: extract_val(leg_str, "position", "sell"),
        lots: extract_val(leg_str, "lots", "1"),
        expiry: extract_val(leg_str, "expiry", "weekly"),
        strike: extract_strike(leg_str),
        sl: extract_sl(leg_str)
      }
    end)
  end

  defp extract_val(str, key, default) do
    case Regex.run(~r/#{key}\s*=\s*"?([^"\n]+)"?/, str) do
      [_, val] -> String.trim(val)
      _ -> default
    end
  end

  defp extract_strike(str) do
    case Regex.run(~r/strike_offset\s*=\s*(-?\d+)/, str) do
      [_, "0"] -> "ATM"
      [_, n] -> "ATM#{if String.starts_with?(n, "-"), do: n, else: "+#{n}"}"
      _ -> "ATM"
    end
  end

  defp extract_sl(str) do
    type = extract_val(str, "stop_loss_type", extract_val(str, "sl_type", "pct"))
    val = extract_val(str, "stop_loss_value", extract_val(str, "sl_value", "—"))
    "#{val} (#{type})"
  end

  defp fmt_date(nil), do: "—"
  defp fmt_date(date), do: Calendar.strftime(date, "%d %b %Y")

  defp fmt_num(nil), do: "—"
  defp fmt_num(val) when is_number(val), do: "#{Float.round(val * 1.0, 2)}"
  defp fmt_num(_), do: "—"

  defp fmt_pct(nil), do: "—"
  defp fmt_pct(val) when is_number(val), do: "#{Float.round(val * 1.0, 1)}%"
  defp fmt_pct(_), do: "—"
end
