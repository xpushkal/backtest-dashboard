import Config

# Production-time settings. Runtime configuration that depends on
# environment variables (DATABASE_URL, SECRET_KEY_BASE, etc.) lives
# in `config/runtime.exs`.

# Logger: drop debug noise in prod
config :logger, level: :info

# Phoenix endpoint defaults — runtime.exs overrides host/port/secret.
config :quantedge_web, QuantEdgeWeb.Endpoint,
  cache_static_manifest: "priv/static/cache_manifest.json",
  server: true
