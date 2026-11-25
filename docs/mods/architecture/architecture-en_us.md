# Architecture and the module map
[简体中文](architecture-zh_cn.md) | **English**

This document explains how the code is layered, what each layer is responsible for, and how one Discord slash command
ends up in a `screen` session. To browse documents by topic, see the [docs index](../../index-en_us.md).

## 1. Layers

`discord_commands` receives interactions, passes the caller ID and sends replies. Only `operations` authorizes user commands; Discord does not precheck or scan owner directories.

`operations` handles **user requests**; `runtime::bot_instances` operates on **local FishingBot instances**. `/stopall` checks administrator access and replies to the caller, whereas a SIGTERM has no caller. Both paths use `runtime::bot_instances::stop` to stop and verify sessions; only process shutdown exits the manager.

## 2. Directory structure

* `src/discord_commands/`: Discord commands, registration, replies, attachments and modals
* `src/operations/single_bot_control/`: single-bot operations, one file per command
* `src/operations/all_bots_control/`: batch bot control; the independent anti-AFK worker is controlled by `src/operations/set_anti_afk.rs`
* `src/operations/accounts/`: accounts, credentials and login
* `src/operations/queries/`: queries and views, including `/log`, bot candidates, `/help` and `/ping`; read-only does not imply public access
* `src/text/report.rs`: Report construction and rendering
* `src/runtime/bot_instances/`: directories, sessions, status and bot operations; `stop.rs` stops bots without exiting the manager
* `src/runtime/`: `bot_startup` owns the bot startup queue, `shutdown` handles manager exit, `log_files` serves both process and bot logs, and `discord_notices` handles per-guild delivery for reconnect and shutdown notices. Other runtime components are `config`, `process_log`, `bot_log`, `watchdog` and `anti_afk`
* `src/runtime/utils/`: private runtime helpers; may depend on configuration and the outer utils, not texts or higher-level business logic
* `src/utils/`: low-level name, command-fragment, file and uptime tools; no dependency on other project modules or texts
* `src/text/`: English and Chinese messages and report rendering; independent of other project modules, with report language and limit supplied by callers

`bot_startup` and `bot_instances` collaborate within runtime: the startup queue uses session and status operations, while restart and stop also use the queue.

Import library APIs from their owning modules, for example `text::report::Report` and
`utils::console_command::ToCleanBotCommand`. `utils` contains low-level implementations, not compatibility re-exports of higher-layer APIs.
The source module rename does not change the deployment layout: bot data remains under `base_dir/bots`.

## 3. The full path of one command

For `/start`:

1. Discord defers and passes the name and caller ID to the operation.
2. `operations::single_bot_control` resolves and authorizes the target once, returning work bound to that directory. A denial produces only an error reply.
3. Only after Discord delivers the progress notice does it execute the work. `src/operations/single_bot_control/start_single_bot.rs` checks credentials and session state before submitting to the startup queue.
4. The queue verifies `/status` after the grace period and the Discord adapter sends the final report.

## 4. Invariants that must hold (the layering-related ones)

The complete list (reply wording, interaction timing, config reading rules) is in
[CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 2. Only the ones that are directly about which layer owns
what are repeated here, so that the two places cannot drift apart:

* One account, one `screen` session: the session name is `runtime.screen_prefix` plus the normalised bot name, with
  `runtime::bot_instances::screen::session_name` as the single source; the private `runtime::utils::screen` parses `screen -list`.
* The directory is the ownership: a bot directory is always `base_dir/bots/Discord user ID/bot name`, and the
  permission boundary is the user ID level (`runtime::bot_instances::bot_dirs`).
* Ordinary users control only their own bots; batch control requires an administrator. `operations` owns both rules, using the same business logic for direct library calls and Discord interactions.
* The framework only appears in the command layer and the process entry: the business layer (`bot_control` and
  friends) and `lib.rs` do not reference serenity or poise; they take plain arguments and return
  `Result<String, String>`. That keeps the business logic unit-testable and makes swapping the transport easier
  later.
* The command layer never blocks an async thread: every business call goes through `tokio::task::spawn_blocking`
  (the business layer is synchronous).
* Stopping must be thorough: every stop entry point reuses `runtime::bot_instances::stop::stop_bot_sessions` (Ctrl+C, wait, force kill,
  re-check), so no orphaned JVM keeps holding a game account.
* Actions that must not run while a bot is running check the session first: `/token`, `/bot del` and `/bot rename`
  all call `is_bot_screen_running` before touching anything; the restart flow is stricter still and drops targets with
  a missing token before it stops anything.
* There is one exit path for signals: SIGTERM or Ctrl+C goes through `runtime::shutdown::exit_on_shutdown_signal(notice_ctx)`, which logs
  first, stops every bot and assembles the same wrap-up as `/stopall` (it goes into the program log and is also sent to
  every server's notice channel), and finally exits with code 0 (systemd does not restart it). A
  disconnect and `client.start()` returning do not exit: they stop every bot, wait one judging window and rebuild the
  connection (see [watchdog-en_us.md section 4](../watchdog/watchdog-en_us.md)); only a wrong config (an
  invalid token, the bot not being invited to the server) exits with code 1 and is handed to systemd (the unit sets
  `RestartPreventExitStatus=1`, so such an error is not restarted).
* Both log streams and the startup steps are documented: see [runtime-en_us.md](../runtime/runtime-en_us.md) section 7 and
  [process-log-en_us.md](../process-log/process-log-en_us.md).
* Replies never overstate: say exactly as much as can be verified. `/start` and `/restart` can only confirm that the
  command was sent and the session appeared, so they say "command sent"; `/stop`'s flow re-checks on the spot that the
  session is gone, so it may say "stopped successfully". New commands pick their wording by the same principle.

## 5. Code conventions (the essentials)

The complete list, with a reason for each, is in [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 2.

* The framework only appears in the command layer and the process entry: the business layer and `lib.rs` do not
  reference serenity or poise; they take plain arguments and return `Result<String, String>`, which keeps the business
  logic unit-testable.
* Names explain themselves: action functions use verb phrases (`start_bot`); things that return a value use nouns
  (`token_ready_bot_names_with_screen_sessions`, no `get_` prefix); conversions use the `to_` and `into_` prefixes (`to_clean_bot_name`,
  `into_report`; `into_` when it consumes `self`, `to_` otherwise, as clippy sees it too); constructors that search the
  disk use `find_*`; predicates use `is_` and `has_`; an ID is always `*_id` (`caller_id`, `owner_id`).
* Add a suffix according to the return shape: lists are `*_list` (`my_bots_list`); status reports use `*_status_report`
  (`bot_and_anti_afk_status_report`); data sent as an attachment is `*_view` (`fishingbot_config_view`, `bot_log_view`).
  One-off probes, checks and text that is already written out in full use a bare noun (`ping`, `checkserver`, `help`,
  `whoami`).
* Config is accessed through `config::Config::get()`, and `config::Config::get().text()` selects its language.
  Access before config loading panics; unsupported `[i18n] language` values fail startup parsing. `text` depends on no
  other project module and has no separate language singleton. The watchdog uses
  `WATCHDOG` in `src/lib.rs` as singleton storage, accessed through `Watchdog::get_or_init()` (the first
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
* The deployment-specific `filter_regex` is in `[keywords]`; JAR status responses stay in i18n as `jar_*` methods and
  follow `[i18n] language`;
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
  [runtime-en_us.md](../runtime/runtime-en_us.md) (the primitives layer).
* Hard constraints and invariants: [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md).
