# utils: the shared utility layer
[简体中文](utils-zh_cn.md) | **English**

* Code: `src/utils.rs` (module declarations and re-exports only) and `src/utils/*.rs` (one file per topic)
* The naming convention is action plus object (`raw_bot_name.to_clean_bot_name()`, `reply_text.utf16_len()`,
  `is_bot_name_taken`, `read_last_lines`); where a `Result<_, String>` is returned, the `Err` is always text that can
  be shown to the user as it is.
* The `pub use` list at the top of `utils.rs` is the public API: the business layer can only take things from there;
  every other `pub` item in a submodule serves utils internally (or only the flows). `bot_status`, for example, is
  mostly an implementation detail (`running_bots` uses it to decide a status) and exposes only two things to the
  business layer: `PauseCommand` and `confirm_pause_command` (`BotPauseStatus` only serves utils internally and does
  not have to be exported).
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
* `running_bots`: the single source of who is running. Grouping by run state, the names of the running ones, a status
  snapshot of the running bots (including the `/show status` header and rows) and the refusal text for running bots. Entry
  points `split_bot_entries_by_is_running`, `all_running_bot_names`, `all_running_bot_infos`, `RunningBotInfo`
  (`into_row`), `status_table_header` and `refusal_if_running`.
* `startup`: starting (four explicit steps; targets without a token are not launched; only the raw details
  `LaunchOutcome` are returned, which carries `into_report()` and `to_failure_text()`). Entry points
  `start_bot_sessions` and `prepare_and_launch_bot` (the `/bot login` path); the launch steps themselves
  (`launch_bot_session`, `launch_command_line` with `LaunchCommandConfig`) are internal to the module.

* `restart`: restarting. The "stop then start" orchestration shared by `/restart` and `/restartall` (only the raw
  details `RestartOutcome` are returned, carrying both the stop and the launch details). Entry point
  `restart_bot_sessions`.
* `shutdown`: stopping sessions (only the raw details `StopOutcome` are returned, which carries `into_report()` and
  `to_failure_text()`). Entry points `stop_single_bot_session` and `stop_all_bot_sessions`.
