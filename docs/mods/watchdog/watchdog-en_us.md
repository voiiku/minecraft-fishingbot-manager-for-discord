# watchdog: the gateway watchdog
[简体中文](watchdog-zh_cn.md) | **English**

* Code: `src/watchdog.rs` (the `Watchdog` itself: timing fields, handshake/probe bookkeeping, the offline record and
  the process-wide singleton `WATCHDOG`) plus its submodules `gateway` (handshake/probe decisions and the probe task)
  and `lifecycle` (offline bookkeeping, the reconnect signal, waiting for an attempt to end, and process wrap-up);
  `src/main.rs` wires events and runs the reconnect loop (it obtains the singleton with `WATCHDOG.get_or_init()`;
  the event callbacks are plain function pointers and use the same entry point).
* Purpose: decide whether this process has lost contact with Discord. On failure, hand the reason to the process entry
  (`request_reconnect()`), which stops every bot and rebuilds the connection. The process does not exit: it waits one
  judging window and builds a fresh `Client` in the same process (section 4).

## 1. Why the watchdog is necessary

When Discord is unreachable we cannot control the bots, yet they still hold the game accounts: FishingBot reconnects
after a drop, the account stays online, and real players cannot get in. The bots should therefore be stopped cleanly
instead of leaving a fishing machine that nobody can command.

The watchdog uses two signals this implementation can observe: the gateway handshake and REST reachability. After a
disconnect is detected, the process stops the bots, stays running and reconnects itself rather than relying on the
process manager to restart it. This avoids a cold start and a new program log file for every retry.

## 2. Detection criteria

* Handshake: within `first_handshake_timeout` of the connection attempt starting, Discord must have sent one `READY`
  or `RESUMED` (the event handler in `main.rs` calls `watchdog.record_handshake_success()`). Otherwise a reconnect is
  requested with the reason `NeverReady`.
* Reachability: every `probe_interval`, call `Http::get_current_user()` over REST to fetch the current account. Success
  calls `watchdog.clear_probe_failures()`; failure calls `watchdog.record_probe_failure_start()`. Once a failure has
  persisted for a full `window` from the first failure, a reconnect is requested with the reason `Unreachable`.

Details:

* The timings come from the judging window `timing.watchdog_window_ms`; the probe interval (one third of the window)
  and the first-handshake timeout (twice the window) are derived from it rather than being separate keys, because those
  values only make sense as ratios of the window. The entry point stores all three in the `Watchdog` (its three fields),
  and both the decision and the probe use the same copy. The derivation guarantees that the probe interval is always
  shorter than the judging window, so a configuration where the probe is slower than the window and the first failure
  already counts as a disconnect is impossible.
* The probe uses REST: a 2xx from `GET /api/v10/users/@me` proves only that the REST endpoint is reachable and the
  current token is usable; it does not prove the gateway is reachable. Timeouts, connection failures and HTTP 401/5xx
  errors all count as a failed probe. The probe goes to `discord.com`, while the gateway connects to
  `gateway.discord.gg`; if a deployment ever has a working API but an unreachable gateway, consider adding a TCP or TLS
  probe of the gateway host.
* The probe task starts before `client.start()`: when the gateway never connects, `READY` never arrives and detection
  must work outside the client. Starting it early also lets "no network at startup" end the attempt after the
  first-handshake timeout.
* The decision is reversible: a recovered probe, or another `READY`/`RESUMED`, returns the watchdog to `Alive`; one bad
  verdict is not permanent.
* The decision depends only on time and recorded state: four states `Alive`, `Waiting`, `NeverReady`, `Unreachable`
  (internal to the watchdog and not exposed) with the moment passed in by the caller (`watchdog.state(now)`), so every
  rule has a unit test, including "only the first failure counts" and "recovery revives".

## 3. Deliberately not reading framework state

serenity 0.12.5 exposes only actions such as `shutdown_clean` and `set_presence` through `ShardMessenger` (that is,
`ctx.shard`); there is no `stage()` or `latency()`, and `Shard` is not reachable from inside the shard runner either.
The half-dead gateway state (connected but receiving nothing) is found and reconnected by serenity's own heartbeat and
ACK machinery (the protocol guarantees it); we neither need to nor can reimplement it, so the watchdog covers only the
two "cannot reach Discord at all" cases above.

## 4. After a disconnect: stop the bots, wait one window, rebuild the connection

