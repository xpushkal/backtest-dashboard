import Config

# Configure your database
config :quantedge, QuantEdge.Repo,
  username: "postgres",
  password: "postgres",
  hostname: "localhost",
  database: "quantedge_dev",
  stacktrace: true,
  show_sensitive_data_on_connection_error: true,
  pool_size: 10

# For development, we disable any cache and enable
# debugging and code reloading.
config :quantedge_web, QuantEdgeWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4000],
  check_origin: false,
  code_reloader: true,
  debug_errors: true,
  secret_key_base: "quantedge_dev_secret_key_base_needs_to_be_at_least_64_bytes_long_for_security",
  watchers: [
    esbuild: {Esbuild, :install_and_run, [:quantedge_web, ~w(--sourcemap=inline --watch)]}
  ]

config :quantedge_web, QuantEdgeWeb.Endpoint,
  live_reload: [
    patterns: [
      ~r"priv/static/(?!uploads/).*(js|css|png|jpeg|jpg|gif|svg)$",
      ~r"lib/quantedge_web/(controllers|live|components)/.*(ex|heex)$"
    ]
  ]

# Disable Oban in dev to avoid needing Postgres running
config :quantedge, Oban,
  testing: :manual

# Do not include metadata nor timestamps in development logs
config :logger, :console, format: "[$level] $message\n"

# Set a higher stacktrace during development. Avoid configuring such
# in production as building large stacktraces may be expensive.
config :phoenix, :stacktrace_depth, 20

# Initialize plugs at runtime for faster development compilation
config :phoenix, :plug_init_mode, :runtime

config :phoenix_live_view,
  # Include HEEx debug annotations as HTML comments in rendered markup
  debug_heex_annotations: true,
  # Enable helpful, but potentially expensive runtime checks
  enable_expensive_runtime_checks: true
