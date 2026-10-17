# FishingBot: command and message reference
[简体中文](BOT_COMMANDS_AND_MESSAGES-zh_cn.md) | **English**

> Applies to: `systems.kinau:FishingBot:2.13.15`
> This document is for people who **drive FishingBot as a child process or a service**. It focuses on:
> 1. every command the bot accepts (how to trigger it, and what to watch out for).
> 2. every message the bot prints (its Chinese value and its original English value), because other projects depend on
>    those messages.

The commands available may differ slightly between versions or builds (for example `/pause` and `/unpause`); treat the
JAR you actually run as authoritative.

Every message text here is taken from the source code and the language files:

* English: `src/main/resources/lang/fb_en_EN.properties`
* Chinese: `src/main/resources/lang/fb_zh_CN.properties`

At run time they are read by `systems.kinau.fishingbot.i18n.I18n`. The language comes from the `misc.language` setting
(`CHINESE_SIMPLIFIED` / `ENGLISH`; a restart is needed).

---

## 0. Key concepts (read this before integrating)

### 0.1 The message prefix and title

`FishingBot.PREFIX` and `TITLE` (in `FishingBot.java`):

* `fishingbot.properties` `name`: `FishingBot`; from `<name>` in `pom.xml`.
* `fishingbot.properties` `version`: `2.13.15`; from `<version>` in `pom.xml`.
* `PREFIX`: `FishingBot v2.13.15 - ` (note the trailing ` - `, spaces included); the chat message prefix and the value
  substituted for `%prefix%`.
* `TITLE`: `FishingBot v2.13.15`; only used in the startup log line `Using FishingBot v2.13.15`.

> When `fishingbot.properties` cannot be read, `PREFIX = "FishingBot vUnknown - "`.

### 0.2 Where a message goes (crucial for finding its source)

* Console/log (produced by `FishingBot.getI18n().info/warning/severe(key,...)` and
  `ConsoleCommandExecutor.sendMessage`): printed to stdout and written to `logs/log%g.log` at the same time.
* In-game chat (public) (produced by `PlayerCommandExecutor.sendMessage` ->
  `net.sendPacket(PacketOutChatMessage(...))`): the bot speaks in the server as an ordinary chat message.
* Both (produced by, for example, the "caught an item" announcement: the console through `log::info`, the chat through a
  chat packet carrying `PREFIX`).
* In-game chat (private) (produced by the private-chat branch of `PlayerCommandExecutor`, which is not enabled in the
  current code - `ChatCommandModule` always uses `new PlayerCommandExecutor()` with no private target): does not happen
  today.

### 0.3 The console log line format

`io/logging/LogFormatter.java`:

```
[yyyy-MM-dd HH:mm:ss] [LEVEL  ] <message>
```

* `LEVEL` is `INFO`, `WARNING` or `SEVERE` (`Level.getLocalizedName()`, left-aligned in 7 characters).
* Every message ends with one newline (`%n`).
* A multi-line message (i18n text containing `\n`) is split into lines by `I18n.info/warning/severe` and printed line by
  line, so every line carries the timestamp and level prefix.

---

## 1. How it starts, and the command line arguments

Entry point: `systems.kinau.fishingbot.Main#main` (`net.minecraft.client.main.Main` is the adapter entry point used when
the official launcher or OneSix arguments start it, and it ends up calling `systems.kinau.fishingbot.Main.main` as
well).

Command line arguments (`Main.java`, based on Apache Commons CLI):

* `-nogui` (takes a value: no): disable the GUI (this repository has no GUI implementation; kept for history, no actual
  output).
* `-help` (takes a value: no): print the help and exit.
* `-logsdir <dir>` (takes a value: yes): the log directory (default: `logs/` next to the program).
* `-config <path>` (takes a value: yes): the config file (default: `config.json` next to the program).
* `-refreshToken <path>` (alias `-accountfile`) (takes a value: yes): the Microsoft login refreshToken file (default:
  `refreshToken` in the program directory).
* `-onlyCreateConfig` (takes a value: no): only generate the config and exit (no server connection).

> As used by the script inside Docker, `docker/fishing-bot`:
> `java -jar FishingBot.jar -nogui -logsdir /var/log/fishingbot/ -config .../config.json -refreshToken .../refreshToken`

---

## 2. The command system

### 2.1 Where commands come from (3 external, 2 internal)

* A. Console (triggered by typing a line into `System.in` at run time; entry code: `ChatProxyModule` reads a line ->
  `bot.runCommand(line, true, new ConsoleCommandExecutor())`).
* B. In-game player chat (triggered by typing `"<bot user name>, <command>"` in the chat bar; entry code:
  `ChatCommandModule#onChat` -> `commandRegistry.dispatchCommand(cmd, new PlayerCommandExecutor())`).
* C. Internal: the startup action (triggered by joining the game and, 1.5 s later, running the configured
  `start-text.text`; entry code: `ClientDefaultsModule#onJoinGame` -> `runCommand(...)`).
* D. Internal: scheduled tasks (triggered periodically by `auto.timer.timers`; entry code: `TimerModule#startTimers` ->
  `runCommand(...)`).
* E. Internal: automatic commands on respawn or near death (triggered by `auto.auto-command-on-respawn.commands` and
  `auto.auto-command-before-death.commands`; entry code: `Player` (event handling) -> `runCommand(...)`).

> A and B are the two external paths that can trigger a command; C/D/E are automatic commands preset in the config and
> go through the same parser.
> Note: a command typed on the console has to start with `/`, otherwise it is treated as "what the bot wants to say" and
> sent to the server as chat (see 2.2).

### 2.2 Trigger and parse details (read every line of this)

1. Console (A): `Bot#runCommand`:
   * a line starting with `/` is first matched against the bot's commands; on a match it is executed and not sent to
     the server;
   * no match, or a line not starting with `/`, is sent to the server as ordinary chat through `Player.sendMessage`;
   * so on the console "chatting" means typing text and "running a bot command" means typing `/command`.
2. Player chat (B): it only tries to parse when the chat text contains both the bot user name and an ASCII comma `,`:
   * the whole message is `split(",")`, everything from `parts[1]` on is concatenated (with the commas removed) and then
     `trim()`ed;
   * when the result starts with whitespace, one whitespace character is removed;
   * it is handed to `dispatchCommand`, which drops one leading `/` before parsing (so both `botname, help` and
     `botname, /help` work);
   * any text containing the bot name and a comma triggers an attempt; a failed parse is ignored silently with no
     output at all;
   * because it joins on `,`, a command argument cannot contain a comma (it would be swallowed).
3. Leading slash: `CommandRegistry#dispatchCommand` drops one leading `/`.
4. Return value semantics: every built-in command `return 0`; `dispatchCommand` treats a return of `0` as "executed"
   (returns `true`) and a thrown `CommandSyntaxException` as unhandled (returns `false`).
