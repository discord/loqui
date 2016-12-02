defmodule Loqui do
  use Application
  require Logger

  ## OTP

  def start(_type, _args) do
    Loqui.Supervisor.start_link
  end

  def pool_name() do
    Application.get_env(:loqui, :pool_name, :loqui)
  end
end
