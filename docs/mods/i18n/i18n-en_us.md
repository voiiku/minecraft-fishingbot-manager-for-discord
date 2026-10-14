# i18n: the language the program runs in
[简体中文](i18n-zh_cn.md) | **English**

* Mechanism: `src/text.rs` (`Language`, the `Text` trait, `install`, `get`) and `src/text/*.rs` (one file per
  language: `en_us.rs` is the default language, `zh_cn.rs` is Simplified Chinese; plus four submodules: `bootstrap`
  (English during bootstrap), `log_text` (`LogText`), `requirement` (`ConfigRequirement`) and `setup_failure` (which
  text a setup failure uses, and whether it counts as permanent)).
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
* The words in the `/show status` table (the `Display` of `ConnectionStatus` and `PauseStatus`).
* `Report` section titles and the "content too long" note.
* The program log, the bootstrap console notes and config errors. They are for the deployer, but they follow
  `[i18n] language` all the same (how that works is section 4); the only exception is the notes written before the
  language has been read, which are fixed English (`src/text/bootstrap.rs`).
* The JAR's own reconnect message (`jar_reconnect_prefix`, `jar_reconnect_suffix`), used to recognise "it
  reconnected" in a bot log.

Deliberately not included:

* The contents of the bot logs themselves: FishingBot prints those and we cannot change them. Only the fixed texts it
  prints itself come in, see below.
* Internal invariant messages: the `expect(...)` and `panic!` texts that only appear when the code itself is wrong,
  plus `cargo test` assertion texts. They are not in the language tables (they do not implement `Text`); the one that
  must be fixed English lives in `bootstrap`, see section 4.