5. There is no permission system: any player who can send `botname, command` in the server chat can drive the bot, with
   no allowlist or admin check. Restrict it yourself when you put it on a public server.

### 2.3 The command table

* `help` (syntax: `/help [command]`): show the help, or the syntax of one command.
* `bye` (aliases: `stop`, `shutdown`; syntax: `/bye`): stop the bot (blocking reconnects, `stopBot(true)`).
* `level` (aliases: `level?`; syntax: `/level`): show the current experience level.
* `stuck` (aliases: `recast`, `reeject`, `refish`, `recatch`; syntax: `/stuck`): recast the rod (triggering the stuck
  fix).
* `pause` (syntax: `/pause`): pause fishing.
* `unpause` (aliases: `resume`; syntax: `/unpause`): resume fishing.
* `empty` (aliases: `clear`, `drop`; syntax: `/empty [direction] [slot]`): drop the inventory (keeping the rod);
  direction and slot are optional.
* `droprod` (aliases: `roddrop`, `droprods`, `rodsdrop`, `emptyrod`, `rodempty`; syntax: `/droprod [Filter]`): drop
  fishing rods, Filter = `ALL` / `SELECTED` / `ALL_BUT_SELECTED`.
* `look` (syntax: `/look <yaw> <pitch> [speed]`): turn the view (yaw/pitch in [-180,180]; speed defaults to the
  `misc.look-speed` setting, 16).
* `rightclick` (aliases: `use`; syntax: `/rightclick [slot (1-9)]`): right-click (optionally switching to a hotbar slot
  first).
* `clickinv` (aliases: `invclick`; syntax: `/clickinv <slot> [left|right]`): click a slot in an open container screen.
* `swap` (aliases: `swapitem`; syntax: `/swap <slot> <hotbar slot 1-9>`): swap an inventory item into the hotbar.
* `wait` (aliases: `sleep`; syntax: `/wait <seconds>`): the bot waits N seconds.
* `summary` (aliases: `summarize`, `stats`, `statistics`, `caught`, `loot`; syntax: `/summary [clear]`): print the
  statistics of everything caught so far (`clear` empties them).
* `acknowledgecoc` (aliases: `acknowledgecodeofconduct`, `acceptcoc`, `acceptcodeofconduct`; syntax:
  `/acknowledgecoc [save]`): accept the server's Code of Conduct (`save` writes it into the config to remember it).

> On the console a "command" is written `/xxx`; in the game chat it is written `bot user name, xxx`.
> Commands constrain their argument types (`empty`'s direction is only `NORTH/EAST/SOUTH/WEST/DOWN`; `clickinv` slots
> are >= 1; `swap` hotbar slots are 1-9); when that is not met Brigadier fails to parse, and it is then ignored silently
> (player side) or sent to the console as chat.

## 3. The complete message table (Chinese and English side by side)

> Notes:
> - "Chinese" in the table is the actual value in `fb_zh_CN.properties`; "English" is the value in
>   `fb_en_EN.properties`.
> - `{0}`, `{1}` are `java.text.MessageFormat` placeholders, filled in argument order; `\n` means a line break (the log
>   splits those into several lines).
> - Entries marked [English only / Chinese falls back to English]: the Chinese language file has no such key, so
>   `I18n.t()` falls back to the English value.
> - Entries marked [Chinese value is still English]: the Chinese language file has the key, but its value is English
>   (no fallback happens; the English text is printed as it is).
> - When a key is not found at all, `I18n.t()` returns the literal `N/A`.

### 3.1 Command responses and command help (`command-*`)

* `command-bye` (Chinese: 再见; English: Goodbye.; destination / trigger: replied to the caller when `/bye` runs).
* `command-empty` (Chinese: 正在丢出背包物品...; English: Dropping inventory...; `/empty` runs).
* `command-empty-unknown-type` (Chinese: 方向 {0} 未知. 试试如下方向: NORTH, EAST, SOUTH, WEST, DOWN; English: The
  direction {0} is unknown. Try one of these directions: NORTH, EAST, SOUTH, WEST, DOWN; `/empty` with an invalid
  direction).
* `command-stuck-executed` (Chinese: 抛出鱼竿; English: Fishing rod cast; `/stuck` runs).
* `command-pause-executed` (Chinese: 已暂停钓鱼; English: Fishing paused; `/pause` runs).
* `command-unpause-executed` (Chinese: 已恢复钓鱼; English: Fishing resumed; `/unpause` runs).
* `command-level` (Chinese: 我有 {0} 级经验; English: I have {0} levels; `/level` runs).
* `command-look-executed` (Chinese: 正在查看 {0} / {1}; English: Now looking at {0} / {1}; `/look` runs, with yaw and
  pitch as the arguments).
* `command-droprod-item-count` (Chinese: 丢弃了 {0} 杆鱼竿(s); English: Dropped {0} fishing rod(s); `/droprod` runs).
* `command-droprod-unknown-type` (Chinese: 筛选器 {0} 未知. 尝试下列筛选器之一:\n- ALL: 丢出所有鱼竿\n- SELECTED:
  丢出当前持有的鱼竿\n- ALL_BUT_SELECTED: 丢出当前持有的鱼竿外所有鱼竿; English: The filter {0} is unknown. Try one of
  these filters:\n- ALL: Drops all fishing rods\n- SELECTED: Drops the currently selected fishing rod\n-
  ALL_BUT_SELECTED: Drops all fishing rods but the currently selected one; `/droprod` with an invalid Filter,
  multi-line).
* `command-summary-empty` (Chinese: 我还没有钓到任何东西！; English: I haven't fished anything yet!; `/summary` runs
  with nothing recorded yet).
* `command-help-invalid-command` (Chinese: 找不到命令。使用 /help 列出所有命令。; English: Command not found. Use /help
  to list all commands.; `/help <unknown command>`).
* `command-clickinv-no-inv` (Chinese: Currently there is no inventory opened [Chinese value is still English];
  English: Currently there is no inventory opened; `/clickinv` with no container open).
* `command-clickinv-invalid-slot` (Chinese: The opened inventory does not have this slot number [Chinese value is
  still English]; English: The opened inventory does not have this slot number; `/clickinv` with a slot that does not
  exist).
* `command-wait-waited` (Chinese: I waited {0} second(s) [Chinese value is still English]; English: I waited {0}
  second(s); after `/wait` finished).
* `command-rightclick-invalid-slot` (Chinese: The given slot is out of range (1-9) [Chinese value is still English];
  English: The given slot is out of range (1-9); `/rightclick` with an out-of-range slot).

The "description / syntax / help" texts of the commands (used by the `/help` list and by `/help <cmd>`; these values are
also the description passed when the command is built):

