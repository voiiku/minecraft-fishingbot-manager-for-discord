# utils: the shared utility layer
[简体中文](utils-zh_cn.md) | **English**

* Code: `src/utils.rs` (module declarations and re-exports only) and `src/utils/*.rs` (one file per topic)
* The naming convention is action plus object (`raw_bot_name.to_clean_bot_name()`, `reply_text.utf16_len()`,
  `is_bot_name_taken`, `read_last_lines`); where a `Result<_, String>` is returned, the `Err` is always text that can
  be shown to the user as it is.
* The `pub use` list at the top of `utils.rs` is the public API and the only surface the business layer may use; every
  other `pub` item in a submodule serves utils internally (or only the flows). `bot_status`, for example, is mostly an
  implementation detail (`running_bots` uses it to decide a status) and exposes only two things to the business layer:
  `PauseCommand` and `confirm_pause_command` (`BotPauseStatus` only serves utils internally and does not have to be
  exported).
* utils returns raw information and never assembles the final report: the flow functions return details
  (`StopOutcome` for stopping, `LaunchOutcome` for starting, `RestartOutcome` for restarting) which carry methods such
  as `into_report()` and `to_failure_text()`, but the caller has to call them explicitly. utils never hands back an
  assembled `Report`, see [lib-en_us.md](../lib/lib-en_us.md).

## 1. Module map

* `bot_name`: normalising and validating bot names (an illegal name is an error, not filtered out). Entry point
  `ToCleanBotName::to_clean_bot_name` (a `str` extension trait, so both `&str` and `String` can call it directly).
* `bot_command`: how an in-game command is assembled (`/command` and `/commandall` share it, written once). Entry
  point `ToCleanBotCommand::to_clean_bot_command` (an extension method on `[&str]`; an empty command is an error).
* `bot_dirs`: locating, walking and creating directories, the token status, and "which batch of directories to take".
  Entry points `BotDirEntry` (built with `BotDirEntry::new` from a raw name and the owning Discord user ID, or with
  `BotDirEntry::find_by_name` from a name only, which checks no permission), `all_bot_dir_entries`,
  `token_ready_bot_dir_entries`, `bot_dir_entries_of_owner`, `is_bot_name_taken`,
  `split_bot_entries_by_token_status`, `create_bot_dir` (returns the entry it created), `BotTokenStatus` (decided by
  `BotTokenStatus::from_dir`) and `token_file_path` (the token file path convention).
* `screen`: the `screen` session snapshot, probing and command injection (injection does not validate anything; the
  waits and the force-kill decision live in `shutdown`). Entry points `active_screen_sessions` (session name ->
  `ScreenSession`, with the PID and whether it is a `(Dead ...)` shell, which still counts as running so that `/stop`
  can clean it up), `is_bot_screen_running`, `session_name`, `send_command_to_single_bot`,
  `send_stop_signal_to_session` and `force_quit_session`.
* `running_bots`: the single source of the bot state (whether it is started comes from the session, whether it is
  queued comes from the queue). Grouping by run state, the names of the running ones, the `/show status` rows and the
  refusal text for running or queued bots. Entry points `split_bot_entries_by_is_running`, `all_running_bot_names`,
  `all_running_bot_status_rows` and `refusal_if_not_stopped`.
