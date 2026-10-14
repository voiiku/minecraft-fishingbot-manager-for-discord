# watchdog: the gateway watchdog
[简体中文](watchdog-zh_cn.md) | **English**

* Code: `src/watchdog.rs` (the `Watchdog` itself: the timing fields, the handshake/probe bookkeeping, the offline
  record and the process-wide singleton `WATCHDOG`) plus its two submodules: `gateway` (the handshake/probe decision plus
  the probe task) and `lifecycle` (the offline bookkeeping, the reconnect signal, waiting for an attempt to end and
  the shutdown/exit helpers); the entry point `src/main.rs` only wires events and runs the reconnect loop (it builds
  that singleton explicitly with `WATCHDOG.get_or_init()` (the first call builds it from the config); the event
  callbacks are plain fn pointers and go through that same entry point).
* Purpose: decide whether this process has lost contact with Discord. A failed decision hands the reason to the
  process entry (`request_reconnect()`), which stops every bot and rebuilds the connection; the process does not
  exit, it waits one judging window and builds a fresh `Client` inside the same process (section 4).

## 1. Why it has to exist

When Discord is unreachable we cannot control the bots any more, yet they still hold the game accounts: FishingBot
reconnects by itself after a drop, the account stays online, and real players cannot get in. Better to stop
everything cleanly than to leave a fishing machine nobody can command.

The watchdog uses two signals available in this implementation: the gateway handshake and REST reachability. When it
detects a disconnect, the process stops the bots, stays running and reconnects itself rather than relying on a process
restart. This avoids a cold start and repeated program log files for each retry.

## 2. The two criteria

* Handshake: within `first_handshake_timeout` of the attempt starting, one `READY` or `RESUMED` must have arrived
  from Discord (`main.rs` records it with `watchdog.record_handshake_success()`). If not, ask for a reconnect with the reason
  `NeverReady`.
* Reachability: every `probe_interval`, fetch our own information over REST (`watchdog.probe()`,
  started once per connection with that connection's `Http` handle): success calls `watchdog.clear_probe_failures()`,
  failure calls `watchdog.record_probe_failure_start()`. When the failures add up to a full `window`, ask for a reconnect
  with the reason `Unreachable`.

Details:

* All three timings come from the config and are independent: the judging window `timing.watchdog_window_ms`, the
  probe interval `timing.probe_interval_ms` (a third of the window in the shipped example) and the first-handshake
  timeout `timing.first_handshake_timeout_ms` (twice the window in the shipped example). All three are required (no
  in-code defaults). The entry point packs them into the `Watchdog` (they are three of its fields), and both the
  decision and the probe use that one value. The probe interval must be less than or
  equal to the judging window: if it is slower, the very first failed probe already fills a window, which makes one
  failure a disconnect; startup rejects that config.
* The probe goes over REST: an HTTP response from `GET /api/v10/gateway` proves only that the REST endpoint is
  reachable, not that the token is valid. Authentication errors are reported by the Discord client's connection flow.
  Timeouts and connection errors count as failed probes. The probe hits `discord.com` while the gateway connects to
  `gateway.discord.gg`; if a deployment shows the API working but the gateway unreachable, consider adding a TCP or TLS
  probe of the gateway host.
* The probe task starts before `client.start()`: when the gateway never connects at all, `READY` never comes and the
  decision has to work outside the client as well. Starting it early also means "no network at startup" exits within
  the first-handshake timeout.
* The decision is reversible: a recovered probe, or another `READY`/`RESUMED`, goes back to `Alive`; one bad verdict
  is not final.
* The decision is pure timing: four states `Alive`, `Waiting`, `NeverReady`, `Unreachable` (internal to the watchdog, not exposed), with the moment passed in
  by the caller (`watchdog.state(now)`), so every rule has a unit test (including "only the first failure counts"
  and "recovery revives").

## 3. Deliberately not reading framework state

serenity 0.12.5 exposes only actions such as `shutdown_clean` and `set_presence` through `ShardMessenger` (that is,
`ctx.shard`); there is no `stage()` or `latency()`, and `Shard` is not reachable from inside the shard runner either.
The half-dead gateway state (connected but receiving nothing) is found and reconnected by serenity's own heartbeat
and ACK machinery (the protocol guarantees it); we must not and cannot reimplement it, so we only cover the two
"cannot reach Discord at all" cases above.

## 4. After a disconnect: stop the bots, wait one window, rebuild the connection

The judgement is `gateway.rs`; the cleanup and the rebuild are `src/watchdog/lifecycle.rs`. The division is "the
judging side only judges and wakes; every visible action (stopping, recording, logging, exiting) lives on the
entry-point side" - its actions are `wait_for_attempt()` (wait for this attempt to end and clean up a
disconnect), `wait_before_reconnect()` (wait out one judging window) and `exit_on_shutdown_signal()` (stop the bots
and exit with code 0 after a shutdown signal).

1. The judging side: a lost verdict leads to `request_reconnect(reason)`, which stores the reason on the watchdog
   and wakes the process entry (a `tokio::Notify`); it does not touch the bots itself.
