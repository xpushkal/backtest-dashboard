defmodule QuantEdgeWeb.Router do
  use QuantEdgeWeb, :router

  pipeline :browser do
    plug :accepts, ["html"]
    plug :fetch_session
    plug :fetch_live_flash
    plug :put_root_layout, html: {QuantEdgeWeb.Layouts, :root}
    plug :protect_from_forgery
    plug :put_secure_browser_headers
  end

  pipeline :api do
    plug :accepts, ["json"]
  end

  scope "/", QuantEdgeWeb do
    pipe_through :browser

    live "/", DashboardLive, :index
    live "/strategies", StrategyLive.Index, :index
    live "/strategies/new", StrategyLive.Index, :new
    live "/strategies/:id/edit", StrategyLive.Index, :edit
    live "/strategies/:id", StrategyLive.Show, :show
    live "/runs", RunLive.Index, :index
    live "/runs/:id", RunLive.Show, :show
    live "/optimizer", OptimizerLive, :index
    live "/portfolio", PortfolioLive, :index
    live "/data", DataExplorerLive, :index
  end

  # Enable LiveDashboard in development
  if Application.compile_env(:quantedge_web, :dev_routes) do
    import Phoenix.LiveDashboard.Router

    scope "/dev" do
      pipe_through :browser
      live_dashboard "/dashboard", metrics: QuantEdgeWeb.Telemetry
    end
  end
end
