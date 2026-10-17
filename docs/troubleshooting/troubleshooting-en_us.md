# Troubleshooting
**English** | [简体中文](troubleshooting-zh_cn.md)

Common problems and how to diagnose them.

## 1. Where are the logs?

This program writes two kinds of logs:

* Program log: `<deployment directory>/logs/year-month-day-hour-minute-second.log`. A new file is created at every startup and cleaned up manually by the administrator.
* Fishing bot log (after filtering): `<deployment directory>/bots/<user ID>/<bot name>/logs/year-month-day-hour-minute-second.log`. This is the log `/log` reads.

FishingBot.jar's own raw log is written to `/dev/null` and discarded by default, so only the filtered log remains on disk. Change this through `[raw_game_logs]`.

## 2. The project's commands do not appear after typing `/` in Discord

1. The invite link is missing the `applications.commands` scope. Re-invite the bot with the link printed in the startup log.
2. Slash commands are registered globally; propagation can take up to one hour.
3. The program is not running. Check `systemctl status minecraft-fishingbot-manager-for-discord` or the program log.

## 3. A command fails or does not respond

1. The bot exited unexpectedly. Check the newest program log under `<deployment directory>/logs/`; use `/log <bot name>` for the bot log.
2. The bot lacks the Send Messages permission in that channel.
3. A network interruption prevented Discord from receiving the acknowledgement within three seconds, so the interaction timed out (rare).

## 4. The program log contains no framework (serenity, poise) logs

By default, framework logs are limited to `WARN` and `ERROR`: serenity and poise emit large amounts of repetitive `INFO` output on every heartbeat. Set `[log] debug` to `true` and restart the program to record them. The level is controlled only by this setting; no environment variable or temporary override exists.

## 5. The program log says `Discord initialization failed: Missing Access`, and the process exits right after connecting

The bot may not have been invited to any server, or the invite may have lacked the `applications.commands` scope, so Discord returns `50001 Missing Access` while registering slash commands. Re-invite the bot with the link printed in the startup log.

## 6. `/start` reports success, but the bot is not running

Use `/log <bot name>` to view the bot's latest log. Common causes:

1. `refreshToken` is no longer valid.
2. `runtime.java_opts` does not match the JAR, or the process was killed because of insufficient memory.

## 7. The log shows `NeverReady` (the handshake never completes after startup)

* Network reachability: `curl -sS -o /dev/null -w '%{http_code}\n' https://discord.com/api/v10/gateway`; a status of `200` means Discord's API is reachable.
* Check that `[discord] token` is correct (an extra space or a missing segment is enough to fail).
* When using a proxy, note that HTTP proxies generally do not forward WebSocket traffic; TUN mode is recommended.

## 8. The log shows `Unreachable`

Possible causes include a network outage, an incorrect proxy node, or a reset token.

