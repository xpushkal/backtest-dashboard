import Config

# Configure your database
config :quantedge, QuantEdge.Repo,
  username: "postgres",
  password: "postgres",
  hostname: "localhost",
  database: "quantedge_test#{System.get_env("MIX_TEST_PARTITION")}",
  pool: Ecto.Adapters.SQL.Sandbox,
  pool_size: System.schedulers_online() * 2

# We don't run a server during test
config :quantedge_web, QuantEdgeWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4002],
  secret_key_base: "quantedge_test_secret_key_base_needs_to_be_at_least_64_bytes_long_for_security",
  server: false

# Oban in testing mode
config :quantedge, Oban, testing: :inline

# Print only warnings and errors during test
config :logger, level: :warning

# Initialize plugs at runtime for faster test compilation
config :phoenix, :plug_init_mode, :runtime
