# CONTEXT: project overview and maintenance constraints
[简体中文](CONTEXT-zh_cn.md) | **English**

This file provides the minimum necessary context for maintainers and automated sessions: the first section defines the
project, and the remaining sections list the hard constraints. Read it before modifying code and update it when a new
convention is established.
Markers follow the code-comment convention (see
[architecture-en_us.md section 5](../mods/architecture/architecture-en_us.md)): a line beginning with `?` requires
confirmation in a real run; after confirmation, remove the marker and retain the conclusion. Section 8 lists deployment
checks, deferred items and the assessment of the batch-operation reply design.

Entry points: [README-en_us.md](../README/README-en_us.md), [tutorial-en_us.md](../tutorial/tutorial-en_us.md) and
[troubleshooting-en_us.md](../troubleshooting/troubleshooting-en_us.md). Module guides are under `mods/`, the upstream
JAR reference is under `refs/`, and project constraints and TODOs are in `CONTEXTS/`. The root `README.md` is the default
English README; `docs/README/README-en_us.md` is a symlink to it, checked by an integration test.

## 1. Project overview

* Display name: **Minecraft FishingBot Manager for Discord**. Cargo package and executable slug:
  `minecraft-fishingbot-manager-for-discord`. This is an unofficial Rust controller for external FishingBot instances;
  Java is only required by the separately installed `FishingBot.jar`.
* License: MIT, in the root `LICENSE`. Binary releases must include `LICENSE` and a third-party notices document
  regenerated and checked for that release's dependency set and target; an SBOM is optional. The generation policy and
  template are `about.toml` and `about.hbs`; regenerate and review the notice for each release.
  Do not bundle FishingBot: it is a separate
  GPL-3.0 program that this project only launches with `java -jar`, without modifying or linking it; if you really do
  put the jar and the binary into one distribution, the jar part of that distribution has to be handled under GPL-3.0.