* `bot_log_files`: creating and picking bot logs (FishingBot's raw log is handled in `startup`). Entry points
  `create_bot_log_file`, `create_timestamped_log_file`, `target_logs_dir` and `latest_log_path`.
* `fishingbot_config`: generating the game-side config (FishingBot's `conf.json`): delete the old file, then take the
  template shipped with the repository (`fishingbot-config.example.json`) as the base and override those
  `[fishingbot]` keys (`misc.language` is written straight from `[i18n] language`, whose value is the jar's constant
  name). What gets written is a complete config and does not rely on the jar filling fields in. Entry point
  `regenerate_fishingbot_config` (called once at process startup, see
  [config-en_us.md](../config/config-en_us.md) section 3.2).
* `fs`: file and directory primitives; they only answer how to touch a file (read lines from the tail, list
  subdirectory names, remove a directory that may not exist). Which file and what to say on failure belong to the
  caller. Entry points `read_lines_backwards_until`, `read_last_lines`, `list_subdirectory_names` and
  `remove_dir_all_if_exists`.
* `bot_status`: bot status. The pause status is remembered inside the process (it only changes when the command echo
  is confirmed) and the online status is read fresh on every call; the `/pause` and `/unpause` confirmation echoes
  live here too. The entry points are `BotDirEntry::connection_status()` (read fresh each time; all three unreadable
  cases are errors; it only reports "online / reconnecting / initializing"), `ConnectionStatus` (the four normal
  states, `from_line` deciding a single line; "process finished" among them comes from `running_bots` based on the
  session shell and not from the log), `BotPauseStatusMemory` (kept per bot name: `to_bot_pause_status`,
  `update_bot_pause_status`, `forget_bot_pause_status`), `BotPauseStatus` (`pause_status`, `set_paused`,
  `set_fishing`), `PauseCommand` (`SetPause` and `SetUnpause`, each with the command name, the echo keyword
  `echo_keyword` - taken from the config's `[command_echo]` - and the state to set) and `confirm_pause_command`.
* `uptime`: uptime. Reads the seconds the process has been running and formats them. Entry points
  `process_elapsed_seconds` and `format_uptime`.
* `reply`: assembling and rendering replies, that is the report container and the length cap. Entry points
  `Report::new` (the only constructor; what comes out is empty), `set_title`, `set_counts`, `render` and
  `IntoReplyText`.
* `permission`: the only way a permission is decided. Every command declares one requirement; the entry point is
  `has_permission(caller_id, RequiredPermission)`, where the requirement is `Everyone`, `MustBeAdmin` or
  `OwnerOrAdmin(owner user ID)`.

## 2. `bot_dirs`: the token status, and which directories to take

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
* Which batch of directories to take is not a filter enum but one function per rule (the name states the scope, so a
  call site does not have to look up what an enum value means):
  * `all_bot_dir_entries()`: every user directory (`/startall`, `/restartall`, `/stopall`, the process shutdown,
    `/show allbots`); a root directory that does not exist yet counts as an empty list, only a real read failure is
    an error;
  * `token_ready_bot_dir_entries()`: only the ones with a configured token (`/commandall`, `/show status`);
  * `bot_dir_entries_of_owner(user ID)`: only the ones under one Discord user ID (`/show mybots`).

  Upper layers no longer write their own `read_dir` loop (the loop itself lives in `fs::list_subdirectory_names`), so
  two modules cannot filter differently by accident.

## 3. `running_bots`: the single source of "who is running"

`screen -list` is called once and cached as a snapshot; every later "is it running" decision is based on that same
snapshot. The join between a directory and a session happens only in this module, so nothing else walks directories to
match session names:

* The `/show status` table: the snapshot `RunningBotInfo` (including `all_running_bot_infos()`) is in this module, and so
  are the header (`status_table_header()`) and the rows (`into_row(self)`). Two four-element arrays sit next to each
  other and are read column by column; both statuses are snapshots of this call, the fields are private and
  `into_row(self)` can only consume them once, so they are discarded right after rendering (see
  [commands-en_us.md](../commands/commands-en_us.md) section 6.2).
* `split_bot_entries_by_is_running(&[BotDirEntry])` returns the running and the non-running group; which batch of
  directories is taken is decided by the caller (`all_bot_dir_entries()` or `token_ready_bot_dir_entries()`), see
  [commands-en_us.md](../commands/commands-en_us.md) section 5.2.
* `all_running_bot_names()` returns the names of every running bot (used by `/commandall`): the scope matches
  `all_running_bot_infos()` (every bot with a configured token), it calls `screen -list` once and reads no logs.

One snapshot reused in many places is deliberate: if a batch flow probed again every time, a session started a moment
ago would immediately count as already running and the statistics would drift.

## 4. The waits of `startup` / `shutdown`

* All launch-capable commands (`/start`, `/startall`, `/restart` and `/restartall`) share one process-wide FIFO start
  queue. Its dispatcher remains alive and blocks while idle. Each command request is split into batches of at most
  `[startup_queue] max_batch_size`; the next FIFO request is not processed until the current request completes. Two
  sign-ins are at least `[startup_queue] login_interval_ms` apart: bots inside a batch are spaced by it, and the wait
  between batches is it minus that batch's recheck wait (`timing.settle_per_bot_ms` times 2 times the batch size). A
  partial batch is closed when the queue becomes idle, but the "last sign-in" timestamp is kept across commands, so
  consecutive commands cannot bypass the rate limit. Queued requests are not persisted and are lost if the process
  restarts. `/bot login` does not use this queue.
* `start_bot_sessions` handles launches within a batch and spaces them; the queue dispatcher handles the wait between
  batches, and `settle_per_bot_ms` still scales the session-recheck wait by the number of bots in that batch, with the
  two snapshots separated by the same interval. The sign-in rate is the only thing that is limited, so one interval
  value covers both inside and between batches: the batch size only changes the length of the verification window and
  the JVM memory peak, not the total duration (total duration is about the number of bots times the interval). See
  [commands-en_us.md](../commands/commands-en_us.md) section 5.5.
* Neither wait of the stop flow takes a parameter; the flow reads the config itself: `stop_grace_ms` is read in
  `shutdown::stop_bot_sessions` (send the signal first, force kill only if that does not work) and `stop_settle_ms` is
  read there too (the deregistration buffer after a force kill and before the re-check).
* The stop flow sends the exit signal first, waits `stop_grace_ms` and force kills if that is not enough, then waits
  `stop_settle_ms` so the old session can deregister completely, because checking immediately would still see it.
* Every re-check goes through `running_bots`, so "launched successfully" means the session is still there after two
  checks (a `screen -list` snapshot), not that the command was sent; and a session being there does not mean the bot is
  online either (logging in and joining the server are up to the JVM), which is why the report and the reply only say
  "the start command was sent and the session has appeared".

## 5. `reply`: the reply format is written in one place

* Fields are sections: `basics` (basic information and list rows, pushed straight in), `succeeded`, `skipped`,
  `failed` (details) and `content` (a whole block of content). `render()` omits whatever was not filled in, so an
  empty section never leaves an empty title behind.
* `set_counts(&[(label, count)])` overwrites the basic information rows and generates `label: N` uniformly; non-count
  rows (a header, a list of names, an explanation) are pushed straight into `basics`.
* `new()` and `set_title()`: there is only this one way to construct a report (what comes out is empty) and the title
  is always added by `set_title()` (some titles are only known once the flow has run).
* The success/failure decision and the one-line error text are not here: they are methods on the details utils
  returns (`has_failed_lines()` and `to_failure_text()` of `LaunchOutcome`, `RestartOutcome` and `StopOutcome`).
* `render()` caps the length by `runtime.max_reply_chars`: when it is too long, lines are dropped from the end and the
  number of omitted lines is noted at the end. Discord has a hard limit per message (2000 characters) and an overlong
  message simply fails to send, which looks to the user as if nothing happened; truncating by line guarantees that the
  title and the count rows survive. The length is measured with `str::utf16_len()` (UTF-16 code units; `.len()` is
  bytes and `chars()` is code points, and neither is right).

For a command whose counts are only known once the flow has run, the flow runs first and `set_counts` then overwrites
the rows as a whole, rather than letting the flow append, which would leave both the placeholder figures and the real
ones in the same report.

## 6. How config values are read

There is no function in `utils` that "takes the whole config and picks fields out of it"; every function touches only
the one or two fields it really uses, in two styles:

* Taking parameters (the caller supplies the values, the function does not touch the config): used where the logic is
  heavy enough to deserve a unit test. `launch_command_line` with `LaunchCommandConfig` (assembling the launch
  command) and `target_logs_dir` (pure string and path assembly), for example.
* Reading only the fields it needs (`config::get().table.field`): used for convenience functions with a line or two of
  logic. `session_name` reads `runtime.screen_prefix`; `create_bot_log_file` and `latest_log_path` read
  `runtime.logs_dir`; `BotTokenStatus::from_dir` reads `runtime.token_file`; `BotDirEntry::find_by_name` reads
  `paths.bots_dir`; `stop_bot_sessions` reads `timing.stop_grace_ms` and `timing.stop_settle_ms`; `Report::render`
  reads `runtime.max_reply_chars`.

The config is read once at startup and then lives in memory (see [config-en_us.md](../config/config-en_us.md) section
1), so a convenience function reading one field costs nothing extra; and a logic function taking parameters buys unit
tests for the places that are easy to get wrong, such as the launch command and the waits (see the tests in
`utils::bot_dirs`, `utils::startup` and `utils::reply`).

One exception is the status decision `BotDirEntry::connection_status()`: its logic is heavy, but it reads the keywords
itself, half from the config's `[keywords]` (the two entries that differ per deployment and must be filled in by hand)
and half from the `jar_*` methods in the language table `text` (the reconnect message the JAR prints, taken according
to `[i18n] language`). Both are one global, fixed copy, so there is no point in passing them down through every caller;
they hang off the entry instead (picking the newest log happens inside the method, so the caller does not have to
assemble that either). The cost is that keywords cannot be injected in a unit test, so the tests cover the
parameter-taking implementation (`connection_status_of_log(bot_name, log_path)`, with the single-line decision in
`ConnectionStatus::from_line`), while the "read backwards, stop at the first conclusion" half is covered by the tests
of `fs` (`read_lines_backwards_until`). It does not produce "process finished": that status looks at the session shell
and is given straight by `running_bots` (see `ScreenSession` in [`super::screen`]).

The same trick is used for the reply truncation (inside `Report::render`): it reads `runtime.max_reply_chars` itself,
at the same cost, since the limit can only come from the config; that truncation logic therefore has no unit test
(covering it would mean turning the limit into a parameter, one layer more).

Inside `bot_status` lives one piece of in-process state (`BotPauseStatus`: stored per bot name, with
`update_bot_pause_status` as the only writer). Only the pause status goes into a struct; the online status is not
stored:

* The pause status is the result of the `/pause` and `/unpause` we sent ourselves, remembered inside the process
  (initially "fishing") and only changed once the JAR's echo is confirmed (`BotPauseStatus::set_paused` and
  `set_fishing`); reading it (`pause_status`) is reading that memory. When a `/pause` and an `/unpause` for the same bot
  arrive almost at the same time, the status follows the last one in the echo window
  (`PauseCommand::last_confirmed_pause_command`) rather than whoever wrote last; and whether my own command took effect
  is judged the same way: when the newest one is the other command, the caller replies with a failure and reports the
  current state.
* The online status can only be confirmed from the log (only the JVM knows whether it is connected or dropped) and a
  stored copy would be stale immediately, so it is not kept across calls: `bot_dir_entry.connection_status()` reads it
  fresh on every call and the caller always gets the newest one (the `RunningBotInfo.connection_status` inside
  `/show status` is a snapshot for that rendering only and is dropped right after).

That way `/show status` (a high-frequency command) does not have to scan a log back to the beginning of the file to find a
pause keyword for a session that never showed one. Both the start and the end of a session forget this state
(`forget_bot_pause_status`), so it always describes this session only.

## 7. The two log lines

### 7.1 Bot logs: only one file

`<bot directory>/<runtime.logs_dir>/` holds exactly these two things:

* `year-month-day-hour-minute-second.log`: our log. `create_bot_log_file` creates it before the session is launched,
  and the session appends the JVM output through `grep -v -E` and `tee -a` (lines matching `keywords.filter_regex` are
  dropped). `/log` reads its tail and `/show status` scans it from the end to decide a status.
* `file_name` under `[raw_game_logs] dir_name` (default `raw/log0.log`): FishingBot's log, a symlink to `discard_path`
  (default `/dev/null`), so whatever is written into it is thrown away by the kernel (the whole directory is emptied
  and recreated before every launch, so only the filtered copy stays on disk). Both the name and the discard target
  are configured in `[raw_game_logs]`.

Why `/show status` scans backwards (`fs::read_lines_backwards_until` with `BotDirEntry::connection_status`): the connection
status only looks at the last occurrence of a keyword, so it reads back from the newest line and stops at the first
conclusion. A long-running account's log can be tens of MB, and reading it all in is slow and uses memory. The pause
status does not scan the log at all, it is remembered inside the process (previous section), which saves the trip all
the way back to the beginning of the file when no pause keyword exists (measured: 72 ms for a 14 MiB log, per bot, per
call). `read_last_lines`, which `/log` uses, is a thin wrapper over the same implementation (stop once N lines are
counted), so it also reads only the tail of the file and never streams the whole log just to show the last 100 lines.
Both reading styles are general file primitives and live in `utils::fs`.

Why FishingBot still gets a file: it refuses to start when it cannot create a log file (in `Bot.java`, a `FileHandler`
throwing `IOException` leads to `setRunning(false)`), so it has to be given a target it can write to that stores
nothing. As a result there is only one real log on disk, and two copies can no longer interfere with each other.

The rule for picking the file lives in exactly one place, `bot_log_files::latest_log_path`, in two steps: it accepts
only timestamp-shaped names (`is_bot_log_file`, whose names come from `create_timestamped_log_file`), then takes the
largest by name. The name is a fixed-width, zero-padded `year-month-day-hour-minute-second`, so lexicographic order is
chronological order, and what it picks is the round that started last rather than the file written last (the latter
would be skewed by an old session still spamming). Neither step can be dropped: an earlier version had no name filter,
and back then FishingBot's `log0.log` sat in the same directory and kept spamming, so "newest" pointed at it
systematically and `/log` read back unfiltered spam; and a name such as `log9.log` sorts before `2026-...` (`l` is
greater than `2`), so that filter is still needed today.

### 7.2 The program log: one copy in the terminal, one in a file

The logger installed by `src/process_log.rs` (`init_process_log`, called by the process entry before the framework is
assembled) writes the frameworks' `log::*` and the library's to two places at once:

* Terminal (stderr): `journalctl -u minecraft-fishingbot-manager-for-discord`, or the foreground output. For watching things
  live, and it is still there when systemd runs the program.
* File: `<deploy dir>/logs/year-month-day-hour-minute-second.log`. For reading afterwards (`journal` rotates).

Details:

* The naming is the same as for bot logs (both come from `create_timestamped_log_file`) and each record looks like
  `[2026-09-28 15:30:12] [INFO] ...`, plain text with no colour, so the file stays clean.
* Levels: `RUST_LOG` wins (single level words only, `debug` for example), otherwise the config's `[log] debug` decides
  (`false` means this project from `info` and the frameworks from `warn`; `true` means both at `debug`). The filter the
  frameworks ship with supports directives such as `module=level`, but this project deliberately keeps a single-level
  convention.
* This project's own diagnostics go through `log` as well (no more `println!`), so the terminal and the log file hold
  the same, complete content. The only exceptions are the two warnings for when the log itself fails (the logger
  cannot be installed, the log file cannot be created), at which point the logger is not usable yet.
* A log file that cannot be created (the deployment directory is not writable) is not fatal: one warning in the
  terminal is all, and it never keeps a bot from running. That warning is written straight to the terminal whenever the
  global level is stricter than `WARN` (`RUST_LOG=error` or `off`), so the level filter cannot swallow it.

## 8. Related documents

* All documents: [docs index](../../index-en_us.md).
* Who calls these primitives: [lib-en_us.md](../lib/lib-en_us.md) (the command entry layer) and
  [architecture-en_us.md](../architecture/architecture-en_us.md) section 1 (the layers).
* The complete convention for the two log lines: [process-log-en_us.md](../process-log/process-log-en_us.md).
