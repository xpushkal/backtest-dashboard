defmodule QuantEdge.Repo do
  use Ecto.Repo,
    otp_app: :quantedge,
    adapter: Ecto.Adapters.Postgres
end