* Code comments, `docs/` and command names (Discord only allows lowercase ASCII). Exceptions: the first paragraph of
  a command's `///` and an option's `#[description]` become the text sent to Discord, and those two are written in
  English (the same sentence as in the default language table, see
  [commands-en_us.md section 1.7.1](../commands/commands-en_us.md#171-what-the-copy-inside-the-macro-is-the-english-fallback)).
* The example config's comments are fixed English: they are for people who edit the config by copying the template,
  and they are kept apart from the Chinese code comments. Only the few entries whose value changes with the language
  and must be changed for a Chinese deployment carry one extra `# CHINESE_SIMPLIFIED:` line (the two `command_echo`
  echoes and `[i18n] language`) - that is not a second explanation but a value hint, and whoever changes the value
  sees it at the same time, so it cannot drift.
* Keywords tied to the game server or the local machine: the server's own join message, the local machine's end
  marker, and the spam noise to drop on your own server. They differ per deployment and stay in `[keywords]`.
* The `/pause` and `/unpause` echoes: the jar prints them, but they only exist in a patched build with those two
  commands, so they stay in `[command_echo]` (`pause_echo`, `unpause_echo`); change them in place when you change
  builds, no rebuild needed.

### Why the JAR's own fixed texts come in, and log content does not

There is one criterion: who decides this string.

* `bot-automatic-reconnect` (the reconnect message) is decided by the JAR's own language resources: a Chinese JAR
  says the Chinese sentence, an English JAR says the English one, whatever game server or machine it runs on. Making
  the deployer copy it into the config turns something the program already knows into a chance for a human error
  (one punctuation mark copied wrong and `/show status` shows "initializing" forever). So it follows `[i18n] language`:
  run whichever language of JAR you have and set `language` to that language. Both strings are written one to one in
  `src/text/zh_cn.rs` and `src/text/en_us.rs`, and the test
  `text::tests::jar_reconnect_messages_are_non_empty_and_distinct_in_both_languages` watches them (non-empty in both
  languages, and the two versions differ).
* `command-pause-executed` and `command-unpause-executed` (the two command echoes) are decided by the JAR as well,
  but they only exist in a build carrying the `/pause` and `/unpause` patch (the upstream build has no such
  commands), so they live in `[command_echo]` as `pause_echo` and `unpause_echo`: change the config when you change
  the jar build, no rebuild needed. Startup validation requires both to be non-empty and neither to contain the other
  (otherwise that log line cannot be told apart).
* The other way round, `online` (the join message, from the game server or a plugin) and `filter_regex` (the spam
  noise of your own server) are only known to the deployer and the JAR has no matching sentence, so they stay in
  `[keywords]`, where startup validation requires them to be non-empty.
* "Process finished" is neither printed by the jar nor configured: it reads the screen session state (a `(Dead ...)`
  shell, see [utils-en_us.md](../utils/utils-en_us.md)). The end marker seen in logs was written by an older
  wrapper script (`>>> 机器人进程已结束或被手动中止 <<<`); this project does not write it, so there is nothing to
  translate.

Change the game server or a plugin and you touch `[keywords]`; change the jar build and you look at the two
`[command_echo]` echoes; change the JAR's config language (or change the JAR version and the texts changed with it)
and you set `[i18n] language` to the matching language (when the texts really did change, edit those two sentences in
`src/text/*.rs`).

## 2. How to add a language (three steps, skip one and it does not compile)

1. Add a file under `src/text/` implementing every method of `Text`. A trait rather than a
   `HashMap<&str, &str>` precisely so that a missing translation fails the build.
2. Add `mod <file name>;` to `src/text.rs`, add an enum variant to `Language`, and add a branch to the `match` in
   `Language::implementation()` (both the `install()` and the `get()` fallbacks come from there).
3. Add the language name to `Language::supported_names()`. That is the single source of valid values: config
   validation, error texts, the example config and this document all take it from there, so there are never two lists
   to drift apart. Copy the value from FishingBot's own `i18n.Language` enum constants - it is also what goes into
   `conf.json` as `misc.language`, so do not invent another set of names (that would add a mapping table).

No business layer and no config parsing changes are needed: the business layer only calls `text::get()`.

## 3. Why not external language files (`.ftl`, `gettext`, a language pack toml)

* This project wants one binary plus one config to run: an external language pack is a third file that has to travel
  along, and losing it falls back to the default language.
* Compile-time completeness beats a run-time lookup: a missed translation is a build error, not an empty string or a
  bare key shown to a user in chat.
* Two dependencies less (no Fluent, no `rust-i18n`).

The cost is that adding a language needs a rebuild (switching between the ones that exist does not: both
implementations are in the binary, change the config). That fits the trade-offs this project already makes: long text
meant to be changed without a rebuild (`/help`) lives in the config as `text.help` anyway, so an English user who
wants `/help` in English just replaces `text.help` (it was always meant to be written by the user).

## 4. How it is read, and the fallback

* Business layers and utils always call `text::get().xxx(...)` (they do not touch the framework, only this).
* Log and console texts go through `text::get().log_text(LogText::...)`. There are many of those and they have a
  uniform shape, so they are funnelled through "an enum carrying data plus one method": the `match` in both language
  files is exhaustive, so a missing translation, or a variant implemented on one side only, does not compile (the
  same guarantee as method-by-method). The level (`info!`, `warn!`) is still decided at the call site, because that
  is semantics rather than text.
* Without an installed language, `get()` falls back to the default (English). The default language is defined in
  exactly one place, `Language::default()`, and `get()` writes both branches out in full (no `unwrap_or`) so that
  "which language do we fall back to" is visible at a glance. Unit tests therefore do not each have to install a
  language, and text assertions fall into two kinds:
  1. Wiring (which table this sentence comes from, and that it follows the language): expected values come from
     `text::get()` (the JAR echoes, a bot name error, ...), so changing the default language does not turn them red;
  2. Wording (sentences assembled from a field, a value and a requirement, such as config errors): pinned to
     fragments of the default language (English).

  That is a different thing from the deployment default: the example config ships `language = "ENGLISH"` (the same
  as the fallback language in code; change it to `CHINESE_SIMPLIFIED` for a Chinese deployment), which only decides
  which language a new deployment runs in and says nothing about whether the code's fallback is English. It is also
  the value written into the generated `conf.json` as `misc.language` (the same value, not a mapped one, see
  [config.md section 3.2](../config/config-en_us.md)), so the jar speaks the same language and the two can
  no longer disagree.
* When the language is installed decides which messages can only be bootstrap English:
  1. `config::get_or_init()` installs once right after parsing succeeds, so config validation errors are in the configured
     language too;
  2. `main.rs` does not install a second time. The language is already installed in step 1, so `init_process_log()`
     starting the program log also uses the configured language. The entry point deliberately omits a second
     `install()`, because it would have no effect and would only blur "where is the language chosen";
  3. Anything sent before step 1 is fixed English: the config file is missing, cannot be read or fails to parse, the
     template cannot be generated, the executable path cannot be located, `[i18n] language` is itself wrong, and the
     `expect` for calling `config::get()` before the config is loaded (`bootstrap::CONFIG_NOT_LOADED`). No language
     is available then, and English is the most neutral fallback. They all live in `crate::text::bootstrap`
     (alongside the language tables but not implementing `Text`). The single criterion: is this note a product of the
     config not having been read successfully.
* Registration-time texts such as command descriptions read the same `get()`: `commands::all_commands()` calls it
  once after building the command tree (by then `install()` has run).

## 5. Layout rules: symbols follow the language, asides use round brackets

Symbols follow the language:

* Chinese: full-width Chinese punctuation (`，`, `：`, `！`, `；`, `……`), with no full stop at the end of a sentence.
* English: plain ASCII punctuation (`.`, `,`, `:`, `!`, `;`, `...`) only; no `…` or `—` and no Chinese punctuation;
  no full stop at the end of a sentence either.

Asides (the explanation in brackets) are written `()` in both languages, for example the description of the `/log`
option: `读取最新多少行 (默认 50，上限见配置 runtime.max_log_lines)` and
`how many trailing lines to read (default 50, cap in runtime.max_log_lines)`. Three exceptions are marks that look
like brackets but are not asides, and keep their own form:

* Config table names copy the brackets: `[admins]`, `[keywords]`, `[raw_game_logs]`.
* `Report` section labels: `[成功明细]`, `[内容]`, `[Success]`, `[Content]`.
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

* `utils/reply.rs`: `Report` section labels, the "content too long" note, the count line `count_line`.
* `utils/bot_status.rs`: the words in the `/show status` table and the fixed `jar_*` texts (the reconnect message prefix
  and suffix; the `/pause` and `/unpause` echoes moved to `[command_echo]` for the "changes with the build" reason,
  section 1).
* `utils/bot_name.rs`: bot name errors.
* `utils/bot_dirs.rs` (including the `Display` of `BotTokenStatus`) and `utils/running_bots.rs` (including the
  `/show status` column names).
* `utils/screen.rs`, `utils/startup.rs`, `utils/shutdown.rs`: the detail lines of the start and stop flows.
* `bot_control/*`, `bot_manage/*`, `all_bots_control/*`, `show/*`, `general/*`.
* `lib.rs` (the not-an-admin refusal) and the fixed command-layer texts ("started running", attachment bodies, usage
  hints, `/whoami`).
* Command metadata: command descriptions, option descriptions and the `/token` modal labels (the macro-generated
  command tree is rewritten before registration, see below).
* The program log (`log::*`), the bootstrap console notes and config errors (`config.rs` and `config/*`): through the
  `LogText` enum and `log_text()`, with an exhaustive `match` in both languages.

Out on purpose, by the boundary above: internal invariant messages (`expect(...)`, `panic!`) and test assertions.

`Text` currently has 153 methods (including `log_text`), 27 command descriptions, plus 67 `LogText` variants and 21
`ConfigRequirement` variants, all of them with a call site. You can check that yourself (the first command cuts the
`pub trait Text` block out of `src/text.rs` with `sed` and deduplicates the `fn` names, so a signature written
across lines cannot be missed - that is how `command_log_attachment_body` is written; that first command also lists
methods without a call site, and prints nothing when all is well; the last two count the variants of the two enums,
and since the `match` is exhaustive the names that appear in a language file are exactly the variants):

```bash
sed -n '/^pub trait Text/,/^}/p' src/text.rs | grep -oE 'fn [a-z_0-9]+' | sort -u | wc -l  # 153
sed -n '/^pub trait Text/,/^}/p' src/text.rs | grep -oE 'fn [a-z_0-9]+' | sed 's/fn //' | sort -u | \
while read -r m; do [ "$(grep -rn "\.$m(" src/ --include='*.rs' | grep -vc '^src/text')" -eq 0 ] && echo "未被调用: $m"; done
grep -oE 'LogText::[A-Za-z]+' src/text/en_us.rs | sort -u | wc -l            # 67
grep -oE 'ConfigRequirement::[A-Za-z]+' src/text/en_us.rs | sort -u | wc -l  # 21
```

The English implementation `src/text/en_us.rs` matches the Chinese one completely (153 methods, 27 command
descriptions, 67 and 21 enum variants, not one missing: a missing method or `match` branch does not compile, a missing
command description fails a test). To confirm English really comes up, set `language = "ENGLISH"` in the config and
run once; the unit test `text::tests::english_impl_is_english` asserts against the English implementation directly
(it does not call `install`, because `OnceLock` is process-wide and installing would disturb other tests running in
parallel).

The wording follows the Chinese: to change a text, change `zh_cn.rs` first (the original of both content and layout),
then align `en_us.rs` line by line, and finally sync the fallback in `commands/*.rs`, which is word for word the same
as the English table. The English table has a second role, "the default language" (where `get()` falls back to, and
the baseline of wording assertions), but that is about the mechanism, not about which file is the original.

### Command metadata (command descriptions, option descriptions, the `/token` modal)

These texts are written into the binary at compile time by the `#[poise::command]` and `#[derive(Modal)]` macros,
which only accept literals, so they cannot be changed at run time. The way round it is to rewrite the texts once
before registration (and once only):

* Command and option descriptions: after `commands::all_commands()` has built the command tree,
  `Text::command_texts()` (the table) overwrites poise's `Command.description` and `CommandParameter.description` one
  by one (both fields are public in poise and can be written directly).
* The `/token` modal title, field labels and placeholders: instead of `#[derive(Modal)]`, a hand-written
  `impl poise::Modal` takes the texts in `create()` and the values in `parse()` by `custom_id` (the shape is exactly
  what derive would generate).

So these texts follow the config language too (not the Discord client language), the same switch as the replies.

How completeness is guaranteed: command descriptions use "a table plus an overwrite test" instead of dozens of trait
methods, because the metadata maps one to one to the names in `src/commands/*.rs` and a table is easier to compare.
Two tests in `commands` pin this down: `every_command_and_option_has_text_in_every_language` checks every command and
every option per language (listing whatever is missing), and `command_descriptions_come_from_the_language_table`
proves the overwrite really happens at registration (rather than the table just sitting there). The effect equals a
missing translation failing the build, only it happens at test time.

The difference from Discord's own `description_localized`: that lets Discord pick which version to show by client
language, a different mechanism from "one language per process". This project does not use it, otherwise a
description could be English while a reply is Chinese.

## 7. Related documents

* All documents: [docs index](../../index-en_us.md).
* Where command descriptions, option descriptions and modal labels come from:
  [commands-en_us.md](../commands/commands-en_us.md).
* Invariants about texts and log matching: [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 2.2.
