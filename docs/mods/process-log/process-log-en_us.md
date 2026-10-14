# process_log: the program log
[简体中文](process-log-zh_cn.md) | **English**

* Code: `src/process_log.rs`
* Public API: `init_process_log(debug: bool)` (re-exported from the crate root; it writes the config path and the
  process id, two startup facts, itself).

## 1. Why we install a logger of our own

serenity and poise log through `tracing`, which only writes to the terminal by default, and this project also wants
a file that can be read afterwards (`journalctl` rotates). So we implement `log::Log` ourselves and depend only on
`log` and `chrono`. Once it is installed, the frameworks' records and our own `log::*` all land here (poise depends
on `tracing` with its `log` feature on, which turns framework records into `log` records - confirmed by actually
running it: serenity records show up in the program log). This project no longer prints diagnostics with
`println!`, so the terminal and the file say the same thing; the only exception is the degraded warning for when the
log itself fails (sections 4 and 5).

## 2. Two outputs

* Terminal (stderr): foreground output, or `journalctl -u minecraft-fishingbot-manager-for-discord`. For watching things
  live, and it is also around when systemd runs the program.
* File: `<deploy dir>/logs/year-month-day-hour-minute-second.log`. For reading afterwards (every line is flushed,
  so `kill -9` loses nothing).

Each record looks like
`[2026-09-28 15:30:12] [INFO] config loaded: /fishingbots/minecraft-fishingbot-manager-for-discord.conf.toml`,
plain text, no colours. The file name comes from `utils::create_timestamped_log_file` (the same naming as the bot
logs, timestamp format `%Y-%m-%d-%H-%M-%S`), and the first line is always
`--- Minecraft FishingBot Manager for Discord process starting ---`.

What gets recorded:

* This project's own events: config load, startup, shutdown, watchdog, bot directories and sessions, the details of
  start, stop, restart and pause, warnings and errors.
* Every reply (`LogText::CommandReplied`): who asked (Discord user ID), which command, and what was answered. All
  command-layer replies go out through `commands::say_and_log` and `log_reply` (see
  [commands-en_us.md](../commands/commands-en_us.md)), so "what the user says they got" can be matched line by line
  against the log.
* Third-party crates (serenity, poise) log at `WARN` and above by default (next section), because they print `INFO`
  on every heartbeat and would drown the two kinds above.

## 3. Levels

Two levels (implemented in `process_log::effective_log_levels`):

* This project (targets starting with `minecraft_fishingbot_manager_for_discord`, both lib and bin): capped at `info` with
  `[log] debug = false`, at `debug` with `true`. Startup, status and reports are enough to read at `info`.
* Third-party crates (`serenity`, `poise`, `tungstenite`, ...): capped at `warn` with `false`, at `debug` with
  `true`. They print `recv; do_heartbeat; recv_event;` on every heartbeat (more at startup) and would drown the
  lines above by default.

In one sentence: with `debug` on you also listen to the frameworks (both sides go to `DEBUG`); with it off the
frameworks only speak up when something is really wrong (`WARN`).

`RUST_LOG` wins, but only single level words are understood (`off`, `error`, `warn`, `info`, `debug`, `trace`, case
insensitive): set it and both levels use that one value, and it is the only way to see the frameworks' `TRACE`
(byte-level traffic) alone. Unset or empty falls back to `[log] debug`; a value that is not recognised (a directive
such as `poise=debug`, say) falls back to the two defaults above and adds a warning. Only one level on purpose: this
project debugs by level, not by module (per-target directives like `serenity=debug` are not supported).

## 4. What happens when it fails

* The deployment directory is not writable (the binary lives in `/usr/local/bin` or the service runs as an ordinary
  user, say): not fatal. It degrades to the terminal only and writes one warning; the bots keep running. That
  warning goes straight to the terminal whenever the global level is stricter than `WARN` (`RUST_LOG=error` or
  `off`), so the level filter cannot swallow it - "the log is not on disk" cannot be reported in the file, because
  the file is exactly what is missing.
* A logger is already installed (only possible by calling twice): the installed logger is kept and a note is
  printed.

The reason: logging is an operations problem and must not turn into game accounts going offline.

That is also why `init_process_log` does not return a `Result`: write failures after the logger is installed (a full
disk, a deleted file) have nowhere to be reported either (the `log::*` signatures are `()`); a `Result` would cover
only the half step before installing and would make `Ok(())` look like a healthy log system; and pushing the
degraded decision onto the caller would only make every call site repeat the same "write a warning, keep going".

## 5. When it is called

It must be called before the poise `Framework` is assembled and before anything else logs; the order is in
`src/main.rs` (`config::get_or_init()`, `init_process_log`, then build the `Framework`/`Client` and call `client.start()`).
When the config fails to load there is no log file yet, so those few
notes only appear in the terminal. Only three places still write without the logger: the template note in
`src/config.rs` (stdout), and the two warnings for when the log itself fails (`src/process_log.rs`: stdout when the
logger cannot be installed, stderr when the log file cannot be created - the same stream as the log, section 4).

## 6. Why there is no size limit and no cleanup

One file per run, named after the startup moment, never touched again; the process does not rotate and does not
delete old files:

* Nothing reads this log programmatically. People open it while troubleshooting, so how big one file gets and how
  many are kept is not the program's business.
* When splitting is really needed (chasing something with `[log] debug = true`, where the frameworks flood too), do
  it by hand: `split`, `logrotate`, or plain `> file`. That is simpler than building a rotation policy into the
  program.
* What piles up files is a process that systemd restarts every 10 seconds (the gateway is unreachable, the proxy
  died). That is solved at the root by a long-running process that reconnects itself (see
  [watchdog-en_us.md](../watchdog/watchdog-en_us.md)), not hidden by deleting old files.

## 7. Related documents

* All documents: [docs index](../../index-en_us.md).
* The two log lines (bot logs and the program log): [utils-en_us.md](../utils/utils-en_us.md) section 7.
* What loss detection and the reconnect leave in the log: [watchdog-en_us.md](../watchdog/watchdog-en_us.md).
