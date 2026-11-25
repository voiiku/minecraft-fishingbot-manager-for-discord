# i18n: the language the program runs in
[简体中文](i18n-zh_cn.md) | **English**

* Mechanism: `src/text/mod.rs` defines `Language`, `Text`, `LogMessages` and `WatchdogText`;
  `src/text/languages/` contains the English and Simplified Chinese implementations. `bootstrap` provides startup-specific
  English messages and `report` formats shared reports. `text` depends on no other project modules: language comes from
  `Config::get().text()`, the crate-root `bootstrap_error_text()` and `complete_startup()` manage the startup error
  language boundary, and callers supply each report's language and length limit.
* Config: `[i18n] language = "ENGLISH"` (what the example config ships; change it to `CHINESE_SIMPLIFIED` for a
  Chinese deployment). One language per process, read once at startup, restart to change it; the values are the
  constant names of FishingBot's own `i18n.Language` enum - this project implements only `ENGLISH` (English) and
  `CHINESE_SIMPLIFIED` (Simplified Chinese), and the values the upstream enum also has
  (`CHINESE_TRADITIONAL`, `FRENCH`, `GERMAN`, ...) are rejected at startup, because we have not written those texts.
* Switching language needs no rebuild: both implementations are already compiled into the binary; the user changes
  that one config line and restarts the process. Only adding a third language touches code (section 2).
* What is not in i18n is deliberately not in it, next section.

## 1. The boundary: Discord texts, and the fixed texts used to match logs

Included:

* Command replies and error messages (the `Err` and `String` a business layer returns).
* The column names and status text in the `/show status` table.
* `Report` section titles and the "content too long" note.
* Program logs and runtime errors use `[i18n] language`. Configuration and other startup errors use English until
  the first successful framework setup; existing non-error startup logs continue to use the configured language.
* The JAR's `/status` response prefix and unavailable message.

Deliberately not included:

* The contents of the bot logs themselves: FishingBot prints those and we cannot change them. Only the fixed texts it
  prints itself come in, see below.
* Internal invariant messages: the `expect(...)` and `panic!` texts that only appear when the code itself is wrong,
  plus `cargo test` assertion texts. Apart from the 14 watchdog lock-poisoning texts, they are not in the language
  tables (they do not implement `Text`); the one that must be fixed English lives in `bootstrap`, see section 4. The
  watchdog texts are the exception: startup errors use English and later errors use `[i18n] language`.
