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

* [`lib`](mods/lib/lib-en_us.md): the command entry layer (routing), including the action enum, `handle_*`, permission checks, error conventions, and the exit and reconnect entry points.
* [`commands` + `commands/`](mods/commands/commands-en_us.md): the command layer (poise), which reads options, validates numbers, `defer`s and turns `Err` into a reply; it contains seven business files: `bot_control`, `bot_manage`, `token`, `log`, `all_bots`, `show`, `general`.
* `bot_control` + `bot_control/`: single-bot operations, one file per command (`start`, `stop`, `restart`, `pause`, `unpause`, `command`, `log`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 3.
* `bot_manage` + `bot_manage/`: account management (`add`, `del`, `rename`, `token`, `login`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 4.
* `all_bots_control` + `all_bots_control/`: batch operations across all bots, admins only (`startall`, `stopall`, `restartall`, `commandall`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 5.
* `show`: read-only queries (`status`, `botconfig`, `mybots`, `allbots`, `admins`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 6.
* `general` + `general/`: general commands (`help`, `ping`, `checkserver`, `whoami`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 7.
* [`config` + `config/`](mods/config/config-en_us.md): the external config. `schema` (the structs corresponding to the TOML), `parse` (deserialisation and path expansion), `validate` (startup validation), `template` (the template generated on first start).
* [`text`](mods/i18n/i18n-en_us.md): text and languages. One language per process, selected from `[i18n] language` at startup: `en_us` (English), `zh_cn` (one to one with `en_us`, only the literals differ), `bootstrap` (English during bootstrap), `log_text`, `requirement`, `setup_failure`, `watchdog` (internal watchdog invariant notes, language-dependent).
* [`watchdog`](mods/watchdog/watchdog-en_us.md): the gateway watchdog. The only place that decides the connection is lost: the handshake and probe criteria, both pure timing; the process entry requests the reconnect.
* [`process_log`](mods/process-log/process-log-en_us.md): the program log, written to both the terminal (stderr) and `<deploy dir>/logs/<timestamp>.log`.
* [`utils`](mods/utils/utils-en_us.md): the primitives layer, providing reusable capabilities for upper layers: `bot_command`, `bot_dirs`, `bot_name`, `bot_logs`, `bot_status`, `fishingbot_config`, `fs`, `permission`, `reply`, `restart`, `running_bots`, `screen`, `shutdown`, `startup`, `uptime`.

## 4. Commands

* [commands-en_us.md](mods/commands/commands-en_us.md): the command layer and every command. Section 1 explains the command-layer rules; sections 3-7 document every command by module (single-bot control, account management, batch operations, queries, general).

## 5. Reference

* [refs/BOT_COMMANDS_AND_MESSAGES-en_us.md](refs/BOT_COMMANDS_AND_MESSAGES-en_us.md): the FishingBot JAR's commands and messages (Chinese and English side by side), used to check configured keywords and echoes. This is a static reference and is not part of the code-change flow.
* [CONTEXTS/CONTEXT-en_us.md](CONTEXTS/CONTEXT-en_us.md): read this before changing code. It covers the project overview, hard constraints and invariants, the code map and current TODOs, for the next maintainer or AI session.