* `startup`: starting. Launching one bot is "check the token -> create the log -> prepare the raw log
  directory -> `screen -dmS`"; targets without a token are not launched. Batch starting is orchestrated by
  `StartupQueue::start_bots()` (create the ledger -> `add_bots_to_queue` hands it over -> wait for its own entries -> take
  the results and re-check them once -> `BotStartResults`, which carries `into_report()` and `to_reply_text()`), shared by
  `/start`, `/startall`, `/restart` and `/restartall`; `/bot login` uses `StartupQueue::add_bot_for_login` directly
  (it does not enter the queue and is not verified); the launch steps themselves (`launch_bot_session`,
  `launch_command_line`) are internal to the module. The command layer reads the queue through `pending_bot_count()`
  (the number of queued bots), `slowest_start_duration(bot_count)` (the pacing-based estimate the first `/startall`
  receipt uses) and `result_timeout()` (the fallback timeout). The first step of "stop every bot" is
  `StartupQueue::request_stop()`: the starter thread takes no new batch, and the batch it is working through records its
  remaining entries as `BotStartOutcome::Cancelled` (not a failure; they go into the skipped details), so whoever waits
  on a ledger (`/start`, `/startall`) never hangs. Queued entries also have a fallback: if an entry leaves scope without
  anyone writing a result (the waiting list is cleared, it is pulled out, the starter thread panics and unwinds),
  `QueuedBot::drop` records a cancellation. `wait_until_done()` has another fallback: a timeout the queue derives from its
  pacing (`StartupQueue::result_timeout`, twice the slowest start of a full queue), which closes whatever has not
  reported back as unconfirmed. It needs no configuration and cannot fall out of step with the pacing values.
* `restart`: restarting. The "stop then start" orchestration shared by `/restart` and `/restartall` (only the raw
  details `RestartOutcome` are returned, carrying both the stop and the launch details). Entry point
  `restart_bot_sessions`.
