import Config

# For production, don't forget to configure the url host
# to something meaningful, Phoenix uses this information
# when generating URLs.

config :quantedge, QuantEdge.Repo,
  # ssl: true,
  url: {:system, "DATABASE_URL"},
  pool_size: String.to_integer(System.get_env("POOL_SIZE") || "10")
