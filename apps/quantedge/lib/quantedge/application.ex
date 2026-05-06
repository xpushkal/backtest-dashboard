defmodule QuantEdge.Application do
  @moduledoc false
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      QuantEdge.Repo,
      {Phoenix.PubSub, name: QuantEdge.PubSub},
      {Oban, Application.fetch_env!(:quantedge, Oban)},
      {QuantEdge.Duck.Writer, []}
    ]

    Supervisor.start_link(children, strategy: :one_for_one, name: QuantEdge.Supervisor)
  end
end
