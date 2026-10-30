# Architecture and the module map
[简体中文](architecture-zh_cn.md) | **English**

This document explains how the code is layered, what each layer is responsible for, and how one Discord slash command
ends up in a `screen` session. To browse documents by topic, see the [docs index](../../index-en_us.md).

## 1. Layers

```
Discord slash command (the gateway pushes INTERACTION_CREATE)
   |
   v
commands (the command layer, poise)   read options + check numbers + defer + dispatch; no permissions, no cleaning
   |  handle_*
   v
lib.rs (the command entry layer)      action enum + permission check (has_permission) + Result -> reply text
   |
   v
business modules                      one file per command
bot_control / bot_manage / all_bots_control / show / general
   |
   v
utils (the primitives layer)          reusable abilities: directories, sessions, logs, reports
   |
   v
the JVM inside a screen session (FishingBot)
```

Name the layers first. `src/commands/` is the command layer: poise's `#[poise::command]` functions that read values,
check argument counts, turn values into numbers, `defer` and turn an `Err` into a reply. `lib.rs`'s `handle_*` is the
command entry layer (the routing layer below): it turns the action enum into a business call, and it checks the
permissions of everyone-commands and batch commands. The business layer consists of `bot_control`, `bot_manage`,
`all_bots_control`, `show` and `general`; below it sits the utils primitive layer.

How arguments travel down: Discord hands the command layer structured options, but an option's value is still the raw
text the user typed. The command layer only reads the value, checks numbers and passes it through (argument names start
with `raw_`); the layer that actually uses it cleans it once: bot names through `ToCleanBotName::to_clean_bot_name`
(called when `BotDirEntry` builds an entry), in-game command fragments through `utils::ToCleanBotCommand`. Number
conversion (the `/log` line count, the `/ping` port) is the command layer's only conversion; a failed check replies with
a usage hint on the spot.

Two more chains that have nothing to do with commands:

* `main.rs`: the process lifecycle. It reads the config, initialises the program log, prepares the REST probe handle,
  assembles the poise `Framework` and serenity `Client`, registers commands and starts the connection loop. During
  normal operation the process exits with code 0 only on SIGTERM or Ctrl+C; permanent failures such as an invalid config
  exit with code 1. A disconnect and `client.start()` returning do not end the process: it stops every bot, waits one
  judging window and rebuilds the connection.
