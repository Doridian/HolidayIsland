# Networking

Network games: humans take the computers' seats of a game over the
network ([port-architecture.md](port-architecture.md#seats-and-clients)
has the seats, co-op and the 13 decisions). Built natively and in the
browser ([below](#the-browser-build)).

- `crates/net`: the transport trait, the WebRTC transport (matchbox),
  an in-memory transport for tests, the lockstep session and its wire
  format.
- `crates/server`: the signaling server, `holiday-server`.
- `crates/holiday`: the command line's options (`--host=`, `--join=`
  and the rest, `crates/holiday/src/network.rs`). There's no UI for it
  yet.

## Playing

Someone runs the signaling server (below). The host starts the game with
`--host=ROOM`, picks the game on the start menu as usual and starts it;
the game is hosted from then on. Each other player starts with
`--join=ROOM`: the game waits, without showing the start menu, until the
host's world comes, sets the game up from it and plays a seat.

```sh
holiday-server                                   # on a machine both can reach
holiday --host=island --signal=ws://server:3536  # the host
holiday --join=island --signal=ws://server:3536  # each other player
```

| Option | |
|---|---|
| `--host=ROOM` | host the game started from the start menu, in `ROOM` (letters, digits, `-`, `_`) |
| `--join=ROOM` | join the game hosted in `ROOM` |
| `--signal=URL` | the signaling server, `ws://` or `wss://`; default `ws://localhost:3536` |
| `--seat=N` | joining: the seat (player) to play, 1 to 8. A computer's seat makes the player its human; a seat another client plays is shared (co-op). Default: the first computer's |
| `--rejoin=ID` | joining: come back as client `ID` after a dropped connection; the game prints the number when it joins |
| `--stun=URL` | a STUN server, `stun:host:port`; repeatable |
| `--turn=URL` | a TURN server, `turn:host:port` or `turns:host:port`; repeatable |
| `--turn-user=`, `--turn-password=` | the TURN server's credentials, needed with `--turn=` |

Without `--host=` or `--join=` nothing of this runs: no connection, no
thread, no runtime. The game says on the terminal what happens (who
joined or left, a desync, the end of the session), with `network:` in
front.

Every player needs the same release of the game's files (the German CD
or the English release, `orig-en/`): the world's tables come from them.
A client whose tables differ from the host's is told so and stops
(`data` in the snapshot, below).

## Running the signaling server

```sh
cargo run --release -p server                           # 0.0.0.0:3536
cargo run --release -p server -- --listen=127.0.0.1:4000
```

The binary is `holiday-server`. The players' games connect to it over
a WebSocket to find each other; once their WebRTC connections are up,
the game's messages go between them directly (or through TURN), not
through the server. Behind a reverse proxy with TLS, the games use
`--signal=wss://example.org/holiday`; the server takes the room as the
whole path, so the proxy rewrites `/holiday/ROOM` to `/ROOM`.

The server is a star per room on matchbox's signaling library
(`server::Star`): the host connects to `/<room>~host`, the clients to
`/<room>`. The host hears of each client of its room and offers it a
connection; a client hears of the host only. A client that comes first
waits for the host. A second host of a room is turned away: its
connection closes, which the game reports as the connection having
closed.

## STUN and TURN

WebRTC connects the peers directly where it can. On one machine or one
LAN no server is needed. Across the Internet the peers need a STUN
server to learn their public addresses (`--stun=`), and players behind
routers that don't let a direct connection through (symmetric NAT,
strict firewalls) need a TURN server, which relays their traffic
(`--turn=`). There's no default: the game contacts no server the
player didn't name.

coturn does both. A minimal `/etc/turnserver.conf`:

```
listening-port=3478
fingerprint
lt-cred-mech
user=holiday:SECRET
realm=example.org
# The relay's ports, open in the firewall (UDP):
min-port=49160
max-port=49200
# With a public address behind NAT (a cloud VM):
# external-ip=PUBLIC_IP/PRIVATE_IP
```

Open 3478 (UDP and TCP) and the relay ports. Then every player uses:

```sh
holiday --join=island --signal=wss://example.org/holiday \
  --stun=stun:example.org:3478 \
  --turn=turn:example.org:3478 --turn-user=holiday --turn-password=SECRET
```

Each player's game uses TURN only if a direct connection fails, so
giving it to everyone costs nothing for those who connect directly.
Static credentials are visible to every player; coturn's REST API
(`use-auth-secret`) gives short-lived ones, which a later UI could
fetch.

## The protocol

**Topology.** A star: each client has one connection, to the host, on
WebRTC's reliable, ordered data channel. The clients aren't connected
to each other.

**Lockstep, host-authoritative** (`net::session`).

- A client sends each command its view makes (`Action`) to the host
  instead of applying it. The host's own go the same way.
- The host stamps the commands with the tick they're carried out on:
  the tick it's on plus the input delay (3 ticks, 300 ms), in the order
  they came. As it runs tick `T` it sends every client the **go** for
  `T + 3`: that tick's list of `ClientCommand`s, empty or not.
- Every peer runs a tick only with its go: it applies the list's
  commands in order (`World::apply_client`), then ticks the world. The
  host's clock paces the game; a client that has more goes than the
  delay keeps in hand runs them at once (`Session::behind`), so it runs
  each tick as soon as the host has, as long as the network is quicker
  than the delay. Without the go it waits.
- While the host is alone in a hosted game its commands go through
  the session too, so they're carried out the delay later than in
  single player.
