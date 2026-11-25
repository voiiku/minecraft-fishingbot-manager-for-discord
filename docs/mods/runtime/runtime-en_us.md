# Runtime: shared operations
[简体中文](runtime-zh_cn.md) | **English**

`src/runtime/` contains sessions, the queue, configuration, logging and shutdown. Bot operations live under `src/operations/`, permission checks under `src/operations/mod.rs`, and reports under `src/text/report.rs`.
Low-level name, command-fragment, file and uptime logic lives under `src/utils/` without depending on other project modules; callers pass configuration values. `runtime::utils` is private to runtime and also does not depend on texts; `runtime::log_files` converts file errors to localized text. `bot_log` owns both filtered bot logs and raw-log preparation.

## 1. Module map

See the [architecture map](../architecture/architecture-en_us.md) for source locations. The following sections describe the behavior of the shared operations.

Process-wide mutable state is declared together in `src/lib.rs`; its owning modules still control initialization and behavior:

* `CONFIG`, `STARTUP_COMPLETE`: configuration and first-setup completion; the language is part of the config.
* `PROCESS_LOGGER`, `STARTUP_QUEUE`, `WATCHDOG`: the logger, startup queue, and watchdog.
* `ANTI_AFK`, `SCREEN_COMMAND_LOCKS`, `COMMAND_SYNC_LOCK`: the anti-AFK task, screen command locks, and command synchronization lock.

These are eight declarations; immutable language objects, test-only state, dependency internals, and individual locks within containers are not counted.

## 2. `bot_dirs`: token status and directory scopes

* `BotTokenStatus::Ready` and `BotTokenStatus::Missing`: they only say whether the token file has contents, and
  validate nothing online, because only FishingBot itself knows whether a token is valid when it starts.
