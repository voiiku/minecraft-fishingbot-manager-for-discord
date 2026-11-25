# The config file
[简体中文](config-zh_cn.md) | **English**

* Code: `src/runtime/config/mod.rs` (the runtime `Config` and its accessors) and `src/runtime/config/{constants,schema,parse,validate,template}.rs`
  (the tables and the file shape of `[paths]`, parsing, validation, the template)
* File: `minecraft-fishingbot-manager-for-discord.conf.toml` next to the executable (the name is fixed by
  `CONFIG_FILE_NAME` in `constants.rs`, not by TOML)
* It does not depend on the working directory and reads no environment variables: placing the binary and the config in the same directory is enough, and systemd and similar supervisors need no extra settings.
* The file contains the bot token; `chmod 600` is recommended.

## 1. Load flow (the very beginning of startup)

1. `config::Config::load()` locates the config file path.
2. When the file does not exist, one is generated from the repository's `minecraft-fishingbot-manager-for-discord.conf.example.toml`: the template is written and the note is printed in this step, then the process exits with code 0 without connecting to Discord. When the file exists, the path found is printed to the terminal before continuing (no logger is installed yet, so the note appears only in the terminal; the `config loaded: ...` line in the program log is written by `ProcessLog::get_or_init`).
3. When the file exists, its text is read. `Config::from_toml_text` parses the TOML into a value tree, recursively trims every string value, and maps it into the runtime `Config` through Serde. `[paths]` expands relative paths under `base_dir`; the language name is parsed as `Language`, rejecting unsupported values.
4. `validate` checks the built-in parameters, remaining fields and runtime environment. Configuration errors use the fixed English messages in `text::bootstrap`.
5. When reading, parsing or validation fails, the error is returned to the process entry, which calls `runtime::shutdown::exit_on_permanent_failure` to report it and exit with code 1. Without a loaded configuration it cannot safely identify bots, so it does not attempt cleanup; the example systemd deployment uses `KillMode=control-group` to clean up remaining processes.
6. Once all checks pass, the complete config is written into a `OnceLock`; its language is not installed separately. Editing the file while the process runs has no effect; restart for changes to take effect.

`Config::from_toml_text` removes whitespace around string values: extra spaces, indentation and line breaks in the file never reach the code, so config readers (`main.rs`, the business layer) use the value as-is and do not call `trim()` themselves. TOML escapes are decoded before trimming, so `"\u0020"` cannot preserve surrounding spaces either.

Deserialization relies on `#[derive(Deserialize)]` on `Config` and its field types. Serde maps TOML table and key names to the corresponding struct fields; a type mismatch, a missing required field or an unknown field returns a parse error. Required file fields have no defaults (`discord.guild_ids` may be omitted), so missing keys and typos fail at startup. Path fields are converted by `Paths`'s own `Deserialize` implementation into runtime paths rooted at `base_dir` (`bots_dir` is derived in code); the remaining value rules are handled by the later `validate` step.

Built-in runtime parameters are defined in `src/runtime/config/constants.rs` and combined with file settings as `Config.constants` during parsing; they do not add TOML fields. Runtime code reads them through `Config::get().constants`. Changing source parameters requires rebuilding and restarting. The config file name and template-creation permissions are internal bootstrap values; the latter shares its definition with runtime private-file permissions. Protocol identifiers, texts, template contents, locks and singletons are not tunable parameters. Do not relax Discord hard limits or security permissions; the bot log timestamp format must contain a complete date and time, and changing it makes old-format logs unrecognizable. The factor 2 in the queue timeout formula below is the default `startup_result_timeout_factor`.

`display_timestamp_format` controls both `/checkserver` system time and reconnect-notice outage time, including the time zone and UTC offset by default. Program logs use a separate format.

