defmodule QuantEdgeWeb.DataExplorerLive do
  @moduledoc "Data explorer showing loaded symbols, date ranges, bar counts, IV coverage."
  use QuantEdgeWeb, :live_view

  import QuantEdgeWeb.UiComponents

  @underlyings ["BANKNIFTY", "NIFTY", "SENSEX"]

  @impl true
  def mount(_params, _session, socket) do
    data_summaries = load_data_summaries()
    parquet_files = scan_parquet_files()

    {:ok,
     socket
     |> assign(:page_title, "Data Explorer")
     |> assign(:active_nav, :data)
     |> assign(:data_summaries, data_summaries)
     |> assign(:parquet_files, parquet_files)
     |> assign(:show_parquet, false)
     |> assign(:total_storage, compute_total_storage(parquet_files))}
  end

  @impl true
  def handle_event("toggle_parquet", _params, socket) do
    {:noreply, assign(socket, :show_parquet, !socket.assigns.show_parquet)}
  end

  def handle_event("refresh_data", _params, socket) do
    {:noreply,
     socket
     |> assign(:data_summaries, load_data_summaries())
     |> assign(:parquet_files, scan_parquet_files())
     |> put_flash(:info, "Data refreshed")}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="page-header">
      <h1>💾 Data Explorer</h1>
      <button class="btn btn-secondary" phx-click="refresh_data">🔄 Refresh</button>
    </div>

    <%!-- Summary Stats --%>
    <div class="grid-3 mb-8">
      <.stat_card label="Underlyings" value={to_string(length(@data_summaries))} subtitle="configured" />
      <.stat_card label="Parquet Files" value={to_string(length(@parquet_files))} subtitle="loaded" />
      <.stat_card label="Total Storage" value={@total_storage} subtitle="on disk" />
    </div>

    <%!-- Per-Underlying Cards --%>
    <div class="grid-3 mb-8">
      <div :for={summary <- @data_summaries} class="card">
        <div class="flex-between mb-4">
          <h3>{summary.symbol}</h3>
          <.underlying_badge underlying={summary.symbol} />
        </div>

        <div class="flex-col" style="gap: 0.75rem;">
          <div class="flex-between">
            <span class="text-sm text-muted">Date Range</span>
            <span class="text-sm text-mono">{summary.date_from} — {summary.date_to}</span>
          </div>
          <div class="flex-between">
            <span class="text-sm text-muted">Bar Count</span>
            <span class="text-sm text-mono">{format_number(summary.bar_count)}</span>
          </div>
          <div class="flex-between">
            <span class="text-sm text-muted">Trading Days</span>
            <span class="text-sm text-mono">{format_number(summary.trading_days)}</span>
          </div>
          <div class="flex-between">
            <span class="text-sm text-muted">IV Coverage</span>
            <span class={"text-sm text-mono #{iv_color(summary.iv_coverage)}"}>
              {summary.iv_coverage}%
            </span>
          </div>

          <%!-- IV Coverage Bar --%>
          <.progress_bar percent={summary.iv_coverage * 1.0} />

          <%!-- Quality Indicators --%>
          <div class="mt-2">
            <div :if={summary.missing_days > 0} class="text-xs" style="color: var(--accent-yellow);">
              ⚠ {summary.missing_days} missing trading days
            </div>
            <div :if={summary.zero_volume_pct > 1.0} class="text-xs" style="color: var(--accent-yellow);">
              ⚠ {summary.zero_volume_pct}% zero-volume bars
            </div>
            <div :if={summary.missing_days == 0 && summary.zero_volume_pct <= 1.0} class="text-xs" style="color: var(--accent-green);">
              ✓ Data quality: Good
            </div>
          </div>
        </div>
      </div>
    </div>

    <%!-- Parquet File Browser --%>
    <div class="card">
      <div class="card-header">
        <span class="card-title">Parquet Files</span>
        <button class="btn btn-sm btn-secondary" phx-click="toggle_parquet">
          {if @show_parquet, do: "Hide", else: "Show"} Files
        </button>
      </div>

      <table :if={@show_parquet && @parquet_files != []} class="data-table">
        <thead>
          <tr>
            <th>Filename</th>
            <th class="col-number">Size</th>
            <th>Last Modified</th>
          </tr>
        </thead>
        <tbody>
          <tr :for={file <- @parquet_files}>
            <td class="text-mono text-sm">{file.name}</td>
            <td class="col-number text-mono">{file.size_mb} MB</td>
            <td class="text-sm text-muted">{file.modified}</td>
          </tr>
        </tbody>
      </table>

      <div :if={@show_parquet && @parquet_files == []} class="text-center text-muted" style="padding: 2rem;">
        No Parquet files found in Data/parquet/ directory.
      </div>
    </div>
    """
  end

  # --- Data Loading ---

  defp load_data_summaries do
    @underlyings
    |> Enum.map(fn symbol ->
      case safe_load_summary(symbol) do
        {:ok, summary} -> summary
        _ -> default_summary(symbol)
      end
    end)
  end

  defp safe_load_summary(symbol) do
    try do
      case apply(QuantEdge.NIF, :load_data_summary, [symbol, "{}", ""]) do
        {:ok, json} ->
          data = Jason.decode!(json)
          {:ok, %{
            symbol: symbol,
            date_from: data["date_from"] || "N/A",
            date_to: data["date_to"] || "N/A",
            bar_count: data["bar_count"] || 0,
            trading_days: data["trading_days"] || 0,
            iv_coverage: data["iv_coverage"] || 0,
            missing_days: data["missing_days"] || 0,
            zero_volume_pct: data["zero_volume_pct"] || 0
          }}
        _ -> {:error, :nif_failed}
      end
    rescue
      _ -> {:error, :nif_unavailable}
    end
  end

  defp default_summary(symbol) do
    %{
      symbol: symbol,
      date_from: "N/A",
      date_to: "N/A",
      bar_count: 0,
      trading_days: 0,
      iv_coverage: 0,
      missing_days: 0,
      zero_volume_pct: 0
    }
  end

  defp scan_parquet_files do
    parquet_dir = Path.join([File.cwd!(), "Data", "parquet"])

    if File.dir?(parquet_dir) do
      parquet_dir
      |> File.ls!()
      |> Enum.filter(&String.ends_with?(&1, ".parquet"))
      |> Enum.sort()
      |> Enum.map(fn name ->
        path = Path.join(parquet_dir, name)
        stat = File.stat!(path)
        %{
          name: name,
          size_mb: Float.round(stat.size / 1_000_000, 1),
          modified: Calendar.strftime(stat.mtime |> NaiveDateTime.from_erl!(), "%d %b %Y")
        }
      end)
    else
      []
    end
  rescue
    _ -> []
  end

  defp compute_total_storage(files) do
    total_mb = files |> Enum.map(& &1.size_mb) |> Enum.sum()
    cond do
      total_mb >= 1000 -> "#{Float.round(total_mb / 1000, 1)} GB"
      total_mb > 0 -> "#{Float.round(total_mb, 1)} MB"
      true -> "0 MB"
    end
  end

  defp format_number(0), do: "0"
  defp format_number(num) when is_integer(num) do
    num
    |> Integer.to_string()
    |> String.graphemes()
    |> Enum.reverse()
    |> Enum.chunk_every(3)
    |> Enum.join(",")
    |> String.reverse()
  end
  defp format_number(num), do: to_string(num)

  defp iv_color(pct) when pct >= 90, do: "text-profit"
  defp iv_color(pct) when pct >= 60, do: ""
  defp iv_color(_), do: "text-loss"
end