* Code comments, `docs/` and command names (Discord only allows lowercase ASCII). Command and option descriptions
  live only in the language tables and are validated before registration (see
  [commands-en_us.md section 1.7.1](../commands/commands-en_us.md#171-command-text-validation)).
* The example config's comments are fixed English, separate from code comments.
* The deployment-specific log filter stays in `[keywords]`.

### Why the JAR's own fixed texts come in, and log content does not

There is one criterion: who decides this string.

* The `/status` response prefix and unavailable message also follow the JAR's language and live in the language table.
* The deployment-specific `filter_regex` stays in `[keywords]`.
* "Process finished" is neither printed by the jar nor configured: it reads the screen session state (a `(Dead ...)`
  shell, see [runtime-en_us.md](../runtime/runtime-en_us.md)). The end marker seen in logs was written by an older
  wrapper script (`>>> 机器人进程已结束或被手动中止 <<<`); this project does not write it, so there is nothing to
  translate.

Change the game server or a plugin and you touch `[keywords]`; change the JAR's config language (or change the JAR version and the texts changed with it)
and you set `[i18n] language` to the matching language (when the texts really did change, edit those two sentences in
`src/text/languages/*.rs`).

## 2. How to add a language (three steps, skip one and it does not compile)

1. Add a file under `src/text/languages/` implementing every method of `Text` and `LogMessages`. Traits rather than a
   `HashMap<&str, &str>` precisely so that a missing translation fails the build.
2. Add `mod <file name>;` to `src/text/languages/mod.rs`, add an enum variant to `Language`, and add branches to
   `Language::from_name()`, `Language::name()` and `Language::implementation()`.
3. Add the language name to `Language::supported_names()` and keep it consistent with parsing and `name()`.
   Copy the value from FishingBot's own `i18n.Language` enum constants - it is also what goes into
   `config.json` as `misc.language`, so do not invent another set of names (that would add a mapping table).

No business layer and no config parsing changes are needed: the business layer only calls `config::Config::get().text()`.

## 3. Why not external language files (`.ftl`, `gettext`, a language pack toml)

* This project wants one binary plus one config to run: an external language pack is a third file that has to travel
  along, and losing it leaves the program without any texts.
* Compile-time completeness beats a run-time lookup: a missed translation is a build error, not an empty string or a
  bare key shown to a user in chat.
* Two dependencies less (no Fluent, no `rust-i18n`).

The cost is that adding a language needs a rebuild (switching between the ones that exist does not: both
implementations are in the binary, change the config). That fits the trade-offs this project already makes: long text
meant to be changed without a rebuild (`/help`) lives in the config as `text.help` anyway, so an English user who
wants `/help` in English just replaces `text.help` (it was always meant to be written by the user).

## 4. Reading texts

* Business and runtime modules call `config::Config::get().text().xxx(...)` for localized messages without depending on the Discord framework.
* Log and console texts use named `LogMessages` methods directly, for example
  `config::Config::get().text().process_log_path(path)`. `Text` inherits `LogMessages`, so missing methods fail compilation in either
  language. The log level (`info!`, `warn!`) remains the caller's responsibility.
* Parsing converts the language name into `Language`, reporting unsupported names in English; the complete config is
  published only after validation. `config::Config::get().text()` selects its language without a separate installed
  language singleton. Accessing config before loading it panics; tests load config explicitly or select a language directly.
  The same name is written into the game-side `config.json` as `misc.language`
  (see [config.md section 3.2](../config/config-en_us.md)).
* `text` does not depend on other project modules; callers supply the language and reply limit when creating a report.
  Startup-specific configuration and game-side configuration errors are rendered by `text::bootstrap` in English.
* Shared errors use the crate-root `bootstrap_error_text()`: English until the first framework setup succeeds, then the configured
  language. Setup completes after the sync-command registration attempt; a per-server registration warning does not
  prevent completion. Failed setup and retries before the first success remain in English; later reconnects do not
  reset the boundary. External error details are preserved as supplied.
  An application embedding the library must call the crate-root `complete_startup()` after its own successful setup;
  otherwise shared errors remain in English.
* Informational messages and command descriptions still use `config::Config::get().text()` and therefore the configured language.
* Watchdog mutex-poisoning texts use `WatchdogText` and `Text::watchdog_text`; they follow the same startup error
  boundary. Static strings avoid allocating on every lock operation.
* Registration-time texts such as command descriptions also use the loaded configuration.

## 5. Layout rules: symbols follow the language, asides use round brackets

Symbols follow the language:

* Chinese: full-width Chinese punctuation (`，`, `：`, `！`, `；`, `……`), with no full stop at the end of a sentence.
* English: plain ASCII punctuation (`.`, `,`, `:`, `!`, `;`, `...`) only; no `…` or `—` and no Chinese punctuation;
  no full stop at the end of a sentence either.

Asides (the explanation in brackets) are written `()` in both languages, for example the description of the `/log`
option: `读取最新多少行 (默认 50，上限见配置 runtime.max_reply_log_lines)` and
`how many trailing lines to read (default 50, cap in runtime.max_reply_log_lines)`. Three exceptions are marks that look
like brackets but are not asides, and keep their own form:

* Config table names copy the brackets: `[admins]`, `[keywords]`, `[raw_game_logs]`.
* `Report` section labels: `[完成明细]`, `[异常明细]`, `[Completed]`, `[Issues]`.
* Argument marks in usage hints: optional arguments as `[行数]` and required ones as `<机器人名>`, matching the
  notation in [README-en_us.md](../../README/README-en_us.md).

Error replies always start with `错误:` in Chinese and `Error: ` in English, so "this is a failure" and "this is a
result" are told apart at a glance; plain result lines (the `/ping` latency, the `/startall` details) carry no prefix.

The reason: with one bracket style the two versions have exactly the same skeleton (only words and punctuation
differ), which makes them easy to compare line by line when maintaining; half-width parentheses keep a stable spacing
in mixed Chinese and English, while `[]` and `<>` are left to table names, section labels and argument marks, so they
do not read as asides. The command description table (`command_texts()`) and the default `/help` body follow the same
rules.

## 6. Coverage and verification

The following text is provided through `Text` and the related enums:

* `report.rs`: `Report` section labels, the "content too long" note, the count line `count_line`.
* `runtime/bot_instances/bot_status.rs`: the `/show status` table and fixed `jar_*` texts for status responses.
* `text::bot_name_error`: localizes validation errors from `utils::bot_name`.
* `runtime/bot_instances/bot_dirs.rs` (including the `Display` of `BotTokenStatus`) and `runtime/bot_instances/bot_sessions.rs` (including the
  `/show status` column names).
* `runtime/bot_instances/screen.rs`, `runtime/bot_startup/mod.rs`, `runtime/bot_instances/stop.rs`: the detail lines of the start and stop flows.
* `operations/`: business operations and query results.
* `operations/mod.rs` uses the not-an-admin refusal; `discord_commands/` uses progress messages, attachment bodies and usage hints.
* Command metadata: command and option descriptions are supplied by the language table before registration; `/token`
  modal labels are read when the modal is created.
* Program logs use the named methods of `LogMessages`. Startup-specific configuration errors live in `bootstrap`.
* Watchdog lock operations use `WatchdogText` and `watchdog_text()`, with the startup error boundary described above.

Out on purpose, by the boundary above: internal invariant messages (`expect(...)`, `panic!`) and test assertions. The
sole exception is the watchdog lock-poisoning texts.

Both language implementations must implement every `Text` and `LogMessages` method and exhaustively match
`WatchdogText`; missing implementations fail compilation. Tests also validate the command-description tables.

The wording follows the Chinese: change `zh_cn.rs` first, then align `en_us.rs`. A test process may not install a
language, so `get()` returns the English implementation and wording assertions use English as their baseline.

### Command metadata (command descriptions, option descriptions, the `/token` modal)

These texts follow the configured language and are not written into the command macros:

* Command and option descriptions: after `discord_commands::all_commands()` has built the command tree,
  `Text::command_texts()` is validated and supplies poise's `Command.description` and `CommandParameter.description`.
* The `/token` modal title, field labels and placeholders: instead of `#[derive(Modal)]`, a hand-written
  `impl poise::Modal` takes the texts in `create()` and the values in `parse()` by `custom_id` (the shape is exactly
  what derive would generate).

So these texts follow the config language too (not the Discord client language), the same switch as the replies.

The command tree and selected language table must match: framework construction rejects missing, duplicate or unknown
entries and invalid description lengths rather than registering placeholders. Tests check both languages and the
serialized registration structure.

The difference from Discord's own `description_localized`: that lets Discord pick which version to show by client
language, a different mechanism from "one language per process". This project does not use it, otherwise a
description could be English while a reply is Chinese.

## 7. Related documents

* All documents: [docs index](../../index-en_us.md).
* Where command descriptions, option descriptions and modal labels come from:
  [commands-en_us.md](../commands/commands-en_us.md).
* Invariants about texts and log matching: [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 2.2.