* `watchdog`: the gateway watchdog. It decides whether this process has lost contact with Discord (the decision is pure
  timing; the process entry requests the reconnect). It is the only place that decides we are offline; the rules are in
  [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 3.

## 2. Directory structure

```
src/
├── main.rs                     process entry: config -> log -> assemble poise (the probe task is started by the watchdog) -> run -> reconnect / exit on signal
├── lib.rs                      command entry layer: action enums + handle_* (+ re-exports for the binary)
├── commands.rs                 command layer: the poise command set (submodules below)
│   ├── autocomplete.rs             the candidate lookup for the `name` option (autocomplete)
│   ├── bot_control.rs              /start /stop /restart /pause /unpause /command
│   ├── bot_manage.rs               /bot add|del|rename|login (parent command + four subcommands)
│   ├── token.rs                    /token (ephemeral)
│   ├── log.rs                      /log (ephemeral + attachment)
│   ├── all_bots.rs                 /startall /stopall /restartall /commandall
│   ├── show.rs                     /show status|botconfig|mybots|allbots|admins
│   └── general.rs                  /help /ping /checkserver /whoami
├── text.rs                     text entry: one language per process (`[i18n] language`) + command metadata rewrite
├── text/                       en_us (English) / zh_cn (one to one, only literals differ)
│                               + bootstrap (English during bootstrap) / log_text / requirement (two enums with data)
│                               + setup_failure (which text a setup failure uses, and whether it is permanent)
│                               + watchdog (internal watchdog mutex-poisoning notes; language-dependent)
├── watchdog.rs                 gateway watchdog (submodules below)
│   ├── gateway.rs              handshake and probe criteria + the REST probe task (pure timing; timers reset per attempt)
│   └── lifecycle.rs            offline bookkeeping, the reconnect signal, waiting for an attempt to end, shutdown/exit
├── process_log.rs              program log: terminal (stderr) + <deploy dir>/logs/timestamp.log (submodules below)
│   ├── levels.rs               level decision (one ceiling for this project, one for third parties)
│   └── format.rs               record rendering (timestamp, level prefix, continuation indent)
├── config.rs                   loading, validation and access for the external config (submodules below)
│   ├── schema.rs               config structs (one to one with the toml)
│   ├── parse.rs                TOML deserialisation + path expansion
│   ├── validate.rs             startup validation
│   └── template.rs             the config template written on first start
├── bot_control.rs              single-bot control commands
│   ├── start.rs  stop.rs  restart.rs
│   └── pause.rs  unpause.rs  command.rs  log.rs
├── bot_manage.rs               account management commands
│   └── add.rs  del.rs  rename.rs  token.rs  login.rs
├── all_bots_control.rs         batch commands (admins only)
│   └── startall.rs  stopall.rs  restartall.rs  commandall.rs
├── show.rs                     read-only query commands
│   └── status.rs  botconfig.rs  mybots.rs  allbots.rs  admins.rs
├── general.rs                  general commands
│   └── help.rs  ping.rs  checkserver.rs  whoami.rs
├── test_support.rs             shared helpers for tests only (compiled under `#[cfg(test)]`)
└── utils.rs                    the primitives layer
    ├── bot_command.rs          how an in-game command is assembled (`/command` and `/commandall` share it, written once)
    ├── bot_dirs.rs             bot directories: paths, lookup, walking, creation (an entry `BotDirEntry` carries the token status)
    ├── bot_name.rs             bot name normalisation
    ├── bot_logs.rs             bot logs: create / pick (the raw log directory is prepared here too, called by startup)
    ├── fishingbot_config.rs    generating the game-side config (FishingBot's conf.json): delete the old file, then
    │                           override the `[fishingbot]` keys on top of the repository template
    │                           (fishingbot-config.example.json)
    ├── fs.rs                   file and directory primitives: read the tail, list subdirectory names, remove a
    │                           directory (not existing counts as success)
    ├── bot_status.rs           bot status: pause memory + echo confirmation, live log reading when online
    ├── running_bots.rs         status summary of the running bots (the `/show status` rows; the header is in `show/status.rs`)
    ├── screen.rs               session detection and command injection
    ├── shutdown.rs             stopping sessions (`/stop`, `/stopall`, the stop stage of a restart, process shutdown)
    ├── startup.rs              starting (four explicit steps)
    ├── restart.rs              restart: stop then start (the stop + start combination)
    ├── uptime.rs               uptime: seconds read from /proc plus formatting into days/hours/minutes
    ├── reply.rs                assembling and rendering replies (Report + length cap)
    └── permission.rs           the admin check
```

The business layer and the command layer are two sets of files: `commands/bot_control.rs` (the command layer) only
declares which commands exist, what the arguments look like, how to `defer` and how to send the result, while the real
action is in `bot_control/start.rs` (the business layer). The file names are deliberately the same on both sides so
they can be read side by side.

## 3. The full path of one command

Taking `/start <bot name>:duck` as the example (the caller is `caller_id = 1234...`, typing in a server channel):

1. Discord pushes `INTERACTION_CREATE` over the gateway, serenity parses it into `FullEvent::InteractionCreate`,
   poise finds `commands::bot_control::start` by command name and parses the options into function arguments (the
   command layer's parameter is called `name`; inside the routing layer the same value is called `raw_bot_name`).
2. The command layer (`commands/bot_control.rs`):
   * `ctx.defer()` takes the 3 second limit out of the way first (the waits inside `stop_bot_sessions` are far longer
     than 3 seconds);
   * `ctx.author().id.get()` reads `caller_id`;
   * first `ctx.say("Request accepted: /start duck. Any start operation will use the global queue. The complete result
     will follow in a separate message.")`, which replaces the "thinking" placeholder so the user sees progress at
     once;
   * then `handle_bot_control(BotControlAction::Start, "duck", caller_id)` inside
     `tokio::task::spawn_blocking(...)`, because the business layer's `screen`, file and `sleep` calls are all
     synchronous and blocking and must leave the async threads.
3. `lib::handle_bot_control` checks the permission with `permitted_bot_dir_entry` first, then calls
   `bot_control::start::start_bot(&bot_dir_entry)`.
4. The entry used for that permission check carries the owning user ID and the token status: `find_by_name` looks the
   name up across every user directory, and then one check decides whether the entry belongs to the caller (admins
   always pass).
5. The token status in the entry confirms a token is configured (`BotTokenStatus::from_dir` reads it when the entry is
   built), and `utils::is_bot_screen_running` confirms it is not already running.
6. `StartupQueue::start_bots()` follows the four explicit steps of [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md)
   section 4.
7. The result is assembled by `Report`, turned into text by `IntoReplyText::to_reply_text` and sent as the second
   message with `ctx.say(report)` (the first message already used up the placeholder's allowance).

## 4. Invariants that must hold (the layering-related ones)

The complete list (reply wording, interaction timing, config reading rules) is in
[CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 2. Only the ones that are directly about which layer owns
what are repeated here, so that the two places cannot drift apart:

* One account, one `screen` session: the session name is `runtime.screen_prefix` plus the normalised bot name, with
  `utils::session_name` as the single source; there is one `screen -list` parsing convention (`utils::screen`).
* The directory is the ownership: a bot directory is always `base_dir/bots/Discord user ID/bot name`, and the
  permission boundary is the user ID level (`utils::bot_dirs`).
* Every command declares one permission requirement and there is one place that checks it. The requirement is
  `utils::RequiredPermission` (`Everyone`, `MustBeAdmin`, `OwnerOrAdmin(owner user ID)`) and every check goes through
  `has_permission`:
  * everyone-commands (`/help`, `/show status`, `/ping`, `/checkserver`, `/show *`): checked once by `lib::handle_*` in
    the routing layer;
  * single-bot and account management commands: the routing layer resolves the target entry with
    `BotDirEntry::find_by_name` first, then checks `OwnerOrAdmin(entry owner)` (ordinary users only touch their own
    directory, admins can cross directories);
  * batch commands: checked once by the routing layer's `lib::handle_all_bots_control` with
    `has_permission(caller_id, RequiredPermission::MustBeAdmin)`, which is why functions under `all_bots_control/*`
    take no `caller_id` and check nothing themselves;
  * the command layer only counts arguments and looks at number ranges, never at permissions;
  * new commands follow the same rule: the command layer must hand the result to `lib::handle_*` (the layer that
    declares the requirement and checks it once) and must not build its own reply - `/whoami` included. There is no
    registry or framework hook to fall back on, so this convention plus review is the guarantee.
* The framework only appears in the command layer and the process entry: the business layer (`bot_control` and
  friends) and `lib.rs` do not reference serenity or poise; they take plain arguments and return
  `Result<String, String>`. That keeps the business logic unit-testable and makes swapping the transport easier
  later.
* The command layer never blocks an async thread: every business call goes through `tokio::task::spawn_blocking`
  (the business layer is synchronous).
* Stopping must be thorough: every stop entry point reuses `utils::stop_bot_sessions` (Ctrl+C, wait, force kill,
  re-check), so no orphaned JVM keeps holding a game account.
* Actions that must not run while a bot is running check the session first: `/token`, `/bot del` and `/bot rename`
  all call `is_bot_screen_running` before touching anything; the restart flow is stricter still and drops targets with
  a missing token before it stops anything.
* There is one exit path for signals: SIGTERM or Ctrl+C goes through `watchdog.exit_on_shutdown_signal()`, which logs
  first, stops every bot and assembles the same wrap-up as `/stopall` (it goes into the program log and is also sent to
  every server's notice channel), and finally exits with code 0 (systemd does not restart it). A
  disconnect and `client.start()` returning do not exit: they stop every bot, wait one judging window and rebuild the
  connection (see [watchdog-en_us.md section 4](../watchdog/watchdog-en_us.md)); only a wrong config (an
  invalid token, the bot not being invited to the server) exits with code 1 and is handed to systemd (the unit sets
  `RestartPreventExitStatus=1`, so such an error is not restarted).
* Both log streams and the startup steps are documented: see [utils-en_us.md](../utils/utils-en_us.md) section 7 and
  [process-log-en_us.md](../process-log/process-log-en_us.md).
* Replies never overstate: say exactly as much as can be verified. `/start` and `/restart` can only confirm that the
  command was sent and the session appeared, so they say "command sent"; `/stop`'s flow re-checks on the spot that the
  session is gone, so it may say "stopped successfully". New commands pick their wording by the same principle.

## 5. Code conventions (the essentials)

The complete list, with a reason for each, is in [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 2.

* The layer names are fixed: `src/commands/` is the command layer (read values, check numbers, `defer`, send the
  result); `lib.rs`'s `handle_*` is the command entry layer (action enum to business function; permissions are checked
  once, only here); below that sit the business layer (`bot_control`, `bot_manage`, `show`, `all_bots_control`) and the
  utils primitive layer.
* The framework only appears in the command layer and the process entry: the business layer and `lib.rs` do not
  reference serenity or poise; they take plain arguments and return `Result<String, String>`, which keeps the business
  logic unit-testable.
* Names explain themselves: action functions use verb phrases (`start_bot`); things that return a value use nouns
  (`all_running_bot_names`, no `get_` prefix); conversions use the `to_` and `into_` prefixes (`to_clean_bot_name`,
  `into_report`; `into_` when it consumes `self`, `to_` otherwise, as clippy sees it too); constructors that search the
  disk use `find_*`; predicates use `is_` and `has_`; an ID is always `*_id` (`caller_id`, `owner_id`).
* Add a suffix according to the return shape: lists are `*_list` (`my_bots_list`); status tables are `*_status`
  (`all_running_bots_status`); data sent as an attachment is `*_view` (`fishingbot_config_view`, `bot_log_view`).
  One-off probes, checks and text that is already written out in full use a bare noun (`ping`, `checkserver`, `help`,
  `whoami`).
* Global singleton accessors share one shape: config `config::Config::get()` and text `text::get()` both mean "the one
  copy in this process". A missing config `expect`s (that is an internal invariant); a missing language makes
  `text::get()` panic in a production build (an entry-point ordering error) and return English in a test build.
  `Language` has no `Default` implementation, and an unsupported `[i18n] language` fails startup validation and
  exits. The process entry installs both once
  (`config::Config::get_or_init()` installs `text::install()` along the way). The watchdog takes a different route:
  `WATCHDOG` in `src/watchdog.rs` is a process-wide slot and consumers call `WATCHDOG.get_or_init()` once (the first
  call creates it from the config). Poise's event callbacks are plain fn pointers that cannot capture anything, so
  they take the same entry point; every other method takes `&self`.
* The order inside a file is fixed: module docs (`//!`), `mod` declarations, `use`, public re-exports (`pub use`),
  then constants, types and functions; the process entry (`lib.rs`, `main.rs`) follows it too, so "which submodules
  does this file have" is visible at the very top.
* Command and option descriptions live only in the language tables and are validated before registration; see
  [commands-en_us.md](../commands/commands-en_us.md) section 1.7.1.
* One piece of text is cleaned once over its whole path: the command layer only reads values (argument names start
  with `raw_`) and whoever really needs a value cleans it (`clean_*`).
* Marks in comments are recognisable at a glance: a leading `?` is an open question still to be confirmed by a real
  run (delete the mark and keep the conclusion once confirmed); a leading `!` is a gap with a fallback path; `⚠️` is
  used in the language tables to warn about a hard convention.
* Where keywords live: only the ones that differ per deployment and must be filled in by hand go into `[keywords]`
  (`online`, `filter_regex`); the `/pause` and `/unpause` echoes go into `[command_echo]` (`pause_echo`,
  `unpause_echo`; they only appear when the JAR in use supports those two commands, and are edited in place when the
  JAR changes); the reconnect message the JAR prints stays in i18n as the `jar_*` methods and follows `[i18n] language`;
  whether the process is gone is read not from the log but from the screen session shell (`(Dead ...)`).

## 6. Building and testing from source

```bash
rustup target add x86_64-unknown-linux-musl   # the default target, see .cargo/config.toml
cargo check --all-targets
cargo build --release                         # target/x86_64-unknown-linux-musl/release/minecraft-fishingbot-manager-for-discord
cargo fmt
cargo test                                    # library, command layer and public contract regression tests
```

* The default build target is musl static linking (the binary does not care about the distribution or the glibc
  version, so it can be deployed directly to a server). Because rustls' crypto backend needs a C compiler, `musl-gcc`
  is needed as well (`musl` on Arch, `musl-tools` on Debian and Ubuntu), otherwise the build fails with
  `failed to find tool "x86_64-linux-musl-gcc"`.
* The release build enables LTO, `panic = "abort"` and `strip` in `Cargo.toml`'s `[profile.release]`.
* Changing the config structure means updating `minecraft-fishingbot-manager-for-discord.conf.example.toml` and
  [config-en_us.md](../config/config-en_us.md) as well, otherwise `cargo test` fails (two tests check exactly these two
  things).
* Documentation links are verified with `cargo doc --no-deps` (Rustdoc comments); Markdown links are verified with `cargo test --test docs_links`.

## 7. Related documents

* All documents: [docs index](../../index-en_us.md).
* Layer details: [config-en_us.md](../config/config-en_us.md) (the config layer),
  [commands-en_us.md](../commands/commands-en_us.md) (the command layer),
  [lib-en_us.md](../lib/lib-en_us.md) (the command entry layer),
  [utils-en_us.md](../utils/utils-en_us.md) (the primitives layer).
* Hard constraints and invariants: [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md).