2. The entry point's cleanup (the "watchdog judged a disconnect" arm of `wait_for_attempt()`): it picks up the
   reason the judging side handed over and calls the private `stop_all_bots_on_disconnect(reason)`, which stops
   every bot (`shutdown_all_bots()`, the same flow as `/stopall`), writes one WARN and records the reason and the
   moment.
   (When there is no reason to pick up, the wake-up was a late one and that round already cleaned up, so nothing is
   touched.) Why stopping is mandatory: with Discord unreachable nobody can command the bots, and FishingBot's
   automatic reconnect keeps holding the game accounts and fighting real players for them. Nothing recovers by itself
   while offline; after a successful reconnect a person sends `/startall` or `/start <name>` in Discord.
3. Then it closes the old connection (`shard_manager.shutdown_all()`, otherwise the old and the new connection fight
   over the same bot session and Discord kicks them back and forth alternately), calls `wait_before_reconnect()` for
   one judging window (reusing `timing.watchdog_window_ms`, no new config field), and builds a fresh `Client` for one
   more attempt; if it still fails it keeps retrying the same way.

The client returning on its own (`client.start()` returning: shut down, or a fatal error such as an invalid token or
Intent - a plain disconnect never makes it return) goes through `wait_for_attempt()` too: the same
`stop_all_bots_on_disconnect()`, only the reason is written by the entry point itself and it has no reason to wake
itself, so both paths clean up and report the recovery identically.

Two easy traps, both pinned down by unit tests:

* Every attempt must call `watchdog.begin_connection_attempt()`, so the first-handshake timeout starts counting from
  this attempt. Without the reset, a reconnect is judged `NeverReady` the moment it starts (that is the previous
  attempt's debt), and you get one reconnect per window that never connects.
* The probe task has to be stoppable (`stop` is an `AtomicBool` checked once per round). The previous round's probe
  must go away, otherwise two tasks probe, keep score and shout "offline" together. A flag instead of a notification
  because a flag is state and cannot be missed, while an event like `notify_waiters()` is lost when nobody waits.

Only errors that retrying cannot fix exit the process (`exit_on_permanent_failure`, exit code 1; the cause is logged
at `ERROR`, because it is a wrong config, and it is told apart from retryable network flakiness at `WARN` at a
glance): an invalid token, a bot that was never invited to a server (`50001 Missing Access`), or the setup being
rejected. Those are config mistakes, and a ten-thousandth retry would fail the same way. Exit code 1 is this
project's deliberate-failure code (a failed config load or validation also
exits `main` with 1), so the unit file should set `RestartPreventExitStatus=1` (see
[deployment tutorial section 12](../../tutorial/tutorial-en_us.md#12-configure-a-systemd-service-optional-recommended)):
such an error stops after one exit and goes to `failed` until someone looks at it. A crash is a different code
(panic 101, killed by OOM or `kill` 137, ...) and is restarted by `Restart=on-failure` as usual.

### The notice after a successful reconnect

When `Ready` or `Resume` arrives again, `watchdog.notify_reconnect_success()` (the entry point hands it the
`serenity::Context`) sends one message to every server this bot is in: `⚠ this bot lost
contact with Discord at <time> and is back online now (offline for <N>s). Every game bot was stopped while offline`.
It lands in that server's system channel, or in its first text channel when there is none; one message per server,
and a server it cannot reach only leaves a warning without affecting the others. (Want the bots running again after
such a notice? Send `/startall`.) The notice also goes into the program log (`ReconnectedAfterOffline`: how many
servers it was sent to, and how many failed).

## 5. Common problems

* The log keeps showing `NeverReady` reconnects (the process does not exit): outbound traffic is blocked, the token
  is wrong, or Discord itself is having trouble. Check from the deployment machine first:
  `curl -sS -o /dev/null -w '%{http_code}\n' https://discord.com/api/v10/gateway` should return 200.
* The log says `Unreachable`: no network, the proxy died, the token expired; the line above it, `REST probe failed:`,
  gives the concrete reason.
* A bit of network flakiness causes one reconnect: raise `timing.watchdog_window_ms` (60000, say, and
  `timing.first_handshake_timeout_ms` with it if needed); recovering inside the window is not a false verdict.
* The log keeps saying the connection could not be rebuilt while the process stays alive: that is the normal retry
  (a disconnect does not exit the process). For a config mistake such as `Missing Access` or `401` the process exits
  with code 1; with `RestartPreventExitStatus=1` in the unit it is not restarted (one exit, then `failed`), while
  `StartLimitBurst` is what stops a crash loop.

When the first `READY` arrives, one line is printed:
`gateway watchdog ready: judging window N s, probe every N s, first handshake timeout N s`. That line is what to
trust while troubleshooting, since it reflects the three values actually in effect.

## 6. Related documents

* All documents: [docs index](../../index-en_us.md).
* Why the watchdog has to be the one to decide: [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 3.
* Where probes and reconnects are recorded: [process-log-en_us.md](../process-log/process-log-en_us.md).