* `shutdown`: stopping sessions (only the raw details `StopOutcome` are returned, which carries `into_report()` and
  `to_failure_text()`). Entry points `stop_single_bot_session` and `stop_all_bot_sessions`; the latter means "stop
  everything", in a fixed order: "ask the starter thread to launch nothing new -> clear the waiting list -> stop every
  session -> sweep the sessions once more". That last sweep catches a session that only appeared after the first
  snapshot (a bot the starter thread was in the middle of launching cannot be held back, only cleaned up afterwards).
  (`/stopall`, the watchdog's disconnect wrap-up and the exit wrap-up all go through it.)
* `bot_logs`: creating and picking bot logs (FishingBot's raw log is handled in `startup`). Entry points
  `create_bot_log_file`, `create_timestamped_log_file`, `target_logs_dir` and `latest_log_path`.
* `fishingbot_config`: generating the game-side config (FishingBot's `conf.json`): delete the old file, then use the
  template shipped with the repository (`fishingbot-config.example.json`) as the base and override the corresponding
  `[fishingbot]` keys (`misc.language` is written directly from `[i18n] language`, whose value is the JAR's constant
  name). The output is a complete config and never relies on the JAR filling fields in. Entry point
  `regenerate_fishingbot_config` (called once at process startup; it only reads the config, while the part that actually
  writes - `write_fishingbot_config`, with the path and values passed in - is unit-tested in a temp directory; see
  [config-en_us.md](../config/config-en_us.md) section 3.2).
* `fs`: file and directory primitives; they only perform the operation itself (read lines from the tail, list
  subdirectory names, remove a directory that may not exist). Which file to touch and how to report a failure belong to
  the caller. Entry points `read_lines_backwards_until`, `read_last_lines`, `list_subdirectory_names` and
  `remove_dir_all_if_exists`.
* `bot_status`: bot status. The pause status is stored inside the process (it changes only when the command echo is
  confirmed) and the online status is read fresh on every call; the `/pause` and `/unpause` confirmation echoes live
  here too. The entry points are `BotDirEntry::connection_status()` (read fresh each time; all three unreadable cases are
  errors; it only reports "online / reconnecting / initializing"), `ConnectionStatus` (four states, `from_line` deciding
  a single line; the `Stopped` "process finished" state comes from `running_bots` based on the session shell, not from
  the log), `BotPauseStatusMemory` (stored per bot name: `to_bot_pause_status`, `update_bot_pause_status`,
  `forget_bot_pause_status`), `BotPauseStatus` (`pause_status`, `set_paused`, `set_fishing`), `PauseCommand` (`SetPause`
  and `SetUnpause`, each with the command name, the echo keyword `echo_keyword` - taken from the config's
  `[command_echo]` - and the state to set) and `confirm_pause_command`.
* `uptime`: uptime. Reads the seconds the process has been running and formats them. Entry points
  `process_elapsed_seconds` and `format_uptime`.
* `reply`: assembling and rendering replies, including the report container and the length cap. Entry points
  `Report::new` (the only constructor; starts empty), `set_title`, `set_counts`, `render` and `IntoReplyText`.
* `permission`: the only way a permission is decided. Every command declares one requirement; the entry point is
  `has_permission(caller_id, RequiredPermission)`, where the requirement is `Everyone`, `MustBeAdmin` or
  `OwnerOrAdmin(owner user ID)`.

## 2. `bot_dirs`: token status and directory scopes

* `BotTokenStatus::Ready` and `BotTokenStatus::Missing`: they only say whether the token file has contents, and
  validate nothing online, because only FishingBot itself knows whether a token is valid when it starts.
* `BotDirEntry` is the struct of a directory entry: the owning Discord user ID, the bot name, the folder path, and
  the token status read when it was built. The path is not passed in (it is assembled from
  `bots_dir/Discord user ID/name`), and the two constructors say where the ownership comes from:
  `BotDirEntry::new(raw name, owning Discord user ID)` and `BotDirEntry::find_by_name(raw name)` (looks the entry up
  by name across every user directory and checks no permission - the routing layer checks that, see
  [lib-en_us.md](../lib/lib-en_us.md)). The name is validated once inside the constructor (the single source of the
  rule is still `ToCleanBotName::to_clean_bot_name`) and an illegal name is an `Err` right there; the third, internal
  constructor `from_dir_name` (used for the on-disk listing) therefore skips directories with illegal names that an
  administrator created by hand, so they never show up in a list or a report.
* Which directories to take is not modelled as a filter enum; instead, each scope has its own function (the name states
  the scope, so call sites do not have to look up an enum value):
  * `all_bot_dir_entries()`: every user directory (`/startall`, `/restartall`, `/stopall`, the process shutdown,
    `/show allbots`); a root directory that does not exist yet counts as an empty list, only a real read failure is
    an error;
  * `token_ready_bot_dir_entries()`: only the ones with a configured token (`/commandall`, `/show status`);
  * `bot_dir_entries_of_owner(user ID)`: only the ones under one Discord user ID (`/show mybots`).

  Upper layers no longer write their own `read_dir` loop (the loop itself lives in `fs::list_subdirectory_names`), so
  two modules cannot filter differently by accident.

## 3. `running_bots`: the single source of run state

`screen -list` is called once and cached as a snapshot; every later "is it running" decision is based on that same
snapshot. The join between a directory and a session happens only in this module, so nothing else walks directories to
match session names:

* The rows of the `/show status` table: `all_running_bot_status_rows()` lives in this module and joins the four cells of
  each bot into one line on the spot (`status_row`, a `String` discarded right after rendering, with no snapshot type);
  the header is built by `show/status.rs` itself, so the two sides must agree on the column order (see
  [commands-en_us.md](../commands/commands-en_us.md) section 6.2).
* `split_bot_entries_by_is_running(&[BotDirEntry])` returns the running and the non-running group; which batch of
  directories is taken is decided by the caller (`all_bot_dir_entries()` or `token_ready_bot_dir_entries()`), see
  [commands-en_us.md](../commands/commands-en_us.md) section 5.2.
* `all_running_bot_names()` returns the names of every running bot (used by `/commandall`): the scope matches
  `all_running_bot_status_rows()` (every bot with a configured token), it calls `screen -list` once and reads no logs.

One snapshot reused in many places is deliberate: if a batch flow probed again every time, a session started a moment
ago would immediately count as already running and the results would become inconsistent.

## 4. Waits during startup and shutdown

* All launch-capable commands (`/start`, `/startall`, `/restart` and `/restartall`) share one process-wide FIFO start queue. Its dispatcher remains alive and blocks while idle. Each command request is split into batches of at most `[startup_queue] max_batch_size`; the next FIFO request is not processed until every entry of the current request has been popped. Adjacent bots in a batch are separated by at least `[startup_queue] bot_login_interval_ms`. Batches are separated by at least `[startup_queue] batch_login_interval_ms`, counted from when the previous batch was popped (the verification runs in parallel on the command side and does not count as this wait). A partial batch is closed when the queue becomes idle, but the "last pop" timestamp is kept across commands, so consecutive commands cannot bypass the rate limit. Queued requests are not persisted and are lost if the process restarts. `/bot login` does not use this queue.
* `StartupQueue::start_bots()` is the only command-side entry (create the ledger → enqueue → wait until every entry has been launched → take the results and verify); the starter thread handles per-batch staggering and launches, and the verification observes session survival with one wait per start request (check immediately after launch, then once more after the grace period). The sign-in rate is the limited resource, controlled by `bot_login_interval_ms` inside a batch and `batch_login_interval_ms` between batches; the batch size only affects the JVM memory peak during launch. See [commands-en_us.md](../commands/commands-en_us.md) section 5.5.
* Neither stop wait takes a parameter; the flow reads the config and performs both waits itself: send the exit signal,
  wait `stop_grace_ms`, force kill if the session is still there, then wait `stop_settle_ms` so the old session can
  deregister completely before the final check (checking immediately would still see it).
* Every re-check goes through `startup::verify_batch`, so "launched successfully" means the session is still there after the second check (a `screen -list` snapshot), not that the command was sent; and a session being there does not mean the bot is online either (logging in and joining the server are up to the JVM), which is why the report and the reply only say "the start command was sent and the session has appeared".

## 5. `reply`: the reply format is written in one place

* Fields are sections: `basics` (basic information and list rows, appended directly), `succeeded`, `skipped`,
  `queued`, `failed` (details) and `content` (a whole block of content). `render()` omits whatever was not filled in, so
  an empty section never leaves an empty title behind.
* `set_counts(&[(label, count)])` overwrites the basic information rows and generates `label: N` uniformly; non-count
  rows (a header, a list of names, an explanation) are appended directly to `basics`.
* `new()` and `set_title()`: there is only this one way to construct a report (it starts empty) and the title
  is always added by `set_title()` (some titles are only known once the flow has run).
* The success/failure decision and the one-line error text are not here: they are methods on the details utils
  returns (`has_failed_lines()` and `to_failure_text()` of `LaunchOutcome`, `RestartOutcome` and `StopOutcome`).
* `render()` truncates by `runtime.max_reply_chars`: when the text is too long, lines are removed from the end and the
  number of omitted lines is noted at the end. Discord has a hard limit per message (2000 characters) and an overlong
  message simply fails to send, which looks to the user as if nothing happened; truncating by line guarantees that the
  title and the count rows survive. The length is measured with `str::utf16_len()` (UTF-16 code units; `.len()` is
  bytes and `chars()` is code points, and neither is correct).

For a command whose counts are only known once the flow has run, the flow runs first and `set_counts` then overwrites
the rows as a whole, rather than letting the flow append, which would leave both the placeholder figures and the real
ones in the same report.

## 6. How config values are read

There is no function in `utils` that receives the whole config and selects fields from it; every function reads only
the one or two fields it actually uses, in two styles:

* Functions that take parameters (the caller supplies the values; the function does not read the config): used where
  the logic is complex enough to justify a unit test, for example `launch_command_line` (assembling the launch command)
  and `target_logs_dir` (pure string and path assembly).
* Functions that read only the fields they need (`config::Config::get().table.field`): used for convenience functions
  with a line or two of logic. `session_name` reads `runtime.screen_prefix`; `create_bot_log_file` and
  `latest_log_path` read `runtime.bot_logs_dir`; `BotTokenStatus::from_dir` reads `runtime.token_file`;
  `BotDirEntry::find_by_name` reads `paths.bots_dir`; `stop_bot_sessions` reads `timing.stop_grace_ms` and
  `timing.stop_settle_ms`; `Report::render` reads `runtime.max_reply_chars`.

The config is read once at startup and then lives in memory (see [config-en_us.md](../config/config-en_us.md) section
1), so a convenience function reading one field costs nothing extra; functions that take parameters, in turn, gain
unit-test coverage for the places that are easy to get wrong, such as the launch command and the waits (see the tests
in `utils::bot_dirs`, `utils::startup` and `utils::reply`).

One exception is the status decision `BotDirEntry::connection_status()`: its logic is complex, but it reads the
keywords itself, half from the config's `[keywords]` (the two entries that differ per deployment and must be filled in
by hand) and half from the `jar_*` methods in the language table `text` (the reconnect message the JAR prints, taken
according to `[i18n] language`). Both are one global, fixed copy, so there is no need to pass them through every
caller; they are exposed on the entry instead (picking the newest log happens inside the method, so the caller does not
have to assemble that either). The cost is that keywords cannot be injected in a unit test, so the tests cover the
parameter-taking implementation (`connection_status_of_log(bot_name, log_path)`, with the single-line decision in
`ConnectionStatus::from_line`), while the "read backwards, stop at the first conclusion" part is covered by the tests
of `fs` (`read_lines_backwards_until`). It does not produce "process finished": that status looks at the session shell
and is supplied by `running_bots` (see `ScreenSession` in [`super::screen`]).

The same trade-off applies to the reply truncation (inside `Report::render`): it reads `runtime.max_reply_chars` itself,
since the limit can only come from the config; that truncation logic therefore has no unit test (covering it would mean
turning the limit into a parameter, one more layer).

`bot_status` stores one piece of in-process state (`BotPauseStatus`: stored per bot name, with
`update_bot_pause_status` as the only writer). Only the pause status is kept in the struct; the online status is not
stored:

* The pause status is the result of the `/pause` and `/unpause` commands this project sends, stored inside the process
  (initially "fishing") and changed only once the JAR's echo is confirmed (`BotPauseStatus::set_paused` and
  `set_fishing`); reading it (`pause_status`) reads that state. When a `/pause` and an `/unpause` for the same bot
  arrive almost at the same time, the status follows the last one in the echo window
  (`PauseCommand::last_confirmed_pause_command`), not whichever write comes last; whether the command just issued took
  effect is judged the same way: when the newest echo belongs to the other command, the caller replies with a failure
  and reports the current state.
* The online status can only be confirmed from the log (only the JVM knows whether it is connected or dropped) and a
  stored copy would be stale immediately, so it is not kept across calls: `bot_dir_entry.connection_status()` reads it
  fresh on every call and the caller always gets the newest one (the cell inside `/show status` is a string for that
  rendering only and is discarded afterwards).

That way `/show status` (a high-frequency command) does not have to scan a log back to the beginning of the file to find a
pause keyword for a session that never showed one. Both the start and the end of a session clear this state
(`forget_bot_pause_status`), so it always describes the current session only.

## 7. The two logging streams

### 7.1 Bot logs: only one file

`<bot directory>/<runtime.bot_logs_dir>/` contains two kinds of files:

* `year-month-day-hour-minute-second.log`: our log. `create_bot_log_file` creates it before the session is launched,
  and the session appends the JVM output through `grep -v -E` and `tee -a` (lines matching `keywords.filter_regex` are
  dropped). `/log` reads its tail and `/show status` scans it from the end to decide a status.
* `file_name` under `[raw_game_logs] dir_name` (default `raw/log0.log`): FishingBot's log, a symlink to `discard_path`
  (default `/dev/null`), so whatever is written into it is thrown away by the kernel (the whole directory is emptied
  and recreated before every launch, so only the filtered copy stays on disk). Both the name and the discard target
  are configured in `[raw_game_logs]`.

Why `/show status` scans backwards (`fs::read_lines_backwards_until` with `BotDirEntry::connection_status`): the connection
status depends only on the last occurrence of a keyword, so it reads back from the newest line and stops at the first
conclusion. A long-running account's log can be tens of MB, and reading it all in is slow and uses memory. The pause
status does not scan the log; it is stored in-process (previous section), which avoids walking all the way back to the
beginning of the file when no pause keyword exists (measured: 72 ms for a 14 MiB log, per bot, per call). `read_last_lines`,
which `/log` uses, is a thin wrapper over the same implementation (stop once N lines are counted), so it also reads only
the tail of the file and never streams the whole log just to show the last 100 lines. Both reading styles are general
file primitives and live in `utils::fs`.

Why FishingBot still gets a file: it refuses to start when it cannot create a log file (in `Bot.java`, a `FileHandler`
throwing `IOException` leads to `setRunning(false)`), so it has to be given a writable target that stores nothing. As a
result there is only one real log on disk, and two copies can no longer interfere with each other.

The rule for picking the log file lives in exactly one place, `bot_logs::latest_log_path`, in two steps: accept only
timestamp-shaped names (`is_bot_log_file`, whose names come from `create_timestamped_log_file`), then take the largest by
name. The name is a fixed-width, zero-padded `year-month-day-hour-minute-second`, so lexicographic order is chronological
order, and what it picks is the round that started last rather than the file written last (the latter would be skewed by
an old session still spamming). Neither step can be dropped: an earlier version had no name filter, and back then
FishingBot's `log0.log` sat in the same directory and kept spamming, so "newest" pointed at it systematically and `/log`
read back unfiltered spam; and a name such as `log9.log` sorts before `2026-...` (`l` is greater than `2`), so that
filter is still necessary today.

### 7.2 The program log: one copy in the terminal, one in a file

The logger installed by `src/process_log.rs` (`ProcessLog::get_or_init`, called by the process entry before the
framework is assembled) writes the frameworks' `log::*` and the library's to two places at once:

* Terminal (stderr): `journalctl -u minecraft-fishingbot-manager-for-discord`, or the foreground output. For watching things
  live, and it is still there when systemd runs the program.
* File: `<deploy dir>/logs/year-month-day-hour-minute-second.log`. For reading afterwards (`journal` rotates).

Details:

* The naming is the same as for bot logs (both come from `create_timestamped_log_file`) and each record looks like
  `[2026-09-28 15:30:12] [INFO] ...`, plain text with no colour, so the file stays clean.
* Levels come from the config's `[log] debug` and nothing else (`false` means this project from `info` and the
  frameworks from `warn`; `true` means both at `debug`); there is no environment variable and no second switch. The
  filter the frameworks ship with supports directives such as `module=level`, but this project deliberately keeps a
  single-level convention.
* This project's own diagnostics go through `log` as well (no more `println!`), so the terminal and the log file hold
  the same, complete content. The only exception is the message emitted when the logger cannot be installed (at that
  point the log system is not usable yet, so it is written straight to the terminal).
* A log file that cannot be created (the deployment directory is not writable) does not affect operation: one warning
  is written to the terminal, and it never prevents a bot from starting.

## 8. Related documents

* All documents: [docs index](../../index-en_us.md).
* Who calls these primitives: [lib-en_us.md](../lib/lib-en_us.md) (the command entry layer) and
  [architecture-en_us.md](../architecture/architecture-en_us.md) section 1 (the layers).
* The complete convention for the two log lines: [process-log-en_us.md](../process-log/process-log-en_us.md).
