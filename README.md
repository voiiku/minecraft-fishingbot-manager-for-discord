<!-- The default README. docs/README/README-en_us.md is a symlink to this file, so the two cannot drift; tests/docs_links.rs checks the link. -->

# Minecraft FishingBot Manager for Discord

**English** | [简体中文](docs/README/README-zh_cn.md)

* **At present, Offline-mode servers and single-player worlds are not supported.**
* **At present, this project only runs on 64-bit Linux.**

## 1. Project overview

This project is an unofficial controller that uses Discord slash commands to manage multiple Minecraft auto-fishing bots powered by the separate [FishingBot](https://github.com/MrKinau/FishingBot) project.

The manager itself is written in Rust. Java is required to run the external `FishingBot.jar` instances.

How this project and FishingBot divide the work:
This project provides ways to manage FishingBot instances; it does not implement the bots’ behavior.

* Bot behavior (logging in, joining the server, fishing, and handling specific in-game events) is handled by FishingBot. Its behavior is controlled by a separate JSON config file, which **this project generates at startup from its own config**. If you run `FishingBot.jar` without this project, FishingBot generates that file itself.
* Bot management (starting, stopping, restarting, checking status, and managing permissions) is handled by this project.

## 2. Important notes and features

* Intended for people who have a Linux machine and need to manage multiple auto-fishing bots on a Minecraft server that uses **online-mode authentication**.
* Not intended for **offline-mode or single-player** players, people who need to run it natively on something other than 64-bit Linux, or people who only need one account to fish.
* The core goal is to be easy to use (within the context of a Linux environment): the project aims to avoid requiring users to read the code or compile it themselves, and to satisfy most needs through the config file. More customisation will come over time.

## 3. Deployment

See [docs/tutorial/tutorial-en_us.md](docs/tutorial/tutorial-en_us.md).

## 4. Usage

There are 18 top-level commands and 9 subcommands, for 27 commands in total (`/bot` and `/show` are parent commands and do nothing by themselves).

Available to everyone:

* `/help`: the help menu. The entire text comes from `text.help` in the config, you can change it.
* `/whoami`: view the caller’s Discord user ID.
* `/show status`: check the status of all running bots.
* `/ping [host] [port]`: probe a game server; without arguments, uses the defaults in the `[ping]` table.
* `/checkserver`: view the fishing server’s CPU, memory, disk, and uptime.
* `/show botconfig`: view the shared config file used by all fishing bots, sent as an attachment.
* `/show mybots`, `/show allbots`, `/show admins`: view your own bots, all bots, or the administrator list.
* `/bot add <name>`: add a bot (you must also set its token before it can be started).

Available to a bot’s owner (the person who created it) or an administrator:

* `/bot del <name>`, `/bot rename <old name> <new name>`: delete or rename a bot.
* `/bot login <name>`: sign in to a Microsoft account through a website and enter the code to obtain the bot token, which is then saved automatically on the server; the bot is started automatically when the login succeeds.
* `/token <name>`: manually enter a bot token (not recommended; use `/bot login` instead).
* `/log <name> [lines]`: view the latest log for a bot, sent as an attachment.
* `/start`, `/stop`, `/restart`: start, stop, or restart one bot.
* `/pause`, `/unpause`: pause fishing without disconnecting, or resume fishing (requires support in `FishingBot.jar`; other features are unaffected).
* `/command <name> <command>`: send a command to the game server where the bot is connected.

Available only to administrators:

* `/startall`, `/stopall`, `/restartall`, `/commandall <command>`: perform batch operations on all bots.

## 5. Project status

* Maintained by one person and still in testing. Config fields and command replies may change with updates; incompatible changes are announced in each release.
* A patched `FishingBot.jar` is recommended. The upstream version does not support the in-game `/pause` and `/unpause` commands, and its pause status is incorrect; all other features are unaffected.
* The project currently supports `ENGLISH` and `CHINESE_SIMPLIFIED` (the names used by the FishingBot project). See the [docs index](docs/index-en_us.md) for the available documentation. All English text and English documentation are AI translations.
* Out of scope: a web management panel (the author finds managing through a chat server simple enough) and a built-in proxy service.
* Future goal: support Windows, and a built-in fishing bot so you can choose freely between `FishingBot.jar` and the built-in one.

## 6. Project structure

See [docs/mods/architecture/architecture-en_us.md](docs/mods/architecture/architecture-en_us.md).

## 7. License

This project is licensed under the [MIT License](LICENSE).

[FishingBot](https://github.com/MrKinau/FishingBot) is a separate program licensed under [GPL-3.0](https://github.com/MrKinau/FishingBot/blob/main/LICENSE). This project only manages it; it does not contain FishingBot or distribute it bundled with this project.

## 8. Contributing
Contributions are welcome. Please [open an issue](https://github.com/voiiku/minecraft-fishingbot-manager-for-discord/issues) to report a bug or suggest a feature. Pull requests are also welcome; for significant changes, please open an issue first to discuss the approach.

### Translations

User-facing text is currently defined in the `src/text/` module and loaded at startup. When updating or adding a translation, follow the existing locale modules, such as `src/text/zh_cn.rs`. Messages shown before the language is loaded fall back to English.
  **The multilingual feature may be refactored in the future**


## 9. Other useful documents

See the [docs index](docs/index-en_us.md) for all documentation. The most frequently used documents are:

* [docs/tutorial/tutorial-en_us.md](docs/tutorial/tutorial-en_us.md): deployment from scratch, written for people new to Discord.
* [docs/troubleshooting/troubleshooting-en_us.md](docs/troubleshooting/troubleshooting-en_us.md): troubleshooting by symptom, including missing Discord commands, `/start` reporting that a command was sent when the bot is absent, incorrect status, orphaned bots, and startup errors after an upgrade.
* [docs/mods/config/config-en_us.md](docs/mods/config/config-en_us.md): config file location, loading process, field meanings, and startup validation.
* [docs/mods/commands/commands-en_us.md](docs/mods/commands/commands-en_us.md): the command layer plus every command.
* [docs/CONTEXTS/CONTEXT-en_us.md](docs/CONTEXTS/CONTEXT-en_us.md): read before changing code. Project overview, hard constraints and invariants, code map, and TODOs.
