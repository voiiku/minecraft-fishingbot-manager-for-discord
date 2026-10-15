# The config file
[简体中文](config-zh_cn.md) | **English**

* Code: `src/config.rs` (the runtime `Config` and its accessors) and `src/config/{schema,parse,validate,template}.rs`
  (the tables and the file shape of `[paths]`, parsing, validation, the template)
* File: `minecraft-fishingbot-manager-for-discord.conf.toml` next to the executable (the name is fixed by
  `CONFIG_FILE_NAME` and cannot be changed)
* It deliberately ignores the working directory and reads no environment variables: put the binary and the config
  together at deployment and it cannot go wrong, and hosting it under systemd needs no extra settings.
* The file holds the bot token; `chmod 600` is recommended.

## 1. Load flow (the very beginning of startup)

1. `config::Config::get_or_init()` locates the config file path.
2. When the file does not exist, one is generated from the repository's
   `minecraft-fishingbot-manager-for-discord.conf.example.toml`: writing the template and printing the note both happen
   inside this call, and the process then exits right there with code 0, without touching Discord. When the file does
   exist, the path it found is printed to the terminal before continuing (no logger is installed at this point, so the
   terminal is the only place; the `config loaded: ...` line in the program log comes from `ProcessLog::get_or_init`).
3. When the file exists, its text is read. `Config::from_toml_text` parses the TOML into a value tree, recursively trims
   every string value and maps the result straight into the runtime `Config` with Serde's `Deserialize` implementation
   (the only cleanup in the whole chain); the three keys of `[paths]` are expanded into paths rooted at `base_dir` by
   `Paths`'s own `Deserialize` implementation. Parsing and conversion do not apply every configuration rule.
4. `validate` checks field values, ranges and the runtime environment (for example required values, port ranges, and
   whether the configured directory and JAR exist).
5. Any failing step exits with an error shaped as "config entry + field name + requirement"; the process never runs
   with a bad value.
6. On success the config is written into a `OnceLock`: read once at startup, kept in memory. Editing the config file
   while the process runs does not affect it; restart the process to make the new config take effect.

Surrounding whitespace of string values is removed in `Config::from_toml_text`: a stray space or an indented line break never reaches
the code, so everyone reading the config (`main.rs`, the business layer and similar entry points) passes the value
as-is and never calls `trim()` itself. To really keep surrounding whitespace, write the escape (in TOML, `"\u0020"`).

Deserialization relies on `#[derive(Deserialize)]` on `Config` and its field types. Serde maps TOML table and key
names to the corresponding struct fields; a type mismatch, a missing required field or an unknown field returns a
parse error. There are **no in-code defaults**: every table and every field must be present in the file, so a missing
key or a typo fails at startup instead of being silently filled in. Path fields are converted by `Paths`'s own
`Deserialize` implementation into runtime paths rooted at `base_dir` (`bots_dir` is derived in code); the later `validate` step handles the remaining value rules.