The decision lives in `gateway.rs`, `lifecycle.rs` provides the wrap-up and wait helpers, and the reconnect loop lives
in `src/main.rs`. The division is: the judging side only decides and wakes, while every visible action (stopping,
recording, logging, exiting) happens on the entry-point side. The entry point uses `wait_for_attempt()` (wait for the
current attempt to end and clean up a disconnect), `wait_before_reconnect()` (wait out one judging window) and
`exit_on_shutdown_signal()` (assemble the same details as `/stopall`, write them to the program log, send them to every
server's notice channel under the title "shutdown signal received, wrap-up", and exit with code 0).

1. The judging side: after a disconnect is detected, `request_reconnect(reason)` stores the reason on the watchdog and
   wakes the process entry (`tokio::Notify`); it does not touch the bots itself.
2. The entry-point cleanup (the "watchdog judged a disconnect" arm of `wait_for_attempt()`): it takes the reason handed
   over by the judging side and calls the private `stop_all_bots_on_disconnect(reason)`, which stops every bot
   (`shutdown_all_bots()`, the same flow as `/stopall`), writes a WARN and records the reason and the moment. If there
   is no reason to take, the wake-up was late and this round has already cleaned up, so nothing is touched. Stopping
   is mandatory: with Discord unreachable nobody can command the bots, while FishingBot's automatic reconnect keeps
   holding the game accounts and competing with real players for login slots. Nothing recovers by itself while
   offline; after a successful reconnect, a person sends `/startall` or `/start <name>` in Discord.
3. It then closes the old connection (`shard_manager.shutdown_all()`, otherwise the old and new connections fight over
   the same bot session and Discord kicks them in turn), calls `wait_before_reconnect()` for one judging window
   (reusing `timing.watchdog_window_ms`, with no new config field), and builds a fresh `Client`; if that still fails,
   it keeps retrying the same way.

The client returning on its own (`client.start()` returning: shut down, or a fatal error such as an invalid token or
Intent - a plain disconnect never makes it return) also goes through `wait_for_attempt()`: the same
`stop_all_bots_on_disconnect()`, except that the reason is written by the entry point and there is no need to wake
itself, so both paths clean up and report recovery identically.

Two key constraints:

* Every attempt must call `watchdog.begin_connection_attempt()`, so the first-handshake timeout starts counting from
  the current attempt. Without the reset, a reconnect is judged `NeverReady` immediately from the previous attempt's
  state, producing a loop that reconnects once per window but never connects.
* The probe task must be stoppable (`stop` is an `AtomicBool` per round, checked before the probe and again after the
  request returns). The previous round's probe must exit; otherwise two tasks probe, record failures and trigger a
  disconnect together. A flag is used instead of a notification because a flag is state and cannot be missed, whereas
  an event such as `notify_waiters()` is lost when nobody is waiting.

Only errors that retrying cannot fix exit the process (`exit_on_permanent_failure`, exit code 1; the cause is logged at
`ERROR`, which distinguishes it from retryable network flakiness at `WARN`): an invalid token, a bot that was never
invited to a server (`50001 Missing Access`), a rejected setup, and similar configuration mistakes. Retrying such an
error would fail the same way. Exit code 1 is this project's deliberate-failure code (a failed config load or
validation also exits `main` with 1), so the unit file should set `RestartPreventExitStatus=1` (see
[deployment tutorial section 12](../../tutorial/tutorial-en_us.md#12-configure-a-systemd-service-optional-recommended)):
such an error stops after one exit and goes to `failed` until someone handles it. A crash uses a different code
(panic 101, killed by OOM or `kill` 137, ...) and is restarted by `Restart=on-failure` as usual.

### The notice after a successful reconnect

When `Ready` or `Resume` arrives again, `notify_reconnect_success()` (the entry point passes it the
`serenity::Context`) sends one message to every server the bot belongs to: "this bot lost contact with Discord at
`<time>` and is back online now (offline for `<N>`s). Every game bot was stopped while offline; send `/startall` to
resume them." The message goes to the server's system channel, or to its first text channel when no system channel
exists. Each server receives at most one message; a failure for one server only leaves a warning and does not affect
the others. The same notice is written to the program log (`ReconnectedAfterOffline`: how many servers were notified
and how many failed).

### The wrap-up when an exit signal arrives

The details assembled by `exit_on_shutdown_signal()` match `/stopall`: they are written to the program log first, then
sent to every server by the same channel rule (the system channel, otherwise the first text channel), logged as
`ShutdownReportNotified` with the number of servers notified and failed. When the connection never handshaked, the
report is only logged under `ShutdownReportNoContext`. The immediate disconnect and permanent-failure wrap-ups send
nothing: the connection is already unusable, so a send could not succeed.

The send has a 30-second overall limit (`SHUTDOWN_REPORT_TIMEOUT`): sends still outstanding at the deadline are
cancelled (logged as `ShutdownReportTimeout`) and the process exits as usual. Messages already delivered are
unaffected. The report is not persisted separately: apart from being written to this run's program log and sent once,
no pending copy is kept and the next startup does not resend it.

## 5. Common problems

* The log keeps showing `NeverReady` reconnects (the process does not exit): outbound traffic may be blocked, the token
  may be wrong, or Discord itself may be having trouble. Check REST reachability from the deployment machine first:
  `curl -sS -o /dev/null -w '%{http_code}\n' https://discord.com/api/v10/gateway` should return 200.
* The log shows `Unreachable`: the network may be down, the proxy may have died, or the token may have expired; the
  previous line, `REST probe failed:`, gives the concrete reason.
* A brief network fluctuation causes one reconnect: raise `timing.watchdog_window_ms` (for example, 60000). The probe
  interval and first-handshake timeout follow it, and recovery inside the window is not mistaken for a disconnect.
* The log keeps reporting that the connection could not be rebuilt while the process stays alive: this is normal retry
  behaviour (a disconnect does not exit the process). For a configuration error such as `Missing Access` or `401`, the
  process exits with code 1; with `RestartPreventExitStatus=1` in the unit it is not restarted (one exit, then
  `failed`), while `StartLimitBurst` limits a crash loop.

When the first `READY` arrives, one line is printed:
`gateway watchdog ready: judging window N s, probe every N s, first handshake timeout N s`. Trust this line while
troubleshooting, because it reflects the three values actually in effect.

## 6. Related documents

* All documents: [docs index](../../index-en_us.md).
* Why the watchdog has to be the one to decide: [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 3.
* Where probes and reconnects are recorded: [process-log-en_us.md](../process-log/process-log-en_us.md).
