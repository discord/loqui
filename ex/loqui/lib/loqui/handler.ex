defmodule Loqui.Handler do
  @type encoding :: String.t
  @type compression :: String.t
  @type options :: %{
    supported_encodings: [encoding],
    supported_compressions: [compression]
  }
  @type reason :: atom | tuple

  @callback loqui_init(:ranch.transport, keyword) :: {:ok, options}
  @callback loqui_request(any, String.t) :: any
  @callback loqui_terminate(reason) :: :ok
end
