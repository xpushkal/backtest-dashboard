# This file is responsible for configuring your umbrella
# and **all applications** and their dependencies with the
# having the umbrella as the root of the config tree.
import Config

# Configure Ecto
config :quantedge,
  ecto_repos: [QuantEdge.Repo],
  generators: [timestamp_type: :utc_datetime, binary_id: true]

# Configure data directory
config :quantedge, :data_dir, "Data/parquet"

# Configure Oban
config :quantedge, Oban,
  repo: QuantEdge.Repo,
  queues: [
    backtests: 2,
    optimizers: 1,
    portfolios: 1
  ]

# Configures the endpoint
config :quantedge_web, QuantEdgeWeb.Endpoint,
  url: [host: "localhost"],
  adapter: Bandit.PhoenixAdapter,
  render_errors: [
    formats: [html: QuantEdgeWeb.ErrorHTML, json: QuantEdgeWeb.ErrorJSON],
    layout: false
  ],
  pubsub_server: QuantEdge.PubSub,
  live_view: [signing_salt: "qe_lv_salt"]

# Configure esbuild (the version is required)
config :esbuild,
  version: "0.17.11",
  quantedge_web: [
    args: ~w(js/app.js --bundle --target=es2017 --outdir=../priv/static/assets),
    cd: Path.expand("../apps/quantedge_web/assets", __DIR__),
    env: %{"NODE_PATH" => Path.expand("../deps", __DIR__)}
  ]

# Configures Elixir's Logger
config :logger, :console,
  format: "$time $metadata[$level] $message\n",
  metadata: [:request_id]

# Use Jason for JSON parsing in Phoenix
config :phoenix, :json_library, Jason

# Import environment specific config. This must remain at the bottom
# of this file so it overrides the configuration defined above.
import_config "#{config_env()}.exs"