* `BotDirEntry` is the struct of a directory entry: the owning Discord user ID, the bot name, the folder path, and
  the token status read when it was built. The path is not passed in (it is assembled from
  `bots_dir/Discord user ID/name`), and the two constructors say where the ownership comes from:
  `BotDirEntry::new(raw name, owning Discord user ID)` and `BotDirEntry::find_by_name(raw name)` (looks the entry up
  by name across every user directory and checks no permission - the business command checks that, see
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

## 3. `bot_sessions`: session snapshots

Each bulk grouping calls `screen -list` once and uses the resulting snapshot for all target bots:

* `/show status` reads the bot directories and screen snapshot once, then reuses them for status rows and the queue list.
  `src/operations/queries/show_status.rs` builds the header and `status_row` in the same column order (see
  [commands-en_us.md](../commands/commands-en_us.md) section 6.2).
* `split_bot_entries_by_is_running(&[BotDirEntry])` returns the running and the non-running group; which batch of
  directories is taken is decided by the caller (`all_bot_dir_entries()` or `token_ready_bot_dir_entries()`), see
  [commands-en_us.md](../commands/commands-en_us.md) section 5.2.
* `token_ready_bot_names_with_screen_sessions()` returns names of bots with a token and a screen session (used by
  `/commandall`), including dead sessions. Its scope matches `/show status`, and it calls `screen -list` once without
  reading logs.

One snapshot reused in many places is deliberate: if a batch flow probed again every time, a session started a moment
ago would immediately count as already running and the results would become inconsistent.

## 4. Waits during startup and shutdown

* All launch-capable commands (`/start`, `/startall`, `/restart` and `/restartall`) share one process-wide FIFO start queue. Its dispatcher remains alive and blocks while idle. Each command request is split into batches of at most `[startup_queue] max_batch_size`; the next FIFO request is not processed until every entry of the current request has been popped. Adjacent bots in a batch are separated by at least `[startup_queue] bot_login_interval`. Batches are separated by at least `[startup_queue] batch_login_interval`, counted from when the previous batch was popped (the verification runs in parallel on the command side and does not count as this wait). A partial batch is closed when the queue becomes idle, but the "last pop" timestamp is kept across commands, so consecutive commands cannot bypass the rate limit. Queued requests are not persisted and are lost if the process restarts. `/bot login` does not use this queue.
* `StartupQueue::start_bots()` is the only command-side entry (create the ledger → enqueue → wait until every entry has been launched → take the results and verify); the starter thread handles per-batch staggering and launches, and the verification observes session survival with one wait per start request (check immediately after launch, then once more after the grace period). The sign-in rate is the limited resource, controlled by `bot_login_interval` inside a batch and `batch_login_interval` between batches; the batch size only affects the JVM memory peak during launch. See [commands-en_us.md](../commands/commands-en_us.md) section 5.5.
* Neither stop wait takes a parameter; the flow reads the config and performs both waits itself: send the exit signal,
  wait `stop_grace`, force kill if the session is still there, then wait `stop_settle` so the old session can
  deregister completely before the final check (checking immediately would still see it).
* Every re-check goes through `bot_startup::verify_start_results`: the session must remain and not be dead at the second `screen -list` check. Joining the game is confirmed separately from the log.

## 5. `report`: the reply format is written in one place

* Fields are sections: `basics` (basic information and list rows, appended directly), `succeeded`, `skipped`,
  `queued`, `failed` (details) and `content` (a whole block of content). `render()` omits whatever was not filled in, so
  an empty section never leaves an empty title behind.
  `succeeded` is labeled "Completed" and may include unconfirmed states; `failed` is labeled "Issues" and includes
  failures and timeouts.
* `set_counts(&[(label, count)])` overwrites the basic information rows and generates `label: N` uniformly; non-count
  rows (a header, a list of names, an explanation) are appended directly to `basics`.
* `new(language, max_utf16_units)` and `set_title()`: the caller supplies the language and length limit; the report starts empty and the title
  is always added by `set_title()` (some titles are only known once the flow has run).
* Reports only present results. The start queue returns each bot's verified status or an issue; a single stop returns confirmed exit or the error.
* `render()` uses the supplied `runtime.max_reply_chars` as a truncation target: when too long, it removes whole lines
  from the end and appends the number of omitted lines. These boundaries are intentional: an overlong line is not split;
  a very small limit can omit the title, counts or all content, while the omission note is still emitted and may itself
  make the output exceed the configured limit. Length is measured in UTF-16 code units (`.len()` counts bytes and
  `chars()` counts code points).
* Configuration only requires `max_reply_chars` to be positive; it imposes no maximum and does not automatically cap
  it at Discord's 2000 UTF-16 code units. Deployers should choose an appropriate value; a message exceeding Discord's
  limit may fail to send. This truncation applies only to reports rendered by `Report::render()`, not necessarily to
  other replies or error messages.

For a command whose counts are only known once the flow has run, the flow runs first and `set_counts` then overwrites
the rows as a whole, rather than letting the flow append, which would leave both the placeholder figures and the real
ones in the same report.

## 6. How config values are read

Shared operations read only the configuration fields they actually use, in two styles:

* Functions that take parameters (the caller supplies the values; the function does not read the config): used where
  the logic is complex enough to justify a unit test, for example `launch_command_line` (assembling the launch command).
* Functions that read only the fields they need (`config::Config::get().table.field`): used for convenience functions
  with a line or two of logic. `session_name` reads `runtime.screen_prefix`; `create_bot_log_file` and
  `latest_log_path` read `runtime.bot_logs_dir`; `BotTokenStatus::from_dir` reads `runtime.token_file`;
  `BotDirEntry::find_by_name` reads `paths.bots_dir`; `stop_bot_sessions` reads `timing.stop_grace` and
  `timing.stop_settle`.

The config is read once at startup and then lives in memory (see [config-en_us.md](../config/config-en_us.md) section
1), so a convenience function reading one field costs nothing extra; functions that take parameters, in turn, gain
unit-test coverage for the places that are easy to get wrong, such as the launch command and the waits (see the tests
in `runtime::bot_instances::bot_dirs`, `runtime::bot_startup` and `text::report`).

`query_bot_status` performs the status query; its parser takes a language table so both response languages can be tested.
Screen sessions remain separate for process management.

Reply truncation in `Report::render` uses the caller-supplied `runtime.max_reply_chars`; the report module does not read config.

Connection and fishing state are parsed into `BotStatus` from a new `/status` response each time and are not stored.
Start, pause and resume use the same query. Unavailable status is reported as an issue without stopping the session;
a dead session is reported as stopped. State can change after the query.

## 7. The two logging streams

### 7.1 Bot logs: only one file

`<bot directory>/<runtime.bot_logs_dir>/` contains two kinds of files:

* `year-month-day-hour-minute-second.log`: our log. `create_bot_log_file` creates it before the session is launched,
  and the session appends the JVM output through `grep -v -E` and `tee -a` (lines matching `keywords.filter_regex` are
  dropped). `/log` reads its tail and `/show status` checks the response appended after its query.
* `file_name` under `[raw_game_logs] dir_name` (default `raw/log0.log`): FishingBot's log, a symlink to `discard_path`
  (default `/dev/null`), so whatever is written into it is thrown away by the kernel (the whole directory is emptied
  and recreated before every launch, so only the filtered copy stays on disk). Both the name and the discard target
  are configured in `[raw_game_logs]`.

`/status` checks only output appended after the query, scanning at most 100 trailing lines. `read_last_lines`, used by
`/log`, also reads only the tail.

Why FishingBot still gets a file: it refuses to start when it cannot create a log file (in `Bot.java`, a `FileHandler`
throwing `IOException` leads to `setRunning(false)`), so it has to be given a writable target that stores nothing. As a
result there is only one real log on disk, and two copies can no longer interfere with each other.

The rule for picking the log file lives in exactly one place, `bot_log::latest_log_path`: parse and filter names using the
built-in timestamp format (the names come from `create_timestamped_log_file`), then select the largest timestamp. The
default name is a fixed-width, zero-padded `year-month-day-hour-minute-second`, and what it picks is the round that
started last rather than the file written last (the latter would be skewed by
an old session still spamming). Neither step can be dropped: an earlier version had no name filter, and back then
FishingBot's `log0.log` sat in the same directory and kept spamming, so "newest" pointed at it systematically and `/log`
read back unfiltered spam; and a name such as `log9.log` sorts before `2026-...` (`l` is greater than `2`), so that
filter is still necessary today.

### 7.2 The program log: one copy in the terminal, one in a file

The logger installed by `src/runtime/process_log/mod.rs` (`ProcessLog::get_or_init`, called by the process entry before the
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
