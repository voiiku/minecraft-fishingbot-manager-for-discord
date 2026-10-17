# docs: documentation index
**English** | [简体中文](index-zh_cn.md)

The Simplified Chinese README, deployment tutorial and troubleshooting guide are author-written. The other Chinese documents are AI-generated (to be improved over time), and every English document is an AI translation; they are being reviewed and improved. See each document for its scope and status.

## 1. README list

* [English](./README/README-en_us.md)
* [简体中文](./README/README-zh_cn.md)

## 2. Getting started

* [Deployment tutorial](tutorial/tutorial-en_us.md)
* [Troubleshooting](troubleshooting/troubleshooting-en_us.md)
* [The config file](mods/config/config-en_us.md)

## 3. Modules

The overview (layers, the directory tree, the full path of one command) is
[architecture-en_us.md](mods/architecture/architecture-en_us.md).

Module documents:

* [`lib`](mods/lib/lib-en_us.md): the command entry layer (routing). Action enum, `handle_*`, permission check, error convention, exit and reconnect entry points.
* [`commands` + `commands/`](mods/commands/commands-en_us.md): the command layer (poise). Reads options, checks numbers, `defer`s, turns `Err` into a reply; seven business files: `bot_control`, `bot_manage`, `token`, `log`, `all_bots`, `show`, `general`.
* `bot_control` + `bot_control/`: single-bot operations, one file per command (`start`, `stop`, `restart`, `pause`, `unpause`, `command`, `log`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 3.
* `bot_manage` + `bot_manage/`: account management (`add`, `del`, `rename`, `token`, `login`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 4.
* `all_bots_control` + `all_bots_control/`: batch operations, admins only (`startall`, `stopall`, `restartall`, `commandall`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 5.
* `show`: read-only queries (`status`, `botconfig`, `mybots`, `allbots`, `admins`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 6.
* `general` + `general/`: general commands (`help`, `ping`, `checkserver`, `whoami`). See [commands-en_us.md](mods/commands/commands-en_us.md) section 7.
* [`config` + `config/`](mods/config/config-en_us.md): the external config. `schema` (the structs, one to one with the TOML), `parse` (deserialisation and path expansion), `validate` (startup validation), `template` (the template written on first start).
* [`text`](mods/i18n/i18n-en_us.md): text and languages; one language per process (chosen from `[i18n] language` at startup). `en_us` (English), `zh_cn` (one to one with `en_us`, only literals differ), `bootstrap` (English during bootstrap), `log_text`, `requirement`, `setup_failure`.
* [`watchdog`](mods/watchdog/watchdog-en_us.md): the gateway watchdog; the only place that decides the connection is lost (handshake and probe criteria, pure timing - the process entry asks for the reconnect).
* [`process_log`](mods/process-log/process-log-en_us.md): the program log, written to the terminal (stderr) and to `<deploy dir>/logs/<timestamp>.log`.
* [`utils`](mods/utils/utils-en_us.md): the primitives layer: `bot_command`, `bot_dirs`, `bot_name`, `bot_logs`, `bot_status`, `fishingbot_config`, `fs`, `permission`, `reply`, `restart`, `running_bots`, `screen`, `shutdown`, `startup`, `uptime`.

## 4. Commands

* [commands-en_us.md](mods/commands/commands-en_us.md): the command layer and every command. Section 1 has the command-layer rules; sections 3 to 7 document every command by module.

## 5. Reference

* [refs/BOT_COMMANDS_AND_MESSAGES-en_us.md](refs/BOT_COMMANDS_AND_MESSAGES-en_us.md): the patched FishingBot JAR's commands and messages, shown in Chinese and English side by side. Use it to check configured keywords and command echoes; it is a static reference.
* [CONTEXTS/CONTEXT-en_us.md](CONTEXTS/CONTEXT-en_us.md): read before changing code. Project overview, hard constraints, invariants, code map and current TODOs.
