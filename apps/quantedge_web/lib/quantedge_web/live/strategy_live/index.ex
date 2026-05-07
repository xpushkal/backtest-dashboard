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
    |> assign(:form, build_form(default_form_params()))
    |> assign(:legs, [default_leg()])
    |> assign(:toml_preview, "")
  end

  defp apply_action(socket, :edit, %{"id" => id}) do
    strategy = safe_get_strategy(id)
    legs = parse_legs_from_toml(strategy)
    form_params = parse_strategy_from_toml(strategy)

    socket
    |> assign(:page_title, "Edit Strategy")
    |> assign(:show_form, true)
    |> assign(:editing_strategy, strategy)
    |> assign(:form, build_form(form_params))
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
    suffix = "_#{idx}"

    # Extract leg-specific params, strip "leg_" prefix and "_N" suffix
    leg_params =
      params
      |> Map.drop(["index", "_target"])
      |> Enum.reduce(%{}, fn {key, val}, acc ->
        clean_key =
          key
          |> String.replace_prefix("leg_", "")
          |> String.replace_suffix(suffix, "")
        Map.put(acc, clean_key, val)
      end)

    legs = List.update_at(socket.assigns.legs, idx, fn leg ->
      Map.merge(leg, leg_params)
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
      <div :for={strategy <- @strategies} class="card" style="cursor: pointer; transition: transform 0.15s, box-shadow 0.15s;" onmouseover="this.style.transform='translateY(-2px)';this.style.boxShadow='0 8px 25px rgba(0,230,230,0.08)';" onmouseout="this.style.transform='';this.style.boxShadow='';">
        <a href={"/strategies/#{strategy.id}"} style="text-decoration: none; color: inherit; display: block;">
          <div class="flex-between mb-3">
            <h3>{strategy.name}</h3>
            <.underlying_badge underlying={strategy.underlying} />
          </div>
          <p class="text-sm text-muted mb-2">
            {count_legs(strategy)} leg(s) · Options · Updated {format_date(strategy.updated_at)}
          </p>
          <div class="text-sm text-muted mb-4" style="color: var(--accent-cyan); opacity: 0.7;">
            {strategy_summary(strategy)}
          </div>
        </a>
        <div class="flex-gap-2">
          <a href={"/strategies/#{strategy.id}"} class="btn btn-sm btn-primary" style="font-size: 0.75rem;">📋 View</a>
          <a href={"/strategies/#{strategy.id}/edit"} class="btn btn-sm btn-secondary">✏ Edit</a>
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
        <div class="grid-3 mb-6">
          <div class="input-group">
            <label class="input-label">Strategy Name</label>
            <input type="text" name="name" value={@form.params["name"]} class="input" placeholder="e.g. Short Straddle Nifty" required />
          </div>
          <div class="input-group">
            <label class="input-label">Underlying</label>
            <select name="underlying" class="input">
              <option value="NIFTY" selected={@form.params["underlying"] != "SENSEX"}>Nifty</option>
              <option value="SENSEX" selected={@form.params["underlying"] == "SENSEX"}>Sensex</option>
            </select>
          </div>
          <div class="input-group">
            <label class="input-label">Instrument Type</label>
            <select name="instrument_type" class="input">
              <option value="options" selected={@form.params["instrument_type"] != "futures"}>Options</option>
              <option value="futures" selected={@form.params["instrument_type"] == "futures"}>Futures</option>
            </select>
          </div>
        </div>

        <div class="grid-4 mb-4">
          <div class="input-group">
            <label class="input-label">Capital (₹)</label>
            <input type="number" name="capital" value={@form.params["capital"]} class="input" phx-debounce="blur" />
          </div>
          <div class="input-group">
            <label class="input-label">Entry Time</label>
            <input type="text" name="entry_time" value={@form.params["entry_time"]} class="input" placeholder="HH:MM" phx-debounce="blur" />
          </div>
          <div class="input-group">
            <label class="input-label">Exit Time</label>
            <input type="text" name="exit_time" value={@form.params["exit_time"]} class="input" placeholder="HH:MM" phx-debounce="blur" />
          </div>
          <div class="input-group">
            <label class="input-label">Brokerage/Lot (₹)</label>
            <input type="number" name="brokerage" value={@form.params["brokerage"]} class="input" phx-debounce="blur" />
          </div>
        </div>
        <div class="grid-4 mb-6">
          <div class="input-group">
            <label class="input-label">Slippage Model</label>
            <select name="slippage_model" class="input">
              <option value="fixed_pts" selected={@form.params["slippage_model"] != "percent"}>Fixed Pts</option>
              <option value="percent" selected={@form.params["slippage_model"] == "percent"}>Percent</option>
            </select>
          </div>
          <div class="input-group">
            <label class="input-label">Slippage Value</label>
            <input type="number" step="0.1" name="slippage_value" value={@form.params["slippage_value"]} class="input" phx-debounce="blur" />
          </div>
          <div class="input-group">
            <label class="input-label">STT on Sell</label>
            <select name="stt_on_sell" class="input">
              <option value="true" selected={@form.params["stt_on_sell"] != "false"}>Yes</option>
              <option value="false" selected={@form.params["stt_on_sell"] == "false"}>No</option>
            </select>
          </div>
          <div class="input-group">
            <label class="input-label">Max Concurrent</label>
            <input type="number" name="max_concurrent" value={@form.params["max_concurrent"]} class="input" phx-debounce="blur" />
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
              <input type="number" name={"leg_lots_#{idx}"} value={leg["lots"] || "1"} class="input" phx-change="update_leg" phx-value-index={idx} phx-debounce="blur" />
            </div>
            <div class="input-group">
              <label class="input-label">Strike Offset</label>
              <input type="number" name={"leg_strike_offset_#{idx}"} value={leg["strike_offset"] || "0"} class="input" phx-change="update_leg" phx-value-index={idx} phx-debounce="blur" />
            </div>
          </div>

          <div class="grid-4 mb-3">
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
              <input type="number" step="0.1" name={"leg_sl_value_#{idx}"} value={leg["sl_value"] || "30"} class="input" phx-change="update_leg" phx-value-index={idx} phx-debounce="blur" />
            </div>
            <div class="input-group">
              <label class="input-label">Target Value</label>
              <input type="number" step="0.1" name={"leg_target_value_#{idx}"} value={leg["target_value"] || ""} class="input" placeholder="Optional" phx-change="update_leg" phx-value-index={idx} phx-debounce="blur" />
            </div>
          </div>

          <%!-- Trailing SL & Re-entry --%>
          <div class="grid-4">
            <div class="input-group">
              <label class="input-label">Trail SL</label>
              <select name={"leg_trail_sl_enabled_#{idx}"} class="input" phx-change="update_leg" phx-value-index={idx}>
                <option value="false" selected={leg["trail_sl_enabled"] != "true"}>Off</option>
                <option value="true" selected={leg["trail_sl_enabled"] == "true"}>On</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">Trail Activate At</label>
              <input type="number" step="0.1" name={"leg_trail_activate_#{idx}"} value={leg["trail_activate"] || "0"} class="input" phx-change="update_leg" phx-value-index={idx} phx-debounce="blur" />
            </div>
            <div class="input-group">
              <label class="input-label">Re-entry on SL</label>
              <select name={"leg_reentry_on_sl_#{idx}"} class="input" phx-change="update_leg" phx-value-index={idx}>
                <option value="false" selected={leg["reentry_on_sl"] != "true"}>Off</option>
                <option value="true" selected={leg["reentry_on_sl"] == "true"}>On</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">Max Re-entries</label>
              <input type="number" name={"leg_reentry_max_#{idx}"} value={leg["reentry_max"] || "2"} class="input" phx-change="update_leg" phx-value-index={idx} phx-debounce="blur" />
            </div>
          </div>
        </div>

        <%!-- Overall SL/Target Section --%>
        <div class="card mb-6" style="background: var(--bg-tertiary);">
          <h4 class="mb-4">Overall Strategy SL / Target</h4>
          <div class="grid-3">
            <div class="input-group">
              <label class="input-label">Overall SL</label>
              <select name="overall_sl_enabled" class="input">
                <option value="false" selected={@form.params["overall_sl_enabled"] != "true"}>Disabled</option>
                <option value="true" selected={@form.params["overall_sl_enabled"] == "true"}>Enabled</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">Overall SL Type</label>
              <select name="overall_sl_type" class="input">
                <option value="percent_of_premium" selected={@form.params["overall_sl_type"] != "points"}>% Premium</option>
                <option value="points" selected={@form.params["overall_sl_type"] == "points"}>Points</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">Overall SL Value</label>
              <input type="number" step="0.1" name="overall_sl_value" value={@form.params["overall_sl_value"]} class="input" phx-debounce="blur" />
            </div>
          </div>
          <div class="grid-3 mt-3">
            <div class="input-group">
              <label class="input-label">Overall Target</label>
              <select name="overall_target_enabled" class="input">
                <option value="false" selected={@form.params["overall_target_enabled"] != "true"}>Disabled</option>
                <option value="true" selected={@form.params["overall_target_enabled"] == "true"}>Enabled</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">Overall Target Type</label>
              <select name="overall_target_type" class="input">
                <option value="percent_of_premium" selected={@form.params["overall_target_type"] != "points"}>% Premium</option>
                <option value="points" selected={@form.params["overall_target_type"] == "points"}>Points</option>
              </select>
            </div>
            <div class="input-group">
              <label class="input-label">Overall Target Value</label>
              <input type="number" step="0.1" name="overall_target_value" value={@form.params["overall_target_value"]} class="input" phx-debounce="blur" />
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

  defp default_form_params do
    %{
      "name" => "",
      "underlying" => "NIFTY",
      "instrument_type" => "options",
      "capital" => "100000",
      "entry_time" => "09:20",
      "exit_time" => "15:15",
      "brokerage" => "40",
      "slippage_model" => "fixed_pts",
      "slippage_value" => "1.0",
      "stt_on_sell" => "true",
      "max_concurrent" => "1",
      "overall_sl_enabled" => "false",
      "overall_sl_type" => "percent_of_premium",
      "overall_sl_value" => "0",
      "overall_target_enabled" => "false",
      "overall_target_type" => "percent_of_premium",
      "overall_target_value" => "0"
    }
  end

  defp parse_strategy_from_toml(%{config_toml: nil} = strategy) do
    Map.merge(default_form_params(), %{"name" => strategy.name, "underlying" => strategy.underlying})
  end
  defp parse_strategy_from_toml(%{config_toml: ""} = strategy) do
    Map.merge(default_form_params(), %{"name" => strategy.name, "underlying" => strategy.underlying})
  end
  defp parse_strategy_from_toml(%{config_toml: toml} = strategy) do
    # Extract [strategy] section (everything before [[legs]] or [overall])
    strategy_block = toml |> String.split(~r/^\[\[legs\]\]|^\[overall\]/m, parts: 2) |> hd()

    %{
      "name" => strategy.name,
      "underlying" => strategy.underlying,
      "instrument_type" => extract_string(strategy_block, "instrument_type", "options"),
      "capital" => extract_number(strategy_block, "capital", "100000"),
      "entry_time" => extract_string(strategy_block, "entry_time", "09:20"),
      "exit_time" => extract_string(strategy_block, "exit_time", "15:15"),
      "brokerage" => extract_number(strategy_block, "brokerage_per_lot", "40"),
      "slippage_model" => extract_string(strategy_block, "slippage_model", "fixed_pts"),
      "slippage_value" => extract_number(strategy_block, "slippage_value", "1.0"),
      "stt_on_sell" => extract_bool(strategy_block, "stt_on_sell"),
      "max_concurrent" => extract_number(strategy_block, "max_concurrent", "1"),
      "overall_sl_enabled" => extract_overall_bool(toml, "overall_sl_enabled"),
      "overall_sl_type" => extract_overall_string(toml, "overall_sl_type", "percent_of_premium"),
      "overall_sl_value" => extract_overall_number(toml, "overall_sl_value", "0"),
      "overall_target_enabled" => extract_overall_bool(toml, "overall_target_enabled"),
      "overall_target_type" => extract_overall_string(toml, "overall_target_type", "percent_of_premium"),
      "overall_target_value" => extract_overall_number(toml, "overall_target_value", "0")
    }
  end

  defp extract_overall_string(toml, key, default) do
    case String.split(toml, ~r/^\[overall\]/m, parts: 2) do
      [_, overall_block] -> extract_string(overall_block, key, default)
      _ -> default
    end
  end

  defp extract_overall_number(toml, key, default) do
    case String.split(toml, ~r/^\[overall\]/m, parts: 2) do
      [_, overall_block] -> extract_number(overall_block, key, default)
      _ -> default
    end
  end

  defp extract_overall_bool(toml, key) do
    case String.split(toml, ~r/^\[overall\]/m, parts: 2) do
      [_, overall_block] -> extract_bool(overall_block, key)
      _ -> "false"
    end
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
    name = "#{escape_str(form["name"] || "Unnamed")}"
    underlying = "#{escape_str(form["underlying"] || "NIFTY")}"
    capital = #{toml_float(form["capital"], 100_000.0)}
    entry_time = "#{escape_str(form["entry_time"] || "09:20")}"
    exit_time = "#{escape_str(form["exit_time"] || "15:15")}"
    brokerage_per_lot = #{toml_float(form["brokerage"], 40.0)}
    slippage_model = "#{escape_str(form["slippage_model"] || "fixed_pts")}"
    slippage_value = #{toml_float(form["slippage_value"], 1.0)}
    stt_on_sell = #{toml_bool(form["stt_on_sell"], true)}
    """

    legs_section =
      legs
      |> Enum.map(fn leg ->
        sl_enabled = leg["sl_type"] != "none" && leg["sl_type"] != ""
        tp_enabled = leg["target_value"] not in [nil, ""]
        trail_enabled = leg["trail_sl_enabled"] == "true"
        reentry_on = leg["reentry_on_sl"] == "true"

        """

        [[legs]]
        option_type = "#{escape_str(leg["option_type"] || "CE")}"
        position = "#{escape_str(leg["position"] || "sell")}"
        lots = #{toml_int(leg["lots"], 1)}
        expiry = "#{escape_str(leg["expiry"] || "weekly")}"
        strike_mode = "atm_offset"
        strike_offset = #{toml_int(leg["strike_offset"], 0)}
        stop_loss_enabled = #{sl_enabled}
        stop_loss_type = "#{escape_str(leg["sl_type"] || "percent_of_premium")}"
        stop_loss_value = #{toml_float(leg["sl_value"], 30.0)}
        target_profit_enabled = #{tp_enabled}
        target_profit_type = "#{escape_str(leg["target_type"] || "percent_of_premium")}"
        target_profit_value = #{toml_float(leg["target_value"], 0.0)}
        trail_sl_enabled = #{trail_enabled}
        trail_sl_activate_at = #{toml_float(leg["trail_activate"], 0.0)}
        trail_sl_lock_in = #{toml_float(leg["trail_lock"], 0.0)}
        reentry_on_sl = #{reentry_on}
        reentry_max_attempts = #{toml_int(leg["reentry_max"], 2)}
        """
      end)
      |> Enum.join("")

    overall_sl = form["overall_sl_enabled"] == "true"
    overall_target = form["overall_target_enabled"] == "true"

    overall_section = """

    [overall]
    overall_sl_enabled = #{overall_sl}
    overall_sl_type = "#{escape_str(form["overall_sl_type"] || "percent_of_premium")}"
    overall_sl_value = #{toml_float(form["overall_sl_value"], 0.0)}
    overall_target_enabled = #{overall_target}
    overall_target_type = "#{escape_str(form["overall_target_type"] || "percent_of_premium")}"
    overall_target_value = #{toml_float(form["overall_target_value"], 0.0)}
    """

    String.trim(strategy_section <> legs_section <> overall_section)
  end

  # ─── TOML serialization helpers ─────────────────────────────

  defp escape_str(nil), do: ""
  defp escape_str(s) when is_binary(s) do
    s |> String.replace("\\", "\\\\") |> String.replace("\"", "\\\"")
  end
  defp escape_str(other), do: escape_str(to_string(other))

  defp toml_float(nil, default), do: format_float(default)
  defp toml_float("", default), do: format_float(default)
  defp toml_float(v, _default) when is_number(v), do: format_float(v)
  defp toml_float(v, default) when is_binary(v) do
    case Float.parse(v) do
      {f, _} -> format_float(f)
      :error ->
        case Integer.parse(v) do
          {i, _} -> format_float(i * 1.0)
          :error -> format_float(default)
        end
    end
  end
  defp toml_float(_, default), do: format_float(default)

  defp format_float(v) when is_integer(v), do: "#{v}.0"
  defp format_float(v) when is_float(v) do
    cond do
      v != v -> "0.0"
      abs(v) > 1.0e308 -> "0.0"
      v == trunc(v) -> "#{trunc(v)}.0"
      true -> Float.to_string(v)
    end
  end
  defp format_float(_), do: "0.0"

  defp toml_int(nil, default), do: to_string(default)
  defp toml_int("", default), do: to_string(default)
  defp toml_int(v, _default) when is_integer(v), do: to_string(v)
  defp toml_int(v, _default) when is_float(v), do: to_string(trunc(v))
  defp toml_int(v, default) when is_binary(v) do
    case Integer.parse(v) do
      {i, _} -> to_string(i)
      :error ->
        case Float.parse(v) do
          {f, _} -> to_string(trunc(f))
          :error -> to_string(default)
        end
    end
  end
  defp toml_int(_, default), do: to_string(default)

  defp toml_bool("true", _), do: "true"
  defp toml_bool("false", _), do: "false"
  defp toml_bool(true, _), do: "true"
  defp toml_bool(false, _), do: "false"
  defp toml_bool(_, default), do: to_string(default)

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

  # Parse the saved TOML back into form-shaped leg maps so editing preserves config.
  defp parse_legs_from_toml(strategy) do
    case strategy do
      %{config_toml: toml} when is_binary(toml) and toml != "" ->
        toml
        |> String.split("[[legs]]")
        |> Enum.drop(1)
        |> Enum.map(&parse_leg_block/1)
        |> case do
          [] -> [default_leg()]
          legs -> legs
        end

      _ ->
        [default_leg()]
    end
  end

  # The TOML body for a single [[legs]] section. We extract each known field
  # with a regex; the form rebuilds the TOML from these on save, so a small
  # number of unparsed fields is acceptable.
  defp parse_leg_block(block) do
    # Stop at the next section header so a leg block doesn't bleed into [overall].
    block = block |> String.split(~r/^\[/m, parts: 2) |> hd()

    %{
      "option_type" => extract_string(block, "option_type", "CE"),
      "position" => extract_string(block, "position", "sell"),
      "lots" => extract_number(block, "lots", "1"),
      "expiry" => extract_string(block, "expiry", "weekly"),
      "strike_offset" => extract_number(block, "strike_offset", "0"),
      "sl_type" => extract_string(block, "stop_loss_type", "percent_of_premium"),
      "sl_value" => extract_number(block, "stop_loss_value", "30"),
      "target_value" => extract_optional_number(block, "target_profit_value"),
      "trail_sl_enabled" => extract_bool(block, "trail_sl_enabled"),
      "trail_activate" => extract_number(block, "trail_sl_activate_at", "0"),
      "trail_lock" => extract_number(block, "trail_sl_lock_in", "0"),
      "reentry_on_sl" => extract_bool(block, "reentry_on_sl"),
      "reentry_max" => extract_number(block, "reentry_max_attempts", "2")
    }
  end

  defp extract_string(block, key, default) do
    case Regex.run(~r/^\s*#{Regex.escape(key)}\s*=\s*"([^"]*)"/m, block) do
      [_, val] -> val
      _ -> default
    end
  end

  defp extract_number(block, key, default) do
    case Regex.run(~r/^\s*#{Regex.escape(key)}\s*=\s*(-?\d+(?:\.\d+)?)/m, block) do
      [_, val] -> val
      _ -> default
    end
  end

  defp extract_optional_number(block, key) do
    case Regex.run(~r/^\s*#{Regex.escape(key)}\s*=\s*(-?\d+(?:\.\d+)?)/m, block) do
      [_, "0"] -> ""
      [_, "0.0"] -> ""
      [_, val] -> val
      _ -> ""
    end
  end

  defp extract_bool(block, key) do
    case Regex.run(~r/^\s*#{Regex.escape(key)}\s*=\s*(true|false)/m, block) do
      [_, "true"] -> "true"
      _ -> "false"
    end
  end

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

  defp strategy_summary(strategy) do
    case strategy.config_toml do
      nil -> "No legs configured"
      toml ->
        toml
        |> String.split("[[legs]]")
        |> Enum.drop(1)
        |> Enum.map(fn leg_str ->
          pos = if String.contains?(leg_str, "\"sell\""), do: "Sell", else: "Buy"
          opt = if String.contains?(leg_str, "\"PE\""), do: "PE", else: "CE"
          offset = case Regex.run(~r/strike_offset\s*=\s*(-?\d+)/, leg_str) do
            [_, "0"] -> "ATM"
            [_, n] -> "ATM#{if String.starts_with?(n, "-"), do: n, else: "+#{n}"}"
            _ -> "ATM"
          end
          "#{pos} #{opt} #{offset}"
        end)
        |> Enum.join(" + ")
        |> case do
          "" -> "No legs configured"
          summary -> summary
        end
    end
  end
end
