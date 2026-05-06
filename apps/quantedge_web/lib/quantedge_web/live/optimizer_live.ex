defmodule QuantEdgeWeb.OptimizerLive do
  @moduledoc "Optimizer dashboard with parameter grid config, progress, and results."
  use QuantEdgeWeb, :live_view

  import QuantEdgeWeb.UiComponents

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      Phoenix.PubSub.subscribe(QuantEdge.PubSub, "optimizer:updates")
    end

    strategies = safe_list_strategies()

    {:ok,
     socket
     |> assign(:page_title, "Optimizer")
     |> assign(:active_nav, :optimizer)
     |> assign(:strategies, strategies)
     |> assign(:selected_strategy, nil)
     |> assign(:params, [default_param()])
     |> assign(:total_combos, 0)
     |> assign(:running, false)
     |> assign(:progress, 0.0)
     |> assign(:results, [])
     |> assign(:heatmap_x, nil)
     |> assign(:heatmap_y, nil)}
  end

  @impl true
  def handle_event("select_strategy", %{"strategy_id" => id}, socket) do
    {:noreply, assign(socket, :selected_strategy, id)}
  end

  def handle_event("add_param", _params, socket) do
    params = socket.assigns.params ++ [default_param()]
    {:noreply, socket |> assign(:params, params) |> update_combos()}
  end

  def handle_event("remove_param", %{"index" => idx}, socket) do
    params = List.delete_at(socket.assigns.params, String.to_integer(idx))
    params = if params == [], do: [default_param()], else: params
    {:noreply, socket |> assign(:params, params) |> update_combos()}
  end

  def handle_event("update_param", params, socket) do
    idx = String.to_integer(params["index"])
    updated = List.update_at(socket.assigns.params, idx, fn p ->
      Map.merge(p, Map.drop(params, ["index", "_target"]))
    end)
    {:noreply, socket |> assign(:params, updated) |> update_combos()}
  end

  def handle_event("start_optimization", _params, socket) do
    case socket.assigns.selected_strategy do
      nil ->
        {:noreply, put_flash(socket, :error, "Select a strategy first")}
      strategy_id ->
        case safe_enqueue_optimizer(strategy_id, socket.assigns.params) do
          {:ok, _} ->
            {:noreply,
             socket
             |> assign(:running, true)
             |> assign(:progress, 0.0)
             |> put_flash(:info, "Optimization started!")}
          {:error, reason} ->
            {:noreply, put_flash(socket, :error, "Failed: #{inspect(reason)}")}
        end
    end
  end

  @impl true
  def handle_info({:optimizer_progress, _run_id, pct}, socket) do
    {:noreply, assign(socket, :progress, pct)}
  end

  def handle_info({:optimizer_completed, _run_id, results}, socket) do
    {:noreply,
     socket
     |> assign(:running, false)
     |> assign(:progress, 100.0)
     |> assign(:results, results)
     |> put_flash(:info, "Optimization complete!")}
  end

  def handle_info(_msg, socket), do: {:noreply, socket}

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>🔧 Optimizer</h1>
    </div>

    <div class="grid-2" style="grid-template-columns: 1fr 1fr; gap: 1.5rem;">
      <%!-- Left: Config --%>
      <div>
        <%!-- Strategy Picker --%>
        <div class="card mb-6">
          <h3 class="mb-4">Strategy</h3>
          <select class="input" phx-change="select_strategy" name="strategy_id">
            <option value="">Select a strategy...</option>
            <option :for={s <- @strategies} value={s.id} selected={s.id == @selected_strategy}>
              {s.name} ({s.underlying})
            </option>
          </select>
        </div>

        <%!-- Parameter Grid --%>
        <div class="card mb-6">
          <div class="flex-between mb-4">
            <h3>Parameter Grid</h3>
            <button type="button" class="btn btn-sm btn-secondary" phx-click="add_param">+ Add Param</button>
          </div>

          <div :for={{param, idx} <- Enum.with_index(@params)} class="card mb-4" style="background: var(--bg-tertiary);">
            <div class="flex-between mb-3">
              <span class="badge badge-info">Param {idx + 1}</span>
              <button :if={length(@params) > 1} type="button" class="btn btn-sm btn-danger" phx-click="remove_param" phx-value-index={idx}>×</button>
            </div>
            <div class="grid-4">
              <div class="input-group">
                <label class="input-label">Parameter</label>
                <select class="input" name="param_name" phx-change="update_param" phx-value-index={idx}>
                  <option value="sl_value" selected={param["param_name"] == "sl_value"}>SL Value</option>
                  <option value="target_value" selected={param["param_name"] == "target_value"}>Target Value</option>
                  <option value="strike_offset" selected={param["param_name"] == "strike_offset"}>Strike Offset</option>
                  <option value="lots" selected={param["param_name"] == "lots"}>Lots</option>
                </select>
              </div>
              <div class="input-group">
                <label class="input-label">Min</label>
                <input type="number" step="any" name="min" value={param["min"]} class="input" phx-change="update_param" phx-value-index={idx} />
              </div>
              <div class="input-group">
                <label class="input-label">Max</label>
                <input type="number" step="any" name="max" value={param["max"]} class="input" phx-change="update_param" phx-value-index={idx} />
              </div>
              <div class="input-group">
                <label class="input-label">Step</label>
                <input type="number" step="any" name="step" value={param["step"]} class="input" phx-change="update_param" phx-value-index={idx} />
              </div>
            </div>
          </div>

          <div class="flex-between mt-4">
            <span class="text-sm text-muted">Total combinations: <strong class="text-mono">{@total_combos}</strong></span>
            <button
              class="btn btn-primary"
              phx-click="start_optimization"
              disabled={@running || @selected_strategy == nil}
            >
              {if @running, do: "Running...", else: "🚀 Start Optimization"}
            </button>
          </div>
        </div>

        <%!-- Progress --%>
        <div :if={@running} class="card mb-6">
          <h3 class="mb-4">Progress</h3>
          <.progress_bar percent={@progress} label="Optimizing..." animated={true} />
        </div>
      </div>

      <%!-- Right: Results --%>
      <div>
        <div :if={@results == []} class="card">
          <.empty_state
            icon="🔧"
            title="No results yet"
            description="Configure parameters and run an optimization to see results."
          />
        </div>

        <div :if={@results != []} class="card">
          <div class="card-header">
            <span class="card-title">Top Results</span>
          </div>
          <table class="data-table">
            <thead>
              <tr>
                <th>#</th>
                <th>Parameters</th>
                <th class="col-number">Sharpe</th>
                <th class="col-number">PnL</th>
                <th class="col-number">Max DD</th>
              </tr>
            </thead>
            <tbody>
              <tr :for={{result, idx} <- Enum.with_index(Enum.take(@results, 20))}>
                <td class="text-muted">{idx + 1}</td>
                <td class="text-mono text-sm">{format_params(result["params"])}</td>
                <td class="col-number text-mono">{result["sharpe"] || "—"}</td>
                <td class={"col-number text-mono #{if (result["pnl"] || 0) >= 0, do: "text-profit", else: "text-loss"}"}>
                  ₹{result["pnl"] || "—"}
                </td>
                <td class="col-number text-mono text-loss">{result["max_dd"] || "—"}%</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
    """
  end

  # --- Helpers ---

  defp default_param do
    %{"param_name" => "sl_value", "min" => "20", "max" => "50", "step" => "5"}
  end

  defp update_combos(socket) do
    total = socket.assigns.params
    |> Enum.map(fn p ->
      min = parse_float(p["min"], 0)
      max = parse_float(p["max"], 0)
      step = parse_float(p["step"], 1)
      if step > 0, do: round((max - min) / step) + 1, else: 1
    end)
    |> Enum.reduce(1, &*/2)

    assign(socket, :total_combos, total)
  end

  defp parse_float(nil, default), do: default
  defp parse_float("", default), do: default
  defp parse_float(str, default) do
    case Float.parse(str) do
      {val, _} -> val
      :error -> default
    end
  end

  defp format_params(nil), do: "—"
  defp format_params(params) when is_map(params) do
    params |> Enum.map(fn {k, v} -> "#{k}=#{v}" end) |> Enum.join(", ")
  end
  defp format_params(params), do: to_string(params)

  defp safe_list_strategies do
    try do
      QuantEdge.Strategies.list_strategies()
    rescue
      _ -> []
    end
  end

  defp safe_enqueue_optimizer(strategy_id, params) do
    try do
      param_grid = Enum.map(params, fn p ->
        %{name: p["param_name"], min: p["min"], max: p["max"], step: p["step"]}
      end)

      attrs = %{
        strategy_id: strategy_id,
        param_grid: param_grid,
        total_combos: Enum.reduce(params, 1, fn p, acc ->
          min = parse_float(p["min"], 0)
          max = parse_float(p["max"], 0)
          step = parse_float(p["step"], 1)
          acc * (if step > 0, do: round((max - min) / step) + 1, else: 1)
        end)
      }

      with {:ok, run} <- QuantEdge.Runs.create_optimizer_run(attrs),
           {:ok, _job} <- QuantEdge.Runs.enqueue_optimizer(run.id) do
        {:ok, run}
      end
    rescue
      e -> {:error, Exception.message(e)}
    end
  end
end
