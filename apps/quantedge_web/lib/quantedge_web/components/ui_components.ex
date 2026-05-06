defmodule QuantEdgeWeb.UiComponents do
  @moduledoc """
  Reusable UI components for the QuantEdge dashboard.
  """
  use Phoenix.Component
  alias Phoenix.LiveView.JS

  # --- Stat Card ---
  attr :label, :string, required: true
  attr :value, :string, required: true
  attr :subtitle, :string, default: nil
  attr :trend, :atom, default: nil, values: [nil, :up, :down]
  attr :class, :string, default: ""

  def stat_card(assigns) do
    ~H"""
    <div class={"stat-card #{@class}"}>
      <span class="stat-label">{@label}</span>
      <span class="stat-value">{@value}</span>
      <span :if={@subtitle} class="stat-subtitle">
        <span :if={@trend == :up} class="trend-up">▲</span>
        <span :if={@trend == :down} class="trend-down">▼</span>
        {@subtitle}
      </span>
    </div>
    """
  end

  # --- Progress Bar ---
  attr :percent, :float, default: 0.0
  attr :label, :string, default: nil
  attr :animated, :boolean, default: false

  def progress_bar(assigns) do
    ~H"""
    <div>
      <div :if={@label} class="flex-between mb-2">
        <span class="text-sm text-muted">{@label}</span>
        <span class="text-sm text-mono">{Float.round(@percent, 1)}%</span>
      </div>
      <div class={"progress-bar #{if @animated, do: "animated"}"}>
        <div class="progress-fill" style={"width: #{min(@percent, 100)}%"}></div>
      </div>
    </div>
    """
  end

  # --- Metric Pill ---
  attr :label, :string, required: true
  attr :value, :string, required: true
  attr :class, :string, default: ""

  def metric_pill(assigns) do
    ~H"""
    <span class={"badge badge-info #{@class}"}>
      {@label}: <strong class="text-mono">{@value}</strong>
    </span>
    """
  end

  # --- Empty State ---
  attr :icon, :string, default: "📊"
  attr :title, :string, required: true
  attr :description, :string, default: nil
  attr :action_label, :string, default: nil
  attr :action_href, :string, default: nil

  def empty_state(assigns) do
    ~H"""
    <div class="empty-state">
      <div class="empty-icon">{@icon}</div>
      <h3 class="empty-title">{@title}</h3>
      <p :if={@description} class="empty-description">{@description}</p>
      <a :if={@action_label} href={@action_href} class="btn btn-primary">{@action_label}</a>
    </div>
    """
  end

  # --- Loading Spinner ---
  def loading_spinner(assigns) do
    ~H"""
    <div class="loading-spinner">
      <div class="dot"></div>
      <div class="dot"></div>
      <div class="dot"></div>
    </div>
    """
  end

  # --- Tab Bar ---
  attr :tabs, :list, required: true
  attr :active, :string, required: true
  attr :on_click, :string, default: "switch_tab"

  def tab_bar(assigns) do
    ~H"""
    <div class="tab-bar">
      <button
        :for={tab <- @tabs}
        class={"tab-item #{if tab == @active, do: "active"}"}
        phx-click={@on_click}
        phx-value-tab={tab}
      >
        {tab}
      </button>
    </div>
    """
  end

  # --- Status Badge ---
  attr :status, :string, required: true

  def status_badge(assigns) do
    ~H"""
    <span class={"badge badge-#{@status}"}>
      <span :if={@status == "running"}>●</span>
      {@status}
    </span>
    """
  end

  # --- PnL Value ---
  attr :value, :float, required: true
  attr :currency, :boolean, default: true

  def pnl_value(assigns) do
    abs_val = abs(assigns.value)

    formatted =
      if assigns.currency do
        "₹#{format_number(abs_val)}"
      else
        "#{Float.round(assigns.value, 2)}"
      end

    sign = if assigns.value >= 0, do: "+", else: "-"
    assigns = assign(assigns, :formatted, "#{sign}#{formatted}")

    ~H"""
    <span class={"text-mono #{if @value >= 0, do: "text-profit", else: "text-loss"}"}>
      {@formatted}
    </span>
    """
  end

  defp format_number(num) when is_float(num) do
    num
    |> round()
    |> Integer.to_string()
    |> String.graphemes()
    |> Enum.reverse()
    |> Enum.chunk_every(3)
    |> Enum.join(",")
    |> String.reverse()
  end

  defp format_number(num) when is_integer(num) do
    num
    |> Integer.to_string()
    |> String.graphemes()
    |> Enum.reverse()
    |> Enum.chunk_every(3)
    |> Enum.join(",")
    |> String.reverse()
  end

  # --- Confirmation Modal ---
  attr :id, :string, required: true
  attr :title, :string, required: true
  attr :body, :string, default: "Are you sure?"
  attr :confirm_label, :string, default: "Confirm"
  attr :on_confirm, :any, required: true

  def confirmation_modal(assigns) do
    ~H"""
    <div id={@id} class="modal-overlay hidden" phx-click={JS.add_class("hidden", to: "##{@id}")}>
      <div class="modal-content" phx-click-away={JS.add_class("hidden", to: "##{@id}")}>
        <div class="modal-header">
          <h3 class="modal-title">{@title}</h3>
          <button class="modal-close" phx-click={JS.add_class("hidden", to: "##{@id}")}>×</button>
        </div>
        <p>{@body}</p>
        <div class="modal-footer">
          <button class="btn btn-secondary" phx-click={JS.add_class("hidden", to: "##{@id}")}>Cancel</button>
          <button class="btn btn-danger" phx-click={@on_confirm}>{@confirm_label}</button>
        </div>
      </div>
    </div>
    """
  end

  # --- Underlying Badge ---
  attr :underlying, :string, required: true

  def underlying_badge(assigns) do
    badge_class =
      case assigns.underlying do
        "BANKNIFTY" -> "badge-bn"
        "NIFTY" -> "badge-nf"
        "SENSEX" -> "badge-sx"
        _ -> "badge-info"
      end

    short =
      case assigns.underlying do
        "BANKNIFTY" -> "BN"
        "NIFTY" -> "NF"
        "SENSEX" -> "SX"
        other -> other
      end

    assigns = assign(assigns, badge_class: badge_class, short: short)

    ~H"""
    <span class={"badge #{@badge_class}"}>{@short}</span>
    """
  end
end
