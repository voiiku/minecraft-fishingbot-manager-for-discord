# process_log: the program log
[简体中文](process-log-zh_cn.md) | **English**

* Code: `src/process_log.rs` (`process_log/levels.rs` decides levels and `process_log/format.rs` renders records;
  both are pure functions with their own tests)
* Public API: `ProcessLog::get_or_init()` (`ProcessLog` is re-exported from the crate root). The first call tries to
  create this run's log file, installs the logger, and records the config path and process ID (plus the log path when
  the file is available).

## 1. Why we install a logger ourselves

serenity and poise log through `tracing`, which only writes to the terminal by default; this project also needs a file
that can be read afterwards (`journalctl` rotates). We therefore implement `log::Log` directly and depend only on `log`
and `chrono`. Once installed, the frameworks' records and this project's own `log::*` calls all land here (poise enables
the `log` feature of `tracing`, so framework records are converted into `log` records; this has been confirmed at
runtime, where serenity records appear in the program log). This project no longer prints diagnostics with `println!`,
so the terminal and the file contain the same messages; the only exceptions occur during the earliest startup steps,
as described in sections 4 and 5.

## 2. Output and format

* Terminal (stderr): foreground output, or `journalctl -u minecraft-fishingbot-manager-for-discord`. Used for live
  observation and still available when systemd runs the process.
* File: `<deploy dir>/logs/year-month-day-hour-minute-second.log`. Used for reading afterwards; every line is flushed
  immediately, so `kill -9` does not lose what has already been written.

Each record looks like
`[2026-09-28 15:30:12] [INFO] config loaded: /fishingbots/minecraft-fishingbot-manager-for-discord.conf.toml`,
in plain text without colour. The file name comes from `utils::create_timestamped_log_file` (the same naming as bot
logs, with the timestamp format `%Y-%m-%d-%H-%M-%S`); the first line describes this startup and is localised according
to `[i18n] language`.

What gets recorded:

* This project's own events: config load, startup, shutdown, watchdog, bot directories and sessions, the details of
  start/stop/restart/pause, warnings and errors.
* Every reply (`LogText::CommandReplied`): who asked (Discord user ID), which command, and what was answered. All
  command-layer replies go through `commands::say_and_log` and `log_reply` (see
  [commands-en_us.md](../commands/commands-en_us.md)), so what a user reports receiving can be checked line by line
  against the log.
* Third-party crates (serenity, poise) log only at `WARN` and above by default (next section), because they emit
  `INFO` on every heartbeat and would drown the two categories above.

## 3. Levels

`LogLevels::from_config` in `process_log/levels.rs` derives two ceilings from `[log] debug` alone:

* This project (targets starting with `minecraft_fishingbot_manager_for_discord`, both lib and bin): `INFO` with
  `false`, `DEBUG` with `true`. Startup, status and report messages are already readable at `INFO`.
* Third-party crates (`serenity`, `poise`, `tungstenite`, ...): `WARN` with `false`, `DEBUG` with `true`. They emit
  `recv; do_heartbeat; recv_event;` on every heartbeat (and more at startup), which would drown the key lines above
  under the default setting.

The global `max_level` is the wider of the two ceilings, and the target prefix then selects the ceiling for each record.
With `debug` enabled both sides reach `DEBUG`; with it disabled the frameworks only speak up at `WARN` when something
is actually wrong.

There is no environment variable and no second switch (this project supported `RUST_LOG` early on; that is gone): the
level comes only from this config line. Neither `TRACE` (byte-level traffic) nor per-module debugging (directives such
as `serenity=debug`) is supported; this project debugs by level, not by module.

## 4. Failure handling

* The deployment directory is not writable (for example, the binary lives in `/usr/local/bin` and the service runs as
  an ordinary user): this does not affect operation. Logging degrades to the terminal and one warning is recorded. The
  warning goes through the logger (so it reaches stderr with the `[time] [WARN]` prefix); the global ceiling is never
  stricter than `WARN` in either configuration, so it cannot be filtered out. The fact that the log is not on disk
  cannot be reported only in the file, because the file is exactly what is missing.
* A logger is already installed: `ProcessLog::get_or_init` checks first; when one is installed it keeps that logger,
  prints a note and returns without creating another file. The file is created before the logger is installed, so
  checking first is what keeps a stray first-line-only file out of `logs/`.

Logging is an operations concern and must not prevent game accounts from coming online.

That is why `ProcessLog::get_or_init` does not return a `Result`: write failures after installation (a full disk, a
deleted file) have nowhere to be reported either (the `log::*` signatures are `()`); a `Result` would cover only the
step before installation and would make `Ok(())` look like a healthy log system. Pushing the degraded decision onto the
caller would only make every call site repeat "write a warning, keep running".

## 5. When it is called

It must be called before the poise `Framework` and `Client` are assembled and before anything logs. The order is in
`src/main.rs`: `config::Config::get_or_init()`, `ProcessLog::get_or_init`, then build the `Framework`/`Client` and call
`client.start()`. It reads `[log] debug` from the config itself, which is why the entry point passes no argument.

When config loading fails, the process has not created a log yet, so the related messages only appear in the terminal.
Only a few direct writes remain from the earliest startup: the template/config-path notes from `src/config.rs` (stdout)
and config errors (stderr), plus the two notes from `src/process_log.rs` (logger installation failure and a repeated
initialization keeping the existing logger, both stdout).

## 6. File retention

One file per run, named after the startup moment and never modified afterwards; the process neither rotates nor deletes
old files:

* Nothing reads this log programmatically. People open it while troubleshooting, so how large one file gets and how
  many are kept is not the program's responsibility.
* When splitting is genuinely needed (for example, `[log] debug = true` with heavy framework output), do it by hand:
  `split`, `logrotate`, or plain `> file`. That is simpler than building a rotation policy into the program.
* Files pile up when systemd restarts the process every 10 seconds (the gateway is unreachable, the proxy died). The
  root fix is a long-running process that reconnects itself (see [watchdog-en_us.md](../watchdog/watchdog-en_us.md)),
  not deleting old files to hide the problem.

## 7. Related documents

* All documents: [docs index](../../index-en_us.md).
* The two log lines (bot logs and the program log): [utils-en_us.md](../utils/utils-en_us.md) section 7.
* What loss detection and the reconnect leave in the log: [watchdog-en_us.md](../watchdog/watchdog-en_us.md).
