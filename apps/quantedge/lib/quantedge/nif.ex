defmodule QuantEdge.NIF do
  @moduledoc """
  Rustler NIF bridge to the quantedge-nif Rust crate.

  All functions run on dirty CPU schedulers to avoid blocking the BEAM.
  Input/output is JSON strings — deserialized on the Rust side.

  The NIF is compiled separately via `cargo build` and loaded at runtime.
  This avoids Rustler compile-time path issues in umbrella projects.
  """
  use Rustler,
    otp_app: :quantedge,
    crate: "quantedge_nif",
    skip_compilation?: true,
    load_from: {:quantedge, "priv/native/libquantedge_nif"}

  @doc "Run a single strategy backtest. Returns {:ok, json} | {:error, reason}."
  def run_backtest(_strategy_toml, _opts_json), do: :erlang.nif_error(:not_loaded)

  @doc "Run optimizer grid sweep. Returns {:ok, json} | {:error, reason}."
  def run_optimizer(_strategy_toml, _param_grid_json, _opts_json), do: :erlang.nif_error(:not_loaded)

  @doc "Run portfolio backtest. Returns {:ok, json} | {:error, reason}."
  def run_portfolio(_strategies_json, _opts_json), do: :erlang.nif_error(:not_loaded)

  @doc "Load data summary for a symbol. Returns {:ok, json} | {:error, reason}."
  def load_data_summary(_symbol, _date_from, _date_to), do: :erlang.nif_error(:not_loaded)
end