* FishingBot must support `/status`, `/pause` and `/unpause`. Upstream merged status queries and plans to add pause commands. Pause state is queried, not stored by the manager.
* What it does: use Discord slash commands to manage [FishingBot](https://github.com/MrKinau/FishingBot)
  auto-fishing instances for many Minecraft accounts in bulk.
* Tech stack: Rust (edition 2024), [serenity](https://github.com/serenity-rs/serenity) 0.12.5,
  [poise](https://github.com/serenity-rs/poise) 0.7.0, `screen`, the JVM. The default target is
  `x86_64-unknown-linux-musl`, aimed only at 64-bit Linux deployments (32-bit is not planned).
* No extra server-side component: the process connects straight to the Discord gateway with the bot token from the
  configuration, so no protocol server such as NapCat is needed, and no state file is written into the deployment
  directory.
* Process model: one bot equals one `screen` session (the session name is `runtime.screen_prefix` plus the bot name).
  One process equals one Discord application identity, and the only commands are slash commands (there are no
  `!prefix` commands, see section 7 for the reason).
* If Discord cannot be reached the bots are stopped and the connection is rebuilt in the same process (a deliberately
  kept behaviour): once this process is judged to have lost contact with Discord, all bots are stopped, and after a
  judgement window the connection is rebuilt in the same process without exiting it. The reason is in section 3: while
  Discord is unreachable we cannot control the bots, yet they still occupy the game accounts, so real players cannot
  log in.

## 2. Global constraints (verify compliance before modification)

### 2.1 Mandatory coding rules

1. The framework boundary: use its connection and command framework, not its logging or configuration. Use: serenity's
   `Client`, gateway and `Http`, poise's `Framework`, `#[poise::command]`, `FrameworkOptions`, `ctx.say` and
   `ctx.defer*`. Do not use: the framework's own logging channel - all logging goes through our own `process_log.rs`
   (a self-implemented `log::Log`). poise's `tracing` `log` feature forwards framework logs to this channel; real runs
   have confirmed the records appear in the program log.
   There is only one configuration file, `minecraft-fishingbot-manager-for-discord.conf.toml` (no environment
   variables at all).
2. Avoid writing files outside the deployment directory as far as possible. Every generated file lives either in the
   deployment directory (the directory of the executable: the configuration, and the program log under `logs/`) or in
   a bot directory (`paths.base_dir/bots/<Discord user ID>/<bot name>`). There are no exceptions today: the framework
   writes no state file and does not look at the working directory.
3. Documentation first. Write the expected behaviour and the external API in this file or under `docs/` before
   implementing; the implementation has to match the documentation item by item, and it is not acceptable for the
   documentation to describe something the code does not do, or for the code to do something the documentation does
   not mention.
4. One file, one job. A file name should say directly which module's which kind of functionality it is; one function
   should not do two things such as "create a resource" and "start a process" at once, and the steps have to be split
   explicitly (see the start-up pipeline in section 4). A thin wrapper with a single caller is kept or inlined
   by whichever reads better: if its name only restates what the call site would already say, inline it; if it
   names a domain concept, or hides a mutex plus its panic message, or is a predicate or an enum mapping, keep it.
   So do thin functions that mirror a structure: config validation keeps one function per table, with `validate()`
   only calling them in order (see section 5 of the config document).
5. Naming: functions use verb phrases (`start_bot`, `create_bot_log_file`); things that return a value use nouns
   without a `get_` prefix (`latest_log_path`, `token_ready_bot_names_with_screen_sessions`); predicates use `is_` and `has_`; conversions
   use `to_`, and a conversion that can fail returns `Result`, where the `Err` is text that can be replied to the
   user directly (`raw_bot_name.to_clean_bot_name()`, `from_dir_name_to_owner_id`); when constructing a type, use
   `new` when the facts are known and `find_*` when they have to be looked up (`BotDirEntry::new` versus
   `BotDirEntry::find_by_name`). Variable names carry context and shape: lists are called `*_names`, `*_entries` or
   `*_paths`, counts are called `*_count` (a bare `count`, `list`, `data` or `arr` is not allowed), and home-made
   abbreviations are not used. The regular forms that are allowed: unit suffixes `_ms` and `_secs`, `|e|` (error)
   inside a one-line closure, and compounds such as `bot_dir_path`. IDs are always `*_id`: the caller is `caller_id`
   (`u64`, the Discord user ID) and the directory owner is `owner_id`; do not use legacy platform-specific ID names.
6. Comments: the module header `//!` says what this file is responsible for and why it is written that way; a function
   `///` describes the external behaviour and the boundaries; an inline `//` only explains why, without restating the
   code. No filler and no "to be added later". The style model is `src/runtime/bot_instances/bot_dirs.rs`: a one or two line module
   header that states the responsibility and the key convention, and function comments that are plain short sentences
   saying what it does, who calls it and where the boundaries are, naming the concrete functions where possible (for
   example "used by `/start` and `/stop`"); mark doubts or better approaches in place, where `?` means undecided or
   questionable and `!` means it should be changed. Do not write long essays about "why it is designed this way". The
   example configuration `minecraft-fishingbot-manager-for-discord.conf.example.toml` is the only exception: its comments are
   in English (aimed at people editing a config from the template), and only the few entries whose value follows the
   language and must be changed for a Chinese deployment carry an extra `# CHINESE_SIMPLIFIED:` hint line, see
   [docs/mods/i18n/i18n-en_us.md section 1](../mods/i18n/i18n-en_us.md). Command and option descriptions live only in
   the language tables and are validated before registration; see
   [command text validation](../mods/commands/commands-en_us.md#171-command-text-validation).
   Wherever it matters who validates something
   or who is responsible, name the layer explicitly (command layer, command entry layer, business layer, runtime
   primitive), for example "permissions are validated in the business layer: this function is called by
   [`operations::all_bots_control::start_all_bots`], which checks `require_admin(caller_id)` at preparation" - writing only "validated uniformly by X" does not show the layer.
7. Who cleans arguments: Discord gives a structured option, but the option's value is still the raw text the user
   typed. The command layer only reads the value, checks numbers and dispatches, passing it through unchanged
   (parameter names start with `raw_`), and whoever really needs it cleans it once: bot names go through
   `ToCleanBotName::to_clean_bot_name` (called when `BotDirEntry` builds an entry; an illegal name returns `Err`
   rather than being filtered out) and in-game command fragments go through `utils::console_command::ToCleanBotCommand`. The same text
   is cleaned only once in the whole chain, and a second `trim()` must not appear anywhere. The config side follows the
   same rule: `config::Config::from_toml_text` trims string values once, and config readers pass the value as-is. Two rules never
   change:
   the single source of truth for bot names is `utils::bot_name`; checks that need business state (whether a token
   exists, whether a session is running, permissions and directory ownership, the `max_reply_log_lines` cap) always stay in
   the business layer. Numbers are the command layer's only conversion: the line count of `/log` is parsed and
   validated in the command layer and then turned into `usize`, the port of `/ping` likewise, and a failed validation
   replies with the usage hint on the spot (Discord's integer option is a signed 64-bit integer, so the option is
   declared as `i64`).
8. Read configuration only where it is needed, and only the few fields that are needed: functions with heavy logic
   that deserve a unit test take parameters (`launch_command_line` takes every value it needs as an argument); convenience
   functions with only one or two lines of logic read `config::Config::get().<table>.<field>` directly (for example
   `session_name` only reads `runtime.screen_prefix`, and `BotTokenStatus::from_dir` only reads `runtime.token_file`);
   `query_bot_status` selects the log and parses `/status` using the language table. No function takes a whole `Config` and then picks fields
   from it. The configuration is read once at start-up and then lives in memory, so a convenience function reading one
   field costs nothing extra, while a logic function taking parameters buys testability. See
   [docs/mods/runtime/runtime-en_us.md section 6](../mods/runtime/runtime-en_us.md) and
   [docs/mods/config/config-en_us.md section 7](../mods/config/config-en_us.md).

### 2.2 Invariant list (verify each item before modification)

* Stopping must be thorough: `/stop` goes through `runtime::bot_instances::stop::stop_single_bot_session`, `/stopall` and
  process shutdown after configuration is loaded go through `runtime::bot_instances::stop::stop_all_bot_sessions`, and the stop stage of a restart calls
  `runtime::bot_instances::stop::stop_bot_sessions` directly. All three paths ultimately reuse `stop_bot_sessions` (Ctrl+C,
  waiting, force kill, re-check), and an orphan JVM holding a game account may never be left behind. Before configuration
  is loaded, permanent errors exit without config-dependent cleanup; the example systemd deployment
  uses `KillMode=control-group` to clean up remaining processes.
* There are two logging lines, but only one bot log on disk:
  * bot log: `<bot dir>/<runtime.bot_logs_dir>/YYYY-MM-DD-HH-MM-SS.log`, created before the session is launched
    (`create_bot_log_file`), and inside the session it is appended through `grep -v -E` filtering plus `tee -a`; `/log`
    and `/show status` read it;
  * FishingBot's own log: `raw/log0.log` next to it, a symlink to `/dev/null`. That program refuses to start when it
    cannot create a log file, so it is given a target that is writable but stores nothing. Do not change this into
    keeping a raw log;
  * program log: `<deployment dir>/logs/YYYY-MM-DD-HH-MM-SS.log`, with a copy on the terminal, one per run, never
    rotated and never cleaned up, see [docs/mods/process-log/process-log-en_us.md](../mods/process-log/process-log-en_us.md);
  * the single source of truth for log file names is `utils::fs::create_timestamped_log_file` - do not assemble a
    timestamped file name anywhere else.
* Bot state is not stored across calls: connection and fishing state are parsed into `BotStatus` from a new `/status`
  response. Unavailable status is an issue but does not stop the session. A `(Dead ...)` session is reported as stopped.
* `/show status`'s table has no dedicated type: the rows are joined on the spot by
  `show::show_status::status_row()` (seven cells and `" | "`) and the header on the spot by
  `src/operations/queries/show_status.rs`, so the two sides must agree on the column order (the column names live in `text`, one set per
  language). The full pattern is in
  [docs/mods/commands/commands-en_us.md section 6.2](../mods/commands/commands-en_us.md#62-show-status).
* Replies must not overstate: `/start` uses the start verification result and answers "join confirmed", "not currently joined"
  or an issue; `/restart` confirms the old session stopped and reports the
  new session and `/status` verification result; `/stop` re-checks its stop flow on the spot, so it may answer
  "stopped successfully"; `/pause` and `/unpause` query `/status` after sending and report a timeout if no valid response
  arrives. A query confirms the current state, not what caused it; "not paused" does not mean actively fishing.
* An interaction has to be answered within 3 seconds, otherwise the user sees "the interaction failed". What we have to
  do (starting a JVM, running `screen -list`, waiting `[startup_queue] bot_login_interval` per bot, reading logs) usually takes far
  longer, so:
  * commands that trigger process operations or external probes (`/start`, `/stop`, `/restart`, `/pause`, `/unpause`,
    `/command`, the six `*all` commands, `/ping`, `/checkserver`, `/log`) always call `ctx.defer()` first; among them
    `/log` uses `ctx.defer_ephemeral()` **and every reply is explicitly `.ephemeral(true)`** (logs are sensitive
    content that only the caller may see; why defer alone is not enough is explained in
    [docs/mods/commands/commands-en_us.md section 3](../mods/commands/commands-en_us.md));
  * commands that are pure local text (`/help`, `/whoami`) do not defer and answer directly;
  * a reply after a defer consists of two messages (a Discord compatibility behaviour, spelled out in the official
    documentation): the first replaces the "thinking" placeholder and only the second is a new message. Long commands
    (the six `*all` ones and the single-bot start, stop and restart) are therefore sent as two messages: first an
    "executing /startall now, the full result follows when it finishes" progress line, so the user sees progress
    immediately, and after it finishes the full report from `Report::render()`. `/startall`'s first reply also states how
    many bots are in the execution snapshot, how many are already queued, the estimated duration and the result timeout.
    The second reply is sent as a new message under Discord's defer behaviour; re-check this in a live deployment if
    changing the response flow.
  * an interaction token lasts only 15 minutes: every reply after a defer has to be sent inside that window. When a
    batch start runs longer than that (many bots, a large `[startup_queue] bot_login_interval`) the followup always
    fails, so the final report of a long command goes through `discord_commands::say_final_report`: if it cannot be sent as a
    reply it is posted as an ordinary channel message (those are not bound to the interaction token) and a
    `LogMessages::report_posted_to_channel` line records why, together with the failure reason. The batch-operation reply
    design is assessed in section 8.1; the current behaviour is unchanged.
* Sensitive input has to go through a Modal and must not be a command parameter: every invocation of a slash command
  creates a message in the channel (that is exactly Discord's `CHAT_INPUT_COMMAND` message type), the parameter values
  live in that message, and `ephemeral` only covers replies, not the invocation itself. `/token` therefore collects it
  with a `#[derive(poise::Modal)]` pop-up (input arrives through the `MODAL` callback and never enters the channel), and
  the result is answered afterwards with a followup plus `ephemeral`. A modal has to be the first response to the
  interaction, so that command must not `defer` first (deferring first makes the modal fail). Follow this pattern for
  new commands that ask the user for sensitive content.
* A reply has to fit into one message: Discord's hard limit is 2000 characters per message. All long user-facing text
  goes through `Report` (`render()` uses `runtime.max_reply_chars`, which defaults to exactly 2000, truncating from the
  end by line and noting how many lines were omitted); an over-long message simply fails to send, and on the user's side
  it looks as if nothing happened, so never assemble an unbounded string and reply with it directly. The counting
  convention is `str::utf16_len()` (`encode_utf16().count()`), which is what `Report::render` closes on, not `chars()`
  (code points): a non-BMP character (an emoji) counts as 2 in UTF-16, and taking the stricter side is what guarantees
  that 2000 can be sent.
  ?Discord does not spell out how it counts "2000 characters", and taking the stricter side means no guessing. `/log` is
  not subject to this limit because it goes as an attachment.
* Every reply leaves one program-log line: the command layer never calls `ctx.say` directly, and every user-facing reply
  goes through `discord_commands::say_and_log` (the business layer through `discord_commands::log_reply`) to
  `LogMessages::command_replied`, which fixes in the program log who received what and when. When something goes wrong, look
  there first.
* `/log` is sent as an attachment and is no longer truncated: the last N lines are read and uploaded as an attachment
  (the file name is the log file name), while the body states the title, the line count and the file name; it uses
  `defer_ephemeral()` plus a reply with an attachment and an explicit `.ephemeral(true)`, so nobody else can see it.
  The attachment size limit is `Config::get().constants.max_attachment_bytes` (8 MiB by default, leaving headroom below
  Discord's ordinary 10 MiB limit; it is not a TOML setting); beyond that it is truncated by line and explained in the body.
* `keywords.filter_regex` must not contain a single quote (it is inlined into the `grep -v -E '...'` inside `bash -c`).
* The configuration is read only once, at start-up: changing it while running has no effect and it takes effect after
  restarting the process (hot reload is deliberately not implemented).
* `bot_dirs` is the single source of truth for the directory convention: `base_dir/bots/<Discord user ID>/<bot name>`,
  with the permission boundary at the user-ID level.
* Do not touch a running bot without checking the session first: `/bot del`, `/bot rename`, `/token` and `/bot login`
  all call `is_bot_screen_running` before doing anything (the text comes from one place, `refusal_if_not_stopped`, and a
  running bot is refused outright), because changing the token of a running bot has no effect, deleting a running
  directory destroys data, renaming a running directory loses its subsequent log lines, and logging in needs exclusive
  access to the credential file. The restart flow is stricter still: targets without a token are removed before
  stopping (a restart stops first and starts afterwards, and stopping for nothing takes an account that is still
  fishing offline).
* Credentials are written only by the JAR, and this project does not implement Microsoft login itself: the session
  started by `/bot login` is the bot's ordinary session, it forwards the device code the JAR prints (login URL plus
  code) to the caller, and credentials are considered granted **when the `refreshToken` file appears** (the
  "authorization succeeded" message is not parsed): as soon as a new credential appears, authentication succeeded, and
  that step also deletes the old credential that was moved aside (`<credential file name>.renewing`). Any existing
  credential has to be moved out of the way before doing this, otherwise the JAR sees it, uses it directly and prints no
  device code, and the command turns into "nothing happened".
  Two observed JAR behaviours: **without a valid credential it simply waits for you to log in and does not connect to
  the game server**, and when Microsoft keeps rejecting the request (returning an empty token) it ends the JVM by
  itself - which is why the second step reports "the session disappeared" directly as "exited after start-up".
  See [docs/mods/commands/commands-en_us.md section 4.6](../mods/commands/commands-en_us.md#46-bot-login).
* The game-side configuration (FishingBot's `config.json`) is a **generated artifact**: at process start-up the old file
  is deleted, the complete template shipped with the repository (`fishingbot-config.example.json`) is used as the base,
  and the `[fishingbot]` keys are overlaid on top (`runtime/bot_instances/fishingbot_config.rs`). What is written is always a complete
  configuration, so it does not depend on the jar adding fields. Measured behaviour (v2.13.15, `-onlyCreateConfig`):
  feeding the generated file to the jar makes it print neither `Your configuration was incomplete...` (which a minimal
  configuration does print) nor change our file **by a single character**, and the language takes effect as the value we
  wrote (`Using language: CHINESE_SIMPLIFIED`). Hand edits to that JSON do not survive to the next start-up; the
  template is the only thing that has to be swapped when upstream changes the configuration structure (swap it and run
  `cargo test`: a managed path that cannot be found makes the test fail).
  `misc.language` is exactly the value of `[i18n] language` (the value is the jar's constant name, it is not mapped from
  something else), and there is no chance to fill it in by hand, so "which language's JAR log to match a reconnect
  against" cannot get out of sync again. A generation failure is treated like a wrong configuration (exit code 1).
  See [docs/mods/config/config-en_us.md section 3.2](../mods/config/config-en_us.md).
* Which set of directories a command works on is each command's own choice: `runtime::bot_instances::bot_sessions::split_bot_entries_by_is_running(&[BotDirEntry])`
  only groups entries, and the caller selects the range by choosing a function:
  `all_bot_dir_entries()` (everything, used by `/startall`, `/restartall`, `/stopall` and shutdown, so that orphan
  processes without a token can be cleaned up too), `token_ready_bot_dir_entries()` (only those with a configured
  token, used by `/commandall` and `/show status`) and `bot_dir_entries_of_owner(owner_id)` (only one account, used by
  `/show mybots`). This joining of directories and sessions is implemented in exactly one place, `runtime::bot_instances::bot_sessions`.
* `operations` authorizes user commands once. Public queries need no permission; batch controls and anti-AFK require an administrator in `[admins]`.
  * `operations::begin_command_sync` checks the administrator and takes the sync lock before the adapter invokes registration APIs.
  * Single-bot control, log reads and account edits resolve the target and check owner or administrator; missing and unauthorized targets return the same reply.
    `/token` checks after the modal submission, before writing credentials.
  * For long controls, prepare the operation, deliver its progress notice, then execute once. Denial or notice delivery failure does not execute it.
    Neither Discord nor runtime duplicates the policy.
* Option candidates do not bring their own permission system: `src/operations/queries/list_bot_candidates.rs` queries the bots
  visible to the caller and their states; `src/discord_commands/autocomplete.rs` filters options by state and command meaning (admins see every bot;
  everyone else sees only their own `bots/<user ID>/`). Discord interaction adaptation
  and conversion to `AutocompleteChoice` live there too. Candidates are only suggestions: a user can still type
  any name by hand, and ownership plus session
  state are still decided by each `operations`. Which set each command picks (running, stopped, all) follows its semantics,
  see [docs/mods/commands/commands-en_us.md section 5](../mods/commands/commands-en_us.md).
* The stop flow returns `StopOutcome`. The bulk stop report is assembled by
  `operations::all_bots_control::stop_all_bots(caller_id)`, and the process shutdown assembles the same details on the spot inside
  `runtime::shutdown::exit_on_shutdown_signal(notice_ctx)` (only the title differs: "shutdown signal received, wrap-up"; that
  report reaches every server's notice channel as well as the log, within a 30-second overall limit past which the
  send is dropped and not retried, while the disconnect and permanent-failure wrap-ups only reach the log).
* Visibility: `lib.rs` exports the permission-checked command entry points. Import bot, permission, report and runtime
  APIs from their owning modules. Internal operations use `pub(crate)` or narrower visibility.
* The gateway watchdog is the only place that decides a disconnect: the
  rules are in section 3, and the cleanup is the library side's private `stop_all_bots_on_disconnect` (stop the bots
  and record the reason and the moment): the judging side only calls `request_reconnect` to hand the reason to the
  entry point and wake it, and the entry point's `wait_for_attempt()` stops the bots and rebuilds the connection. Only
  a wrong configuration goes through `exit_on_permanent_failure`. Do
  not try to judge from framework internals: serenity 0.12.5's `ShardMessenger` (that is, `ctx.shard`) exposes neither
  the connection stage nor the latency (measured: only `shutdown_clean`, `set_presence`, `chunk_guild` and so on), and
  `Shard` cannot be reached from inside the shard runner; a half-dead gateway is what serenity's own heartbeat and ACK
  mechanism detects and reconnects, and we only cover the two cases "cannot even connect" and "the network went away".
* No concurrency mutex: concurrent Discord users can operate on the same bot at once, so simultaneous `/startall` or
  `/stop` calls may interfere. If real usage shows a need, add a lock keyed by bot name in the command layer and document
  the convention here.
* No single-instance lock; "one machine, one configuration, one instance" is relied on instead: serenity is a gateway
  client that occupies no port, so two instances both connect to Discord and both receive interactions, and the only
  symptom is that one command is answered twice; this process holds no authoritative state (state lives in `screen`
  sessions and on disk), so duplicate instances do not corrupt each other's state. A lock could only be machine-wide,
  and it would block the legitimate "run a second copy with a different `base_dir`" setup.
* Discord-facing text has to go through `config::Config::get().text()` (the mechanism is in
  [docs/mods/i18n/i18n-en_us.md](../mods/i18n/i18n-en_us.md)): business and runtime modules must not contain any more
  user-facing Chinese literals. Adding a language means adding a file under `src/text/languages/` implementing `Text` and `LogMessages` (including
  the command-description table returned by `command_texts()`) and registering it in `Language`: a missing method fails
  to compile, and a missing command description is caught by the coverage test; the legal values live in exactly one
  language table (the method count and self-check command are in
  [docs/mods/i18n/i18n-en_us.md section 6](../mods/i18n/i18n-en_us.md)): reply texts and program logs use the language
  table, while startup-specific configuration errors live in `text::bootstrap`. Log texts use named `LogMessages`
  methods implemented by both languages; command descriptions and the `/token` modal labels are handled by rewriting the command tree poise
  generates before registration plus a hand-written `impl poise::Modal`; the JAR's `/status` messages are in the
  language table (`jar_*` methods). `[i18n] language` controls both manager and JAR language. Application-generated
  startup errors use English until the first framework setup succeeds; shared errors then use the configured language
  and do not revert on reconnect. External error details remain unchanged. Text comes from the loaded config; accessing
  it before loading config panics. Tests must load config or select a language explicitly. `Language` has no `Default`
  implementation, and an unsupported `language` value fails startup parsing
  and exits: there is no language fallback. The example configuration's
  `language = "ENGLISH"` only decides which language a new deployment runs in; a Chinese deployment changes it to
  `CHINESE_SIMPLIFIED`.
  The Chinese wording is authoritative: to change a text, change `zh_cn.rs` first (the original of both content and
  layout), then align `en_us.rs` item by item. Discord adapters use the language tables rather than keeping a second copy.
  Layout conventions: the symbols of user- and deployer-facing texts follow the language (full width in Chinese, half
  width in English) and no full stop is left at the end of a sentence (exclamation marks stay); extra explanations use
  round brackets `()`; the exceptions are configuration table names (`[admins]`), the `Report` section labels and the
  parameter notation in usage hints (optional as `[lines]`, required as `<bot name>`); error replies always start with
  `Error: ` (Chinese `错误:`), see [docs/mods/i18n/i18n-en_us.md section 5](../mods/i18n/i18n-en_us.md).
* `[keywords]` holds only the deployment-specific `filter_regex`; JAR status messages come from the i18n `jar_*` methods.
  **"The process has ended" needs neither configuration nor a keyword**: the end marker an older
  wrapper script wrote into the log (the `fishingbots` script: `java ... | tee bot.log; echo marker >> bot.log`) is a
  product of that script, whereas this project launches sessions itself and does not write it, deciding instead from the
  screen session state (a `(Dead ...)` shell), so it is unaffected by the jar version and by how the script is written.

## 3. Gateway watchdog (the sole disconnect decision point)

Purpose: once this process has been judged to have lost contact with Discord, no longer assume that it can still be
commanded. While Discord is unreachable we cannot control the bots, yet they still occupy game accounts (FishingBot's
automatic reconnect competes with real players for the accounts), so the shutdown has to be clean; the process then does
not exit, but waits one judgement window before rebuilding the connection (details are in
[docs/mods/watchdog/watchdog-en_us.md section 4](../mods/watchdog/watchdog-en_us.md)).

Both criteria are implemented in `src/runtime/watchdog/gateway.rs` (pure timing, independent of the framework, unit-testable):

* Handshake: within `first_handshake_timeout` after this attempt started, a `Ready` or `Resume` must have been received
  at least once (the binary side's `event_handler` records it with `watchdog.record_handshake_success()`). When that is not met, a
  rebuild of the connection is requested with the reason `NeverReady`.
* Reachability: every `probe_interval`, `Http` actively sends a `GET /gateway`, and any HTTP response counts as
  reachable (401 and 403 included); only a timeout or a connection error counts as unhealthy, and when unhealthy
  results accumulate to `window` in a row, a rebuild of the connection is requested with the reason `Unreachable`.

Details:

* The timings come from the judging window `timing.watchdog_window` in the configuration; the probe interval (one
  third of the window) and the first-handshake timeout (twice the window) are derived from it
  (`TimingConfig::probe_interval` / `first_handshake_timeout`) instead of being separate keys, because those two only
  make sense as a ratio of the window. The entry point builds them into the `Watchdog` (they are three of its fields),
  and judging and probing share that one instance; deriving them also means "a probe slower than the window, where the
  first failed probe is already a disconnect" can no longer be configured.
* The probe only looks at network reachability, not at whether the credentials are right: a wrong token makes
  `client.start()` fail and exit on its own, which is not the watchdog's business.
  ?The probe hits `discord.com`, whereas the gateway connects to `gateway.discord.gg`; if a deployment shows the API
  reachable but the gateway unavailable, add a TCP or TLS probe against the gateway domain.
* The probe task is started before `client.start()` (in `main.rs`): when the gateway cannot be reached at all, `Ready`
  never arrives, so the judgement has to be possible outside the client as well; starting it early also lets "offline
  right at start-up" exit within the first-handshake timeout.
* The judgement is reversible: when the probe recovers, or a `Ready` or `Resume` arrives again, the state goes back to
  `Alive` - it is not a one-way decision.
* The four states are `Alive`, `Waiting`, `NeverReady`, `Unreachable` (internal to the watchdog, not public), with the current moment passed
  in by the caller (`state(now)`), so every rule can be covered by unit tests.
* On the first `Ready` it prints one line saying that the gateway watchdog is ready, with the effective judgement
  window, probe interval and first-handshake timeout, and that line is the reference when troubleshooting (it reflects
  the three values actually in effect).

When the judgement fails, the library side's `request_reconnect` runs: all bots are stopped, the reason and the
disconnect moment are recorded, and the process entry point is woken to rebuild the connection (it assembles the
`Client` once more after one judgement window). The process does not exit, because disconnects are normal, and leaving
the restart to systemd would only flush one log file every 10 seconds and pay a cold start every time; only a wrong
configuration (an invalid token, the bot not being invited) goes through `exit_on_permanent_failure` (exit code 1, this
project's dedicated code for a deliberate failure, which a systemd unit can keep from restarting with
`RestartPreventExitStatus=1`).

## 4. Start-up pipeline (example of explicit steps)

The start-up queue runs the pipeline in four stages; the command side only touches the first:

1. `StartupQueue::start_bots` creates and enqueues a ledger. `wait_for_results` settles timeouts and takes the results under the same lock; late reports do not overwrite settled entries. Only launches that were actually sent are verified.
2. The starter thread `run_starter`: wait for a non-empty queue, wait for `[startup_queue] batch_login_interval` (counted from when the previous batch was popped), then `pop_bots_from_head(max_batch_size)`.
3. `start_single_batch` calls `launch_one_bot` → `bot_startup::create_logs_and_launch_bot_session` per bot (login also calls it directly, without queuing): create this run's log file (on failure no session is launched) → prepare FishingBot's raw log directory (`<bot_dir>/<bot_logs_dir>/raw`, with `raw/log0.log` pointing at `/dev/null`) → start the session with `screen -dmS` and a command shaped like `java ... | grep -v -E '<filter_regex>' | tee -a '<log_path>'`; adjacent bots in a batch are separated by `bot_login_interval`.
4. `verify_start_results`: take a `screen -list` snapshot immediately after launch, wait for `timing.check_grace`, then check the session again. Query `/status`: `joined` indicates server entry, and unavailable status or query errors are reported as issues.

Follow this style for new flows: one function per step, and every step readable on its own.

## 5. Build and verification (run them all before committing)

```bash
cargo fmt --check                      # formatting
cargo test --all-targets               # library, binary and integration tests
cargo check --all-targets              # the real target: x86_64-unknown-linux-musl (static)
cargo doc --no-deps                    # doc links: must be warning-free (cargo check alone does not catch broken links)
cargo clippy --all-targets -- -D warnings
```

Tests are layered:

* Unit tests live in the same file as the code (`#[cfg(test)] mod tests` inside `src/*.rs`): they cover private logic
  and cases that need access to internal data; behaviour that can be verified through the public API belongs in
  `tests/` instead.
* Integration tests live in `tests/`: they use the library as an external dependency and cover public behaviour and
  the external contract.
* Shared test helpers: unit tests use `tests/common/unit_support.rs`, included by `lib.rs` only under `cfg(test)`;
  integration tests use `tests/common/mod.rs` for temporary directories, isolated config loading and the example
  config. The two helpers differ in whether they create the temporary directory.
* Migration rule: when a test does not depend on private functions, private types or binary internals, move it to
  `tests/`; tests that depend on those implementation details stay in the source tree, so the public API is not
  widened only for testing.

A few more rules:

* Changing a configuration field requires updating `minecraft-fishingbot-manager-for-discord.conf.example.toml` and
  [docs/mods/config/config-en_us.md](../mods/config/config-en_us.md) at the same time, otherwise `cargo test` fails
  (there is a test checking both).
* Code items of this crate inside backticks in comments are written as clickable links (`` [`logger`] `` and the like),
  the wording is formal prose without bold markers, dashes or book-title quotes, and section-header comments use
  `============ X ============`.
* In doc comments, public items must not reference private items (rustdoc cannot link them and readers cannot see them):
  state the meaning directly instead, and add links where linking is possible. A private item's own documentation may
  link to private items normally.

## 6. Code map

The [architecture guide](../mods/architecture/architecture-en_us.md#2-directory-structure) lists the owning modules and entry points.

## 7. Established technical decisions

1. Only one bot log is kept: it is filtered before writing (`grep -v -E` plus `tee -a`), and FishingBot's own log file
   points at `/dev/null`. The two-copy scheme ("raw log into `/tmp`, filtered log into the bot directory") was not
   adopted because it costs extra disk (or tmpfs memory), needs extra cleanup and permission logic, and historically it
   was exactly the two logs landing in the same directory that made `/log` read the wrong file. To debug the filter
   rules, temporarily empty `keywords.filter_regex` and restart the process to see every line.
2. Shared types live with the behavior they describe. Import permissions, reports and bot operations from their owning modules.
3. Slash commands only, no `!prefix` commands: prefix commands have to read message content, which needs the privileged
   `MESSAGE_CONTENT` intent (an extra step in the developer panel, plus a review on large servers); slash commands need
   no privileged intent at all, one switch in the developer panel, and they come with parameter hints and completion.
   This also decides that the command layer parses no text.
4. `/log` uses an attachment instead of truncation: 2000 characters cannot hold 100 lines of log, and truncation loses
   far too much information; an attachment can give everything and naturally carries the file name. Privacy is
   guaranteed by `defer_ephemeral()` plus an explicit `.ephemeral(true)` on every reply.
5. Long commands reply with two messages (the `*all` ones and the single-bot start, stop and restart): first an
   acknowledgement (start-type commands state that the request was accepted into the queue, the others that execution has
   started), then the full report when it finishes. This follows the defer behaviour in Discord's official documentation
   (the first replaces the placeholder, the second and later messages are new). The assessment of the batch-operation
   final-reply design is in section 8.1.
6. The game-side configuration is generated by this program and the template ships with the repository: FishingBot only
   understands JSON and has no per-item command line overrides, so `[fishingbot]` is the single source of truth, and at
   start-up the old file is deleted and the template is used as the base with our few keys overlaid, so what is written
   is always a complete configuration (it does not depend on the jar filling fields in). The template is
   `fishingbot-config.example.json` (a complete FishingBot configuration with defaults adjusted for this project's
   requirements). Keep its schema in sync with FishingBot and run `cargo test` after template changes.

## 8. Current status and pending items

Current status: the project is the Discord implementation and has been run in a real Linux environment. Code and
documentation are maintained together; see the [docs index](../index-en_us.md) for the document list. Run the automated
checks after modifications; the items below require end-to-end validation in a real deployment.

Build prerequisite: the rustls crypto backend requires a C compiler, and a static musl build also requires `musl-gcc`
(install `musl` on Arch and `musl-tools` on Debian and Ubuntu). Once installed, `cargo build --release` (default target
`x86_64-unknown-linux-musl`) produces a static binary directly.

### 8.1 Assessment of the batch-operation reply design (not implemented)

Goal: send final batch-command results as an ordinary channel message, avoiding the 15-minute
interaction-token limit; the message should identify the caller, invocation time and command, and preferably use a
native quoted reply.

Current implementation: `run_control_with_notice` takes a closure that owns its inputs: `ctx.defer()` → progress followup →
business operation → `say_final_report`. `say_final_report` first attempts `ctx.say`; on failure it posts an ordinary
channel message and records `LogMessages::report_posted_to_channel`. Single-bot commands share the same path.

Findings:

1. Sending the final report as an ordinary channel message is feasible. Ordinary messages are not bound to the interaction
   token, but remain subject to the 2000-character limit and channel send permission. `ctx.defer()` and the progress
   acknowledgement must remain; the batch path must be separated from the single-bot path so that single-bot behaviour
   does not change.
2. A native quotation of the slash-command invocation is not feasible. `CommandInteraction` contains no source message
   ID; `Interaction::message` exists only for component and modal interactions; the project subscribes only to
   `GatewayIntents::GUILDS`, so it cannot receive `CHAT_INPUT_COMMAND` message events. `MessageReference` must target a
   real message ID, and a native reply requires `READ_MESSAGE_HISTORY`. The user's `/startall` invocation therefore cannot
   be the reference target.
3. Caller metadata is available through `ctx.author()`, `ctx.created_at()` (derived from the interaction ID) and
   `ctx.command().qualified_name`. The invocation time can be rendered with the Discord timestamp `<t:...:f>`. The text
   should be added as a `Text` method with Chinese and English implementations.
4. If a native quotation card is required, the bot can quote its own progress message: store the progress message
   `MessageId` and send the final ordinary message with
   `CreateMessage::reference_message((channel_id, progress_message_id))`. The card quotes the progress message, not the
   user's command; it requires `READ_MESSAGE_HISTORY`, must fall back to an ordinary message without a reference when the
   quotation fails, and requires live validation.
5. Before implementation, the following must also be addressed: the combined length limit of the final message and its
   prefix, logging of channel-send failures, adjustment of the `ReportPostedToChannel` semantics (a fixed channel send is
   no longer a token-expiry fallback), and isolation of the single-bot path. The implementation can reuse the existing
   `run_control_with_notice`, `Report` and `say_final_report` paths without adding a new struct; the batch path still has to be
   separated from the single-bot path.

Recommendation: implement the first stage as "final report fixed to an ordinary channel message + textual reference";
evaluate the native quotation card separately, after validating permissions and reference behaviour.

### 8.2 Live deployment checks

1. Validate Discord interactions end to end: `/log` and `/show botconfig` attachments, the second reply after defer, and
   the `/token` modal followup. The modal flow is described in
   [section 1.3 of the command guide](../mods/commands/commands-en_us.md#13-sensitive-content-ephemeral-and-modal).
2. Validate `/bot login`: confirm delivery of the login link and code, credential creation in the bot directory, and
   deletion of `refreshToken.renewing` after a successful refresh. See
   [section 4.6 of the command guide](../mods/commands/commands-en_us.md#46-bot-login).
3. Validate the generated FishingBot `config.json` and the bot connection against a real game server, including the
   language-specific `/status` responses after changing `[i18n] language`. Template completeness has already
   been validated with a real JAR; see [section 3.2 of the config guide](../mods/config/config-en_us.md).

### 8.3 Deferred items

The following items do not block release: multi-bot variants of `/start`, `/stop` and `/restart` (the existing `*all`
commands remain available); automatic rotation of the program log; a command-layer concurrency mutex; and a concurrency
cap for `/bot login`. Reassess them when actual usage requires it; the login trade-off is recorded in
[section 4.6 of the command guide](../mods/commands/commands-en_us.md#46-bot-login).

## 9. Related documents

* All documents are in the [docs index](../index-en_us.md).
* By topic: [tutorial/tutorial-en_us.md](../tutorial/tutorial-en_us.md) (deployment) and
  [architecture-en_us.md](../mods/architecture/architecture-en_us.md) (layering and directory structure).
* Module technical documents: one per module under `mods` (see section 3 of the index).
