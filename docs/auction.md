# Auctions (`TAuktionsform`, segment `1028`)

Ported in [`game::auction`](../crates/game/src/auction.rs) (the world's
part), [`game::auktionsform`](../crates/game/src/auktionsform.rs) (the
form, the view's) and the computers' bids in
[`game::computer`](../crates/game/src/computer.rs). The human's clicks and
keys reach the world as commands
([`game::command`](../crates/game/src/command.rs), `World::apply`).

## The form

`TAuktionsform` (DFM: borderless, `fsStayOnTop`) has three images, laid
out by 1028:0dde, and three timers. The published fields, from the RTTI
at 1028:015e: `+0x17c` `AuktionsImage`, `+0x180` `Timer1`, `+0x184`
`Timer2`, `+0x188` `Timer3`, `+0x18c` `leiste`, `+0x190` `knopfimage`. The
method table at 1028:00ac gives the handlers:

| Handler | Address | Does |
|---|---|---|
| `Timer1Timer` | 1028:190d | 3000 ms after the third call: off; closes the form unless `Timer1.Tag` says it's closed; the winner gets the item (1028:08e7) |
| `Timer2Timer` | 1028:1940 | 1000 ms (the default interval) after an event started the auction: unless the TV shows a message (`1110:662d`), off, and the form opens (1028:0dde) |
| `Timer3Timer` | 1028:1973 | Every 100 ms while the form is open: a step, [below](#a-step-10281973) |
| `leisteMouseDown`/`Move`/`Up` | 1028:1b77, 1c4d, 1ce0 | The close button (16 × 16 at the left) and the bar to drag the form by |
| `knopfimageMouseDown`/`Up` | 1028:1e23, 2208 | The bid and pass buttons |
| `FormKeyDown` | 1028:26e4, en 1028:3620 | Escape, Space, Alt+F4: close and pass (v1.0: Enter bids) |

| Image | Bounds | Drawn from |
|---|---|---|
| `leiste` | 0, 0, 340 × 16 | `auktleis.BMP`, `AUKTION.TXT` 10 centred in 324, colour 10 |
| `AuktionsImage` | 0, 16, 340 × 235 | `auktbild.BMP`; the item's picture (`FILENAME.DAT`'s for the item's message, 125 × 125) at 54, 42; the auctioneer from `auktanim.BMP`; the paddles from `aukthand.BMP` |
| `knopfimage` | 0, 251, 340 × 65 | `auktfens.BMP`; the texts; a held button from `auktknop.BMP` |

The form is 340 × 316, centred by `AuktionsImage`'s height (so 40 pixels
low; with `enhance.form_positions` by its own). The click rectangles (inclusive) are at `1110:1654 + 8·n`: 3 the bid
button (197, 7, 300, 33), 4 the pass button (197, 35, 300, 61), 5 the drag
bar (16, 0, 340, 16), 6 the close button (0, 0, 16, 16), tested by
1028:01d4 (1030:1517).

`AUKTION.TXT` (12 `string[30]`s at `*1110:65b0`): 0 `Auktion`, 1
`verlassen`, 2 `Mehr bieten`, 3 `Aktuelles Gebot:`, 4 `von`, 5–7 `zum
ersten` .. `zum dritten`, 8 `Startgebot:`, 9 `erhielt den Zuschlag`, 10
`AUKTION`, 11 `starten`.

## State (unit 1028:2506)

| Global | Saved | What |
|---|---|---|
| `1110:6548` | yes | The last bidder, 0 for nobody |
| `1110:654a`, `659c` | yes | The price, the start price (each bid sets both) |
| `1110:654e + 4(p-1)` | no | Each player's limit, worked out when the form opens (10a8:0373) |
| `1110:656e` | yes | The item: the message number of the event that started it |
| `1110:6596` | yes | The last bidder's colour, for the paddle |
| `1110:65a0` | yes | The calls: `0xff` before the first bid, 0 after a bid, then 1–3 |
| `1110:65a2`, `65a6` | yes | Steps since the last call, and a wait added to the 10 (always 0) |
| `1110:6599` | yes | The button held down: 3 bid, 4 pass, 5 drag, 6 close |
| `1110:659a`, `6598` | yes | The form rolled up by a double click on the bar; unknown |
| `1110:65aa` | no | `GetTickCount` at the last call |
| `1110:65ae` | no | The player has started the auction |
| `1110:65b4`..`65b9` | no | The hammer's and the hand's frame and whether they move |
| `1110:0063` | map | The option "Auktion wartet": the auction waits for the player |

## The flow

1. An event (`events_tick`, [simulation.md](simulation.md#events-10103084))
   calls `start_auction(item, price)` (1028:08ac): price, start price,
   item, and `Timer2` on.
2. `Timer2` (1 s later): the form opens (1028:0dde). First 10a8:0373
   works out every player's limit at the price. For item 0x15 (a
   building sale, event 9) whose seller (event 9's argument) is the human,
   no form opens: the computers bid among themselves at once (1028:0c82).
   Otherwise the form is drawn, the main screen's `AnimationsTimer` stops
   (1020:24a0) and the other forms are disabled (1010:2970): **the
   simulation stands while the form is open.** `Timer3` starts; nobody has
   bid; the auction has started unless "Auktion wartet" is on.
3. `Timer3` steps until the item is sold (below). A bid (1028:15ef):
   price += price div 20 (also the start price), the calls start again,
   and the bidder's paddle (in their colour) slides up from the bottom and
   back, 35 frames each at least 25 ms after the last by `GetTickCount`,
   with the message loop blocked and `Timer3` off.
4. `Timer1` (3 s after the third call): the form closes (1028:0476: the
   main screen and its timer come back) and the last bidder gets the item
   (1028:08e7).

### A step (1028:1973)

Nothing until the player has started the auction. Then the step counter
goes up, and if someone has bid and it has passed the wait plus 10:
`Random(20) = 1` calls the next "going" (with sounds 0x1a–0x1c, the hammer,
the counter back to 0; at the third call "*name* erhielt den Zuschlag" and
`Timer1` on), then `Random(15) = 1` waves the hand unless the hammer moves.
Unless `Timer1` runs: `Random(7) + 2` picks a computer; if it isn't the
last bidder and no button is held, `Random(30) = 1` and its limit above
the price, it bids, though its bid (5 % more) may be above its limit
(`fix.auction_computer_limit`: only when the bid is within it). Then the auctioneer's animation (1028:0221): his
mouth `Random(7)` within 800 ms of a call; eyes and brows each with
`Random(20) = 1` then `Random(4) + 1`; the hammer's and hand's next frames
(the hammer's sound 0x14 at its fifth).

### The player's buttons

- **Bid** (mouse down, unless `Timer1` runs): with the auction started,
  only with the money (price + 5 % up to the player's limit, else error
  0xe); mouse up over it bids. If the auction waits for the player, the
  button says "Auktion starten" and starts it instead. In v1.0 Enter
  did the same without either check (1028:26e4); the later build took
  Enter out (en 1028:3620, [versions.md](versions.md)), and so does the
  port.
- **Pass** (mouse down, unless `Timer1` runs) and the **close** button
  (mouse down, always): the computers bid among themselves (1028:0c82):
  if any limit is above the price, up to 101 tries of a raise of
  `Random(20)` × 5 % and a computer `Random(7) + 2` until one's limit is
  above it; it gets the item at that price. If none's is and the item is a
  bankrupt player's property (0x16), it's demolished (10a0:09ea). A bid
  that leads is ignored, and after the third call the close button gives
  the item away a second time; with `fix.auction_pass` nothing happens
  once it's sold, and when no computer takes it a leading bidder gets it
  at their bid. Mouse up over the button closes the form. Escape, Space
  and Alt+F4 close the form first.
- **Drag bar**: the form shrinks to its title bar while dragged (and
  stays so after a double click) and follows the mouse, kept on screen.
  The title is drawn again a row higher than the form opened with it
  (1028:1ce0; with `enhance.auction_title` on its row).

Any button held down keeps the computers from bidding.

### What the winner gets (1028:08e7)

The winner pays the price (`add_money`; for player 1 the books' new
buildings, 0x67). Then by item:

- 0x15 (a building, event 9): message 0x29 at the building with the
  seller and the winner; the building's squares (by its type, a site's
  the type it becomes) become the winner's (10a0:03ca), and the seller
  gets the price.
- 0x16 (a bankrupt player's property, event 10): message 0x2a; all of
  their squares become the winner's (10a0:0346).
- 7, 0x55, 10, 1, 8, 0xd, 0x10, 0x3c, 0x33, 0x13, 0x39, 0x36: their
  message (0x25, 0x56, 0xb, 2, 0x28, 0xe, 0x11, 0x3d, 0x34, 0x14, 0x3a,
  0x37) and `make_available(type, winner)` of 0x71, 0x79, 0x1e, 0x1d,
  0x6f, 0x3c, 0x23, 0x6e, 0x6c, 0x25, 0x7e, 0x7d: the build menu for
  player 1, building it next for 2–7 (and for 8 with
  `fix.prize_player8`; the original has no case for 8).
- 6: its message 0x27 only. Anything else: nothing.

## The computers' bids (10a8:0373)

For players 1 to 8 (10a8:0221 with the price):

- Player 1 (the human): their money, but 0 for their own building
  (item 0x15, event 9's square theirs).
- A computer (2 to the player count) that isn't out: for item 0x15, 0 if
  it owns the square, else its bid; for 0x16 its bid; for anything else
  its bid only if its area doesn't wait for its first roads
  (`*1110:70d6`).
- Anyone else: 0.

A computer's bid (10a8:0167): what it can pay is its money less its loan
plus `MaxSchulden`; below the price, 0; else `price + (min(can, 3 ×
price) − price) × Random(1000) div 999`.

## The port

- The timers count the world's 100 ms ticks: `Timer2` 10, `Timer1` 30,
  `Timer3` every tick from the one after the form opens. They run before
  the clock's steps in `World::tick`, in the DFM's order, and the clock's
  steps don't run while the form is open. A bid blocks the timers for 9
  ticks (35 frames of 25 ms); the timers that come due meanwhile fire
  after it, as the original's queued `WM_TIMER`s would. The auctioneer's
  800 ms are 8 ticks.
- The auctioneer's animation is cosmetic, but its random numbers come
  from `RandSeed` between the computers' bids, and it depends on nothing
  of a player's view, so the world draws them in the original's order
  and keeps the frames (`Auction::frames`) for the view.
- The held button (`1110:6599`) changes the world (no computer bids
  meanwhile), so holding a button is a command (`Action::Hold`).
- Commands during a paddle's animation act at once; the original handles
  the input after the animation.
- `Timer2` doesn't open the form while the TV's speaker is open
  (`1110:662d`): the view's TV says so with `Action::Speaker`, and the
  timer tries again 10 ticks later ([sound.md](sound.md#the-tvs-speaker)).
  The sounds are the world's `Tick::sounds`; the three calls' files
  aren't where the game looks for them, so they're silent.
- A bid without the money shows `FEHLER.TXT`'s error 0xe on the main
  screen's status bar with sound 0xc (`show_error_message`, 1030:232d,
  from 1028:1e23, en 1028:301f), not a message box: the view's
  (`Auktionsform::status_error`), no command.
- The won item's `make_available` (10a0:0691) puts the type on the end
  of its kind's submenu of the build menu (1018:23f8,
  `World::add_to_build_menu`), which is the world's
  ([player-tools.md](player-tools.md)); the original doesn't redraw an
  open build menu for it, and neither does the port.

The bugs kept are in [original-bugs.md](original-bugs.md).

## Multiplayer

Decision 7 in [port-architecture.md](port-architecture.md#decided),
built in `game::auction` for games with more than one human client
(`Seats::multiplayer`); with one the auction is the original's, the form
player 1's. The form stops the simulation for everyone while it's open,
in the original and here: the auction ends by its timers, which are
world ticks, so no client's dialog holds the world.

- Every human seat that's in has the form (`World::auction_seats`), and
  each seat's view bids with its seat's limit (`Auktionsform::seat`). A
  human seat's limit is its money, as the original's human's, and
  nothing for its own building (`bid_limit`); a seat a computer plays
  while its clients are away bids as the computer.
- A co-op seat's clients act for it together: any one's start, bid,
  hold or pass is the seat's (`World::auction_seat_command`).
- With "Auktion wartet" (the host's `1110:0063`) the auction waits until
  every human seat has started it (the bid button, `Auction::started`)
  or passed (`Auction::passed`), but no longer than the host's timeout
  (`Options::auction_timeout`, default 300 ticks, 30 s;
  `Action::AuctionTimeout`, saved in the port's chunk as
  `options.auction_timeout`): then the seats that haven't count as
  passed. Without it the auction starts as it opens, as the original's.
- Any seat holding a button (`Auction::held_by`) keeps the computers from
  bidding; a seat that passes, or whose clients have all gone, holds
  none.
- Passing, closing the form and leaving it with a key are the seat's
  pass: its view hides its form, and the seat can't bid again. The item
  goes to the computers (1028:0c82) only once every human seat has
  passed; then the form closes for everyone. A seat that sells its own
  building has passed from the start; with no other human seat the
  computers bid among themselves at once, as for the original's human.
- The winner's books are their seat's ([port-architecture.md](port-architecture.md#seats-and-clients)),
  and a prize goes onto a human seat's own build menu, not as a
  computer's order (`make_available`).
- A human seat that goes bankrupt (on the 2nd of January, decision 9)
  has its property auctioned as a computer's: event 10, item 0x16
  (`World::auction_property`); it has no form for it, being out.
- With `fix.saved_state` the port's chunk keeps the seats' auction
  (`state.auction.seats`).

## Open questions
- What `1110:6598` is: saved and loaded, never used here.
- Why the form is centred by `AuktionsImage`'s height rather than its
  own.
