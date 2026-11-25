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

* [`lib`](mods/lib/lib-en_us.md): public command APIs, runtime components and view types.
* [`discord_commands`](mods/commands/commands-en_us.md): reads Discord options, calls the corresponding operation, and sends replies. For example, `/start` calls `operations::single_bot_control::prepare_start_single_bot` directly.
* `operations`: single-bot and batch control, accounts, queries and anti-AFK control. Restricted operations check permissions at entry. See [commands-en_us.md](mods/commands/commands-en_us.md) sections 3–7.
* [`config` + `config/`](mods/config/config-en_us.md): the external config. `schema` (the structs corresponding to the TOML), `parse` (deserialisation and path expansion), `validate` (startup validation), `template` (the template generated on first start).
* [`text`](mods/i18n/i18n-en_us.md): localized text and reports; `languages` contains the English and Chinese implementations, while `bootstrap` contains startup-specific English errors.
* [`watchdog`](mods/watchdog/watchdog-en_us.md): the gateway watchdog. The only place that decides the connection is lost: the handshake and probe criteria, both pure timing; the process entry requests the reconnect.
* [`process_log`](mods/process-log/process-log-en_us.md): the program logger under `runtime`, writing to the terminal and a log file.
* [`runtime`](mods/runtime/runtime-en_us.md): bot directories, sessions, status, the startup queue and shared file, name and uptime helpers.

## 4. Commands

* [commands-en_us.md](mods/commands/commands-en_us.md): the command layer and every command. Section 1 explains the command-layer rules; sections 3-7 document every command by module (single-bot control, account management, batch operations, queries, general).

## 5. Reference

* [refs/BOT_COMMANDS_AND_MESSAGES-en_us.md](refs/BOT_COMMANDS_AND_MESSAGES-en_us.md): the FishingBot JAR's commands and messages (Chinese and English side by side), used to check configured keywords and echoes. This is a static reference and is not part of the code-change flow.
* [CONTEXTS/CONTEXT-en_us.md](CONTEXTS/CONTEXT-en_us.md): read this before changing code. It covers the project overview, hard constraints and invariants, the code map and current TODOs, for the next maintainer or AI session.
