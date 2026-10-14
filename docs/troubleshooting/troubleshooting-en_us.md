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
If network latency is high or unstable, increase `timing.watchdog_window_ms` in the config: the probe interval (a third of it) and the first-handshake timeout (twice it) follow it. See [watchdog.md, section 5](../mods/watchdog/watchdog-en_us.md#5-common-problems) for the detection details.

## 9. The fishing bot log contains very little output

A small number of log lines is expected: the filtering rules are working. Lines that match the filters are discarded by default; you can change this behavior in the config.
Try removing the relevant expression from `keywords.filter_regex` and restarting the process. Those lines should then appear in logs from new sessions.

## 10. The fishing bot’s reported state does not match what is happening in game

The log keywords do not match the messages actually emitted by the game server. Check `[keywords]` in the config.
If reconnects or `/pause` are not recognized, first check whether `[i18n] language` matches the language actually used in the JAR’s output.
Reconnect messages and the replies to `/pause` and `/unpause` are fixed JAR text that varies with the language setting; they cannot be configured. See [i18n.md](../mods/i18n/i18n-en_us.md) for how this works.

## 11. Some bots fail to start with `/startall`

* Microsoft rate-limits sign-ins per egress IP. `/start`, `/startall`, `/restart` and `/restartall` share a global start
  queue. The shipped example config leaves at least 20 seconds between two sign-ins
  (`[startup_queue] login_interval_ms`); tune `max_batch_size` and `login_interval_ms` as needed. See section 12 for the
  mechanism, how the real gap between two sign-ins is computed, and the recommended values.
* The transient memory peak during a batch start may exceed the server’s available memory. A process killed this way will not appear in `screen -list`, so the result may say: “The start command was sent, but the process does not exist (usually due to insufficient memory).” To mitigate this, reduce the memory allocation in `runtime.java_opts`, increase `timing.settle_per_bot_ms` and `[startup_queue] login_interval_ms`, or retry later (JVM memory usage decreases over time).

## 12. The same bot fails to start every time (Microsoft sign-in rate limit)

Symptom: the `/startall` report counts a bot as "the start command was sent, the session appeared", but the bot is
missing from the `/show status` you run right after; the account does not matter — it is **the one late in the launch
order** (usually the last one), and starting that same account on its own later works.

Cause: the XSTS -> Minecraft token step (`POST https://api.minecraftservices.com/authentication/login_with_xbox`) is
rate-limited per **egress IP** and answers `HTTP 429` once you exceed it. FishingBot does not check the status code; it
reads `access_token` straight out of the response, so it throws, and the process exits by itself 2-5 seconds after
launch. Whether that is reported as a failure depends on the verification window:

* Verification is two `screen -list` snapshots taken **after the whole batch is up, plus
  `2 x timing.settle_per_bot_ms x batch size`**. The smaller the batch, the closer that window sits to the launch:
  with `max_batch_size = 1` (one bot per batch) and for the trailing partial batch the only check happens about
  `2 x timing.settle_per_bot_ms` after launch, so "rejected by a 429 after 2-5 seconds" is often invisible and gets
  reported as a success.

The real gap between two sign-ins (what the rate limit actually sees) is set by the single key
`[startup_queue] login_interval_ms`:

* two bots inside one batch: `login_interval_ms`;
* last bot of a batch -> first bot of the next batch: `login_interval_ms` (the wait between batches is it minus
  2 x `timing.settle_per_bot_ms` x batch size).

Empirical values for one egress IP: **one sign-in every 20 seconds (about 3 per minute) is stable**; one every 12
seconds (5 per minute) loses the last one; several sign-ins within a few seconds trip a penalty window, and every
sign-in for the next few tens of seconds is rejected. Other sign-ins from the same egress IP (the official launcher,
another instance, a manual login) spend the same budget.

What to do:

1. Set `[startup_queue] login_interval_ms` to 18-20 seconds (the shipped example uses 20000). `max_batch_size` does not
   change the pace: it only changes the length of the verification window and the JVM memory peak.
2. Wait 3-5 minutes before starting the failed bot again (`/start <bot name>` or `/restart <bot name>`); do not retry
   immediately.
3. There are only two ways to really go faster: use more than one egress IP, or let the JAR reuse the Minecraft access
   token (valid for 24 hours) instead of signing in on every start.
4. How this differs from "not enough memory" (the second cause in section 11): a rate-limited process exits by itself
   2-5 seconds after launch and its log shows authentication errors, while a process killed by the OOM killer is gone
   within 0.3 seconds, its log is empty or says `Killed`, and `dmesg` shows it.
5. Field-by-field details are in [config-en_us.md](../mods/config/config-en_us.md).

## 13. Will changing the config affect bots that are already running?

No. The config is read once at startup and kept in memory. Changes made while the program is running, including deleting the config file, do not affect existing processes. Restart this program for the new config to take effect.

## 14. How can I check for orphaned bots?

```bash
screen -ls | grep fb_                 # Use the prefix defined in your config
pgrep -af "java .*-refreshToken"      # Use the token filename defined in your config
```

If you find orphaned bots, use `/stopall` to stop all fishing bots, terminate those JVMs with `pkill`, or restart this program.

## 15. A config field is rejected at startup after upgrading

At startup, this program validates the config and rejects unknown fields. After upgrading, you can delete the old config file and run the program to generate a fresh template with the latest fields.

Fields touched by this slimming (the shipped template is already updated):

* `[startup_queue] batch_cooldown_ms` is now `login_interval_ms`: the meaning changed from "cooldown between batches"
  to "the minimum gap between two sign-ins" (bots inside a batch are spaced by it), and it replaces the former
  `[timing] stagger_ms`.
* `[timing]` no longer has `probe_interval_ms` and `first_handshake_timeout_ms`: they are derived from
  `watchdog_window_ms` (the probe interval is a third of the window, the first-handshake timeout twice it).

## 16. The same command is answered twice

Some commands cannot be confirmed right away, so they append a few more replies later.
If the exact same reply appears twice, you are running several bots on the same server at the same time: stop them all and start one bot again.

* Running several bots on one machine is fine. Give them separate working directories and make sure nothing in the config clashes (the session prefix especially), and they can serve different game servers as independent services.

## 17. Related documents

* [tutorial/tutorial-en_us.md](../tutorial/tutorial-en_us.md): step-by-step deployment from scratch (most of the conclusions here are explained in context there).
* [config-en_us.md, section 3.2](../mods/config/config-en_us.md#32-the-game-side-config-confjson-is-a-generated-file): the rule that the game-side `conf.json` is generated.
* [i18n-en_us.md](../mods/i18n/i18n-en_us.md): how `[i18n] language` decides both what we say and which language’s reconnect messages we recognize.
* [watchdog-en_us.md, section 5](../mods/watchdog/watchdog-en_us.md#5-common-problems): how `NeverReady` and `Unreachable` are detected.
