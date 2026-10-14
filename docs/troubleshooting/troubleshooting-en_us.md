# Troubleshooting
**English** | [简体中文](troubleshooting-zh_cn.md)

Some common problems and tips.

## 1. Where are the logs?

This program keeps two kinds of logs:

* Program log: `<deployment directory>/logs/year-month-day-hour-minute-second.log`. A new file is created at every startup; administrators clean them up by hand.
* Fishing bot logs (after filtering): `<deployment directory>/bots/<user ID>/<bot name>/logs/year-month-day-hour-minute-second.log`. This is what `/log` reads.

FishingBot.jar’s own raw log is directed at `/dev/null` and discarded, so on disk each fishing bot only keeps the filtered log. You can change that in the config.

## 2. The project’s commands do not appear after I type `/` in Discord

1. The invite link did not include the `applications.commands` scope. Invite the bot again using the link printed in the program’s startup log.
2. Commands are registered globally, so it can take up to an hour for them to propagate.
3. The program is not running. Check `systemctl status minecraft-fishingbot-manager-for-discord` or inspect the logs.

## 3. Command interaction failed

1. The bot crashed unexpectedly. Check the newest program log under `<deployment directory>/logs/`; use
   `/log <bot name>` for the fishing bot's log.
2. The bot does not have the Send Messages permission in that channel.
3. In rare cases, a network interruption prevents the command from being acknowledged within three seconds, so the interaction times out.

## 4. The program log file contains no framework (serenity, poise) logs

By default, framework logs are recorded at `WARN` and `ERROR` only. The frameworks emit a large amount of repetitive `INFO` output with each heartbeat. Set `[log] debug` to `true` in the config and restart this program to enable it.

Alternatively, start the program temporarily with `RUST_LOG=debug ./minecraft-fishingbot-manager-for-discord`.
`RUST_LOG` takes precedence over the config setting.

## 5. The program log says `Discord initialization failed: Missing Access`, and the process exits right after connecting to Discord

The bot may not have been invited to any server, or the invite may have been missing the `applications.commands` scope. When registering slash commands, Discord then returns `50001 Missing Access`. Invite the bot again using the invite link printed in the startup log.

## 6. `/start` reports success, but the bot is not running

Use `/log <bot name>` to view the bot’s latest log. Common causes:

1. `refreshToken` has expired or become invalid.
2. `runtime.java_opts` does not match the jar, or the process was killed because of insufficient memory.

## 7. The log shows `NeverReady` (the handshake never succeeds after startup)

* Test the network: `curl -sS -o /dev/null -w '%{http_code}\n' https://discord.com/api/v10/gateway`. A status of `200` indicates that the network is reachable.
* Check that `[discord] token` is correct (an extra space or a missing segment is enough to make it fail).
* If outbound traffic is blocked or a proxy is required, note that WebSocket traffic does not use an HTTP proxy. TUN mode is recommended.

## 8. The log shows `Unreachable`

This can happen because the network is disconnected, the proxy node is incorrect, or the token has been reset.
If network latency is high or unstable, increase `timing.watchdog_window_ms` in the config. If necessary, increase `timing.first_handshake_timeout_ms` as well. See [watchdog.md, section 5](../mods/watchdog/watchdog-en_us.md#5-common-problems) for the detection details.

## 9. The fishing bot log contains very little output

A small number of log lines is expected: the filtering rules are working. Lines that match the filters are discarded by default; you can change this behavior in the config.
Try removing the relevant expression from `keywords.filter_regex` and restarting the process. Those lines should then appear in logs from new sessions.

## 10. The fishing bot’s reported state does not match what is happening in game

The log keywords do not match the messages actually emitted by the game server. Check `[keywords]` in the config.
If reconnects or `/pause` are not recognized, first check whether `[i18n] language` matches the language actually used in the JAR’s output.
Reconnect messages and the replies to `/pause` and `/unpause` are fixed JAR text that varies with the language setting; they cannot be configured. See [i18n.md](../mods/i18n/i18n-en_us.md) for how this works.

## 11. Some bots fail to start with `/startall`

* Microsoft may rate-limit repeated sign-in requests. `/start`, `/startall`, `/restart` and `/restartall` share a global
  start queue. The shipped example config launches at most 3 bots per batch and waits 12 seconds between batches; tune
  `max_batch_size` and `batch_cooldown_ms` in `[startup_queue]` as needed. This reduces request frequency but cannot
  guarantee that Microsoft will accept every sign-in attempt.
* The transient memory peak during a batch start may exceed the server’s available memory. A process killed this way will not appear in `screen -list`, so the result may say: “The start command was sent, but the process does not exist (usually due to insufficient memory).” To mitigate this, reduce the memory allocation in `runtime.java_opts`, increase `timing.stagger_ms` and `timing.settle_per_bot_ms`, or retry later (JVM memory usage decreases over time).

## 12. Will changing the config affect bots that are already running?

No. The config is read once at startup and kept in memory. Changes made while the program is running, including deleting the config file, do not affect existing processes. Restart this program for the new config to take effect.

## 13. How can I check for orphaned bots?

```bash
screen -ls | grep fb_                 # Use the prefix defined in your config
pgrep -af "java .*-refreshToken"      # Use the token filename defined in your config
```

If you find orphaned bots, use `/stopall` to stop all fishing bots, terminate those JVMs with `pkill`, or restart this program.

## 14. A config field is rejected at startup after upgrading

At startup, this program validates the config and rejects unknown fields. After upgrading, you can delete the old config file and run the program to generate a fresh template with the latest fields.

## 15. The same command is answered twice

Some commands cannot be confirmed right away, so they append a few more replies later.
If the exact same reply appears twice, you are running several bots on the same server at the same time: stop them all and start one bot again.

* Running several bots on one machine is fine. Give them separate working directories and make sure nothing in the config clashes (the session prefix especially), and they can serve different game servers as independent services.

## 16. Related documents

* [tutorial/tutorial-en_us.md](../tutorial/tutorial-en_us.md): step-by-step deployment from scratch (most of the conclusions here are explained in context there).
* [config-en_us.md, section 3.2](../mods/config/config-en_us.md#32-the-game-side-config-confjson-is-a-generated-file): the rule that the game-side `conf.json` is generated.
* [i18n-en_us.md](../mods/i18n/i18n-en_us.md): how `[i18n] language` decides both what we say and which language’s reconnect messages we recognize.
* [watchdog-en_us.md, section 5](../mods/watchdog/watchdog-en_us.md#5-common-problems): how `NeverReady` and `Unreachable` are detected.
