# Loqui, A minimal transport Layer

Loqui allows you to easily create a fast, multiplexed client and server in several languages. 
We're going to show you how to build a simple Elixir client and server using its primitives. 

### Creating the Server

First, let's create the server. Loqui servers consist of two parts; a `Handler` and a `Supervisor`. 
Since Loqui connections start as HTTP requests and upgrade to the Loqui protocol, we need to 
set up a Cowboy server to handle the initial HTTP requests. This is accomplished with the 
following code:

```elixir
defmodule TestServer.Supervisor do  
  use Supervisor

  def start_link() do
    cowboy_routes = :cowboy_router.compile([
      {:_, [
        {"/_rpc", TestServer.Handler, []},
      ]}
    ])

    children = [
      :ranch.child_spec(:http, 100,
        :ranch_tcp, [
          port: 8080,
          max_connections: :infinity
        ],
        :cowboy_protocol, [env: [dispatch: cowboy_routes]]),
    ]

    Supervisor.start_link(children, strategy: :one_for_one, name: __MODULE__)
  end
end
```

This makes the HTTP path "/_rpc" available to service Loqui requests. There are also 100 acceptor 
processes available to handle requests. This is not the same thing as HTTP workers, so please see
the Cowboy docs for clarification on how to set this.

Now, we need to build our handler. Our handler implements several callbacks, prefaced with `loqui`. 
These are:


| Callback |  Description             | 
|----------|-------------------------|
|  `loqui_init`       | Called when the handler process is created, receives the transport (socket) the request and the options given to the handler |
| `loqui_request`     | Handles a request. Return values are serialized by the negotiated encoder for requests and thrown out for pushes |
| `loqui_terminate`   | Called when the handler process exits. Useful for cleanup | 


So, a minimal server handler would look like this:


```elixir
defmodule TestServer.Handler do
  alias Loqui.Protocol.Codecs.Erlpack

  def init(_transport, req, _opts) do
    # HTTP Behavior
    case :cowboy_req.header("upgrade", req) do
      {"loqui", _req} -> 
         {:upgrade, :protocol, Loqui.CowboyProtocol}

      {:undefined, req} -> 
         {:ok, req, nil}
    end
  end

  def handle(req, _state) do
    :cowboy_req.reply(401, [], "", req)
  end

  def terminate(_reason, _req, _state), 
    do: :ok

  ## Loqui Callbacks

  def loqui_init(_transport, req, _opts) do
    opts = %{supported_encodings: [Erlpack], supported_compressions: []}
    {:ok, req, opts}
  end

  def loqui_request(_request, _encoding), 
    do: "Worked!"

  def loqui_terminate(_reason, _req), 
    do: :ok
end
```

During the protocol upgrade, both the client and server negotiate an encoder and a compressor. 
Encoders turn Erlang terms to and from binary data and compressors compress and
decompress that data. In the above example, we've disabled compressors and have used the `Erlpack` 
codec, which uses `:erlang.term_to_binary` and `:erlang.binary_to_term` to encode and decode data. 

Simply place your Supervisor into your Application's supervision tree and the server will start. 


### Creating a client

Not surprisingly, creating a client is just a matter of calling `start_link` on the client. 

```elixir
alias Loqui.Protocol.Codecs.Erlpack
{:ok, client} = Loqui.Client.start_link("localhost", 8080, "/_rpc", loqui_opts: [codecs: [Erlpack]])
```

The client will then connect, negotiate an upgrade and be ready to make requests. To make a synchronous
request, use the `request` function:

```elixir
> {:ok, response} = Loqui.Client.request(client, "hey!")
{:ok, "Worked!"}
> 
```

You can also use the `push` function to send an asynchronous request:

```elixir
> Loqui.Client.push(client, "async")
:ok
> 
```

Be careful with sending out async requests, as you can overload the server. 
You can also close the client with the `close` function or ping the server with the `ping` function. 

```elixir
> Loqui.Client.ping(client)
:ok
> Loqui.Client.close(client)
:ok
```

Both `request` and `ping` take a timeout, so you can have your calling process wait as long 
as needed for a response.