All configuration times use seconds without a unit suffix. Former millisecond fields accept integer or fractional seconds in multiples of `0.001`; for example, 300 milliseconds becomes `0.3`. They are still computed internally in milliseconds. Anti-AFK `execution_interval` is whole seconds, and `input_delay` retains its existing floating-point seconds precision. When upgrading, remove the obsolete `[command_echo]` table; rename `[ping] timeout_ms` to `timeout` and divide its value by 1000; remove `_ms` and divide the value by 1000 for `[startup_queue]`'s `bot_login_interval_ms` and `batch_login_interval_ms`, and `[timing]`'s `check_grace_ms`, `stop_grace_ms`, `stop_settle_ms` and `watchdog_window_ms`. Rename `[anti_afk] execution_interval_seconds` and `input_delay_seconds` to `execution_interval` and `input_delay` without changing their values. Old keys are not supported.

When upgrading, also remove `online` from `[keywords]` and keep `filter_regex`.

After the config is loaded, the process entry **generates the game-side config** from `[fishingbot]` during startup (FishingBot's `config.json`, at the path from `paths.fishingbot_config_file_name`): the old file is deleted first, then a new one is written. A failure is handled as a config error (an `ERROR` record and exit code 1). Section 3.2 explains why it is generated and what it contains.

## 2. Public interface

At process entry call `Config::load()` first, then obtain the complete config through `Config::get()`:

* `Config::load()`: locate, read, parse, validate and publish the configuration, returning `Result<(), String>`. A newly generated template exits with code 0; invalid configuration returns an error to the caller.
* `Config::get()`: the complete loaded `Config`, including file settings and `constants`; the admin table is `Config::get().admins`.
* `Config::get().text()`: select the text implementation from the configured language.
* `Config::install_dir()`: the deployment directory (where the executable is); returns an error message if the path cannot be determined. The config and the program log live there.
* the `config_file_path` field: the config file path used by this run (computed from the executable's location at load time and stored in the struct).

`Config::get()` panics while no config is loaded, with the fixed English text from `text::bootstrap::CONFIG_NOT_LOADED`. The first thing the process entry does is `Config::load()`, so a panic here means the call order is wrong. The config file path is stored as a field of `Config`; whether it is loaded is answered by the single `OnceLock<Config>`.

## 3. Table structure

The following sections describe configuration fields and constraints; the example config includes English comments. See [i18n-en_us.md section 1](../i18n/i18n-en_us.md) for language and JAR echo settings.

* `[discord]`: `token`, `guild_ids`. The bot token is required; `guild_ids` is an array of Discord server ID strings. Empty or omitted selects global registration; a nonempty array selects those servers. Restart and run `/sync-commands` after changing it.
* `[paths]`: `base_dir` (absolute), `jar`, `fishingbot_config_file_name` (relative). The data root, plus the JAR and game-side config file names, resolved relative to it.
* `[fishingbot]`: `server_ip`, `server_port`, `default_protocol`, `auto_reconnect`, `auto_reconnect_time`, `start_text`, `announce_type_console`, `announce_type_chat`. The keys of the game-side config (FishingBot's `config.json`) that **this project exposes**; that file is generated from them at startup (section 3.2). The whole table is required; the values can be copied from the example config.
* `[admins]`: `"Discord user ID" = "name"`. The admin list; keys must be plain numeric Discord user IDs (snowflakes).
* `[keywords]`: `filter_regex`, the regular expression that filters spam lines before they are written (`grep -v -E`). It must not discard `/status` responses. Bot state is queried from FishingBot; process termination is checked through screen.
* `[i18n]`: `language`. The language the program runs in; it affects Discord texts, the program log and error messages. The value is the constant name from FishingBot's own `i18n.Language` and is also written into `config.json` as `misc.language` (the same value, not mapped), so the two values never need manual alignment. This project implements `ENGLISH` and `CHINESE_SIMPLIFIED`; changing it and restarting is enough, with no rebuild. See section 3.1.
* `[runtime]`: `java_opts`, `screen_prefix`, `token_file`, `bot_logs_dir`, `max_reply_log_lines`, `max_reply_chars`, `max_autocomplete_choices`. The runtime parameters of a single bot. `bot_logs_dir` holds only this project's logs (FishingBot's own raw log sits beside it under `[raw_game_logs] dir_name` as `[raw_game_logs] file_name` and points at `discard_path`, see [runtime-en_us.md section 7](../runtime/runtime-en_us.md#7-the-two-logging-streams)); `max_reply_chars` is the positive UTF-16 truncation target for reports, not automatically capped at Discord's 2000-unit limit, and a tiny value can be exceeded by the omission note; see [runtime-en_us.md section 5](../runtime/runtime-en_us.md#5-report-the-reply-format-is-written-in-one-place). `max_autocomplete_choices` limits autocomplete suggestions for bot-name options (range 1–25, Discord's hard limit).
* `[anti_afk]`: `command`, `execution_interval`, `input_delay`. Anti-AFK input, the wait in seconds after each completed cycle, and the delay in seconds after each character or key. Input uses [input-pulse](https://github.com/voiiku/input-pulse) action syntax; the example is `!/!!/!!ENTER!`. The target is every GNU screen session whose name **contains** `runtime.screen_prefix`, not just sessions created by this project; avoid matching unrelated sessions. All three fields are required: the input must be nonempty and valid for screen mode, the execution interval must exceed 0, and the input delay must be finite, non-negative and representable (0 is allowed). The configuration is validated at startup; admins can use `/anti-afk` to control the feature, which does not start automatically after a process restart. Add this table when upgrading an existing config.
* `[startup_queue]`: `max_batch_size`, `bot_login_interval`, `batch_login_interval` and `max_pending_requests`. They define, in order, the maximum bots per batch in the global FIFO start queue, the sign-in gap between adjacent bots in a batch (seconds), the gap between batches (seconds), and the maximum number of waiting requests (excluding the active one). The table and every field are required, and every value must be greater than zero. `bot_login_interval` staggers adjacent bots inside a batch. `batch_login_interval` spaces batches: it counts from the moment the previous batch was popped, and the next batch starts only after at least this long (verification runs in parallel and is not counted as batch-to-batch waiting); this also applies when the previous batch held a single bot, so the first command after a long idle does not wait. Microsoft rate-limits sign-ins per egress IP; see [troubleshooting section 12](../../troubleshooting/troubleshooting-en_us.md) for the values. `/start`, `/startall`, `/restart` and `/restartall` share this queue; `/bot login` does not use it. At most `max_pending_requests` bots wait in the queue; the rest are returned to the caller immediately (the reply states this), and pending entries are dropped when the process restarts. The result timeout is not configurable: it is derived from this table and `timing.check_grace` (`StartupQueue::result_timeout`; the grace period is counted once): `2 * (ceil(max_pending_requests / max_batch_size) * max(batch_login_interval, (max_batch_size - 1) * bot_login_interval) + check_grace)`. When it expires, bots without a result are reported as "unconfirmed" in the reply (and one line is written to the program log). A slower per-bot stagger or a larger queue extends it automatically.
* `[timing]`: `check_grace`, `stop_grace`, `stop_settle` and `watchdog_window`. These per-flow waits are in seconds. `check_grace` is the session-survival grace period after every bot in a start request has been launched (the check runs twice: immediately after launch, then after the grace period; one wait per start request, not per bot). `stop_grace` is how long to wait for the JVM to exit gracefully after the stop signal, before force-killing it; `stop_settle` is the settling wait after a force kill and before the session state is checked again. `stop_grace` and `stop_settle` are only read and have no value validation. `watchdog_window` is the gateway watchdog's judging window, also used as the wait before rebuilding the connection; the REST probe interval (a third of the window) and the first-handshake timeout (twice the window) are derived from it instead of being separate keys, see [watchdog-en_us.md](../watchdog/watchdog-en_us.md).
* `[raw_game_logs]`: `dir_name`, `file_name`, `discard_path`. The convention for FishingBot.jar's own raw log (directory name, file name, and where the contents are discarded). It follows the JAR and the whole table is required; the example config uses `raw`, `log0.log` and `/dev/null`.
* `[ping]`: `default_ping_host`, `default_ping_port`, `timeout`. The default target and timeout of `/ping` when it is called without arguments. Explicit hosts must resolve only to public addresses; see [troubleshooting](../../troubleshooting/troubleshooting-en_us.md#ping-refuses-a-local-or-private-address) if proxy Fake-IP DNS causes a rejection.
* `[log]`: `debug`. The program log level switch. With `false`, this project's records start at `INFO` and third-party crates (serenity, poise) at `WARN`; with `true`, both go down to `DEBUG` (fully open; finer `TRACE` records are still not captured). This is the only level switch (environment variables are no longer read); details in [process-log-en_us.md](../process-log/process-log-en_us.md).
* `[text]`: `help`. The full output of `/help` (a multi-line string, so the wording can be changed without a rebuild).

### 3.1 Where the keywords live: `[keywords]` and `[i18n]`

* The deployment-specific log filter is `[keywords] filter_regex`; join-message keywords are no longer used.
* FishingBot's `/status` response is matched using the language table; query timeout and scan limits are internal.
* "Process finished" needs no keyword: an older wrapper script wrote an end marker into the log with `java ... | tee bot.log; echo marker >> bot.log`, while this project starts the session itself and does not write it. The check therefore reads the screen session state instead (a `(Dead ...)` shell, see [runtime-en_us.md](../runtime/runtime-en_us.md)), which is more reliable than copying a marker: it does not depend on the JAR version, the script or the language.

Deployment-specific keywords go into `[keywords]`, fixed JAR messages follow `[i18n]`, and process termination is read from the session state.

### 3.2 The game-side config (`config.json`) is a generated file

The example uses `paths.fishingbot_config_file_name = "config.json"`. Existing installations must update this field in the manager TOML and restart; the previous `conf.json` is not automatically renamed or deleted. Custom file names remain supported.

FishingBot only reads JSON (`-config <path>`) and has **no command-line argument that overrides an individual setting** (its startup arguments are only `-nogui`, `-help`, `-logsdir`, `-config`, `-refreshToken` and `-onlyCreateConfig`). Therefore, merging `config.json` into the TOML can only be **one-way generation**: the TOML is the single source of truth, and the JSON is written when the process starts (the code is in `runtime/bot_instances/fishingbot_config.rs`).

Generation method: **use the template shipped with the repository as the base and override only the corresponding keys in `[fishingbot]`.**

* The template is the repository's `fishingbot-config.example.json`, a **complete** FishingBot config (taken from the default file upstream generates, with defaults adjusted for this project: console and chat announcements off, `/home fishing` as the default cast command, a longer `auto-reconnect-time`, and so on). It is compiled into the binary with `include_str!`, so no extra file is needed at deployment.
* The overridden keys are the managed list in `runtime/utils/fishingbot_config.rs` (ten dotted paths), one to one with the fields of `[fishingbot]`; `misc.language` is written directly from `[i18n] language` (whose value is the JAR's constant name, see `text::Language::from_name` / `name`), so there is no manual entry and the two values need no alignment.
* Every other key is left as the template has it, so the output is **always a complete config** and never relies on the JAR to fill in missing fields. (Measured on v2.13.15: it does fill in missing fields and writes the file back, but the JAR decides what is added, and every process start leaves one more `Your configuration was incomplete. The missing fields were added!` line in the bot log.)
* **The old file is deleted first**: it is a generated file, so hand edits do not survive the next start. Change `[fishingbot]` instead, or change the template to change a default.

When upstream changes the config structure (adds, renames or removes a key): take a default `config.json` generated by the new JAR, copy it over the template and run `cargo test`. `template_holds_every_managed_path` fails first (a managed path is missing from the template); follow its message to sync `managed_values` and `[fishingbot]`. This is the **only** maintenance point that must follow upstream.

`[fishingbot]` is the single entry point for the desired game-side config; FishingBot reads the configured JSON file, which `/show botconfig` also sends. Changing `[i18n] language` also changes the JAR's language.

## 4. Field rules

* Every field except the optional `discord.guild_ids` is required; omitting a required field or misspelling one (caught by `deny_unknown_fields`) fails at startup. The example config has every key filled in and can be used as the starting point.
* Path rules (`paths`): `base_dir` must be absolute; `jar` and `fishingbot_config_file_name` must be relative and are resolved against `base_dir`; `bots_dir` is derived in code as `base_dir/bots` and is not written in the config. A relative `base_dir` or an absolute `jar` fails at startup.
* `[discord]`: `token` must not be empty. Omit `guild_ids` or set it to `[]` for global registration (propagation can take up to an hour). A nonempty array such as `["123456789012345678"]` selects servers: each ID must be a nonzero, unique decimal u64 string, and the bot must be invited to every server. Server commands are normally available immediately. Restart after changing it and have an admin run `/sync-commands`; see [commands](../commands/commands-en_us.md#15-registration-and-contexts).
* `[admins]` may be empty (then there are no admins and everyone manages their own bots), but a key must be a plain numeric Discord user ID and a name cannot be blank. A mistake in either silently breaks permissions, so it is rejected at startup. Use `/whoami` to get your own user ID.
* `[i18n]`: `language` must be a supported value, namely a constant name from FishingBot's own `i18n.Language` (this project implements `ENGLISH` and `CHINESE_SIMPLIFIED`; upstream values such as `CHINESE_TRADITIONAL` and `FRENCH` are rejected because this project does not provide those texts). The valid values are defined in one place, `text::Language`, and the error lists them all. The value must match exactly (case sensitive, like the JAR's enum names; surrounding whitespace is ignored). The example config uses `ENGLISH` (change it to `CHINESE_SIMPLIFIED` for a Chinese JAR deployment); both languages are compiled into the binary, so changing the line and restarting is enough.
* `[log] debug` decides whether framework logs are collected too. With `false`, this project's records start at `INFO` and third-party crates (serenity, poise) at `WARN`, because they print `INFO` on every heartbeat and would drown the important lines (startup, commands, replies); with `true`, both go down to `DEBUG` (fully open; finer `TRACE` records are still not captured).
* `[fishingbot]`: the whole table is required (the values can be copied from the example config), and invalid values are reported at startup with the field named. A wrong value otherwise only makes the JAR **run quietly with its defaults** (measured: an unknown enum value falls back silently, and an unrecognised version is probed by the JAR itself), so rejecting it at startup is clearer. Rejected values: an empty `server_ip`, a zero `server_port`, a `default_protocol` that is neither `AUTOMATIC` nor version-shaped (`1.20`, `1.20.4`), an empty string inside `start_text`, and either `announce_type_*` outside upstream's five values (`ALL`, `ALL_BUT_FISH`, `ONLY_ENCHANTED`, `ONLY_BOOKS`, `NONE`). Online mode, offline usernames, automatic cast rules, realms and GUI entries are **not exposed**: they use the template's values (this project runs in online mode, and every account logs in through Microsoft).

## 5. Validation (`validate`)

Validation is split by table: each TOML table has a `validate_<table>` function, and `validate()` contains only those calls, in startup order; `[log]` has only the boolean `debug` switch and no validator.

The language name is checked during parsing; other fields and built-in parameters are validated here. The config is published only after all checks pass, and errors use English `text::bootstrap` messages.

It covers the following three basic categories of problem (see the module docs of `validate.rs`):

* Required entries must not be empty: `ping.default_ping_host`, `discord.token`, `keywords.*`, `text.help`, `runtime.screen_prefix`, `runtime.token_file`, `runtime.bot_logs_dir` (the last three must also contain no spaces or slashes and must not be `.` or `..`), `raw_game_logs.discard_path`, and `raw_game_logs.dir_name` / `raw_game_logs.file_name` (these two must be plain names as well).
* Numeric ranges: `ping.default_ping_port` must not be 0; `runtime.max_autocomplete_choices` must be between 1 and 25 (Discord's hard limit on suggestions); `startup_queue.max_batch_size`, `startup_queue.bot_login_interval`, `startup_queue.batch_login_interval`, `startup_queue.max_pending_requests`, `ping.timeout`, `runtime.max_reply_log_lines`, `runtime.max_reply_chars`, `timing.watchdog_window` and `timing.check_grace` must be greater than 0 (a watchdog window of 0 degrades the decision and can never prove "still connected"; the probe interval and the first-handshake timeout are derived from that window, so the "probe must not be slower than the window" rule no longer exists).
* Strings that get spliced into a command or path: `runtime.java_opts` forbids `'`, `"`, backtick, `$`, `;`, `&`, `|`, `<`, `>` and newlines (it is inlined unquoted); `keywords.filter_regex` must not contain a single quote (it is inlined into `grep -v -E '...'`); `paths.jar` and `paths.fishingbot_config_file_name` must not contain one either.

The `[anti_afk]` values and screen input action syntax are also validated at startup; this does not check whether screen is installed or target sessions exist.

Two more checks belong to "is the environment ready" and also fail at startup: `paths.base_dir` must be a directory and `paths.jar` must be a file. If either is wrong, the failure only appears when a session is launched, with a much less clear message.

Everything else is out of scope: timing values, address selection and whether a name may contain Chinese characters are left to the user's judgement. `[discord]` checks the token and parses `guild_ids`; `[log]`'s `debug` is a boolean switch with no rule; `[timing]` has two (`check_grace` greater than 0 and the gateway window greater than 0).

## 6. The template (`template`)

* The template is the repository root's `minecraft-fishingbot-manager-for-discord.conf.example.toml`, compiled into the binary with `include_str!`, so a release does not need to carry an extra file.
* The generated template is identical to the example file. A unit test checks every table and field in the example (a missing field would make a user who copies the template fail to start) and checks that every key is documented here.

## 7. Why the config is split into tables (instead of one flat struct)

In code there is one `Config` plus one struct per TOML table (`Config::ping`, `runtime`, `timing`, ...). This is not an extra layer; it keeps the name identical in three places:

* the key in the config file: `bot_logs_dir` under `[runtime]`;
* the way the code reads it: `config::Config::get().runtime.bot_logs_dir`;
* a startup validation error: `config item runtime.bot_logs_dir must not be empty`.

Flattening the structs into one large struct of concrete fields requires deciding what happens to the TOML tables; none of the three options is worthwhile:

* Flatten the file as well (`runtime_logs_dir = "logs"`): the file still matches the struct one to one, but every existing deployment must rewrite its config (this project runs with `deny_unknown_fields`, so the old form fails to start), and `[admins]` (user ID to name) is a map, so it cannot be flattened into "one concrete setting each". Field names would also clash (`log.debug` and `runtime.debug`, for example) and need prefixes, after which file keys and field names no longer match and validation errors cannot reuse the names from the file.
* Keep the file split and flatten the structs: serde cannot do this in that direction. `#[serde(flatten)]` means "spread a struct into the file"; spreading `[runtime]` into a flat struct would require a hand-written path-based lookup layer, which adds code rather than reducing it, and field-level errors would degrade into hand-written strings that drift away from the file.
* Generate a "flat view" with a macro: this keeps both the file and the structs, but the per-field doc comments become macro arguments and `cargo doc` no longer shows the current explanations, which is a poor trade-off for a project where the config is the documentation.

The conclusion is to keep the tables: the dotted layer after `config::Config::get()` is an acceptable cost, and it provides exactly "file key name equals struct path equals validation error". "Read once at startup, then read fields directly" also holds: `Config::load()` writes the config into a `OnceLock` at the very beginning, and every later `config::Config::get()` is a memory read with no I/O.

## 8. Related documents

* All documents: [docs index](../../index-en_us.md).
* Who generates the game-side `config.json` and where its fields come from: the `fishingbot_config` section of
  [runtime-en_us.md](../runtime/runtime-en_us.md).
* How config entries reach the commands: [commands-en_us.md](../commands/commands-en_us.md).
* Config invariants (startup validation, the template shipping with the repository):
  [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 2.
