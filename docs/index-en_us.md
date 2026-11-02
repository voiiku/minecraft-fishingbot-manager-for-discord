# Documentation index
**English** | [简体中文](index-zh_cn.md)

The Simplified Chinese README, deployment tutorial and troubleshooting guide are author-written. The remaining Chinese documents are currently AI-generated and will be improved over time; all English documents are AI translations and are still under review.

## 1. README

* [English](./README/README-en_us.md)
* [简体中文](./README/README-zh_cn.md)

## 2. Getting started

* [Deployment tutorial](tutorial/tutorial-en_us.md)
* [Troubleshooting](troubleshooting/troubleshooting-en_us.md)
* [The config file](mods/config/config-en_us.md)

## 3. Module documentation

For the overview (layers, directory tree and the full path of one command), see [architecture-en_us.md](mods/architecture/architecture-en_us.md).

The remaining module documents:

* [`lib`](mods/lib/lib-en_us.md): the public library interface; `handlers/routes` contains action enums, `handle_*` routes and permission checks.
* [`discord_commands`](mods/commands/commands-en_us.md): the Discord command layer (poise), which reads options, validates numbers, `defer`s and turns `Err` into a reply; its command categories are `bot_control`, `bot_manage`, `token`, `log`, `all_bots`, `show`, and `general`.
* `handlers`: business entry points for bot control, account management, bulk operations, read-only queries and general commands. See [commands-en_us.md](mods/commands/commands-en_us.md) sections 3–7.
* [`config` + `config/`](mods/config/config-en_us.md): the external config. `schema` (the structs corresponding to the TOML), `parse` (deserialisation and path expansion), `validate` (startup validation), `template` (the template generated on first start).
* [`text`](mods/i18n/i18n-en_us.md): text and languages. One language per process, selected from `[i18n] language` at startup: `en_us` (English), `zh_cn` (one to one with `en_us`, only the literals differ), `bootstrap` (English during bootstrap), `log_text`, `requirement`, `setup_failure`, `watchdog` (internal watchdog invariant notes, language-dependent).
* [`watchdog`](mods/watchdog/watchdog-en_us.md): the gateway watchdog. The only place that decides the connection is lost: the handshake and probe criteria, both pure timing; the process entry requests the reconnect.
* [`process_log`](mods/process-log/process-log-en_us.md): the program logger under `runtime`, writing to the terminal and a log file.
* [`utils`](mods/utils/utils-en_us.md): globally shared utilities, including bot directories, logs, status, the startup queue and the process logger.

## 4. Commands

* [commands-en_us.md](mods/commands/commands-en_us.md): the command layer and every command. Section 1 explains the command-layer rules; sections 3-7 document every command by module (single-bot control, account management, batch operations, queries, general).

## 5. Reference

* [refs/BOT_COMMANDS_AND_MESSAGES-en_us.md](refs/BOT_COMMANDS_AND_MESSAGES-en_us.md): the FishingBot JAR's commands and messages (Chinese and English side by side), used to check configured keywords and echoes. This is a static reference and is not part of the code-change flow.
* [CONTEXTS/CONTEXT-en_us.md](CONTEXTS/CONTEXT-en_us.md): read this before changing code. It covers the project overview, hard constraints and invariants, the code map and current TODOs, for the next maintainer or AI session.
