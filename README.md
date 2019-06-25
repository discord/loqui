# Loqui
Loqui is a transport that implements a very simple framing protocol over a raw socket. The framing protocol is similar
to http2, except that we've chosen deliberately to not implement a bunch of stuff, like flow control.
Instead, the RPC implements very simple request and response semantics. For our purposes, requests and responses are both adequately small so we can just send the entire payload in one frame, instead of having streams (like http2 does).

The RPC protocol does not care how you encode your data - treating all data passed through it as opaque binaries. However,
the protocol does support encoding negotiation, and compression where the client sends the server a list of encodings it can speak and compression algos it can use, and the server picks the encoding and compression it wants to use. Compression can be toggled on a per frame basis with frame flags.

# The protocol
The protocol is 9 opcodes, with a binary frame format.

Each frame starts with the opcode as an unsigned 8 bit integer (`uint8`). The opcodes are:

| Name              | Value | Client or Server | Has Payload ? |
| ----------------- | ----- | ---------------- | ------------- |
| `HELLO`           | `1`   | Client           | Yes           |
| `HELLO_ACK`       | `2`   | Server           | Yes           |
| `PING`            | `3`   | Both             | No            |
| `PONG`            | `4`   | Both             | No            |
| `REQUEST`         | `5`   | Client           | Yes           |
| `RESPONSE`        | `6`   | Server           | Yes           |
| `PUSH`            | `7`   | Both             | Yes           |
| `GOAWAY`          | `8`   | Server           | Yes           |
| `ERROR`           | `9`   | Server           | Yes           |

Following the opcode is the frame header - and then if applicable - the payload.
All integers are encoded in `Big Endian` format.

## `Hello`
The hello opcode is sent by the client to the server upon connecting. It advertises the client's Loqui version and a payload containing a a list of connection settings. Settings are in order and split by `|`and a specific setting can have a list of values split by `,`. In our current version the 2 settings are **supported encodings** and **supported compressions**.

| Offset | Type    | Description     |
| ------ | ------- | --------------- |
| `0`    | uint8   | opcode          |
| `1`    | uint8   | flags           |
| `2`    | uint8   | Loqui Version   |
| `3`    | uint32  | Payload Size    |
| `7`    | binary  | Payload Data    |



## `HelloAck`
The helloAck opcode is sent by the server upon receiving hello from the client. It contains the interval in which the server will ping (and that it expects the client to ping the server) - and the supported encodings within the payload data, contains the **encoding** and **compression** separated by `|`.

| Offset | Type    | Description       |
| ------ | ------- | ----------------- |
| `0`    | uint8   | opcode            |
| `1`    | uint8   | flags             |
| `2`    | uint32  | Ping Interval(ms) |
| `6`    | uint32  | Payload Size      |
| `10`   | binary  | Payload Data      |


## `Ping/Pong`
Ping and pong are sent back between the client and server. Either end can initiate a ping - and both ends are expected
to reply to a ping with a pong, with the seq that the remote end pinged with.

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint8    | flags            |
| `2`    | uint32   | Sequence Num     |

## `Request/Response`
The client can send requests to the server. The server is expected to reply with a `RESPONSE` payload with the sequence set to
request sequence.

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint8    | flags            |
| `2`    | uint32   | Sequence Num     |
| `6`    | uint32   | Payload Size     |
| `10`   | binary   | Payload Data     |

## `Push`
The client and server can send pushes to their remote connection. These are good for one off messages to
the service that do not need to be acknowledged.

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint8    | flags            |
| `2`    | uint32   | Payload Size     |
| `6`    | binary   | Payload Data     |

## `Go Away`
The server is getting ready to shut down the connection. It sends this opcode to tell the client end to finish sending
requests and to disconnect. The payload data can be empty, or a string with an error message. Or whatever else you want it to be.

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint8    | flags            |
| `2`    | uint16   | close code       |
| `4`    | uint32   | Payload Size     |
| `8`    | binary   | Payload Data     |

## `Error`
The server had an internal error processing a given request for a specific seq.

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint8    | flags            |
| `2`    | uint32   | Sequence Num     |
| `6`    | uint16   | error code       |
| `8`    | uint32   | Payload Size     |
| `12`   | binary   | Payload Data     |