* `command-help-desc` (Chinese: 显示此帮助消息; English: Displays this help message).
* `command-bye-desc` (Chinese: 停止钓鱼机器人; English: Stops the bot).
* `command-level-desc` (Chinese: 显示机器人的当前等级; English: Shows the bot's current levels).
* `command-stuck-desc` (Chinese: 再次抛出鱼竿; English: Cast out the fishing rod again).
* `command-pause-desc` (Chinese: 暂停钓鱼; English: Pauses fishing).
* `command-unpause-desc` (Chinese: 恢复钓鱼; English: Resumes fishing).
* `command-empty-desc` (Chinese: 丢出我背包的所有物品（钓鱼竿除外）; English: Drops all contents of my inventory (except
  the fishing rod)).
* `command-droprod-desc` (Chinese: 丢出当前持有的鱼竿、除持有的鱼竿以外的所有鱼竿，或所有鱼竿; English: Drops either
  your current selected rod, all but your selected rod or all of your rods).
* `command-look-desc` (Chinese: 让机器人在别处寻找; English: Makes the bot look elsewhere).
* `command-summary-desc` (Chinese: 打印捕获物品的摘要; English: Prints a summary of the caught items).
* `command-swap-desc` (Chinese: Swaps two items in the inventory [Chinese value is still English]; English: Swaps two
  items in the inventory).
* `command-clickinv-desc` (Chinese: Let the bot click a slot in an opened inventory [Chinese value is still English];
  English: Let the bot click a slot in an opened inventory).
* `command-wait-desc` (Chinese: Let the bot wait some time (in seconds) [Chinese value is still English]; English: Let
  the bot wait some time (in seconds)).
* `command-rightclick-desc` (Chinese: Let the bot perform a rightclick [Chinese value is still English]; English: Let
  the bot perform a rightclick).
* `command-ackowledgecoc-desc` (Chinese: Acknowledges the server's Code of Conduct [English only / Chinese falls back
  to English]; English: Acknowledges the server's Code of Conduct).

Command syntax (the output of `/help <cmd>`; most strings are undefined in the Chinese file, so they fall back to
English):

* `command-syntax-no-args` (Chinese: /{0} [English only]; English: /{0}).
* `command-help-syntax` (Chinese: /{0} [Command] [English only]; English: /{0} [Command]).
* `command-empty-syntax` (Chinese: /{0} [Direction] [Slot] [English only]; English: /{0} [Direction] [Slot]).
* `command-droprod-syntax` (Chinese: /{0} [Filter] [English only]; English: /{0} [Filter]).
* `command-look-syntax` (Chinese: /{0} <Yaw> <Pitch> [Speed] [English only]; English: /{0} <Yaw> <Pitch> [Speed]).
* `command-clickinv-syntax` (Chinese: /{0} <Slot> [left|right] [English only]; English: /{0} <Slot> [left|right]).
* `command-rightclick-syntax` (Chinese: /{0} [Hotbar slot] [English only]; English: /{0} [Hotbar slot]).
* `command-summary-syntax` (Chinese: /{0} [clear] [English only]; English: /{0} [clear]).
* `command-wait-syntax` (Chinese: /{0} <Time in seconds> [English only]; English: /{0} <Time in seconds>).
* `command-swap-syntax` (Chinese: /swap <Slot> <Hotbar slot> [Chinese value is still English]; English: /swap <Slot>
  <Hotbar slot>).
* `command-ackowledgecoc-syntax` (Chinese: /{0} [save] [English only]; English: /{0} [save]).

The output format of the `/help` list (`CommandHelp`, one line per command):

* `command-help-console` (Chinese: /{0} - {1}; English: /{0} - {1}): on the console, `/command - description`.
* `command-help-other-player` (Chinese: 我的名字是 {0} - {1}; English: My name, {0} - {1}): triggered by a player
  (public).
* `command-help-other-player-private` (Chinese: /msg 我的名字 {0} - {1}; English: /msg My name {0} - {1}): triggered in
  a private chat (does not happen today).

The body of `/summary`:

* `ui-tabs-loot` (Chinese: 统计 ({0}); English: Loot ({0})): `{0}` is the total count.
* (no key, hard-coded) (Chinese: `<数量>x <物品显示名>`; English: `<count>x <displayName>`): one line per item, for
  example `3x 鳕鱼` / `3x Cod` (following the client's language resources).

### 3.2 Module run-time messages (`module-*`)

* `module-chat-proxy-chat-message` (Chinese: [CHAT] {0}; English: [CHAT] {0}; destination / trigger: console; printed
  for every server chat message received (ChatProxy)).
* `module-client-disconnected` (Chinese: 已断开连接 {0}; English: Disconnected: {0}; console; a disconnect packet
  arrived (`{0}` is the disconnect reason text)).
* `module-fishing-start-fishing` (Chinese: 我开始钓鱼了!; English: I start to fish!; console; entering the game and
  starting to cast).
* `module-fishing-no-rod-available` (Chinese: 未找到任何鱼竿...; English: No rod has been found...; console; no rod
  available).
* `module-fishing-new-rod-available` (Chinese: 新的鱼杆可用。我又开始钓鱼了！; English: A new rod is available. I start
  fishing again!; console; a rod became available again).
* `module-fishing-bot-is-slow` (Chinese: 机器人当前的钓鱼速度较慢。也许它被卡住了。正在尝试重新抛竿！; English: The bot
  is currently slow. Maybe he is stuck. Trying to restart!; console; nothing caught for 60 seconds (stuck fix)).
* `module-fishing-entity-hooked` (Chinese: 警告：捕获到实体！将在几秒后重试。; English: Caution: An entity has been
  caught! Trying again in a few seconds.; console; an entity was hooked instead of an item).
* `module-fishing-health-threshold-reached` (Chinese: 达到设定退出前的生命值阈值，机器人将退出；English: Health
  threshold before quit reached. Bot will be stopped.; console; the "quit before death" rule fired).
* `module-fishing-caught-item` (Chinese: 捕获到 \"{0}\"; English: Caught \"{0}\"; console + in-game chat; `{0}` is the
  item display name, and the chat form is described in 3.8).
* `module-handshake-disabling` (Chinese: 无法禁用握手模块.; English: Cannot disable the HandshakeModule.; console; an
  attempt to disable the handshake module).
* `module-login-hash-error` (Chinese: 无法对服务器id进行哈希处理。机器人将被停止...; English: The server id could not be
  hashed. The bot will be stopped...; console; the encrypted login failed).
* `module-login-failed` (Chinese: 登录失败: {0}; English: Login failed: {0}; console; the login was rejected (`{0}` is
  the reason)).
* `module-login-successful` (Chinese: 登录成功!\nName: {0}\nUUID: {1}; English: Login successful!\nName: {0}\nUUID:
  {1}; console; the login succeeded, multi-line).
* `module-enabled` (Chinese: 模块 \"{0}\" 已启用!; English: Module \"{0}\" enabled!; console; whenever a module is
  enabled).
* `module-disabled` (Chinese: 模块 \"{0}\" 已禁用!; English: Module \"{0}\" disabled!; console; whenever a module is
  disabled).

> Common module names (the `{0}` value is the class name): `ChatProxyModule`, `ClientDefaultsModule`, `HandshakeModule`,
> `LoginModule`, `FishingModule`, `EjectionModule`, `TimerModule`, `ChatCommandModule`.

### 3.3 Network and connection messages (`network-*`, `server-pinging`, `bot-*`, `packet-*`)

* `network-invalid-server-address` (Chinese: 设置的服务器似乎不存在。请检查配置文件中的服务器地址(server.ip); English:
  The given server seems to be not existing. Please check the server address in the Settings (server.ip); console;
  `server.ip` is empty).
* `network-could-not-ping` (Chinese: Minecraft版本{0}无法访问服务器。自动版本检测可能不起作用。请在配置文件中设置默
  认版本协议; English: The server {0} is not reachable with the Minecraft version. The automatic version detection may
  not work. Please set a default-protocol in the Settings.; console; the ping failed (reused in two places)).
* `network-unknown-host` (Chinese: 服务器 {0} 未知; English: The server {0} is unknown.; console; DNS resolution
  failed).
* `network-received-pong` (Chinese: 接收到的标题: {0}, 版本: {1} ({2}), 在线玩家数: {3}; English: Received pong: {0},
  version: {1} ({2}), players online: {3}; console; the ping returned MOTD, version and player count).
* `network-server-is-full` (Chinese: 服务器已满。机器人将被停止!; English: The server is full. Bot will be stopped!;
  console; the player count reached the auto-disconnect threshold).
* `network-changed-address` (Chinese: 服务器主机已更改: {0}; English: The server host has been changed: {0}; console;
  SRV resolution produced a new host).
* `network-changed-port` (Chinese: 服务器端口已更改: {0}; English: The server port has been changed: {0}; console; SRV
  resolution produced a new port).
* `network-ping-differs-protocol` (Chinese: 服务器响应的协议版本 {0} 与bot选择的协议版本 {1}不同。您可能需要在配置中
  更改它; English: The server responded with a different protocol version {0} than what the bot selected {1}. You may
  need to change it in the Config.; console; the server protocol differs from the selected one).
* `network-unknown-packet-id` (Chinese: 未设置 {1} 中 {0} 的数据包ID。请报告：\nhttps://discord.com/invite/xHpCDYf;
  English: The packet id for {0} in {1} is not set. Please report this: https://discord.com/invite/xHpCDYf; console; an
  unmapped packet).
* `server-pinging` (Chinese: Pinging {0}:{1} 协议为 MC-{2}; English: Pinging {0}:{1} with protocol of MC-{2}; console;
  the ping before connecting (`{2}` is the default protocol version name)).
* `packet-could-not-be-received` (Chinese: 无法接收数据包！正在关闭机器人...; English: Could not receive packet!
  Shutting down...; console; a packet read error exits the main loop).
* `bot-could-not-be-started` (Chinese: 发生意外错误: {0}\n机器人将被停止...; English: An unexpected error occured:
  {0}\nThe bot will be stopped...; console; a connection or network error, multi-line).
* `bot-automatic-reconnect` (Chinese: 钓鱼机器人将在{0}秒内重新连接...; English: FishingBot will reconnect in {0}
  seconds...; console; printed before the automatic reconnect wait (`{0}` is `auto.auto-reconnect-time`, default 5)).
* `network-not-supported-server-version` (Chinese: 此服务器未运行受支持的协议版本: {0}\n机器人可能无法正常工作; English:
  This server is not running a supported protocol version: {0}\nThe bot might not work properly.; defined but not
  referenced by the current code, so it is unused).

> The line matching the example: the reconnect notice is `bot-automatic-reconnect`, Chinese `钓鱼机器人将在{0}秒内重新
> 连接...`, English `FishingBot will reconnect in {0} seconds...`, printed to the console log (not to the in-game
> chat).

### 3.4 Login and account messages (`credentials-*`, `auth-*`, `*keys*`, `config-account-*`, `dialog-*`)

* `credentials-using-offline-mode` (Chinese: 以脱机模式启动，用户名:{0}; English: Starting in offline-mode with user
  name: {0}): started in offline mode.
* `credentials-invalid` (Chinese: 指定的账户无效。这可能有几个原因（参见：
  https://github.com/MrKinau/FishingBot/wiki/Troubleshooting).; English: The credentials you specified are invalid.
  This can have several reasons (see: https://github.com/MrKinau/FishingBot/wiki/Troubleshooting).): authentication
  failed.
* `auth-username` (Chinese: 用户名: {0}; English: Username: {0}): prints the user name after authentication.
* `auth-using-microsoft` (Chinese: 使用微软进行身份验证; English: Trying to authenticate at Microsoft...): the Microsoft
  authentication starts.
* `auth-using-onesix` (Chinese: Trying to authenticate using OneSix arguments...; English: Trying to authenticate using
  OneSix arguments...): identical in both languages.
* `auth-change-onesix` (Chinese: Changing authentication to use OneSix; English: Changing authentication to use
  OneSix): identical in both languages.
* `auth-create-refresh-token` (Chinese: Please log in with your Microsoft account using the following code {0} at {1}.\nThis is
  a one-time step ... your data!; English: (the same text, which is English)): the Chinese value is still English; the
  device code login prompt (`{0}` is the user code, `{1}` the verification URL).
* `auth-found-refresh-token` (Chinese: Found refreshToken at {0}; English: Found refreshToken at {0}): the Chinese value
  is still English.
* `auth-could-not-get-refresh-token` (Chinese: Error while creating refresh token... Please try again; English: Error
  while creating refresh token... Please try again): the Chinese value is still English.
* `auth-could-not-get-access-token` (Chinese: Error while creating access token... Please try again; English: Error
  while creating access token... Please try again): the Chinese value is still English.
* `auth-try-refreshing-access-token` (Chinese: 正在尝试刷新访问令牌...; English: Trying to refresh access token...).
* `auth-refreshed-access-token` (Chinese: Access token successfully refreshed; English: Access token successfully
  refreshed): the Chinese value is still English.
* `auth-authorization-succeeded` (Chinese: The authentication was successful; English: The authentication was
  successful): the Chinese value is still English.
* `could-not-get-keys` (Chinese: Could not retrieve the certificates required for chat signing from {0}: {1}; English:
  Could not retrieve the certificates required for chat signing from {0}: {1}): the Chinese value is still English.
* `retrieved-keys` (Chinese: Obtained the required keys for chat signing!; English: Obtained the required keys for chat
  signing!): the Chinese value is still English.
* `config-account-mail` (Chinese: 电子邮箱（或用户名）; English: Username (only offline mode)): a config label (for the
  GUI, which this repository does not have).
* `config-account-microsoft` (Chinese: You don't need to provide any credentials ...; English: (the same text, which is
  English)): the Chinese value is still English; GUI.
* `config-account-onesix` (Chinese: Using OneSix authentication, ...; English: (the same text, which is English)): the
  Chinese value is still English; GUI.
* `config-account-logout` (Chinese: Logout; English: Logout): identical in both languages; GUI.
* `dialog-credentials-invalid-header` (Chinese: 账户无效; English: Credentials invalid): a GUI dialog.
* `dialog-credentials-invalid-content` (Chinese: 指定的账户无效。这可能有几个原因：; English: The credentials you
  specified are invalid. This can have several reasons:): a GUI dialog.
* `dialog-authorization-header` (Chinese: Authorization process; English: Authorization process): the Chinese value is
  still English; GUI.
* `dialog-authorization-failed-header` (Chinese: Authorization failed; English: Authorization failed): the Chinese value
  is still English; GUI.

> Two entries in this section are used by this project's `/bot login`: `auth-create-refresh-token` (the verification URL
> and the device code are parsed out of that line and handed back to the caller) and
> `auth-could-not-get-refresh-token` (which identifies "Microsoft refused this login"). Whether the authorization
> actually succeeded is not decided by a message but by the presence of the `refreshToken` file, so
> `auth-authorization-succeeded` needs no parsing. Both keywords live in constants in `src/bot_manage/login.rs`, and a
> unit test compares them word by word against this list.

### 3.5 Realms messages (`realms-*`)

* `realms-id-not-set` (Chinese: 未设置领域ID。正在关闭...; English: Realms ID was not set. Shutting down...): with
  `realm-id==0` the realms are listed and the bot then shuts down.
* `realms-tos-agreement` (Chinese: (asterisk box)如果你想使用领域，你必须在配置文件中接受ToS; English: (asterisk box)
  If you want to use realms you have to accept the tos in the Settings (realm-accept-tos)): the ToS was not accepted.
* `realms-determining-address` (Chinese: 正在尝试解析服务器地址...(尝试{0}); English: Trying to resolve the server
  address... (Try {0})): a retry of the realm address resolution.
* `realms-could-not-accept-tos` (Chinese: 无法接受领域的服务条款: {0}; English: The terms of service for realms could
  not be accepted: {0}).
* `realms-accepted-tos` (Chinese: 已接受领域的服务条款!; English: The terms of service for realms have been accepted!).
* `realms-could-not-determine-address` (Chinese: 无法从 {0}: {1} 检索服务器地址; English: Could not retrieve server
  address from {0}: {1}).
* `realms-connecting-to` (Chinese: 正在连接到领域 {0}...; English: Connecting to realm {0}...): `{0}` is the JSON
  returned by the API (in practice the address information).
* `realms-could-not-connect-to-endpoint` (Chinese: 无法连接到领域 ({0}): {1}; English: Could not connect to the realms
  endpoint ({0}): {1}).
* `realms-no-realms` (Chinese: 没有可供您加入的领域。记得在加入领域之前接受邀请; English: There are no realms available
  for you to join. Remember to accept an invitation before you can join the realm.): there are no realms.
* `realms-could-not-parse-server-entry` (Chinese: 无法解析 Realms 服务器条目：{0}; English: Could not parse realm
  server entry: {0}).

> When the realms are listed, hard-coded lines are printed as well: an empty line, `ID: <id>`, `name: <name>`,
> `motd: <motd>` and `owner: <owner>` (see 3.8).

### 3.6 Config and log messages (`config-*`, `log-*`, `could-not-detect-theme`)

* `config-loaded-from` (Chinese: 配置已加载: {0}; English: Configuration was loaded: {0}): prints the absolute path of
  the config file at startup.
* `config-failed-parsing` (Chinese: (asterisk box)无法分析您的配置，因为它与json格式不匹配:\n{0}; English: (asterisk
  box) Your configuration could not be parsed because it does not match the json-format:\n{0}): the config could not be
  parsed (multi-line, `{0}` is the exception).
* `config-unknown-announce-type` (Chinese: 通知类型 {0} 无效.; English: The AnnounceType {0} is invalid.): an invalid
  enumeration value in the config (it falls back to the default and is printed).
* `config-missing-fields-detected` (Chinese: 您的配置不完整。已添加缺少的字段！; English: Your configuration was
  incomplete. The missing fields were added!): missing config fields were added automatically.
* `config-converting` (Chinese: 正在转换配置文件的旧版本...; English: An older version of your configuration file is
  being converted...): an old config field is being migrated (for example `start-text.text` in the old format).
* `config-converted-properties-to-json` (Chinese: 旧的.properties配置文件已迁移到新的.json配置文件！; English: Your old
  .properties configuration file has been migrated to the new .json configuration file!): an old config was migrated.
* `log-inventory-opened` (Chinese: New inventory opened: {0}; English: New inventory opened: {0}): the Chinese value is
  still English; printed when a container opens (`{0}` is the title).
* `log-failed-creating-log` (Chinese: 无法创建日志; English: Log could not be created.): creating the log file failed
  and the bot stops.
* `log-failed-creating-folder` (Chinese: 创建日志文件夹失败。正在退出; English: Failed creating logs folder. Exiting.):
  creating the log directory failed and the bot stops.
* `could-not-detect-theme` (Chinese: 无法检测系统主题 {0}，也许正在使用旧版本 Java？{1}; English: Could not detect system
  theme {0}. Maybe using legacy java version? {1}): GUI theme detection (this repository has no GUI, so it normally
  does not appear).

### 3.7 Other: level-up, Code of Conduct, Discord, constants

* `announce-level-up` (Chinese: 已达到等级: {0}; English: Achieved level: {0}): console output on a level-up (`{0}` is
  the level).
* `received-code-of-conduct` (Chinese: (English only) Received Code of Conduct:\n{0}\nUse /acknowledgecoc to
  acknowledge the Code of Conduct; English: Received Code of Conduct:\n{0}\nUse /acknowledgecoc to acknowledge the Code
  of Conduct): [English only / Chinese falls back to English], multi-line.
* `acknowledged-code-of-conduct` (Chinese: (English only) Acknowledged Code of Conduct; English: Acknowledged Code of
  Conduct): [English only]; on auto-accept or when it had been remembered.
* `rejected-code-of-conduct` (Chinese: (English only) Rejected Code of Conduct. Disconnecting; English: Rejected Code
  of Conduct. Disconnecting): [English only]; not referenced by the current code.
* `discord-webhook-damage` (Chinese: 我受到了伤害！我现在的生命值是: {0}; English: I got damage! My current life is:
  {0}): this repository has no Discord module, so it is not referenced.
* `discord-webhook-respawn` (Chinese: 我重生了!; English: I respawned!): this repository has no Discord module, so it is
  not referenced.
* `command-ackowledgecoc-no-coc` (Chinese: (English only) There is no Code of Conduct to acknowledge; English: There is
  no Code of Conduct to acknowledge): [English only]; `/acknowledgecoc` when there is no CoC.

> - Code of Conduct: when `auto.auto-accept-code-of-conduct=false` and no CoC was remembered, `LoginModule` prints one
>   extra hard-coded English line through `getLog().severe(...)`: `A Code of Conduct must be accepted. Enable
>   "auto.auto-accept-code-of-conduct" in your config.json to accept it automatically.`, and then stops the bot.
> - Discord: keys such as `config-announces-discord*` and `discord-webhook-*` still exist in the language files, but
>   the current source code has no Discord or webhook module (only one old config-key mapping for `discord.web-hook`
>   inside `io/config/PropertyProcessor`). No Discord message therefore appears at run time.

### 3.8 Messages that do not come from the language files (hard-coded or assembled dynamically)

These messages do not come from the language files and are not affected by `misc.language`; most of them are fixed
English (for debugging) and have to be matched separately when integrating.

#### 3.8.1 What the bot says in the in-game chat

* Caught-an-item announcement (chat), content template `FishingBot v<version> - 捕获到 "<item name>"` (zh) /
  `FishingBot v<version> - Caught "<item name>"` (en): `PREFIX + t("module-fishing-caught-item", item name)`.
* Enchantment line (chat, 200 ms after the caught item), content template `-> <enchantment display name>[ <roman
  numeral>]`, for example `-> 海之眷顾 III` / `-> Lure III`: no prefix; the numeral is omitted at level 1.
* Level-up announcement (chat), content template the `announces.announce-lvl-up-text` setting (default `I am level
  %lvl% now`), with `%lvl%` replaced by the level: only when `announce-lvl-up=true` and the text is not `false`.
* Command replies (chat), content templates as in the entries of 3.1, sent to the public chat: the caller is the player
  who spoke in the chat.

> Everything sent through `Player.sendMessage` has `%prefix%` replaced by `FishingBot.PREFIX` first; with server
> protocol >= 1.19 a line starting with `/` is sent as a command and anything else as chat; on 1.8 lines are split
> automatically at about 100 characters.

#### 3.8.2 Hard-coded messages in the console log

* `Using FishingBot v2.13.15` (level: INFO; trigger: the bot starts).
* `Using language: <LANG> @ /lang/fb_<code>.properties` (level: INFO; trigger: the language was initialised
  successfully; `<LANG>` is the enumeration name, for example `CHINESE_SIMPLIFIED`).
* `Failed loading language <LANG>: /lang/fb_<code>.properties` (level: SEVERE; trigger: loading the language file
  failed).
* `Falling back to default language ENGLISH` (level: SEVERE; trigger: the same, falling back to English).
* `Could not find language <value>. Falling back to default language ENGLISH` (level: SEVERE; trigger: `misc.language`
  in the config is invalid).
* `Could not load registry file for pvn <pvn>` (level: SEVERE; trigger: a registry file is missing).
* `Could not find key for value: <value>` (level: SEVERE; trigger: a registry lookup failed).
* `Could not map packet id <id>` (level: WARNING; trigger: a packet mapping is missing).
* `Tried to register null packet for: <x>` / `Tried to register packet twice: <x>` (level: SEVERE; trigger: a
  registration conflict during development).
* `Unhandled data component <x>` / `Tried to add null to data component registry` (level: SEVERE; trigger: an item
  component of a newer version is not handled).
* `Could not instantiate <class>` (level: WARNING; trigger: reflective packet instantiation failed).
* `Error while trying to send: <exception>` (level: SEVERE; trigger: sending a packet failed).
* `Could not create new instance of <class>` (level: WARNING; trigger: creating a packet instance failed).
* `Packet <class> has <n> byte(s) left` / `Handling packet <class> took <n>ms` (level: WARNING / INFO; trigger: only
  when `logs.log-packets=true`).
* `[<STATE>] |C| <<< S : <packet name>` or `0x<id> (<mojmapped>)` (level: INFO; trigger: printed per packet, only when
  `logs.log-packets=true`).
* `Start reading <X> ...` / `End of reading <X> ...` / `Read slot` / `» + ...` / `» - ...` / `Entity data for <id>
  ...` and so on (level: INFO; trigger: only when `logs.log-item-data` / `logs.log-entity-data` is `true`
  (debugging)).
* `Error while waiting for access token: <exception>` (level: WARNING; trigger: polling the Microsoft access token
  failed).
* `A Code of Conduct must be accepted. Enable "auto.auto-accept-code-of-conduct" in your config.json to accept it
  automatically.` (level: SEVERE; trigger: a CoC has to be accepted manually; see 3.7).
* `Received unregistered consume_effect_type: <x>` (level: INFO; trigger: an unregistered consume effect
  (debugging)).
* `Invalid component: <x>` (level: SEVERE; trigger: an item component could not be parsed).

#### 3.8.3 The Realms list (`printRealms`)

```
(empty line)
ID: <id>
name: <name>
motd: <motd>
owner: <owner>
```

> With `realm-id==0` in online mode, the startup prints the list of realms that can be joined (five lines each).

#### 3.8.4 A real sample (this repository's jar, offline mode, connecting to a server that does not exist)

Below is the actual output (English; the language is decided by `misc.language`), and it can serve directly as a
template when integrating:

```
[2026-10-03 02:30:11] [INFO   ] Using language: ENGLISH @ /lang/fb_en_EN.properties
[2026-10-03 02:30:11] [INFO   ] Using FishingBot v2.13.15
[2026-10-03 02:30:11] [INFO   ] Configuration was loaded: /tmp/fbtest/config.json
[2026-10-03 02:30:11] [INFO   ] Starting in offline-mode with user name: FishingBot
[2026-10-03 02:30:11] [INFO   ] Username: FishingBot
[2026-10-03 02:30:11] [INFO   ] Pinging 127.0.0.1:25565 with protocol of MC-AUTOMATIC
[2026-10-03 02:30:11] [SEVERE ] The server 127.0.0.1 is not reachable with the Minecraft version. ...
[2026-10-03 02:30:11] [SEVERE ] An unexpected error occured: Connection refused
[2026-10-03 02:30:11] [SEVERE ] The bot will be stopped...
[2026-10-03 02:30:11] [INFO   ] FishingBot will reconnect in 5 seconds...
```

> Important: the output of `e.printStackTrace()` is forwarded to the log by `CustomPrintStream` at level INFO, which
> shows up as several lines of `[INFO   ] at ...` / `[INFO   ] java.net.ConnectException: ...`. When integrating,
> ignore lines that start with `at ` (or that contain `Exception`) and do not confuse them with real messages.

### 3.9 Keys missing or untranslated in the Chinese language file (important)

The 19 keys that are missing from the Chinese file (they print English in Chinese mode):

```
acknowledged-code-of-conduct            command-ackowledgecoc-desc
command-ackowledgecoc-no-coc            command-ackowledgecoc-syntax
command-clickinv-syntax                 command-droprod-syntax
command-empty-syntax                    command-help-syntax
command-look-syntax                     command-rightclick-syntax
command-summary-syntax                  command-syntax-no-args
command-wait-syntax                     config-auto-auto-accept-code-of-conduct
dialog-code-of-conduct-checkbox         dialog-code-of-conduct-content
dialog-code-of-conduct-header           received-code-of-conduct
rejected-code-of-conduct
```

Keys that are defined in the Chinese file but whose value is still English (no fallback happens, the English text is
printed directly):

```
auth-authorization-succeeded   auth-change-onesix
auth-could-not-get-access-token auth-could-not-get-refresh-token
auth-create-refresh-token       auth-found-refresh-token
auth-refreshed-access-token     auth-using-onesix
command-clickinv-desc           command-clickinv-invalid-slot
command-clickinv-no-inv         command-rightclick-desc
command-rightclick-invalid-slot command-swap-desc
command-swap-syntax             command-wait-desc
command-wait-waited             config-account-logout
config-account-microsoft        config-account-onesix
config-announces-discord        config-announces-discord-web-hook
dialog-authorization-failed-header dialog-authorization-header
log-inventory-opened
```

> Among them `config-announces-discord`=`Discord` and `config-announces-discord-web-hook`=`Webhook` are brand names, so
> being identical in both languages is expected.

> If another project of yours matches against fixed Chinese or English texts, pay special attention to these two
> groups: Chinese mode does not guarantee that everything is Chinese.

---

## 4. Text format rules and placeholders

1. Placeholder replacement order (`I18n.t` + `Player.sendMessage`):
   * first `{0}`, `{1}`, ... are replaced by `MessageFormat.format(value, args)`;
   * then `%prefix%` is replaced by `FishingBot.PREFIX` (only when the text goes through `Player.sendMessage`; the
     console log path does not replace `%prefix%`, but the texts that go to the log do not contain `%prefix%` either).
2. MessageFormat notes: a single quote `'` is an escape character and is written as `''` in the properties (for
   example `bot''s`). If you build a matching regex containing `{` or `}`, escape it yourself.
3. A key that is not found means the string `N/A` is printed (when both the English and the Chinese file lack it).
4. Line breaks `\n`:
   * console: `I18n.info/warning/severe` splits the whole text by line and prints it line by line, each line with the
     timestamp and the level;
   * chat: `Player.sendMessage` splits on `\n` and sends the parts as several chat packets in order.
5. Numbers and strings: many placeholders receive `String.valueOf(...)` (such as the reconnect seconds or the player
   count), so treat them as "any integer or text" when matching.
6. Language selection: `misc.language` takes an enumeration name (`CHINESE_SIMPLIFIED` or `ENGLISH`); an invalid value
   falls back to English and prints `Could not find language ...`.

### 4.1 The main placeholders

* `bot-automatic-reconnect` (placeholder meaning: `{0}` = seconds to wait before reconnecting
  (`auto.auto-reconnect-time`, default 5)).
* `server-pinging` (`{0}` = host, `{1}` = port, `{2}` = default protocol version name).
* `network-received-pong` (`{0}` = MOTD, `{1}` = version name, `{2}` = protocol number, `{3}` = players online).
* `network-changed-address` / `network-changed-port` (`{0}` = new host / new port).
* `module-fishing-caught-item` (`{0}` = item display name).
* `module-login-successful` (`{0}` = player name, `{1}` = UUID).
* `module-login-failed` (`{0}` = failure reason).
* `module-client-disconnected` (`{0}` = disconnect reason).
* `announce-level-up` (`{0}` = new level).
* `command-level` (`{0}` = level).
* `command-look-executed` (`{0}` = yaw, `{1}` = pitch).
* `command-droprod-item-count` (`{0}` = number of rods dropped).
* `command-wait-waited` (`{0}` = seconds waited).
* `command-empty-unknown-type` / `command-droprod-unknown-type` (`{0}` = the invalid input value).
* `module-enabled` / `module-disabled` (`{0}` = module class name).
* `config-loaded-from` (`{0}` = absolute path of the config file).
* `auth-username` (`{0}` = user name).
* `realms-connecting-to` (`{0}` = JSON returned by the realms API).
* `ui-tabs-loot` (`{0}` = total number of items caught).

---

## 5. Things to watch out for (required reading before integrating)

1. Command permissions: there is no authentication. Any player who sends `botname, <command>` in the chat can drive the
   bot (including `/bye` to shut it down, `/empty` to drop items and `/droprod` to drop rods). Use it on a server you
   control, or add an allowlist in the code.
2. Hard conditions for a player trigger: the chat text has to contain both the bot user name and an ASCII comma `,`,
   and the command part must not contain a comma (it would be swallowed by `split(",")`). A failed parse gives no
   feedback at all.
3. A console trigger needs the `/`: `/help` runs a command, while `help` (without the slash) is sent to the server as
   chat by the bot.
4. Command name case: Brigadier literals are case sensitive, so stick to lower case; aliases are lower case as well.
5. Where the "reconnect notice" comes from: `bot-automatic-reconnect` is a console log line (not in-game chat), English
   `FishingBot will reconnect in {0} seconds...`, Chinese `钓鱼机器人将在{0}秒内重新连接...`; `{0}` comes from
   `auto.auto-reconnect-time` (default `5`). It only appears when `auto.auto-reconnect=true` and the reconnect was not
   prevented.
6. The chat prefix changes with the version: `PREFIX = "<name> v<version> - "` comes from `pom.xml` (currently
   `FishingBot v2.13.15 - `). If you match caught-item announcements on the prefix, upgrading changes it.
7. The language is not guaranteed to be all Chinese: see 3.9 - in Chinese mode 19 keys still fall back to English and
   about 26 keys have English values.
8. Messages can appear in either language: if you depend on certain messages (reconnect, login, caught item), support
   both Chinese and English by keyword semantics or a regex instead of matching Chinese only.
9. Debug noise: setting `logs.log-packets` / `logs.log-item-data` / `logs.log-entity-data` to `true` produces a lot of
   English log lines such as `Start reading...`, `[PLAY] |C| <<< S : ...` and `Read slot`, which interferes with
   message matching. Keep them `false` in production.
10. GUI keys produce no real output: the `ui-*`, `dialog-*` and `config-*` labels are for the GUI and the config
    screen, and this repository has no GUI implementation, so they never appear on the console or in the chat.
11. Discord is not wired up: `discord-webhook-*` and `config-announces-discord*` have no code reference today and
    produce no output.
12. Log files: written to `<logsdir>/log%g.log` (rolling), with the number kept given by `logs.log-count` (default 15).
    The console and the log file have the same content.
13. `-onlyCreateConfig`: only generates the config file without connecting or fishing (useful to initialise or
    validate a config).
14. Config changes are not hot-reloaded: editing `config.json` needs a restart, and so does the language (it takes
    effect after a restart).

### 5.1 Suggestions for matching messages

* Prefer key phrases over whole sentences, for example:
  * reconnect: `reconnect in (\d+) seconds` / `将在(\d+)秒内重新连接`, or match the `自动重新连接` keyword directly (a
    config label, not recommended);
  * caught item: `Caught "(.+)"` / `捕获到 "(.+)"`;
  * level-up: `Achieved level: (\d+)` / `已达到等级: (\d+)`;
  * successful login: `Login successful!` / `登录成功!` (followed by `Name:` / `UUID:`).
* If you control the start-up arguments or the environment, you can pin `misc.language` to `CHINESE_SIMPLIFIED` or
  `ENGLISH` to keep matching simple.

---

## Appendix A: default values of the config options at a glance (`config.json` / `SettingsConfig`)

> The "config path" column below is the logical dotted path used in the code (`@Property.key`). The actual `config.json`
> is nested, for example `server.ip` -> `{"server":{"ip":...}}` and `auto.auto-eject.rules` ->
> `{"auto":{"auto-eject":{"rules":[...]}}}`.

* `server.ip` (default: `127.0.0.1`): the server address (a port in it is split off automatically).
* `server.port` (default: `25565`): the server port.
* `server.realm-id` (default: `-1`): `-1` means no realms; `0` lists the realms at startup; anything else picks that
  realm.
* `server.realm-accept-tos` (default: `false`): whether the realms ToS is accepted automatically.
* `server.online-mode` (default: `true`): online (premium) mode.
* `server.default-protocol` (default: `AUTOMATIC`): the default MC version / protocol.
* `server.spoof-forge` (default: `false`): spoof the Forge handshake.
* `auto.auto-reconnect` (default: `true`): reconnect automatically after a disconnect.
* `auto.auto-reconnect-time` (default: `5`): seconds to wait before reconnecting (the `{0}` of the reconnect message).
* `auto.auto-disconnect` (default: `false`): disconnect automatically once the player count reaches a threshold.
* `auto.auto-disconnect-players-threshold` (default: `5`): that player-count threshold.
* `auto.auto-sneak` (default: `false`): sneak automatically.
* `auto.auto-accept-code-of-conduct` (default: `false`): accept the server's Code of Conduct automatically.
* `auto.auto-command-on-respawn.enabled` (default: `false`): run commands automatically on respawn or on joining.
* `auto.auto-command-on-respawn.delay` (default: `1000`): the delay in ms.
* `auto.auto-command-on-respawn.commands` (default: `["%prefix%I respawned", "/home fishing"]`): the commands or chat.
* `auto.auto-command-before-death.enabled` (default: `false`): run commands automatically when close to death.
* `auto.auto-command-before-death.commands` (default: `["%prefix%I am about to die", "/home"]`).
* `auto.auto-command-before-death.min-health-before-death` (default: `6.0`): the health that triggers it (2 = one heart).
* `auto.auto-quit-before-death.enabled` (default: `false`): quit automatically when close to death.
* `auto.auto-quit-before-death.min-health-before-quit` (default: `6.0`): the health that triggers it.
* `auto.auto-eject.enabled` (default: `false`): eject the loot automatically.
* `auto.auto-eject.rules` (default: three DROP rules for fish/treasure/junk): the ejection rules (direction,
  allowlist and type).
* `auto.timer.enabled` (default: `false`): the scheduled-task switch.
* `auto.timer.timers` (default: one example entry (every 5 minutes)): commands to run on a schedule.
* `account.mail` (default: `FishingBot`): the offline-mode user name.
* `logs.log-count` (default: `15`): how many log files are kept.
* `logs.log-packets` (default: `false`): log packets (debugging).
* `logs.log-entity-data` (default: `false`): log entity data (debugging).
* `logs.log-item-data` (default: `false`): log item data (debugging).
* `announces.announce-type-chat` (default: `ONLY_ENCHANTED`): the chat announcement type (`ALL` / `ALL_BUT_FISH` /
  `ONLY_ENCHANTED` / `ONLY_BOOKS` / `NONE`).
* `announces.announce-type-console` (default: `ALL`): the console announcement type.
* `announces.announce-lvl-up` (default: `true`): whether to announce level-ups in the chat.
* `announces.announce-lvl-up-text` (default: `I am level %lvl% now`): the level-up chat text (with the `%lvl%`
  placeholder).
* `start-text.enabled` (default: `true`): run the start-up action automatically after joining.
* `start-text.text` (default: `["%prefix%Starting fishing", "/trigger Bot"]`): the start-up text or command.
* `misc.stucking-fix-enabled` (default: `true`): the stuck fix.
* `misc.prevent-rod-breaking` (default: `true`): prevent the rod from running out.
* `misc.disable-rod-checking` (default: `false`): disable the rod-availability check.
* `misc.language` (default: the system default language, falling back to English): the language.
* `misc.look-speed` (default: `16`): the view turning speed.

## Appendix B: keys defined in the language files but not referenced by the current code

`network-not-supported-server-version`, `rejected-code-of-conduct`, `discord-webhook-damage`,
`discord-webhook-respawn`, `command-help-other-player-private` (only used by the private-chat branch, which is not
enabled), plus all the `ui-*`, `dialog-*` and `config-*` (except the few that are referenced in the log) GUI and
config-screen texts.

> These keys never appear in the run log or in the chat, so there is nothing to match when integrating; changing them
> in the language files has no side effect either.

---

## Appendix C: where the main source code lives (for further development)

* Language loading and lookup (files: `i18n/I18n.java`, `i18n/Language.java`, `resources/lang/fb_*.properties`).
* Prefix and title (files: `FishingBot.java` (`PREFIX`/`TITLE`), `resources/fishingbot.properties`, `pom.xml`).
* Reading console commands (file: `modules/ChatProxyModule.java`).
* Parsing player chat commands (file: `modules/command/ChatCommandModule.java`).
* Command registration and dispatch (files: `modules/command/CommandRegistry.java`, `modules/command/commands/*`).
* Command executors and where replies go (files: `modules/command/executor/*.java`, `bot/Player.java#sendMessage`).
* Reconnecting after a disconnect (file: `Bot.java` (`bot-automatic-reconnect`)).
* Fishing and caught-item announcements (file: `modules/fishing/FishingModule.java`).
* Level-up, health and respawn (file: `bot/Player.java`).
* Network and connection messages (files: `network/ping/ServerPinger.java`, `network/protocol/NetworkHandler.java`).
* Realms (files: `network/mojangapi/MojangAPI.java`, `Bot.java`).
* Log formatting (file: `io/logging/LogFormatter.java`).
* Config defaults (file: `io/config/SettingsConfig.java`).
