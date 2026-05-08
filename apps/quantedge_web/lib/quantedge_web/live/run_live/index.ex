defmodule QuantEdgeWeb.RunLive.Index do
  @moduledoc "Backtest runs list with real-time status updates via PubSub."
  use QuantEdgeWeb, :live_view

  import QuantEdgeWeb.UiComponents

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      Phoenix.PubSub.subscribe(QuantEdge.PubSub, "runs:updates")
    end

    runs = safe_list_runs()
    strategies = safe_list_strategies()

    {:ok,
     socket
     |> assign(:page_title, "Backtest Runs")
     |> assign(:active_nav, :runs)
     |> assign(:runs, runs)
     |> assign(:strategies, strategies)
     |> assign(:filter, "all")
     |> assign(:show_config, false)
     |> assign(:config_form, default_config())
     |> assign(:progress, %{})}
  end

  # --- Events ---

  @impl true
  def handle_event("show_config", _params, socket) do
    {:noreply, assign(socket, :show_config, true)}
  end

  def handle_event("hide_config", _params, socket) do
    {:noreply, assign(socket, :show_config, false)}
  end

  def handle_event("update_config", params, socket) do
    config = Map.merge(socket.assigns.config_form, Map.drop(params, ["_target"]))
    {:noreply, assign(socket, :config_form, config)}
  end

  def handle_event("start_run", params, socket) do
    config = Map.merge(socket.assigns.config_form, Map.drop(params, ["_target"]))

    case create_and_enqueue_run(config) do
      {:ok, _run} ->
        {:noreply,
         socket
         |> assign(:show_config, false)
         |> assign(:runs, safe_list_runs())
         |> put_flash(:info, "Backtest queued!")}

      {:error, reason} ->
        {:noreply, put_flash(socket, :error, "Failed to start run: #{inspect(reason)}")}
    end
  end

  def handle_event("filter_status", %{"status" => status}, socket) do
    {:noreply, assign(socket, :filter, status)}
  end

  # --- PubSub ---

  @impl true
  def handle_info({:run_progress, run_id, percent}, socket) do
    progress = Map.put(socket.assigns.progress, run_id, percent)
    {:noreply, assign(socket, :progress, progress)}
  end

  def handle_info({:run_completed, _run_id, _summary}, socket) do
    {:noreply, assign(socket, :runs, safe_list_runs())}
  end

  def handle_info({:run_started, _run_id}, socket) do
    {:noreply, assign(socket, :runs, safe_list_runs())}
  end

  def handle_info({:run_failed, _run_id, _reason}, socket) do
    {:noreply, assign(socket, :runs, safe_list_runs())}
  end

  def handle_info(_msg, socket), do: {:noreply, socket}

  # --- Render ---

  @impl true
  def render(assigns) do
    filtered_runs = filter_runs(assigns.runs, assigns.filter)
    assigns = assign(assigns, :filtered_runs, filtered_runs)

    ~H"""
    <div class="page-header">
      <h1>🚀 Backtest Runs</h1>
      <button class="btn btn-primary" phx-click="show_config">+ New Run</button>
    </div>

    <%!-- Status Filter --%>
    <div class="tab-bar mb-6">
      <button class={"tab-item #{if @filter == "all", do: "active"}"} phx-click="filter_status" phx-value-status="all">All ({length(@runs)})</button>
      <button class={"tab-item #{if @filter == "running", do: "active"}"} phx-click="filter_status" phx-value-status="running">Running</button>
      <button class={"tab-item #{if @filter == "completed", do: "active"}"} phx-click="filter_status" phx-value-status="completed">Completed</button>
      <button class={"tab-item #{if @filter == "failed", do: "active"}"} phx-click="filter_status" phx-value-status="failed">Failed</button>
    </div>

    <%!-- Runs Table --%>
    <div :if={@filtered_runs != []} class="card">
      <table class="data-table">
        <thead>
          <tr>
            <th>Strategy</th>
            <th>Underlying</th>
            <th>Date Range</th>
            <th>Capital</th>
            <th>Status</th>
            <th class="col-number">PnL</th>
            <th class="col-number">Sharpe</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr :for={run <- @filtered_runs}>
            <td>{run.strategy_name || "—"}</td>
            <td><.underlying_badge underlying={run.underlying || "BANKNIFTY"} /></td>
            <td class="text-sm">{fmt_date(run.date_from)} — {fmt_date(run.date_to)}</td>
            <td class="col-number text-mono">₹{run.capital || "—"}</td>
            <td>
              <.status_badge status={run.status} />
              <div :if={run.status == "running" && Map.get(@progress, run.id)}>
                <.progress_bar percent={Map.get(@progress, run.id, 0.0)} animated={true} />
              </div>
            </td>
            <td class="col-number">
              <span :if={run.result_summary["total_pnl_net"]} class={"text-mono #{if run.result_summary["total_pnl_net"] >= 0, do: "text-profit", else: "text-loss"}"}>
                ₹{round(run.result_summary["total_pnl_net"])}
              </span>
              <span :if={!run.result_summary["total_pnl_net"]} class="text-muted">—</span>
            </td>
            <td class="col-number text-mono">{run.result_summary["sharpe_ratio"] || "—"}</td>
            <td>
              <a :if={run.status == "completed"} href={"/runs/#{run.id}"} class="btn btn-sm btn-primary">Results</a>
              <span :if={run.status != "completed"} class="btn btn-sm btn-secondary" style="opacity:0.5;">Pending</span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div :if={@filtered_runs == []}>
      <.empty_state
        icon="🚀"
        title="No runs found"
        description="Configure and launch your first backtest to see results here."
      />
    </div>

    <%!-- New Run Config Modal --%>
    <div :if={@show_config} class="modal-overlay">
      <div class="modal-content" phx-click-away="hide_config" style="max-width: 640px;">
        <div class="modal-header">
          <h3 class="modal-title">🚀 Configure Backtest</h3>
          <button class="modal-close" phx-click="hide_config">×</button>
        </div>

        <form phx-submit="start_run" phx-change="update_config">
          <div class="input-group">
            <label class="input-label">Strategy</label>
            <select name="strategy_id" class="input" required>
              <option value="">Select a strategy...</option>
              <option :for={s <- @strategies} value={s.id}>{s.name} ({s.underlying})</option>
            </select>
          </div>

          <div class="grid-2">
            <div class="input-group">
              <label class="input-label">Date From</label>
              <input type="date" name="date_from" value={@config_form["date_from"]} class="input" required />
            </div>
            <div class="input-group">
              <label class="input-label">Date To</label>
              <input type="date" name="date_to" value={@config_form["date_to"]} class="input" required />
            </div>
          </div>

          <div class="grid-3">
            <div class="input-group">
              <label class="input-label">Capital (₹)</label>
              <input type="number" name="capital" value={@config_form["capital"]} class="input" />
            </div>
            <div class="input-group">
              <label class="input-label">Brokerage/Order (₹)</label>
              <input type="number" name="brokerage" value={@config_form["brokerage"]} class="input" />
            </div>
            <div class="input-group">
              <label class="input-label">Slippage (pts)</label>
              <input type="number" step="0.1" name="slippage" value={@config_form["slippage"]} class="input" />
            </div>
          </div>

          <div class="modal-footer">
            <button type="button" class="btn btn-secondary" phx-click="hide_config">Cancel</button>
            <button type="submit" class="btn btn-primary">🚀 Run Backtest</button>
          </div>
        </form>
      </div>
    </div>
    """
  end

  # --- Helpers ---

  defp default_config do
    %{
      "strategy_id" => "",
      "date_from" => "2021-01-01",
      "date_to" => "2024-12-31",
      "capital" => "100000",
      "brokerage" => "20",
      "slippage" => "0.5"
    }
  end

  defp create_and_enqueue_run(config) do
    case config["strategy_id"] do
      nil -> {:error, "No strategy selected"}
      "" -> {:error, "No strategy selected"}
      strategy_id ->
        attrs = %{
          strategy_id: strategy_id,
          date_from: Date.from_iso8601!(config["date_from"]),
          date_to: Date.from_iso8601!(config["date_to"]),
          capital: Decimal.new(config["capital"] || "100000")
        }

        with {:ok, run} <- QuantEdge.Runs.create_run(attrs),
             {:ok, _job} <- QuantEdge.Runs.enqueue_backtest(run.id) do
          {:ok, run}
        end
    end
  end

  defp safe_list_runs do
    try do
      QuantEdge.Runs.list_recent_runs(50)
    rescue
      _ -> []
    end
  end

  defp safe_list_strategies do
    try do
      QuantEdge.Strategies.list_strategies()
    rescue
      _ -> []
    end
  end

  defp filter_runs(runs, "all"), do: runs
  defp filter_runs(runs, status), do: Enum.filter(runs, &(&1.status == status))

  defp fmt_date(nil), do: "—"
  defp fmt_date(date), do: Calendar.strftime(date, "%d %b %Y")
end
