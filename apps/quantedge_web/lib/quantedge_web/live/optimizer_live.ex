defmodule QuantEdgeWeb.OptimizerLive do
  @moduledoc "Optimizer dashboard with parameter grid config, heatmap visualization, and results."
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
     |> assign(:heatmap_y, nil)
     |> assign(:heatmap_data, nil)
     |> assign(:selected_combo, nil)
     |> assign(:show_tab, :table)}
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

  def handle_event("switch_tab", %{"tab" => tab}, socket) do
    {:noreply, assign(socket, :show_tab, String.to_existing_atom(tab))}
  end

  def handle_event("select_heatmap_x", %{"param" => param}, socket) do
    socket = assign(socket, :heatmap_x, param)
    {:noreply, rebuild_heatmap(socket)}
  end

  def handle_event("select_heatmap_y", %{"param" => param}, socket) do
    socket = assign(socket, :heatmap_y, param)
    {:noreply, rebuild_heatmap(socket)}
  end

  def handle_event("heatmap_cell_click", %{"combo_index" => idx}, socket) do
    combo = Enum.find(socket.assigns.results, fn r -> r["combo_index"] == idx end)
    {:noreply, assign(socket, :selected_combo, combo)}
  end

  def handle_event("close_modal", _params, socket) do
    {:noreply, assign(socket, :selected_combo, nil)}
  end

  @impl true
  def handle_info({:optimizer_progress, _run_id, pct}, socket) do
    {:noreply, assign(socket, :progress, pct)}
  end

  def handle_info({:optimizer_completed, _run_id, results}, socket) do
    # Auto-select heatmap axes from first two param names
    param_names = extract_param_names(results)
    hx = Enum.at(param_names, 0)
    hy = Enum.at(param_names, 1, hx)

    socket =
      socket
      |> assign(:running, false)
      |> assign(:progress, 100.0)
      |> assign(:results, results)
      |> assign(:heatmap_x, hx)
      |> assign(:heatmap_y, hy)
      |> put_flash(:info, "Optimization complete! #{length(results)} results.")

    {:noreply, rebuild_heatmap(socket)}
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

        <div :if={@results != []}>
          <%!-- Tab Switcher --%>
          <div class="card mb-4" style="padding: 0.5rem 1rem;">
            <div style="display: flex; gap: 0.5rem;">
              <button
                class={"btn btn-sm #{if @show_tab == :table, do: "btn-primary", else: "btn-secondary"}"}
                phx-click="switch_tab" phx-value-tab="table"
              >📊 Table</button>
              <button
                class={"btn btn-sm #{if @show_tab == :heatmap, do: "btn-primary", else: "btn-secondary"}"}
                phx-click="switch_tab" phx-value-tab="heatmap"
              >🗺️ Heatmap</button>
            </div>
          </div>

          <%!-- Table View --%>
          <div :if={@show_tab == :table} class="card">
            <div class="card-header">
              <span class="card-title">Top Results ({length(@results)} combos)</span>
            </div>
            <table class="data-table">
              <thead>
                <tr>
                  <th>#</th>
                  <th>Parameters</th>
                  <th class="col-number">Sharpe</th>
                  <th class="col-number">PnL</th>
                  <th class="col-number">Max DD</th>
                  <th class="col-number">Win%</th>
                </tr>
              </thead>
              <tbody>
                <tr :for={{result, idx} <- Enum.with_index(Enum.take(@results, 20))}>
                  <td class="text-muted">{idx + 1}</td>
                  <td class="text-mono text-sm">{format_params(result["params"])}</td>
                  <td class="col-number text-mono">{format_number(result["sharpe"])}</td>
                  <td class={"col-number text-mono #{pnl_class(result["total_pnl"])}"}>
                    ₹{format_number(result["total_pnl"])}
                  </td>
                  <td class="col-number text-mono text-loss">{format_number(result["max_dd_pct"])}%</td>
                  <td class="col-number text-mono">{format_number(result["win_rate"])}%</td>
                </tr>
              </tbody>
            </table>
          </div>

          <%!-- Heatmap View --%>
          <div :if={@show_tab == :heatmap} class="card">
            <div class="card-header mb-4">
              <span class="card-title">Sharpe Heatmap</span>
            </div>

            <%!-- Axis Selectors --%>
            <div :if={length(extract_param_names(@results)) >= 2} class="grid-2 mb-4" style="gap: 1rem;">
              <div class="input-group">
                <label class="input-label">X Axis</label>
                <select class="input" phx-change="select_heatmap_x" name="param">
                  <option :for={p <- extract_param_names(@results)} value={p} selected={p == @heatmap_x}>{p}</option>
                </select>
              </div>
              <div class="input-group">
                <label class="input-label">Y Axis</label>
                <select class="input" phx-change="select_heatmap_y" name="param">
                  <option :for={p <- extract_param_names(@results)} value={p} selected={p == @heatmap_y}>{p}</option>
                </select>
              </div>
            </div>

            <%!-- Heatmap Canvas --%>
            <div
              :if={@heatmap_data}
              id="optimizer-heatmap"
              phx-hook="OptimizerHeatmap"
              data-heatmap-data={Jason.encode!(@heatmap_data)}
              style="overflow-x: auto;"
            />

            <div :if={!@heatmap_data} class="text-center text-muted" style="padding: 2rem;">
              Need at least 2 parameters for heatmap visualization.
            </div>
          </div>
        </div>
      </div>
    </div>

    <%!-- Combo Detail Modal --%>
    <div :if={@selected_combo} class="modal-overlay" phx-click="close_modal">
      <div class="card" style="max-width: 500px; margin: 10vh auto; position: relative; z-index: 1001;" phx-click-away="close_modal">
        <div class="flex-between mb-4">
          <h3>Combo #{@selected_combo["combo_index"]}</h3>
          <button class="btn btn-sm btn-secondary" phx-click="close_modal">✕</button>
        </div>

        <div class="mb-4">
          <h4 class="text-sm text-muted mb-2">Parameters</h4>
          <div class="grid-2" style="gap: 0.5rem;">
            <div :for={{k, v} <- @selected_combo["params"] || %{}} class="card" style="background: var(--bg-tertiary); padding: 0.5rem 0.75rem;">
              <span class="text-sm text-muted">{k}</span>
              <span class="text-mono" style="display: block;">{v}</span>
            </div>
          </div>
        </div>

        <div class="grid-3" style="gap: 0.75rem;">
          <.stat_card label="Sharpe" value={format_number(@selected_combo["sharpe"])} />
          <.stat_card label="PnL" value={"₹#{format_number(@selected_combo["total_pnl"])}"} />
          <.stat_card label="Max DD" value={"#{format_number(@selected_combo["max_dd_pct"])}%"} />
          <.stat_card label="Win Rate" value={"#{format_number(@selected_combo["win_rate"])}%"} />
          <.stat_card label="PF" value={format_number(@selected_combo["profit_factor"])} />
          <.stat_card label="Trades" value={@selected_combo["trade_count"] || 0} />
        </div>
      </div>
    </div>
    """
  end

  # --- Heatmap Helpers ---

  defp rebuild_heatmap(socket) do
    results = socket.assigns.results
    hx = socket.assigns.heatmap_x
    hy = socket.assigns.heatmap_y

    if hx && hy && hx != hy && results != [] do
      heatmap_data = build_heatmap_data(results, hx, hy)
      assign(socket, :heatmap_data, heatmap_data)
    else
      assign(socket, :heatmap_data, nil)
    end
  end

  defp build_heatmap_data(results, x_param, y_param) do
    x_values =
      results
      |> Enum.map(fn r -> get_in(r, ["params", x_param]) end)
      |> Enum.reject(&is_nil/1)
      |> Enum.uniq()
      |> Enum.sort()

    y_values =
      results
      |> Enum.map(fn r -> get_in(r, ["params", y_param]) end)
      |> Enum.reject(&is_nil/1)
      |> Enum.uniq()
      |> Enum.sort()

    cells =
      results
      |> Enum.map(fn r ->
        x = get_in(r, ["params", x_param])
        y = get_in(r, ["params", y_param])
        if x && y do
          %{
            x: x,
            y: y,
            sharpe: r["sharpe"] || 0,
            pnl: r["total_pnl"] || 0,
            trade_count: r["trade_count"] || 0,
            combo_index: r["combo_index"] || 0
          }
        end
      end)
      |> Enum.reject(&is_nil/1)

    %{
      x_param: x_param,
      y_param: y_param,
      x_values: x_values,
      y_values: y_values,
      cells: cells
    }
  end

  defp extract_param_names(results) do
    case results do
      [first | _] ->
        case first["params"] do
          params when is_map(params) -> Map.keys(params) |> Enum.sort()
          _ -> []
        end
      _ -> []
    end
  end

  # --- General Helpers ---

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

  defp format_number(nil), do: "—"
  defp format_number(n) when is_float(n), do: :erlang.float_to_binary(n, decimals: 2)
  defp format_number(n), do: to_string(n)

  defp pnl_class(nil), do: ""
  defp pnl_class(n) when is_number(n) and n >= 0, do: "text-profit"
  defp pnl_class(_), do: "text-loss"

  defp safe_list_strategies do
    try do
      QuantEdge.Strategies.list_strategies()
    rescue
      _ -> []
    end
  end

  defp safe_enqueue_optimizer(strategy_id, params) do
    try do
      # Coerce form strings into numbers so the Rust optimizer can deserialize.
      param_grid =
        Enum.map(params, fn p ->
          %{
            "name" => p["param_name"] || "sl_value",
            "min" => parse_float(p["min"], 0.0),
            "max" => parse_float(p["max"], 0.0),
            "step" => parse_float(p["step"], 1.0)
          }
        end)

      total_combos =
        Enum.reduce(params, 1, fn p, acc ->
          min = parse_float(p["min"], 0)
          max = parse_float(p["max"], 0)
          step = parse_float(p["step"], 1)
          acc * if step > 0, do: round((max - min) / step) + 1, else: 1
        end)

      attrs = %{
        strategy_id: strategy_id,
        param_grid: param_grid,
        total_combos: total_combos
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
