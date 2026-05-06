defmodule QuantEdgeWeb.PageController do
  use QuantEdgeWeb, :controller

  def home(conn, _params) do
    render(conn, :home)
  end
end