After the config is read and before anything else, the process entry also **generates the game-side config** from
`[fishingbot]` (FishingBot's `conf.json`, at the path from `paths.fishingbot_config_file_name`): the old file is
deleted first and a new one written. A failure there is handled under the same "the config is wrong" rule as
everything else (an `ERROR` record and exit code 1). Why it is a generated file, and what goes into it, is section
3.2.

## 2. Public interface

These are all associated functions on `Config` (the module has no free accessor functions; the runtime `Config` is
defined in this file), so the process entry is a single line, `let settings = config::Config::get_or_init();`:

* `Config::get_or_init()`: fetch the config, the process entry's only entry point. The first call locates, reads,
  parses and validates it; later calls return that same `&'static Config`. A generated template (with its note already
  printed) exits right there with code 0, and an invalid config reports the error and exits with code 1 - both failure
  paths end the process inside the function, so what it returns is always a usable config and the caller never checks a
  result.
* `Config::get()`: the loaded `Config`.
* `Config::config_file_path`: the config file path this run uses (computed once at load time from the executable's
  location, and kept in the struct).
* `Config::admins()`: the admin table (short for `Config::get().admins`).
* `Config::install_dir()`: the deployment directory (where the executable is). The config and the program log live there.

`Config::get()` panics while nothing is loaded, with the text from `text::bootstrap::CONFIG_NOT_LOADED` (an internal
invariant message: no language is installed yet, so it is fixed English). The first thing the process entry does is
`Config::get_or_init()`, so a panic here means the call order is wrong. The config file path is a field of `Config`
(computed once at load time), so "is it loaded" still has exactly one answer: that single `OnceLock<Config>`.

## 3. Table structure

The order matches the example config, arranged as "whatever a new deployment touches first comes first": credentials,
paths, the game-side config (`[fishingbot]`), then the three tables you fill in against your game server or jar
(keywords, echoes, language), then the runtime and timing values that are usually left alone, and finally the purely
optional switches and display text. The example config's explanatory comments are mostly English; the
complete Chinese original of `[text].help` is also kept in comments above that setting for Chinese readers.
A few lines start with `# CHINESE_SIMPLIFIED:`; these tell Chinese deployments what value to use (only
the two `[command_echo]` echoes and `[i18n] language` need them), see
[i18n-en_us.md section 1](../i18n/i18n-en_us.md).

* `[discord]`: `token`. The bot token. The only credential, required.
* `[paths]`: `base_dir` (absolute), `jar`, `fishingbot_config_file_name` (relative). The data root and the shared
  files.
* `[fishingbot]`: `server_ip`, `server_port`, `default_protocol`, `auto_reconnect`, `auto_reconnect_time`,
  `start_text`, `announce_type_console`, `announce_type_chat`. Exactly those keys of the game-side config
  (FishingBot's `conf.json`) that **this project exposes**: that file is generated from them at startup (section
  3.2). The whole table is required; copy the values from the example config that ships with the repository.
* `[admins]`: `"Discord user ID" = "name"`. The admin list; keys must be plain numeric user IDs (snowflakes).
* `[keywords]`: `online`, `filter_regex`. The only log keywords that differ per deployment and must be filled in by
  hand: the game server's join message, and the regular expression for spam lines to drop before writing
  (`grep -v -E`). There is deliberately **no** "process finished" keyword here: that marker was written into the log
  by an older wrapper script (`>>> 机器人进程已结束或被手动中止 <<<`), this project does not write it, and whether
  the process is gone is read from the screen session state (a `(Dead ...)` shell), so it needs no config. Why only
  two entries are left is section 3.1.

* `[command_echo]`: `pause_echo`, `unpause_echo`, `wait_ms`, `scan_lines`. Confirming that `/pause` and
  `/unpause` really ran: the first two are the echo line the JAR prints when it receives the command (upstream
  messages `command-pause-executed` and `command-unpause-executed`; fill in the ones for your jar's language - a
  Chinese jar prints `已暂停钓鱼` / `已恢复钓鱼`). After sending, wait `wait_ms` milliseconds and then look at the end
  of the newest log (only the last `scan_lines` lines) for those two sentences.
* `[i18n]`: `language`. The language the program runs in; it affects the Discord texts, the program log and error
  messages. The value is the constant name from FishingBot's own `i18n.Language`, and it is also what goes into
  `conf.json` as `misc.language` (the same value, not mapped), so the two never have to be aligned by hand. This
  project implements `ENGLISH` (the default) and `CHINESE_SIMPLIFIED`; changing it and restarting is enough, no
  rebuild. See section 3.1.
* `[runtime]`: `java_opts`, `screen_prefix`, `token_file`, `logs_dir`, `max_log_lines`, `max_reply_chars`,
  `max_autocomplete_choices`. The runtime parameters of a single bot. `logs_dir` holds our logs only (FishingBot's
  own raw log sits beside it under `[raw_game_logs] dir_name` as `[raw_game_logs] file_name`, pointing at
  `discard_path`, see [utils-en_us.md section 7](../utils/utils-en_us.md#7-the-two-log-lines)); `max_reply_chars` is
  the length limit of a single reply (Discord hard-limits a message to 2000 characters, and an overlong one simply
  fails to send); `max_autocomplete_choices` caps the autocomplete suggestions for the bot-name options (1..=25,
  Discord's hard limit).
* `[startup_queue]`: `max_batch_size`, `login_interval_ms` and `max_pending_requests` set the global FIFO queue's
  maximum bots per batch, the minimum gap between two sign-ins and the maximum waiting requests (excluding the active
  request). The table and all of its fields are required and every value must be greater than zero.
  `login_interval_ms` is the only knob that decides the pace: bots inside a batch are spaced by it, and the wait between
  batches is it minus `timing.settle_per_bot_ms` times 2 times the batch size. Microsoft rate-limits sign-ins per egress
  IP, see [troubleshooting section 12](../../troubleshooting/troubleshooting-en_us.md) for the numbers.
  `/start`, `/startall`, `/restart` and `/restartall` share this queue; `/bot login` does not use it. The dispatcher
  remains alive while idle, closes a partial batch when the queue becomes idle, and drops pending work if the process
  restarts; the login interval is kept across idle either way.
* `[timing]`: `settle_per_bot_ms`, `stop_grace_ms`, `stop_settle_ms` and `watchdog_window_ms`. These per-flow waits are
  in milliseconds. `settle_per_bot_ms` is the wait per bot before checking the session (the check runs twice, so the
  total wait is twice the number of bots times it, and it is also the part the wait between batches subtracts);
  `watchdog_window_ms` is the gateway watchdog's judging window, also used as the wait before rebuilding the
  connection; the REST probe interval (a third of it) and the first-handshake timeout (twice it) are derived from it
  instead of being separate keys, see [watchdog-en_us.md](../watchdog/watchdog-en_us.md).
* `[raw_game_logs]`: `dir_name`, `file_name`, `discard_path`. The convention for FishingBot.jar's own raw log (its
  directory name, its file name, and where the contents are thrown away). It follows the JAR; the example config
  ships `raw`, `log0.log` and `/dev/null`.
* `[ping]`: `default_ping_host`, `default_ping_port`, `timeout_ms`. The default target and timeout of `/ping` when it
  is called without arguments.
* `[log]`: `debug`. The level switch of the program log. With `false` (the default) our own records start at `INFO`
  and third-party crates (serenity, poise) at `WARN`; with `true` both go down to `DEBUG` (turning debug on means
  hearing everything). This is the only level switch (environment variables are no longer read); details in
  [process-log-en_us.md](../process-log/process-log-en_us.md).
* `[text]`: `help`. The full output of `/help` (a multi-line string, so the wording can be changed without a
  rebuild).

### 3.1 Where the keywords live: `[keywords]`, `[command_echo]` and `[i18n]`

* Strings that differ per deployment and only the deployer can supply go into `[keywords]`: the game server's own
  join message (`online`) and the spam noise to drop on your own server (`filter_regex`).
* The `/pause` and `/unpause` echoes go into `[command_echo]` (`pause_echo`, `unpause_echo`): the jar prints them
  too, but they only exist in a build carrying those two commands (the upstream build has neither), so putting them
  in the config lets a different build be configured in place, without a rebuild.
* The reconnect message's prefix and suffix stay in the program (`src/text/*.rs`, the `jar_*` methods) and follow
  `[i18n] language`: it changes only with the jar's language and is the same for every deployment, so copying it
  would only add one more place to get wrong. That language is also the value we write into `conf.json` as
  `misc.language` (section 3.2), so the jar is guaranteed to speak the same language - aligning the two by hand used
  to be a job, now they cannot disagree.
* "Process finished" needs no keyword: an older wrapper script used to write an end marker into the log with
  `java ... | tee bot.log; echo marker >> bot.log`, while this project starts the session itself and does not write
  it, so the check reads the screen session state instead (a `(Dead ...)` shell, see [utils-en_us.md](../utils/utils-en_us.md)). That is steadier than copying a marker: it does not depend on the jar version, the script or the
  language.

The criterion is "who decides this string, and does it have to change with the build": what your environment decides
goes into `[keywords]`, what the jar's patched build decides goes into `[command_echo]`, what the jar's language
decides follows `[i18n]`; and a judgement that is not about a string at all (is the process gone) reads the session
state directly.

### 3.2 The game-side config (`conf.json`) is a generated file

FishingBot only understands JSON (`-config <path>`) and has **no command-line argument that overrides a single
setting** (its startup arguments are only `-nogui`, `-help`, `-logsdir`, `-config`, `-refreshToken` and
`-onlyCreateConfig`), so "merging its `conf.json` into the TOML" can only ever be **one-way generation**: the TOML
is the single source of truth, and that JSON is written out when the process starts (the code is in
`utils/fishingbot_config.rs`).

How it is generated: **take the template shipped with the repository as the base, and override only those keys in
`[fishingbot]`.**

* The template is the repository's `fishingbot-config.example.json` - a **complete** FishingBot config (taken from
  the default file upstream generates, with the defaults tuned to this project's taste: console and chat
  announcements off, `/home fishing` as the default cast starter, a longer `auto-reconnect-time`, and so on). It can
  be compiled into the binary with `include_str!`, so it does not have to travel along at deployment.
* The keys it overrides are the managed list in `utils/fishingbot_config.rs` (ten dotted paths), one to one with the
  fields of `[fishingbot]`; among them `misc.language` is written straight from `[i18n] language` (whose value is the
  jar's own constant name, see `text::Language::from_name` / `name`), so there is no chance to fill it in wrongly and
  no "keep the two aligned" job.
* Every other key is left as the template has it, so what gets written is **always a complete config** - it never
  relies on the jar filling missing fields in. (Measured on v2.13.15: it does fill in missing fields and write the
  file back, but then what gets added is the jar's decision, and every process start leaves one more
  `Your configuration was incomplete. The missing fields were added!` line in the bot log.)
* **The old file is deleted first**: it is a generated artefact, so hand edits do not survive the next start - change
  `[fishingbot]` instead, or change the template to change a default.

When upstream changes the config structure (adds, renames or removes a key): take a default `conf.json` generated by
the new jar, copy it over the template and run `cargo test` - `template_holds_every_managed_path` goes red first (a
managed path is not in the template), and following its message you sync `managed_values` and `[fishingbot]`. That is
the **only** maintenance point that has to follow upstream.

`[fishingbot]` is the single entry point for "what you want"; the `conf.json` on disk is "what FishingBot is actually
reading" (and what `/show botconfig` sends). Note also: changing `[i18n] language` changes the jar's language, and the
two `[command_echo]` echoes (`pause_echo`, `unpause_echo`) are printed by the jar, so they have to be changed to the
matching language too.

## 4. Field rules

* Every field is required: there are no in-code defaults, so omitting one (or misspelling it, caught by
  `deny_unknown_fields`) fails at startup. The example config that ships with the repository has every key filled in,
  so it can be copied as the starting point.
* Path rules (`paths`): `base_dir` must be absolute; `jar` and `fishingbot_config_file_name` must be relative and are
  resolved against `base_dir`; `bots_dir` is derived in code as `base_dir/bots` and is not written in the config. A
  relative `base_dir` or an absolute `jar` fails at startup.
* `[discord]`: `token` is the only credential and an empty one can never connect (startup fails); there is no other
  field, and slash commands are always registered globally (up to an hour to propagate on Discord's side).
* `[admins]` may be empty (then there are no admins and everyone manages their own bots), but a key must be a plain
  numeric Discord user ID and a name cannot be blank. A mistake in either silently breaks permissions, so it is
  rejected outright. Get your own user ID with `/whoami`.
* `[i18n]`: `language` must be a supported value - that is, the constant name from FishingBot's own `i18n.Language`
  (this project implements `ENGLISH` and `CHINESE_SIMPLIFIED`; upstream values such as `CHINESE_TRADITIONAL` and
  `FRENCH` are rejected, because we have not written those texts). The valid values are defined in one place,
  `text::Language`, and the error lists them all. The value must match exactly (case sensitive, like the jar's enum
  names; surrounding whitespace is ignored). The example config ships `ENGLISH` (the same as the default language in
  code; change it to `CHINESE_SIMPLIFIED` for a Chinese jar deployment); both languages are compiled into the binary,
  so changing this line and restarting is enough.
* `[log] debug` is "do we also listen to the frameworks". With `false` (the default) our own records start at `INFO`
  and third-party crates (serenity, poise) at `WARN`, because they print `INFO` on every heartbeat and would drown the
  important lines (startup, commands, replies); with `true` both go down to `DEBUG` (turning debug on means hearing
  everything; `TRACE`, which is finer than `DEBUG`, is out of reach).
* `[fishingbot]`: the whole table has defaults (the same as the template shipped with the repository), and anything
  wrong is reported at startup with the field named - because a wrong value only makes the jar **run quietly with its
  defaults** (measured: an unknown enum value falls back silently, an unrecognised version is probed by the jar
  itself), and catching it here is cheaper. What gets caught: an empty `server_ip`, a zero `server_port`, a
  `default_protocol` that is neither `AUTOMATIC` nor version-shaped (`1.20`, `1.20.4`), an empty string inside
  `start_text`, and either `announce_type_*` outside upstream's five values (`ALL`, `ALL_BUT_FISH`, `ONLY_ENCHANTED`,
  `ONLY_BOOKS`, `NONE`). Online mode, offline usernames, automatic cast rules, realms and the GUI entries are **not
  exposed**: they keep the template's values (this project runs in online mode, and every account logs in through
  Microsoft).
* `[command_echo]`: `wait_ms` is how long to wait after sending a command before looking at the log (too short and
  the echo is not there yet); `scan_lines` is how many lines at the end to look at (a larger number tolerates more
  noise - the echo can be pushed up by a few unrelated lines - at the cost of reading a few more lines). Both must be
  greater than 0: waiting 0 means looking the instant the command is sent, before the echo can reach the log.

## 5. Validation (`validate`)

Validation is split per table: every field of `Config` (that is, every TOML table) gets one `validate_<table>`
function, and `validate()` holds nothing but those calls, in start-up order; `[log]` only has the boolean `debug`
switch and no validator.

It only looks at the three kinds of problem that make the program fail to run (see the module docs of `validate.rs`):

* Required entries must not be empty: `ping.default_ping_host`, `discord.token`, `keywords.*`,
  `command_echo.pause_echo`, `command_echo.unpause_echo`, `text.help`, `runtime.screen_prefix`, `runtime.token_file`
  and `runtime.logs_dir` (the last three must also contain no spaces or slashes and must not be `.` or `..`),
  `raw_game_logs.discard_path`, and `raw_game_logs.dir_name` / `raw_game_logs.file_name` (these two must be plain names
  as well).
* Numeric ranges: `ping.default_ping_port` must not be 0; `runtime.max_autocomplete_choices` must be between 1 and 25
  (Discord's hard limit on suggestions); `startup_queue.max_batch_size`, `startup_queue.login_interval_ms`,
  `startup_queue.max_pending_requests`, `ping.timeout_ms`, `runtime.max_log_lines`, `runtime.max_reply_chars`,
  `timing.watchdog_window_ms`, `timing.settle_per_bot_ms`, `command_echo.wait_ms` and `command_echo.scan_lines` must be
  greater than 0 (a watchdog window of 0 degrades the decision - it can never prove "still connected"). The probe
  interval and the first-handshake timeout are derived from that window, so the "the probe must not be slower than the
  window" rule no longer exists.

The two `command_echo` echoes must also differ, and neither may contain the other (complete equality included):
otherwise that log line cannot be attributed and the decision always leans towards whichever is checked first.

* Strings that get spliced into a command or a path: `runtime.java_opts` forbids `'`, `"`, backtick, `$`, `;`, `&`,
  `|`, `<`, `>` and newlines (it is inlined unquoted); `keywords.filter_regex` must not contain a single quote (it is
  inlined into `grep -v -E '...'`); `paths.jar` and `paths.fishingbot_config_file_name` must not contain one either.

Two more checks belong to "is the environment ready" and also fail at startup: `paths.base_dir` must be a directory
and `paths.jar` must be a file. Getting those wrong would only fail when a session is launched, and with a far less
clear message.

Nothing else is policed: how large a timing value should be, which address to use, and whether a name may contain
Chinese characters are all the user's judgement. `[discord]` has only one rule (the token must not be empty); `[log]`'s
`debug` is a boolean switch with no rule; `[timing]` has two (`settle_per_bot_ms` greater than 0 and the gateway window
greater than 0); `[command_echo]` has two (both echoes non-empty and neither containing the
other, and both durations greater than 0).

## 6. The template (`template`)

* The template is the repository root's `minecraft-fishingbot-manager-for-discord.conf.example.toml`, compiled into the
  binary with `include_str!`, so a release does not have to carry an extra file.
* The generated template is character for character the example file. A unit test checks every table and field that
  appears in the example (a missing field would make a user who copies the template fail to start) and checks that
  every key is written down in this document.

## 7. Why the config is split into tables (instead of one flat struct)

In code there is one `Config` plus one struct per TOML table (`Config::ping`, `runtime`, `timing`, ...). Not to have
one more layer, but to keep the name identical in three places:

* the key in the config file: `logs_dir` under the `[runtime]` table;
* the way the code reads it: `config::Config::get().runtime.logs_dir`;
* a startup validation error: `配置项 runtime.logs_dir 不能为空`.

Before flattening the structs into one big struct of concrete fields, you have to decide what happens to the TOML
tables, and none of the three ways pays off:

* Flatten the file along with them (`runtime_logs_dir = "logs"`): the file still matches the struct one to one, but
  every existing deployment has to rewrite its config (this project runs with `deny_unknown_fields`, so the old form
  fails to start outright), and `[admins]` (user ID to name) is a map anyway, so it cannot be flattened into "one
  concrete setting each". Field names would start clashing too (`log.debug` against `runtime.debug`, say) and would
  need prefixes, after which file keys and field names no longer match and validation errors cannot copy the names
  from the file either.
* Keep the file split and flatten the structs: serde cannot do it in that direction. `#[serde(flatten)]` means
  "spread a struct into the file"; to spread `[runtime]` into a flat struct you would have to write your own
  path-based lookup layer, which is more code rather than less, and field-level errors would degrade into hand-written
  strings that drift away from the file.
* Generate a "flat view" with a macro: that keeps both the file and the structs, but the per-field doc comments
  become macro arguments and `cargo doc` no longer shows the explanations we have now - a bad trade for a project
  where the config is the documentation.

So the conclusion is to keep the tables: the dotted layer after `config::Config::get()` is an acceptable cost, and what it
buys is exactly "file key name equals struct path equals validation error". What is more, "read once at startup, then
read fields directly" already holds: `Config::get_or_init()` writes the config into a `OnceLock` at the very beginning, and every
later `config::Config::get()` is a memory read with no IO.

## 8. Related documents

* All documents: [docs index](../../index-en_us.md).
* Who generates the game-side `conf.json` and where its fields come from: the `fishingbot_config` section of
  [utils-en_us.md](../utils/utils-en_us.md).
* How config entries reach the commands: [commands-en_us.md](../commands/commands-en_us.md).
* Config invariants (startup validation, the template shipping with the repository):
  [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 2.
