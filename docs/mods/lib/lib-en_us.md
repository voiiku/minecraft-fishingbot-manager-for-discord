# Library API and command boundaries
[简体中文](lib-zh_cn.md) | **English**

## 1. Naming

`operations` exposes named operations independent of Discord. `/start` calls `start_single_bot(name, caller_id)` directly; `/restartall` calls `restart_all_bots(caller_id)`. There is no action enum or central dispatcher.

## 2. Error handling conventions

Text commands return localized results; log and configuration attachments and login prompts have typed results. `discord_commands` owns deferral, blocking tasks, ephemeral replies and attachments, not bot operations.

## 3. Permission rules

Single-bot control, log access, editing an account and starting a login require ownership or administrator access. Unauthorized access and a missing bot give the same response. Batch control, anti-AFK and command sync require an administrator. New bots belong to the caller; public queries do not require administrator access.

## 4. Process-level lifecycle

Connection lifecycle methods live on `Watchdog` in `src/runtime/watchdog/lifecycle.rs`; bot stopping lives in `src/runtime/bot_instances/stop.rs` and manager exit in `src/runtime/shutdown.rs`:

* `Watchdog::get_or_init()`: fetch the process-wide watchdog (the first call builds it from the config `[timing]`;
  later calls return the same instance; the entry point and the event callbacks both use it). Call it after
  `config::Config::load()`, because the first call reads the config. `Watchdog::new(...)` takes explicit
  timings and is for tests only.
* `runtime::bot_instances::stop::shutdown_all_bots()`: stop every running bot session (without exiting the process), returning errors explicitly.
* `runtime::shutdown::exit_on_shutdown_signal(notice_ctx)`: the cleanup after SIGTERM / Ctrl+C (the entry point calls it when a signal
  arrives): log it, stop every bot, send the wrap-up report to every server's notice channel (when this connection
  never completed a handshake, only the log is written), and exit with code 0 (systemd will not restart it).
* `runtime::shutdown::exit_on_permanent_failure(reason)`: with configuration loaded, report the error and stop bots before
  exiting with code 1; without configuration, report to the terminal and exit without attempting bot cleanup. Terminal output is also used when logging is unavailable. The example
  systemd service uses `KillMode=control-group` to clean up remaining processes and `RestartPreventExitStatus=1` to avoid restarting.
* `watchdog.request_reconnect(reason)`: whoever judged the disconnect (the watchdog's decision side, or the entry
  point's setup-error path) hands the reason over and wakes the process entry; the entry point then performs the
  cleanup and rebuilds the connection, without exiting the process (see
  [watchdog-en_us.md section 4](../watchdog/watchdog-en_us.md)).
* `watchdog.wait_for_attempt(client)`: the entry point uses it to wait for this attempt to end and to clean up
  accordingly: `client.start()` returning calls the private `stop_all_bots_on_disconnect()` (stop every bot and record
  the reason and time); a watchdog disconnect hands its reason over and cleans up the same way; a shutdown signal exits
  the process.
* `watchdog.wait_before_reconnect()`: wait one judging window before rebuilding the connection; a shutdown signal
  during that wait exits the process.
* `watchdog.remember_notice_context(ctx)`: the entry point records the current context on every successful handshake
  (`Ready` / `Resume`); the exit-signal wrap-up uses it to send the channel message (a disconnect wrap-up sends no
  channel message, so it does not need it).
* `watchdog.notify_reconnect_success(ctx)`: after an outage, when `Ready` / `Resume` arrives again, send one notice to
  every server this bot is in; does nothing when there was no outage.

`main.rs` classifies setup failures and chooses the corresponding localized log message. Missing access or an invalid
token is permanent and calls `exit_on_permanent_failure`; other errors can be retried.

## 5. Related documents

* How commands are registered, parsed and dispatched: [commands-en_us.md](../commands/commands-en_us.md) (the
  command layer) and [architecture-en_us.md](../architecture/architecture-en_us.md) section 3 (the full path of one
  command).
* Every command: [commands-en_us.md](../commands/commands-en_us.md), sections 3 to 7.