If network latency is high or unstable, increase `timing.watchdog_window_ms`: the probe interval (one third of it) and the first-handshake timeout (twice it) follow it. See [watchdog.md, section 5](../mods/watchdog/watchdog-en_us.md#5-common-problems) for the detection details.

## 9. The fishing bot log contains very little output

This is expected: the filters are working. Lines matching `keywords.filter_regex` are discarded by default; remove the matching expression and restart the process to see those lines again in logs from new sessions.

## 10. The reported bot state does not match what is happening in game

The log keywords do not match the messages the game server actually emits. Check `[keywords]`.

* If reconnects or `/pause` are not recognized, first verify that `[i18n] language` matches the language the JAR actually outputs.
* Reconnect messages and the `/pause` / `/unpause` echoes are fixed JAR text that changes with the language setting; they cannot be configured separately. See [i18n.md](../mods/i18n/i18n-en_us.md) for how this works.

## 11. Some bots fail to start with `/startall`

* Microsoft rate-limits sign-ins per egress IP. `/start`, `/startall`, `/restart` and `/restartall` share a global start queue. The example config uses `max_batch_size = 1` and `batch_login_interval_ms = 20000`, so sign-ins are at least 20 seconds apart; if you increase the batch size, set `bot_login_interval_ms` to 18-20 seconds as well. See section 12 for the mechanism, how the actual sign-in gap is computed, and the recommended values.
* The transient memory peak of a batch start may exceed the server's available memory. A process killed by the system does not appear in `screen -list`, so the report may say "failed to start (usually because Microsoft refuses frequent logins, or out of memory)". To reduce the peak, lower the heap limit in `runtime.java_opts`; if `max_batch_size` is greater than 1, reduce it, or increase `bot_login_interval_ms` to stagger sign-ins; if memory is temporarily tight, retry later (JVM memory usage decreases over time). Increase `timing.check_grace_ms` if the verification window needs to cover a longer startup phase.

## 12. The same bot fails to start every time (Microsoft sign-in rate limit)

Symptom: the `/startall` report counts a bot as "start command sent, session appeared", but that bot is missing from the `/show status` you run afterwards. The failure is not account-specific: it is always the bot late in the launch order (usually the last one), while starting the same account alone later succeeds.

Cause: the XSTS -> Minecraft token request (`POST https://api.minecraftservices.com/authentication/login_with_xbox`) is rate-limited per **egress IP** and returns `HTTP 429` when the limit is exceeded. FishingBot does not check the status code; it reads `access_token` directly from the response, throws an exception, and the process exits 2-5 seconds after launch. Whether the failure is reported depends on the verification window:

* Verification takes two `screen -list` snapshots: one immediately after the whole batch has launched, and one after `timing.check_grace_ms`. The smaller the batch, the closer this window is to launch: with `max_batch_size = 1` (one bot per batch) and for the trailing partial batch, the observation covers only about one `check_grace_ms`, so a 429 rejection 2-5 seconds after launch often goes unnoticed and is reported as success.

Rate limiting sees the actual gap between sign-ins, which is controlled by the two interval keys in `[startup_queue]`:

* two bots within one batch: `bot_login_interval_ms`;
* last bot of a batch -> first bot of the next batch: `batch_login_interval_ms` minus the time elapsed since the batch was popped through the launch of its last bot (verification runs in parallel on the command side and does not consume this wait).

Empirical values for one egress IP: one sign-in every 20 seconds (about 3 per minute) is stable; one every 12 seconds (5 per minute) loses the last one; several sign-ins within a few seconds trigger a penalty window, and every sign-in for the next few tens of seconds is rejected. Other sign-ins from the same egress IP (the official launcher, another instance, a manual login) count against the same budget.

What to do:

1. Ensure the effective sign-in gap is 18-20 seconds: the example config uses `max_batch_size = 1` and `batch_login_interval_ms = 20000`; if you increase the batch size, set `bot_login_interval_ms` to 18-20 seconds as well. `max_batch_size` does not set the minimum gap itself; it determines how the two interval keys combine within and between batches.
2. Wait 3-5 minutes before starting the failed bot again (`/start <bot name>` or `/restart <bot name>`); do not retry immediately.
3. There are only two main ways to speed up starts significantly: use multiple egress IPs, or let the JAR reuse the Minecraft access token (valid for 24 hours) instead of signing in on every start.
4. How this differs from "not enough memory" (the second cause in section 11): a rate-limited process exits by itself 2-5 seconds after launch and its log shows authentication errors, while a process killed by the OOM killer is gone within 0.3 seconds, its log is empty or contains `Killed`, and `dmesg` records it.
5. Field-by-field details are in [config-en_us.md](../mods/config/config-en_us.md).

## 13. Will changing the config affect bots that are already running?

No. The config is read once at startup and kept in memory. Editing or even deleting the config file while the program runs does not affect running processes; restart the program for changes to take effect.

## 14. How can I check for orphaned bot processes?

```bash
screen -ls | grep fb_                 # Adjust to the session prefix in your config
pgrep -af "java .*-refreshToken"      # Adjust to the token filename in your config
```

If orphaned processes remain, use `/stopall` to stop all fishing bots, terminate the JVMs with `pkill`, or restart the program.

## 15. Startup rejects a config field after an upgrade

At startup, this program validates the config and rejects unknown fields. After an upgrade, back up the old config first, then delete it and run the program again to generate the latest template.

Fields changed by the config cleanup (the shipped template is already updated):

* `[startup_queue] batch_cooldown_ms` is now `bot_login_interval_ms`: the meaning changed from "cooldown between batches" to "the minimum gap between two sign-ins" (bots within a batch are also spaced by it), replacing the former `[timing] stagger_ms`.
* `[timing]` no longer has `probe_interval_ms` or `first_handshake_timeout_ms`: both are derived from `watchdog_window_ms` (the probe interval is one third of the window, the first-handshake timeout twice it).

## 16. The same command receives multiple replies

Some commands cannot be confirmed immediately and send additional replies later. If an identical reply appears twice, multiple processes are usually using the same bot token; stop all of them and start only one instance.

* Running multiple instances on one machine is fine, but each instance should use its own working directory, config and bot token, with no clashes (especially the session prefix). They can then serve different game servers as independent services.

## 17. Related documents

* [tutorial/tutorial-en_us.md](../tutorial/tutorial-en_us.md): step-by-step deployment from scratch (most of the conclusions here are explained in context there).
* [config-en_us.md, section 3.2](../mods/config/config-en_us.md#32-the-game-side-config-confjson-is-a-generated-file): the rule that the game-side `conf.json` is generated.
* [i18n-en_us.md](../mods/i18n/i18n-en_us.md): how `[i18n] language` decides both the program language and the language FishingBot uses, and how keywords follow it.
* [watchdog-en_us.md, section 5](../mods/watchdog/watchdog-en_us.md#5-common-problems): how `NeverReady` and `Unreachable` are detected.
