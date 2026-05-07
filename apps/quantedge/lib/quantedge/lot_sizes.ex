defmodule QuantEdge.LotSizes do
  @moduledoc """
  Historical lot-size lookup for Indian FNO instruments.

  Reads `config/lot_sizes.toml` once at boot and caches in :persistent_term.
  Use `get/2` to look up the lot size for a (symbol, date) pair — entries
  are date-ranged so transitions like NIFTY 50→25→75 are handled correctly.
  """

  @cache_key {__MODULE__, :table}

  @doc """
  Look up lot size for a symbol on a given date.

  Returns the configured size, or a sane default (1) when no entry covers
  the date. Pass the trade entry date — never re-look up mid-trade.
  """
  @spec get(String.t(), Date.t()) :: pos_integer()
  def get(symbol, %Date{} = date) do
    table = load_or_get_table()
    sym = String.upcase(symbol)

    case Map.get(table, sym) do
      nil ->
        1

      entries ->
        case Enum.find(entries, fn e ->
               Date.compare(date, e.from) != :lt and Date.compare(date, e.to) != :gt
             end) do
          nil -> 1
          %{size: size} -> size
        end
    end
  end

  @doc "Return the most-recent (current) lot size for a symbol."
  @spec current(String.t()) :: pos_integer()
  def current(symbol) do
    get(symbol, Date.utc_today())
  end

  defp load_or_get_table do
    case :persistent_term.get(@cache_key, nil) do
      nil ->
        table = load_table()
        :persistent_term.put(@cache_key, table)
        table

      table ->
        table
    end
  end

  defp load_table do
    path = lot_sizes_path()

    case File.read(path) do
      {:ok, content} ->
        parse_toml(content)

      {:error, _} ->
        %{}
    end
  end

  defp lot_sizes_path do
    Application.get_env(:quantedge, :lot_sizes_path) ||
      Path.join([File.cwd!(), "config", "lot_sizes.toml"])
  end

  # Minimal TOML parser for our specific array-of-tables shape.
  # Avoids adding a TOML dependency when the format is fixed and tiny.
  defp parse_toml(content) do
    content
    |> String.split("\n")
    |> Enum.reduce({%{}, nil, %{}}, fn line, {acc, current_sym, current_entry} ->
      line = String.trim(line)

      cond do
        line == "" or String.starts_with?(line, "#") ->
          {acc, current_sym, current_entry}

        match = Regex.run(~r/^\[\[([A-Za-z_][A-Za-z_0-9]*)\]\]$/, line) ->
          [_, sym] = match
          acc = flush(acc, current_sym, current_entry)
          {acc, sym, %{}}

        match = Regex.run(~r/^([a-z_]+)\s*=\s*"([^"]+)"$/, line) ->
          [_, key, val] = match
          {acc, current_sym, Map.put(current_entry, key, val)}

        match = Regex.run(~r/^([a-z_]+)\s*=\s*(\d+)$/, line) ->
          [_, key, val] = match
          {acc, current_sym, Map.put(current_entry, key, String.to_integer(val))}

        true ->
          {acc, current_sym, current_entry}
      end
    end)
    |> then(fn {acc, sym, entry} -> flush(acc, sym, entry) end)
  end

  defp flush(acc, nil, _), do: acc
  defp flush(acc, _sym, entry) when map_size(entry) == 0, do: acc

  defp flush(acc, sym, entry) do
    parsed = %{
      from: Date.from_iso8601!(Map.fetch!(entry, "from")),
      to: Date.from_iso8601!(Map.fetch!(entry, "to")),
      size: Map.fetch!(entry, "size")
    }

    Map.update(acc, sym, [parsed], fn list -> list ++ [parsed] end)
  end
end