- The host applies each go as the clients read it: it encodes the list
  and decodes it again before applying it, so a command that doesn't
  survive the wire (a loaded game's file) is the same on every peer.
- Every 10 ticks each peer hashes its world and sends the hash (the
  host to every client, a client to the host); each compares with its
  own for that tick. A mismatch is reported as a desync (on the
  terminal: the tick and both hashes), and the game goes on.
- The world's hash is its derived `Hash` fed to `net::hash::StableHasher`:
  integers as little-endian bytes, lengths as 64 bits, so it's the same
  on 32-bit wasm and 64-bit native, and pinned by a test.

**Joining.** A client connects and says hello (the seat it wants, or
the client it was). The host gives it a client id and a seat and sends
them back (welcome), and stamps `SeatChange::Join` (or `Rejoin`) like
any command, for tick `E`. Before it runs tick `E` it sends the client
a **snapshot**: its world before `E` as a saved game
(`World::snapshot`, the original's format with the port's chunk, with
`fix.saved_state` on in it so nothing is lost), then the goes from `E`
on. The client reads the snapshot over a world set up from its own
game files (`World::load_snapshot`, which keeps the file's seats as
they are) and runs from `E`, whose go seats it. The snapshot is tested
to be the host's world bit for bit. A joining client's commands go to
the host from the welcome on; the seat change comes before them.

A client asks for a seat (`--seat=`); without, it gets the first
computer's. The host refuses a seat the game hasn't, or, without one
asked for, a game with no computer's seat left.

**Leaving.** A client's connection dropping is
`SeatChange::Disconnect` for it, stamped by the host: the seat goes to
the computer while none of its clients is there. Coming back with
`--rejoin=ID` is `SeatChange::Rejoin`, with a new snapshot. A client
that ends its game (back to the start menu, or quitting) says goodbye
first: `SeatChange::Leave`. When the host's game ends (back to the
start menu, or quitting), it tells its clients, which print why and go
back to the start menu; if the host's connection just drops, the
clients notice that instead. A host whose own connection ends plays on
alone: its clients' seats go to the computers by its own `Disconnect`
commands.

**Wire format** (`net::wire`), hand-written so the same message is
always the same bytes: the protocol version (`u16`, now 1; the host
refuses other versions), a tag byte, the fields. Integers
little-endian; lists and byte strings with a `u32` length; a fix by its
key; a saved game as its file. The messages: `Hello`, `Welcome`,
`Reject`, `Command` (client to host), `Snapshot`, `Go`, `Hash` (both
ways), `Bye` (client), `End` (host). Under them, frames of at most
16 KiB (what every browser's data channel takes): a byte saying whole,
more to come or last, then the piece. The snapshot is a few hundred KiB
and goes in pieces.

**The transport trait** (`net::transport::Transport`): connect to a
room as host or client, send bytes to a peer, broadcast, and poll the
messages and the peers that came and went. It never blocks; the game
loop polls it once a frame. The WebRTC transport and the in-memory one
implement it, and a WebSocket relay (every message through a server,
for networks where WebRTC can't get through even with TURN) can be
another without touching the session.

## The browser build

Built ([web.md](web.md)). `crates/net` compiles for
`wasm32-unknown-unknown`: the trait has no blocking calls, the session
is polled from the game loop, and tokio is a native-only, optional
dependency.

- `MatchboxTransport`'s message loop is spawned on the page's event
  loop with `wasm_bindgen_futures::spawn_local` instead of a thread
  with a tokio runtime (`spawn` in `net::matchbox`); matchbox uses the
  browser's `RTCPeerConnection` there.
- The options come from the page's query instead of the command line,
  each without its `--`, through the same parser:
  `?host=ROOM&signal=wss://example.org/holiday&stun=stun:example.org:3478`,
  `?join=ROOM&seat=3`, `?join=ROOM&rejoin=2`. `stun=` and `turn=` may
  repeat.
- matchbox always gives the browser's `RTCPeerConnection` one ICE server,
  which the browser refuses without a URL. Without `stun=` or `turn=`
  the page gives it `stun:127.0.0.1:9` (this machine's discard port),
  so it still contacts no server the player didn't name
  (`net::matchbox::IceServers::config`).
- `wss://` for the signaling server when the page is served over HTTPS
  (browsers refuse `ws://` from a secure page).
- What the game says about the network (`network: ...`) goes to the
  browser's console.
- The hash and the wire format are the same on every target; a native
  host and a browser client play together, and the other way round.
  Both need the same release of the game's files: the browser plays
  the German CD's, so a native peer needs those too.

## Open

- **The host leaving.** Ending the session cleanly for the clients is
  built (they're told and go back to the start menu); what should
  happen instead (another client takes over as host, the game pauses
  for the host to come back, the clients play on without one) isn't
  decided.
- A UI: hosting, joining and the seats from the start menu, and the
  network's messages on screen rather than the terminal.
- The players' language: the world's sabotage table comes from the
  texts of the language a world is set up in (`SABOTAGE.TXT`: the
  English and German texts differ in one target), so a client sets its
  base world up in the snapshot's world language. A host and a client
  with different releases of the game's files can't play together.
- Reconnecting without a restart: a client whose connection dropped
  comes back by starting the game again with `--rejoin=`.
- A client keeps its own view settings; the views of a joining client
  start where a default view looks, not where its last one did
  (`World::to_map_with_views` could carry them in the snapshot).
