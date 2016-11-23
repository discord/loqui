# drpc
DRPC or Discord RPC (not to be confused with our other RPC) is an RPC protocol that 
implements a very simple framing protocol over a raw socket. The framing protocol is similar to http2, except
that we've chosen deliberately to not implement a bunch of stuff, like flow control, or server push. Instead, the RPC
implements very simple request and response semantics, and a fire and forget client to server call called a "push". 

The RPC protocol does not care how you encode your data - treating all data passed through it as opaque binaries. However,
the protocol does support encoding negotiation, where the server sends the client a list of encodings it can speak, 
and the client picks the encoding it wants to use, and sends it back.

# The protocol
The protocol is 8 opcodes, with a binary frame format.

Each frame starts with the opcode as an unsigned 8 bit integer (`uint8`). The opcodes are:

| Name              | Value | Client or Server | Has Payload ? |
| ----------------- | ----- | ---------------- | ------------- | 
| `HELLO`           | `1`   | Server           | Yes           |
| `SELECT_ENCODING` | `2`   | Client           | Yes           |
| `PING`            | `3`   | Both             | No            |
| `PONG`            | `4`   | Both             | No            |
| `REQUEST`         | `5`   | Client           | Yes           |
| `RESPONSE`        | `6`   | Server           | Yes           |
| `PUSH`            | `7`   | BOTH             | Yes           |
| `GOAWAY`          | `8`   | Both             | Yes           |
| `ERROR`           | `9`   | Server           | Yes           |

Following the opcode is the frame header - and then if applicable - the payload.
All integers are encoded in `Big Endian` format. 

## `HELLO`
The hello opcode is sent to the client upon connecting with the server. It advertises the server's DRPC version, the interval
in which the server will ping (and that it expects the client to ping the server) - and the supported encodings within the 
payload data, as a string of comma separated encodings.

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint8    | DRPC Version     |
| `2`    | uint32   | Ping Interval(ms)|
| `6`    | uint32   | Payload Size     | 
| `10`   | binary   | Payload Data     |


## `Select Encoding`
The select encoding opcode is sent by the client to the server, telling it which encoding it will speak to the server,
and what encoding the server should speak while talking to the client. 

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint32   | Payload Size     | 
| `5`    | binary   | Payload Data     |


## `Ping/Pong`
Ping and pong are sent back between the client and server. Either end can initiate a ping - and both ends are expected
to reply to a ping with a pong, with the seq that the remote end pinged with. 

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint32   | Sequence Num     | 

## `Request/Response`
The client can send requests to the server. The server is expected to reply with a `RESPONSE` payload with the sequence set to 
request sequence.

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint32   | Sequence Num     | 
| `5`    | uint32   | Payload Size     | 
| `9`    | binary   | Payload Data     |

## `Push`
The client and server can send pushes to their remote connection. These are good for one off messages to
the service that do not need to be acknowledged. 

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint32   | Payload Size     | 
| `5`    | binary   | Payload Data     |

## `Go Away`
The client or server is getting ready to shut down the connection. It sends this opcode to tell the other end to finish sending 
requests and to disconnect. The payload data can be empty, or a string with an error message. Or whatever else you want it to be.

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint8    | close code       | 
| `2`    | uint32   | Payload Size     |
| `6`    | binary   | Payload Data     |

## `Error`
The server had an internal error processing a given request for a specific seq.

| Offset | Type     | Description      |
| ------ | -------- | -----------------|
| `0`    | uint8    | opcode           |
| `1`    | uint8    | error code       |
| `2`    | uint32   | Sequence Num     |
| `6`    | uint32   | Payload Size     |
| `10`   | binary   | Payload Data     |
