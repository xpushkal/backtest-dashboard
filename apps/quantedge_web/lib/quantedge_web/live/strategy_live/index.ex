defmodule QuantEdgeWeb.StrategyLive.Index do
  @moduledoc "Strategy list and CRUD with filtering."
  use QuantEdgeWeb, :live_view

  import QuantEdgeWeb.UiComponents

  @impl true
  def mount(_params, _session, socket) do
    strategies = safe_list_strategies()

    {:ok,
     socket
     |> assign(:page_title, "Strategies")
     |> assign(:active_nav, :strategies)
     |> assign(:strategies, strategies)
     |> assign(:filter, "all")
     |> assign(:show_delete_modal, nil)}
  end

  @impl true
  def handle_params(params, _url, socket) do
    {:noreply, apply_action(socket, socket.assigns.live_action, params)}
  end

  defp apply_action(socket, :new, _params) do
    socket
    |> assign(:page_title, "New Strategy")
    |> assign(:show_form, true)
    |> assign(:editing_strategy, nil)
    |> assign(:form, build_form(%{}))
    |> assign(:legs, [default_leg()])
    |> assign(:toml_preview, "")
  end

  defp apply_action(socket, :edit, %{"id" => id}) do
    strategy = safe_get_strategy(id)
    legs = parse_legs_from_toml(strategy)

    socket
    |> assign(:page_title, "Edit Strategy")
    |> assign(:show_form, true)
    |> assign(:editing_strategy, strategy)
    |> assign(:form, build_form(%{
      "name" => strategy.name,
      "underlying" => strategy.underlying,
      "capital" => "100000",
      "entry_time" => "09:20",
      "exit_time" => "15:15",
      "lot_size" => "15"
    }))
    |> assign(:legs, legs)
    |> assign(:toml_preview, strategy.config_toml || "")
  end

  defp apply_action(socket, _action, _params) do
    socket
    |> assign(:show_form, false)
    |> assign(:editing_strategy, nil)
  end

  # --- Events ---

  @impl true
  def handle_event("filter", %{"filter" => filter}, socket) do
    strategies = safe_list_strategies()
    filtered =
      case filter do
        "all" -> strategies
        underlying -> Enum.filter(strategies, &(&1.underlying == underlying))
      end

    {:noreply,
     socket
     |> assign(:filter, filter)
     |> assign(:strategies, filtered)}
  end

  def handle_event("delete_strategy", %{"id" => id}, socket) do
    {:noreply, assign(socket, :show_delete_modal, id)}
  end

  def handle_event("confirm_delete", _params, socket) do
    case socket.assigns.show_delete_modal do
      nil -> {:noreply, socket}
      id ->
        try do
          strategy = QuantEdge.Strategies.get_strategy!(id)
          QuantEdge.Strategies.delete_strategy(strategy)
        rescue
          _ -> :ok
        end

        {:noreply,
         socket
         |> assign(:show_delete_modal, nil)
         |> assign(:strategies, safe_list_strategies())
         |> put_flash(:info, "Strategy deleted")}
    end
  end

  def handle_event("cancel_delete", _params, socket) do
    {:noreply, assign(socket, :show_delete_modal, nil)}
  end

  def handle_event("add_leg", _params, socket) do
    legs = socket.assigns.legs ++ [default_leg()]
    {:noreply, socket |> assign(:legs, legs) |> update_toml_preview()}
  end

  def handle_event("remove_leg", %{"index" => index}, socket) do
    idx = String.to_integer(index)
    legs = List.delete_at(socket.assigns.legs, idx)
    legs = if legs == [], do: [default_leg()], else: legs
    {:noreply, socket |> assign(:legs, legs) |> update_toml_preview()}
  end

  def handle_event("update_leg", params, socket) do
    idx = String.to_integer(params["index"])
    legs = List.update_at(socket.assigns.legs, idx, fn leg ->
      Map.merge(leg, Map.drop(params, ["index", "_target"]))
    end)
    {:noreply, socket |> assign(:legs, legs) |> update_toml_preview()}
  end

  def handle_event("update_form", params, socket) do
    form = build_form(Map.merge(socket.assigns.form.params, Map.drop(params, ["_target"])))
    {:noreply, socket |> assign(:form, form) |> update_toml_preview()}
  end

  def handle_event("save_strategy", params, socket) do
    form_data = Map.merge(socket.assigns.form.params, Map.drop(params, ["_target"]))
    toml = generate_toml(form_data, socket.assigns.legs)

    attrs = %{
      name: form_data["name"] || "Unnamed",
      underlying: form_data["underlying"] || "BANKNIFTY",
      config_toml: toml
    }

    result =
      case socket.assigns.editing_strategy do
        nil -> QuantEdge.Strategies.create_strategy(attrs)
        strategy -> QuantEdge.Strategies.update_strategy(strategy, attrs)
      end

    case result do
      {:ok, _strategy} ->
        {:noreply,
         socket
         |> put_flash(:info, "Strategy saved!")
         |> push_navigate(to: "/strategies")}

      {:error, _changeset} ->
        {:noreply, put_flash(socket, :error, "Failed to save strategy")}
    end
  end

  # --- Render ---

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>⚡ Strategies</h1>
      <a href="/strategies/new" class="btn btn-primary">+ New Strategy</a>
    </div>

    <%!-- Filter Tabs --%>
    <div class="tab-bar mb-6">
      <button class={"tab-item #{if @filter == "all", do: "active"}"} phx-click="filter" phx-value-filter="all">All</button>
      <button class={"tab-item #{if @filter == "BANKNIFTY", do: "active"}"} phx-click="filter" phx-value-filter="BANKNIFTY">BankNifty</button>
      <button class={"tab-item #{if @filter == "NIFTY", do: "active"}"} phx-click="filter" phx-value-filter="NIFTY">Nifty</button>
      <button class={"tab-item #{if @filter == "SENSEX", do: "active"}"} phx-click="filter" phx-value-filter="SENSEX">Sensex</button>
    </div>

    <%!-- Strategy Cards Grid --%>
    <div :if={@strategies != [] and !@show_form} class="grid-3 mb-8">
      <div :for={strategy <- @strategies} class="card">
        <div class="flex-between mb-4">
          <h3>{strategy.name}</h3>
          <.underlying_badge underlying={strategy.underlying} />
        </div>
        <p class="text-sm text-muted mb-4">
          {count_legs(strategy)} leg(s) · Updated {format_date(strategy.updated_at)}
        </p>
        <div class="flex-gap-2">
          <a href={"/strategies/#{strategy.id}/edit"} class="btn btn-sm btn-secondary">Edit</a>
          <button class="btn btn-sm btn-danger" phx-click="delete_strategy" phx-value-id={strategy.id}>Delete</button>
        </div>
      </div>
    </div>

    <div :if={@strategies == [] and !@show_form}>
      <.empty_state
        icon="⚡"
        title="No strategies yet"
        description="Create your first multi-leg options strategy to start backtesting."
        action_label="Create Strategy"
        action_href="/strategies/new"
      />
    </div>

    <%!-- Strategy Builder Form --%>
    <div :if={@show_form} class="card">
      <h2 class="mb-6">{if @editing_strategy, do: "Edit Strategy", else: "New Strategy"}</h2>

      <form phx-submit="save_strategy" phx-change="update_form">
        <div class="grid-2 mb-6">
          <div class="input-group">
            <label class="input-label">Strategy Name</label>
            <input type="text" name="name" value={@form.params["name"]} class="input" placeholder="e.g. Short Straddle BN" required />
          </div>
          <div class="input-group">
            <label class="input-label">Underlying</label>
            <select name="underlying" class="input">
              <option value="BANKNIFTY" selected={@form.params["underlying"] == "BANKNIFTY"}>BankNifty</option>
              <option value="NIFTY" selected={@form.params["underlying"] == "NIFTY"}>Nifty</option>
              <option value="SENSEX" selected={@form.params["underlying"] == "SENSEX"}>Sensex</option>
            </select>
          </div>
        </div>

        <div class="grid-4 mb-6">
          <div class="input-group">
            <label class="input-label">Capital (₹)</label>
            <input type="number" name="capital" value={@form.params["capital"] || "100000"} class="input" />
          </div>
          <div class="input-group">
            <label class="input-label">Entry Time</label>
            <input type="text" name="entry_time" value={@form.params["entry_time"] || "09:20"} class="input" placeholder="HH:MM" />
          </div>
          <div class="input-group">
            <label class="input-label">Exit Time</label>
            <input type="text" name="exit_time" value={@form.params["exit_time"] || "15:15"} class="input" placeholder="HH:MM" />
          </div>
          <div class="input-group">
            <label class="input-label">Lot Size</label>
            <input type="number" name="lot_size" value={@form.params["lot_size"] || "15"} class="input" />
          </div>
        </div>

        <%!-- Legs --%>
        <div class="flex-between mb-4">
          <h3>Legs</h3>
          <button type="button" class="btn btn-sm btn-secondary" phx-click="add_leg">+ Add Leg</button>
        </div>

        <div :for={{leg, idx} <- Enum.with_index(@legs)} class="card mb-4" style="background: var(--bg-tertiary);">
          <div class="flex-between mb-4">
            <span class="badge badge-info">Leg {idx + 1}</span>
            <button :if={length(@legs) > 1} type="button" class="btn btn-sm btn-danger" phx-click="remove_leg" phx-value-index={idx}>Remove</button>
          </div>

          <div class="grid-4 mb-4">
            <div class="input-group">
              <label class="input-label">Option Type</label>
              <select name={"leg_option_type_#{idx}"} class="input" phx-change="update_leg" phx-value-index={idx}>
                <option value="CE" selected={leg["option_type"] == "CE"}>CE</option>
                <option value="PE" selected={leg["option_type"] == "PE"}>PE</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">Position</label>
              <select name={"leg_position_#{idx}"} class="input" phx-change="update_leg" phx-value-index={idx}>
                <option value="sell" selected={leg["position"] == "sell"}>Sell</option>
                <option value="buy" selected={leg["position"] == "buy"}>Buy</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">Lots</label>
              <input type="number" name={"leg_lots_#{idx}"} value={leg["lots"] || "1"} class="input" phx-change="update_leg" phx-value-index={idx} />
            </div>
            <div class="input-group">
              <label class="input-label">Strike Offset</label>
              <input type="number" name={"leg_strike_offset_#{idx}"} value={leg["strike_offset"] || "0"} class="input" phx-change="update_leg" phx-value-index={idx} />
            </div>
          </div>

          <div class="grid-4">
            <div class="input-group">
              <label class="input-label">Expiry</label>
              <select name={"leg_expiry_#{idx}"} class="input" phx-change="update_leg" phx-value-index={idx}>
                <option value="weekly" selected={leg["expiry"] == "weekly"}>Weekly</option>
                <option value="monthly" selected={leg["expiry"] == "monthly"}>Monthly</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">SL Type</label>
              <select name={"leg_sl_type_#{idx}"} class="input" phx-change="update_leg" phx-value-index={idx}>
                <option value="percent_of_premium" selected={leg["sl_type"] == "percent_of_premium"}>% of Premium</option>
                <option value="points" selected={leg["sl_type"] == "points"}>Points</option>
                <option value="percent_of_margin" selected={leg["sl_type"] == "percent_of_margin"}>% of Margin</option>
                <option value="index_points" selected={leg["sl_type"] == "index_points"}>Index Points</option>
                <option value="none" selected={leg["sl_type"] == "none"}>None</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">SL Value</label>
              <input type="number" step="0.1" name={"leg_sl_value_#{idx}"} value={leg["sl_value"] || "30"} class="input" phx-change="update_leg" phx-value-index={idx} />
            </div>
            <div class="input-group">
              <label class="input-label">Target Value</label>
              <input type="number" step="0.1" name={"leg_target_value_#{idx}"} value={leg["target_value"] || ""} class="input" placeholder="Optional" phx-change="update_leg" phx-value-index={idx} />
            </div>
          </div>
        </div>

        <%!-- TOML Preview --%>
        <div :if={@toml_preview != ""} class="card mb-6" style="background: var(--bg-tertiary);">
          <div class="card-header">
            <span class="card-title">TOML Preview</span>
          </div>
          <pre class="text-mono text-sm" style="white-space: pre-wrap; color: var(--accent-cyan);">{@toml_preview}</pre>
        </div>

        <div class="flex-gap-3">
          <button type="submit" class="btn btn-primary btn-lg">
            {if @editing_strategy, do: "Update Strategy", else: "Save Strategy"}
          </button>
          <a href="/strategies" class="btn btn-secondary btn-lg">Cancel</a>
        </div>
      </form>
    </div>

    <%!-- Delete Modal --%>
    <div :if={@show_delete_modal} class="modal-overlay" phx-click="cancel_delete">
      <div class="modal-content" phx-click-away="cancel_delete">
        <div class="modal-header">
          <h3 class="modal-title">Delete Strategy?</h3>
          <button class="modal-close" phx-click="cancel_delete">×</button>
        </div>
        <p>This action cannot be undone. All associated runs will be preserved.</p>
        <div class="modal-footer">
          <button class="btn btn-secondary" phx-click="cancel_delete">Cancel</button>
          <button class="btn btn-danger" phx-click="confirm_delete">Delete</button>
        </div>
      </div>
    </div>
    """
  end

  # --- Helpers ---

  defp build_form(params) do
    %{params: params}
  end

  defp default_leg do
    %{
      "option_type" => "CE",
      "position" => "sell",
      "lots" => "1",
      "expiry" => "weekly",
      "strike_offset" => "0",
      "sl_type" => "percent_of_premium",
      "sl_value" => "30",
      "target_value" => ""
    }
  end

  defp update_toml_preview(socket) do
    toml = generate_toml(socket.assigns.form.params, socket.assigns.legs)
    assign(socket, :toml_preview, toml)
  end

  defp generate_toml(form, legs) do
    strategy_section = """
    [strategy]
    name = "#{form["name"] || "Unnamed"}"
    underlying = "#{form["underlying"] || "BANKNIFTY"}"
    capital = #{form["capital"] || "100000"}
    entry_time = "#{form["entry_time"] || "09:20"}"
    exit_time = "#{form["exit_time"] || "15:15"}"
    lot_size = #{form["lot_size"] || "15"}
    """

    legs_section =
      legs
      |> Enum.map(fn leg ->
        target_line = if leg["target_value"] && leg["target_value"] != "",
          do: "\ntarget_type = \"percent_of_premium\"\ntarget_value = #{leg["target_value"]}",
          else: ""

        """

        [[legs]]
        option_type = "#{leg["option_type"] || "CE"}"
        position = "#{leg["position"] || "sell"}"
        lots = #{leg["lots"] || "1"}
        expiry = "#{leg["expiry"] || "weekly"}"
        strike_mode = "atm_offset"
        strike_offset = #{leg["strike_offset"] || "0"}
        sl_type = "#{leg["sl_type"] || "percent_of_premium"}"
        sl_value = #{leg["sl_value"] || "30.0"}#{target_line}
        """
      end)
      |> Enum.join("")

    String.trim(strategy_section <> legs_section)
  end

  defp safe_list_strategies do
    try do
      QuantEdge.Strategies.list_strategies()
    rescue
      _ -> []
    end
  end

  defp safe_get_strategy(id) do
    try do
      QuantEdge.Strategies.get_strategy!(id)
    rescue
      _ -> %{name: "Unknown", underlying: "BANKNIFTY", config_toml: "", id: id, updated_at: nil}
    end
  end

  defp parse_legs_from_toml(_strategy), do: [default_leg()]

  defp count_legs(strategy) do
    case strategy.config_toml do
      nil -> 0
      toml ->
        toml
        |> String.split("[[legs]]")
        |> length()
        |> Kernel.-(1)
        |> max(0)
    end
  end

  defp format_date(nil), do: "—"
  defp format_date(%NaiveDateTime{} = dt), do: Calendar.strftime(dt, "%d %b %Y")
  defp format_date(%DateTime{} = dt), do: Calendar.strftime(dt, "%d %b %Y")
  defp format_date(date), do: to_string(date)
end
