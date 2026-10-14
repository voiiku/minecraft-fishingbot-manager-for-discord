# FishingBot 魔改版：指令与消息完整文档
[English](BOT_COMMANDS_AND_MESSAGES-en_us.md) | **简体中文**

> 适用版本：`systems.kinau:FishingBot:2.13.15`（本仓库魔改版）
> 本文档面向“把 FishingBot 当作子进程/服务来对接”的使用者，重点是：
> 1. 机器人能接收的全部指令（含触发方式与注意事项）。
> 2. 机器人输出的全部消息（中文 + 英文原文），因为其它项目依赖这些消息做判断。

本文所有消息文案均取自源码与语言文件：

* 英文：`src/main/resources/lang/fb_en_EN.properties`
* 中文：`src/main/resources/lang/fb_zh_CN.properties`

运行时由 `systems.kinau.fishingbot.i18n.I18n` 读取。语言由配置项 `misc.language` 决定（`CHINESE_SIMPLIFIED` / `ENGLISH`，需重启生效）。

---

## 0. 关键概念（对接前必读）

### 0.1 消息前缀 / 标题

`FishingBot.PREFIX` 与 `TITLE`（见 `FishingBot.java`）：

* `fishingbot.properties` `name`：`FishingBot`；来自 `pom.xml` 的 `<name>`。
* `fishingbot.properties` `version`：`2.13.15`；来自 `pom.xml` 的 `<version>`。
* `PREFIX`：`FishingBot v2.13.15 - `（注意结尾是 ` - `，含空格）；聊天消息前缀、`%prefix%` 的替换值。
* `TITLE`：`FishingBot v2.13.15`；仅用于启动日志 `Using FishingBot v2.13.15`。

> 若 `fishingbot.properties` 读取失败，`PREFIX = "FishingBot vUnknown - "`。

### 0.2 消息“去向”（对判断消息来源极其重要）

* 控制台/日志（产生方式：`FishingBot.getI18n().info/warning/severe(key,...)` 以及 `ConsoleCommandExecutor.sendMessage`）：打印到标准输出，同时写入 `logs/log%g.log`。
* 游戏内聊天（公共）（产生方式：`PlayerCommandExecutor.sendMessage` → `net.sendPacket(PacketOutChatMessage(...))`）：机器人以普通聊天方式在服务器里发言。
* 两者都有（产生方式：例如“钓到物品”公告：控制台走 `log::info`，聊天走带 `PREFIX` 的聊天包）。
* 游戏内聊天（私聊）（产生方式：`PlayerCommandExecutor` 的私聊分支，当前代码未启用（`ChatCommandModule` 恒用 `new PlayerCommandExecutor()`，无 private 目标））：目前不会出现。

### 0.3 控制台日志行格式

`io/logging/LogFormatter.java`：

```
[yyyy-MM-dd HH:mm:ss] [LEVEL  ] <message>
```

* `LEVEL` 为 `INFO` / `WARNING` / `SEVERE`（`Level.getLocalizedName()`，7 字符左对齐）。
* 每条消息末尾带一个换行（`%n`）。
* 多行消息（含 `\n` 的 i18n 文案）会被 `I18n.info/warning/severe` 按行拆分后逐行打印，因此每行都会带时间戳/级别前缀。

---

## 1. 启动方式与命令行参数

入口：`systems.kinau.fishingbot.Main#main`（`net.minecraft.client.main.Main` 是“被官方启动器/OneSix 参数拉起”时的适配入口，最终同样调用 `systems.kinau.fishingbot.Main.main`）。

命令行参数（`Main.java`，基于 Apache Commons CLI）：

* `-nogui`（是否带值：否）：禁用 GUI（本仓库无 GUI 实现，历史保留，无实际输出）。
* `-help`（是否带值：否）：打印帮助并退出。
* `-logsdir <dir>`（是否带值：是）：指定日志目录（默认：程序所在目录 `logs/`）。
* `-config <path>`（是否带值：是）：指定配置文件（默认：程序所在目录 `config.json`）。
* `-refreshToken <path>`（别名 `-accountfile`）（是否带值：是）：指定微软登录 refreshToken 文件（默认：程序目录 `refreshToken`）。
* `-onlyCreateConfig`（是否带值：否）：仅生成配置后退出（不连接服务器）。

> Docker 内脚本 `docker/fishing-bot` 使用的形式：
> `java -jar FishingBot.jar -nogui -logsdir /var/log/fishingbot/ -config .../config.json -refreshToken .../refreshToken`

---

## 2. 指令系统

### 2.1 指令来源（3 类外部 + 2 类内部）

* A. 控制台（触发方式：运行时向 `System.in` 输入一行；入口代码：`ChatProxyModule` 读一行 → `bot.runCommand(line, true, new ConsoleCommandExecutor())`）。
* B. 游戏内玩家聊天（触发方式：玩家聊天栏输入 `"<机器人用户名>, <指令>"`；入口代码：`ChatCommandModule#onChat` → `commandRegistry.dispatchCommand(cmd, new PlayerCommandExecutor())`）。
* C. 内部：启动动作（触发方式：进入游戏并延迟 1.5s 后，执行配置 `start-text.text`；入口代码：`ClientDefaultsModule#onJoinGame` → `runCommand(...)`）。
* D. 内部：定时任务（触发方式：按 `auto.timer.timers` 周期性执行；入口代码：`TimerModule#startTimers` → `runCommand(...)`）。
* E. 内部：重生/濒死自动指令（触发方式：`auto.auto-command-on-respawn.commands`、`auto.auto-command-before-death.commands`；入口代码：`Player`（事件处理）→ `runCommand(...)`）。

> 能触发指令的是 A、B 两条外部路径；C/D/E 是配置里预置的自动指令，走同一套解析。
> 注意：控制台输入的指令必须带前导 `/`，否则会被当作“机器人要说的话”直接发到服务器聊天（见 2.2）。

### 2.2 触发/解析细节（务必逐条看）

1. 控制台（A）：`Bot#runCommand`：
   * 行以 `/` 开头 → 先匹配机器人指令；匹配成功即执行且不再发往服务器。
   * 不匹配、或行不以 `/` 开头 → 通过 `Player.sendMessage` 当作普通聊天内容发给服务器。
   * 因此：控制台“聊天”= 直接敲文本；“执行机器人指令”= 敲 `/指令`。
2. 玩家聊天（B）：仅当聊天文本同时包含机器人用户名 + 英文逗号 `,` 才尝试解析：
   * 用 `,` 对整条消息 `split(",")`，取 `parts[1]` 起的所有片段去掉逗号直接拼接，再 `trim()`。
   * 拼接结果若以空白开头，去掉一个空白字符。
   * 交由 `dispatchCommand`：解析前去掉前导 `/`（故 `机器人名, help` 与 `机器人名, /help` 均可）。
   * 只要文本里出现机器人名和逗号就会尝试触发；解析失败时静默忽略，无任何输出。
   * 因为按 `,` 拼接，指令参数里不能带逗号（会被吞掉）。
3. 前导斜杠：`CommandRegistry#dispatchCommand` 会去掉开头的 `/`（只去一个）。
4. 返回值语义：全部内置指令 `return 0`；`dispatchCommand` 在返回 `0` 时视为“已执行”（返回 `true`），抛 `CommandSyntaxException` 时视为未处理（返回 `false`）。
5. 无权限系统：任何能在服务器聊天里发出 `机器人名, 指令` 的玩家都能操控机器人，无白名单/管理员校验。放公共服务器时请自行限制。

### 2.3 指令表（全部内置指令）

* `help`（语法：`/help [指令]`）：显示帮助/某指令语法。
* `bye`（别名：`stop`, `shutdown`；语法：`/bye`）：停止机器人（阻止重连，`stopBot(true)`）。
* `level`（别名：`level?`；语法：`/level`）：显示当前经验等级。
* `stuck`（别名：`recast`, `reeject`, `refish`, `recatch`；语法：`/stuck`）：重新抛竿（触发卡顿修复）。
* `pause`（语法：`/pause`）：暂停钓鱼。
* `unpause`（别名：`resume`；语法：`/unpause`）：恢复钓鱼。
* `empty`（别名：`clear`, `drop`；语法：`/empty [方向] [槽位]`）：丢弃背包（保留鱼竿）；可指定方向+槽位。
* `droprod`（别名：`roddrop`, `droprods`, `rodsdrop`, `emptyrod`, `rodempty`；语法：`/droprod [Filter]`）：丢弃鱼竿，Filter=`ALL`/`SELECTED`/`ALL_BUT_SELECTED`。
* `look`（语法：`/look <yaw> <pitch> [speed]`）：转动视角（yaw/pitch ∈ [-180,180]，speed 默认取配置 `misc.look-speed`=16）。
* `rightclick`（别名：`use`；语法：`/rightclick [槽位(1-9)]`）：右键使用（可先切到指定快捷栏槽位）。
* `clickinv`（别名：`invclick`；语法：`/clickinv <槽位> [left\|right]`）：在已打开容器界面点击槽位。
* `swap`（别名：`swapitem`；语法：`/swap <槽位> <快捷栏槽位1-9>`）：交换背包物品到快捷栏。
* `wait`（别名：`sleep`；语法：`/wait <秒>`）：机器人等待 N 秒。
* `summary`（别名：`summarize`, `stats`, `statistics`, `caught`, `loot`；语法：`/summary [clear]`）：打印已钓获物品统计（`clear` 后清空）。
* `acknowledgecoc`（别名：`acknowledgecodeofconduct`, `acceptcoc`, `acceptcodeofconduct`；语法：`/acknowledgecoc [save]`）：接受服务器 Code of Conduct（`save` 会写入配置记住）。

> “指令”在控制台写成 `/xxx`；在游戏聊天写成 `机器人用户名, xxx`。
> 指令对参数类型有约束（如 `empty` 的方向仅 `NORTH/EAST/SOUTH/WEST/DOWN`；`clickinv` 槽位 ≥1；`swap` 快捷栏 1-9），不满足时 Brigadier 解析失败 → 静默忽略（玩家侧）或在控制台被当作聊天发送。

---

## 3. 输出消息总表（中英对照）

> 说明：
> - 表中“中文”为 `fb_zh_CN.properties` 的实际值；“English”为 `fb_en_EN.properties` 的值。
> - `{0}`、`{1}` 为 `java.text.MessageFormat` 占位符，按参数顺序填充；`\n` 表示换行（日志里会拆成多行）。
> - 标注 [仅英文定义/中文回退英文] 的条目：中文语言文件里没有该 key，`I18n.t()` 会回退到英文值。
> - 标注 [中文值仍为英文] 的条目：中文语言文件里有该 key，但值就是英文（不会回退，只会照原样输出英文）。
> - 未找到 key 时 `I18n.t()` 返回字面量 `N/A`。

### 3.1 指令响应 / 指令帮助（`command-*`）

* `command-bye`（中文：再见；English：Goodbye.；去向 / 触发：执行 `/bye` 时回复给触发者）。
* `command-empty`（中文：正在丢出背包物品...；English：Dropping inventory...；去向 / 触发：执行 `/empty`）。
* `command-empty-unknown-type`（中文：方向 {0} 未知. 试试如下方向: NORTH, EAST, SOUTH, WEST, DOWN；English：The direction {0} is unknown. Try one of these directions: NORTH, EAST, SOUTH, WEST, DOWN；去向 / 触发：`/empty` 方向非法）。
* `command-stuck-executed`（中文：抛出鱼竿；English：Fishing rod cast；去向 / 触发：执行 `/stuck`）。
* `command-pause-executed`（中文：已暂停钓鱼；English：Fishing paused；去向 / 触发：执行 `/pause`）。
* `command-unpause-executed`（中文：已恢复钓鱼；English：Fishing resumed；去向 / 触发：执行 `/unpause`）。
* `command-level`（中文：我有 {0} 级经验；English：I have {0} levels；去向 / 触发：执行 `/level`）。
* `command-look-executed`（中文：正在查看 {0} / {1}；English：Now looking at {0} / {1}；去向 / 触发：执行 `/look`（参数为 yaw / pitch））。
* `command-droprod-item-count`（中文：丢弃了 {0} 杆鱼竿(s)；English：Dropped {0} fishing rod(s)；去向 / 触发：执行 `/droprod`）。
* `command-droprod-unknown-type`（中文：筛选器 {0} 未知. 尝试下列筛选器之一:\n- ALL: 丢出所有鱼竿\n- SELECTED: 丢出当前持有的鱼竿\n- ALL_BUT_SELECTED: 丢出当前持有的鱼竿外所有鱼竿；English：The filter {0} is unknown. Try one of these filters:\n- ALL: Drops all fishing rods\n- SELECTED: Drops the currently selected fishing rod\n- ALL_BUT_SELECTED: Drops all fishing rods but the currently selected one；去向 / 触发：`/droprod` Filter 非法（多行））。
* `command-summary-empty`（中文：我还没有钓到任何东西！；English：I haven't fished anything yet!；去向 / 触发：执行 `/summary` 但无记录）。
* `command-help-invalid-command`（中文：找不到命令。使用 /help 列出所有命令。；English：Command not found. Use /help to list all commands.；去向 / 触发：`/help <未知指令>`）。
* `command-clickinv-no-inv`（中文：Currently there is no inventory opened [中文值仍为英文]；English：Currently there is no inventory opened；去向 / 触发：`/clickinv` 无打开的容器）。
* `command-clickinv-invalid-slot`（中文：The opened inventory does not have this slot number [中文值仍为英文]；English：The opened inventory does not have this slot number；去向 / 触发：`/clickinv` 槽位不存在）。
* `command-wait-waited`（中文：I waited {0} second(s) [中文值仍为英文]；English：I waited {0} second(s)；去向 / 触发：`/wait` 完成后）。
* `command-rightclick-invalid-slot`（中文：The given slot is out of range (1-9) [中文值仍为英文]；English：The given slot is out of range (1-9)；去向 / 触发：`/rightclick` 槽位越界）。

指令的“描述/语法/帮助”文案（用于 `/help` 列表与 `/help <cmd>`；这些值也作为指令构造时的 description）：

* `command-help-desc`（中文：显示此帮助消息；English：Displays this help message）。
* `command-bye-desc`（中文：停止钓鱼机器人；English：Stops the bot）。
* `command-level-desc`（中文：显示机器人的当前等级；English：Shows the bot's current levels）。
* `command-stuck-desc`（中文：再次抛出鱼竿；English：Cast out the fishing rod again）。
* `command-pause-desc`（中文：暂停钓鱼；English：Pauses fishing）。
* `command-unpause-desc`（中文：恢复钓鱼；English：Resumes fishing）。
* `command-empty-desc`（中文：丢出我背包的所有物品（钓鱼竿除外）；English：Drops all contents of my inventory (except the fishing rod)）。
* `command-droprod-desc`（中文：丢出当前持有的鱼竿、除持有的鱼竿以外的所有鱼竿，或所有鱼竿；English：Drops either your current selected rod, all but your selected rod or all of your rods）。
* `command-look-desc`（中文：让机器人在别处寻找；English：Makes the bot look elsewhere）。
* `command-summary-desc`（中文：打印捕获物品的摘要；English：Prints a summary of the caught items）。
* `command-swap-desc`（中文：Swaps two items in the inventory [中文值仍为英文]；English：Swaps two items in the inventory）。
* `command-clickinv-desc`（中文：Let the bot click a slot in an opened inventory [中文值仍为英文]；English：Let the bot click a slot in an opened inventory）。
* `command-wait-desc`（中文：Let the bot wait some time (in seconds) [中文值仍为英文]；English：Let the bot wait some time (in seconds)）。
* `command-rightclick-desc`（中文：Let the bot perform a rightclick [中文值仍为英文]；English：Let the bot perform a rightclick）。
* `command-ackowledgecoc-desc`（中文：Acknowledges the server's Code of Conduct [仅英文定义/中文回退英文]；English：Acknowledges the server's Code of Conduct）。

指令语法（`/help <cmd>` 输出；多数字符串中文文件未定义 → 回退英文）：

* `command-syntax-no-args`（中文：/{0} [仅英文]；English：/{0}）。
* `command-help-syntax`（中文：/{0} [Command] [仅英文]；English：/{0} [Command]）。
* `command-empty-syntax`（中文：/{0} [Direction] [Slot] [仅英文]；English：/{0} [Direction] [Slot]）。
* `command-droprod-syntax`（中文：/{0} [Filter] [仅英文]；English：/{0} [Filter]）。
* `command-look-syntax`（中文：/{0} <Yaw> <Pitch> [Speed] [仅英文]；English：/{0} <Yaw> <Pitch> [Speed]）。
* `command-clickinv-syntax`（中文：/{0} <Slot> [left\|right] [仅英文]；English：/{0} <Slot> [left\|right]）。
* `command-rightclick-syntax`（中文：/{0} [Hotbar slot] [仅英文]；English：/{0} [Hotbar slot]）。
* `command-summary-syntax`（中文：/{0} [clear] [仅英文]；English：/{0} [clear]）。
* `command-wait-syntax`（中文：/{0} <Time in seconds> [仅英文]；English：/{0} <Time in seconds>）。
* `command-swap-syntax`（中文：/swap <Slot> <Hotbar slot> [中文值仍为英文]；English：/swap <Slot> <Hotbar slot>）。
* `command-ackowledgecoc-syntax`（中文：/{0} [save] [仅英文]；English：/{0} [save]）。

`/help` 列表的输出格式（`CommandHelp`，每条指令一行）：

* `command-help-console`（中文：/{0} - {1}；English：/{0} - {1}）：控制台：`/指令名 - 描述`。
* `command-help-other-player`（中文：我的名字是 {0} - {1}；English：My name, {0} - {1}）：玩家触发（公共）。
* `command-help-other-player-private`（中文：/msg 我的名字 {0} - {1}；English：/msg My name {0} - {1}）：私聊触发（当前不会出现）。

`/summary` 的正文：

* `ui-tabs-loot`（中文：统计 ({0})；English：Loot ({0})）：`{0}` = 数量合计。
* （无 key，硬编码）（中文：`<数量>x <物品显示名>`；English：`<count>x <displayName>`）：每个物品一行，如 `3x 鳕鱼` / `3x Cod`（随客户端语言资源）。

### 3.2 模块运行消息（`module-*`）

* `module-chat-proxy-chat-message`（中文：[CHAT] {0}；English：[CHAT] {0}；去向 / 触发：控制台；每收到一条服务器聊天就打印（ChatProxy））。
* `module-client-disconnected`（中文：已断开连接 {0}；English：Disconnected: {0}；去向 / 触发：控制台；收到断开包（`{0}`=断开原因文本））。
* `module-fishing-start-fishing`（中文：我开始钓鱼了!；English：I start to fish!；去向 / 触发：控制台；进入游戏开始抛竿）。
* `module-fishing-no-rod-available`（中文：未找到任何鱼竿...；English：No rod has been found...；去向 / 触发：控制台；没有可用鱼竿）。
* `module-fishing-new-rod-available`（中文：新的鱼杆可用。我又开始钓鱼了！；English：A new rod is available. I start fishing again!；去向 / 触发：控制台；重新获得鱼竿）。
* `module-fishing-bot-is-slow`（中文：机器人当前的钓鱼速度较慢。也许它被卡住了。正在尝试重新抛竿！；English：The bot is currently slow. Maybe he is stuck. Trying to restart!；去向 / 触发：控制台；60 秒未钓到鱼（卡顿修复））。
* `module-fishing-entity-hooked`（中文：警告：捕获到实体！将在几秒后重试。；English：Caution: An entity has been caught! Trying again in a few seconds.；去向 / 触发：控制台；钓到实体而非物品）。
* `module-fishing-health-threshold-reached`（中文：达到设定退出前的生命值阈值，机器人将退出；English：Health threshold before quit reached. Bot will be stopped.；去向 / 触发：控制台；触发“濒死自动退出”）。
* `module-fishing-caught-item`（中文：捕获到 "{0}"；English：Caught "{0}"；去向 / 触发：控制台 + 游戏聊天；`{0}`=物品显示名，聊天形式见 3.8）。
* `module-handshake-disabling`（中文：无法禁用握手模块.；English：Cannot disable the HandshakeModule.；去向 / 触发：控制台；尝试禁用握手模块）。
* `module-login-hash-error`（中文：无法对服务器id进行哈希处理。机器人将被停止...；English：The server id could not be hashed. The bot will be stopped...；去向 / 触发：控制台；加密登录失败）。
* `module-login-failed`（中文：登录失败: {0}；English：Login failed: {0}；去向 / 触发：控制台；登录被拒（`{0}`=原因））。
* `module-login-successful`（中文：登录成功!\nName: {0}\nUUID: {1}；English：Login successful!\nName: {0}\nUUID: {1}；去向 / 触发：控制台；登录成功（多行））。
* `module-enabled`（中文：模块 "{0}" 已启用!；English：Module "{0}" enabled!；去向 / 触发：控制台；每个模块启用时）。
* `module-disabled`（中文：模块 "{0}" 已禁用!；English：Module "{0}" disabled!；去向 / 触发：控制台；每个模块禁用时）。

> 常见模块名（`{0}` 取值 = 类名）：`ChatProxyModule`、`ClientDefaultsModule`、`HandshakeModule`、`LoginModule`、`FishingModule`、`EjectionModule`、`TimerModule`、`ChatCommandModule`。

### 3.3 网络 / 连接消息（`network-*`、`server-pinging`、`bot-*`、`packet-*`）

* `network-invalid-server-address`（中文：设置的服务器似乎不存在。请检查配置文件中的服务器地址(server.ip)；English：The given server seems to be not existing. Please check the server address in the Settings (server.ip)；去向 / 触发：控制台；`server.ip` 为空）。
* `network-could-not-ping`（中文：Minecraft版本{0}无法访问服务器。自动版本检测可能不起作用。请在配置文件中设置默认版本协议；English：The server {0} is not reachable with the Minecraft version. The automatic version detection may not work. Please set a default-protocol in the Settings.；去向 / 触发：控制台；ping 失败（两处复用））。
* `network-unknown-host`（中文：服务器 {0} 未知；English：The server {0} is unknown.；去向 / 触发：控制台；DNS 解析失败）。
* `network-received-pong`（中文：接收到的标题: {0}, 版本: {1} ({2}), 在线玩家数: {3}；English：Received pong: {0}, version: {1} ({2}), players online: {3}；去向 / 触发：控制台；ping 得到 MOTD/版本/人数）。
* `network-server-is-full`（中文：服务器已满。机器人将被停止!；English：The server is full. Bot will be stopped!；去向 / 触发：控制台；人数 ≥ 自动断开阈值）。
* `network-changed-address`（中文：服务器主机已更改: {0}；English：The server host has been changed: {0}；去向 / 触发：控制台；SRV 解析到新主机）。
* `network-changed-port`（中文：服务器端口已更改: {0}；English：The server port has been changed: {0}；去向 / 触发：控制台；SRV 解析到新端口）。
* `network-ping-differs-protocol`（中文：服务器响应的协议版本 {0} 与bot选择的协议版本 {1}不同。您可能需要在配置中更改它；English：The server responded with a different protocol version {0} than what the bot selected {1}. You may need to change it in the Config.；去向 / 触发：控制台；服务器协议与所选不一致）。
* `network-unknown-packet-id`（中文：未设置 {1} 中 {0} 的数据包ID。请报告：\nhttps://discord.com/invite/xHpCDYf；English：The packet id for {0} in {1} is not set. Please report this: https://discord.com/invite/xHpCDYf；去向 / 触发：控制台；未映射数据包）。
* `server-pinging`（中文：Pinging {0}:{1} 协议为 MC-{2}；English：Pinging {0}:{1} with protocol of MC-{2}；去向 / 触发：控制台；连接前 ping（{2}=默认协议版本名））。
* `packet-could-not-be-received`（中文：无法接收数据包！正在关闭机器人...；English：Could not receive packet! Shutting down...；去向 / 触发：控制台；读包异常，退出主循环）。
* `bot-could-not-be-started`（中文：发生意外错误: {0}\n机器人将被停止...；English：An unexpected error occured: {0}\nThe bot will be stopped...；去向 / 触发：控制台；连接/网络异常（多行））。
* `bot-automatic-reconnect`（中文：钓鱼机器人将在{0}秒内重新连接...；English：FishingBot will reconnect in {0} seconds...；去向 / 触发：控制台；断线自动重连等待前打印（`{0}`=`auto.auto-reconnect-time`，默认 5））。
* `network-not-supported-server-version`（中文：此服务器未运行受支持的协议版本: {0}\n机器人可能无法正常工作；English：This server is not running a supported protocol version: {0}\nThe bot might not work properly.；去向 / 触发：已定义但当前代码未引用（未使用））。

> 与你示例对应的那条：断线重连提示 = `bot-automatic-reconnect`，中文 `钓鱼机器人将在{0}秒内重新连接...`，英文 `FishingBot will reconnect in {0} seconds...`，输出到控制台日志（不是游戏聊天）。

### 3.4 登录 / 账户消息（`credentials-*`、`auth-*`、`*keys*`、`config-account-*`、`dialog-*`）

* `credentials-using-offline-mode`（中文：以脱机模式启动，用户名:{0}；English：Starting in offline-mode with user name: {0}）：离线模式启动。
* `credentials-invalid`（中文：指定的账户无效。这可能有几个原因（参见：https://github.com/MrKinau/FishingBot/wiki/Troubleshooting).；English：The credentials you specified are invalid. This can have several reasons (see: https://github.com/MrKinau/FishingBot/wiki/Troubleshooting).）：认证失败。
* `auth-username`（中文：用户名: {0}；English：Username: {0}）：认证后打印用户名。
* `auth-using-microsoft`（中文：使用微软进行身份验证；English：Trying to authenticate at Microsoft...）：开始微软认证。
* `auth-using-onesix`（中文：Trying to authenticate using OneSix arguments...；English：Trying to authenticate using OneSix arguments...）：中英相同。
* `auth-change-onesix`（中文：Changing authentication to use OneSix；English：Changing authentication to use OneSix）：中英相同。
* `auth-create-refresh-token`（中文：Please log in with your Microsoft account using the following code {0} at {1}.\nThis is a one-time step ... your data!；English：（同左，英文））：中文值仍为英文；设备码登录提示（{0}=user code，{1}=验证网址）。
* `auth-found-refresh-token`（中文：Found refreshToken at {0}；English：Found refreshToken at {0}）：中文值仍为英文。
* `auth-could-not-get-refresh-token`（中文：Error while creating refresh token... Please try again；English：Error while creating refresh token... Please try again）：中文值仍为英文。
* `auth-could-not-get-access-token`（中文：Error while creating access token... Please try again；English：Error while creating access token... Please try again）：中文值仍为英文。
* `auth-try-refreshing-access-token`（中文：正在尝试刷新访问令牌...；English：Trying to refresh access token...）。
* `auth-refreshed-access-token`（中文：Access token successfully refreshed；English：Access token successfully refreshed）：中文值仍为英文。
* `auth-authorization-succeeded`（中文：The authentication was successful；English：The authentication was successful）：中文值仍为英文。
* `could-not-get-keys`（中文：Could not retrieve the certificates required for chat signing from {0}: {1}；English：Could not retrieve the certificates required for chat signing from {0}: {1}）：中文值仍为英文。
* `retrieved-keys`（中文：Obtained the required keys for chat signing!；English：Obtained the required keys for chat signing!）：中文值仍为英文。
* `config-account-mail`（中文：电子邮箱（或用户名）；English：Username (only offline mode)）：配置标签（GUI 用；本仓库无 GUI）。
* `config-account-microsoft`（中文：You don't need to provide any credentials ...；English：（同左，英文））：中文值仍为英文；GUI。
* `config-account-onesix`（中文：Using OneSix authentication, ...；English：（同左，英文））：中文值仍为英文；GUI。
* `config-account-logout`（中文：Logout；English：Logout）：中英相同；GUI。
* `dialog-credentials-invalid-header`（中文：账户无效；English：Credentials invalid）：GUI 对话框。
* `dialog-credentials-invalid-content`（中文：指定的账户无效。这可能有几个原因：；English：The credentials you specified are invalid. This can have several reasons:）：GUI 对话框。
* `dialog-authorization-header`（中文：Authorization process；English：Authorization process）：中文值仍为英文；GUI。
* `dialog-authorization-failed-header`（中文：Authorization failed；English：Authorization failed）：中文值仍为英文；GUI。

> 本节里有两条被本项目的 `/bot login` 使用：`auth-create-refresh-token`（从它那行解析出验证网址与设备码，
> 转给调用者）与 `auth-could-not-get-refresh-token`（判定"微软拒绝了这次登录"）。授权到底成没成不看消息，
> 只看 `refreshToken` 文件有没有出现，所以 `auth-authorization-succeeded` 不必解析。这两个关键字写在
> `src/bot_manage/login.rs` 的常量里，有单测与这份清单逐字核对。

### 3.5 Realms（领域服）消息（`realms-*`）

* `realms-id-not-set`（中文：未设置领域ID。正在关闭...；English：Realms ID was not set. Shutting down...）：`realm-id==0` 时列出领域后关闭。
* `realms-tos-agreement`（中文：（星号框）如果你想使用领域，你必须在配置文件中接受ToS；English：（星号框）If you want to use realms you have to accept the tos in the Settings (realm-accept-tos)）：未接受 ToS。
* `realms-determining-address`（中文：正在尝试解析服务器地址...(尝试{0})；English：Trying to resolve the server address... (Try {0})）：解析领域地址重试。
* `realms-could-not-accept-tos`（中文：无法接受领域的服务条款: {0}；English：The terms of service for realms could not be accepted: {0}）。
* `realms-accepted-tos`（中文：已接受领域的服务条款!；English：The terms of service for realms have been accepted!）。
* `realms-could-not-determine-address`（中文：无法从 {0}: {1} 检索服务器地址；English：Could not retrieve server address from {0}: {1}）。
* `realms-connecting-to`（中文：正在连接到领域 {0}...；English：Connecting to realm {0}...）：`{0}`=接口返回的 JSON（实际为地址信息）。
* `realms-could-not-connect-to-endpoint`（中文：无法连接到领域 ({0}): {1}；English：Could not connect to the realms endpoint ({0}): {1}）。
* `realms-no-realms`（中文：没有可供您加入的领域。记得在加入领域之前接受邀请；English：There are no realms available for you to join. Remember to accept an invitation before you can join the realm.）：无领域。
* `realms-could-not-parse-server-entry`（中文：无法解析 Realms 服务器条目：{0}；English：Could not parse realm server entry: {0}）。

> 列出领域时还会打印硬编码行：空行、`ID: <id>`、`name: <name>`、`motd: <motd>`、`owner: <owner>`（见 3.8）。

### 3.6 配置 / 日志消息（`config-*`、`log-*`、`could-not-detect-theme`）

* `config-loaded-from`（中文：配置已加载: {0}；English：Configuration was loaded: {0}）：启动时打印配置文件绝对路径。
* `config-failed-parsing`（中文：（星号框）无法分析您的配置，因为它与json格式不匹配:\n{0}；English：（星号框）Your configuration could not be parsed because it does not match the json-format:\n{0}）：配置解析失败（多行，`{0}`=异常）。
* `config-unknown-announce-type`（中文：通知类型 {0} 无效.；English：The AnnounceType {0} is invalid.）：配置里枚举值非法（会回退默认并打印）。
* `config-missing-fields-detected`（中文：您的配置不完整。已添加缺少的字段！；English：Your configuration was incomplete. The missing fields were added!）：自动补全配置字段。
* `config-converting`（中文：正在转换配置文件的旧版本...；English：An older version of your configuration file is being converted...）：旧配置字段迁移时（如 `start-text.text` 为旧格式）。
* `config-converted-properties-to-json`（中文：旧的.properties配置文件已迁移到新的.json配置文件！；English：Your old .properties configuration file has been migrated to the new .json configuration file!）：老配置迁移。
* `log-inventory-opened`（中文：New inventory opened: {0}；English：New inventory opened: {0}）：中文值仍为英文；打开容器时（`{0}`=标题）。
* `log-failed-creating-log`（中文：无法创建日志；English：Log could not be created.）：日志文件创建失败，机器人停止。
* `log-failed-creating-folder`（中文：创建日志文件夹失败。正在退出；English：Failed creating logs folder. Exiting.）：日志目录创建失败，机器人停止。
* `could-not-detect-theme`（中文：无法检测系统主题 {0}，也许正在使用旧版本 Java？{1}；English：Could not detect system theme {0}. Maybe using legacy java version? {1}）：GUI 主题检测（本仓库无 GUI，一般不出现）。

### 3.7 其它：升级 / Code of Conduct / Discord / 常量

* `announce-level-up`（中文：已达到等级: {0}；English：Achieved level: {0}）：升级时控制台输出（`{0}`=等级）。
* `received-code-of-conduct`（中文：（仅英文）Received Code of Conduct:\n{0}\nUse /acknowledgecoc to acknowledge the Code of Conduct；English：Received Code of Conduct:\n{0}\nUse /acknowledgecoc to acknowledge the Code of Conduct）：[仅英文定义/中文回退英文]，多行。
* `acknowledged-code-of-conduct`（中文：（仅英文）Acknowledged Code of Conduct；English：Acknowledged Code of Conduct）：[仅英文]；自动接受或已记住时。
* `rejected-code-of-conduct`（中文：（仅英文）Rejected Code of Conduct. Disconnecting；English：Rejected Code of Conduct. Disconnecting）：[仅英文]；当前代码未引用。
* `discord-webhook-damage`（中文：我受到了伤害！我现在的生命值是: {0}；English：I got damage! My current life is: {0}）：本仓库无 Discord 模块，未被引用。
* `discord-webhook-respawn`（中文：我重生了!；English：I respawned!）：本仓库无 Discord 模块，未被引用。
* `command-ackowledgecoc-no-coc`（中文：（仅英文）There is no Code of Conduct to acknowledge；English：There is no Code of Conduct to acknowledge）：[仅英文]；`/acknowledgecoc` 无 CoC 时。

> - Code of Conduct：`LoginModule` 在配置 `auto.auto-accept-code-of-conduct=false` 且未记住时，会额外用 `getLog().severe(...)` 打印一条硬编码英文：`A Code of Conduct must be accepted. Enable "auto.auto-accept-code-of-conduct" in your config.json to accept it automatically.`，然后停止机器人。
> - Discord 相关：`config-announces-discord*`、`discord-webhook-*` 等键仍存在于语言文件，但当前源码没有 Discord/Webhook 模块（仅 `io/config/PropertyProcessor` 里有一条老配置键映射 `discord.web-hook`）。因此运行时不会出现 Discord 消息。

### 3.8 非语言文件（硬编码 / 动态拼接）的输出消息

这些消息不来自语言文件，不受 `misc.language` 影响，多为固定英文（调试用），对接时需单独匹配。

#### 3.8.1 游戏内聊天中的机器人发言

* 钓到物品公告（聊天）（内容模板：`FishingBot v<版本> - 捕获到 "<物品名>"`（zh）/ `FishingBot v<版本> - Caught "<物品名>"`（en））：= `PREFIX + t("module-fishing-caught-item", 物品名)`。
* 附魔行（聊天，紧跟在钓到物品后 200ms）（内容模板：`-> <附魔显示名>[ <罗马数字>]`，例：`-> 海之眷顾 III` / `-> Lure III`）：无前缀；等级=1 时不显示罗马数字。
* 升级公告（聊天）（内容模板：配置 `announces.announce-lvl-up-text`（默认 `I am level %lvl% now`），其中 `%lvl%` 替换为等级）：仅当 `announce-lvl-up=true` 且文本 ≠ `false`。
* 指令回复（聊天）（内容模板：见 3.1 各条（发到公共聊天））：触发者=聊天里喊话的玩家。

> 所有通过 `Player.sendMessage` 发出的文本都会先把 `%prefix%` 替换成 `FishingBot.PREFIX`；服务器协议 ≥ 1.19 时，以 `/` 开头的行会作为命令发送、否则作为聊天；1.8 时按 ~100 字符自动拆行。

#### 3.8.2 控制台日志中的硬编码消息

* `Using FishingBot v2.13.15`（级别：INFO；触发：机器人启动）。
* `Using language: <LANG> @ /lang/fb_<code>.properties`（级别：INFO；触发：语言初始化成功（`<LANG>` 为枚举名，如 `CHINESE_SIMPLIFIED`））。
* `Failed loading language <LANG>: /lang/fb_<code>.properties`（级别：SEVERE；触发：语言文件加载失败）。
* `Falling back to default language ENGLISH`（级别：SEVERE；触发：同上，回退英文）。
* `Could not find language <值>. Falling back to default language ENGLISH`（级别：SEVERE；触发：配置里 `misc.language` 非法）。
* `Could not load registry file for pvn <pvn>`（级别：SEVERE；触发：注册表文件缺失）。
* `Could not find key for value: <值>`（级别：SEVERE；触发：注册表查值失败）。
* `Could not map packet id <id>`（级别：WARNING；触发：数据包映射缺失）。
* `Tried to register null packet for: <x>` / `Tried to register packet twice: <x>`（级别：SEVERE；触发：注册冲突（开发期））。
* `Unhandled data component <x>` / `Tried to add null to data component registry`（级别：SEVERE；触发：物品组件（新版本）未处理）。
* `Could not instantiate <类>`（级别：WARNING；触发：反射实例化包失败）。
* `Error while trying to send: <异常>`（级别：SEVERE；触发：发包失败）。
* `Could not create new instance of <类>`（级别：WARNING；触发：创建包实例失败）。
* `Packet <类> has <n> byte(s) left` / `Handling packet <类> took <n>ms`（级别：WARNING / INFO；触发：仅 `logs.log-packets=true` 时）。
* `[<STATE>]（级别：C；触发：<<<  S : <包名>` 或 `0x<id> (<mojmapped>)`；：INFO；：仅 `logs.log-packets=true` 时逐包打印）。
* `Start reading <X> ...` / `End of reading <X> ...` / `Read slot` / `» + ...` / `» - ...` / `Entity data for <id> ...` 等（级别：INFO；触发：仅 `logs.log-item-data` / `logs.log-entity-data` = true 时（调试））。
* `Error while waiting for access token: <异常>`（级别：WARNING；触发：微软 access token 轮询失败）。
* `A Code of Conduct must be accepted. Enable "auto.auto-accept-code-of-conduct" in your config.json to accept it automatically.`（级别：SEVERE；触发：需人工接受 CoC（见 3.7））。
* `Received unregistered consume_effect_type: <x>`（级别：INFO；触发：未注册消耗效果（调试））。
* `Invalid component: <x>`（级别：SEVERE；触发：物品组件解析异常）。

#### 3.8.3 Realms 列表（`printRealms`）

```
<空行>
ID: <id>
name: <name>
motd: <motd>
owner: <owner>
```

> 在 `realm-id==0` 且在线模式时，启动会打印可加入的领域列表（每条 5 行）。

#### 3.8.4 实测样例（本仓库 jar，离线模式，连接一个不存在的服务器）

下方为实际运行输出（英文；语言由 `misc.language` 决定），可直接作为对接时的“样板”：

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

> 重要：`e.printStackTrace()` 的输出会被 `CustomPrintStream` 转发到日志，且级别是 INFO，表现为若干行 `[INFO   ] at ...` / `[INFO   ] java.net.ConnectException: ...`。对接时请忽略这些以 `at `（或包含 `Exception`）开头的行，不要与业务消息混淆。

### 3.9 中文语言文件缺失 / 未翻译清单（重要）

中文文件缺失的 19 个 key（中文模式下会输出英文）：

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

中文文件里定义了、但值仍是英文的 key（不会回退，直接输出英文）：

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

> 其中 `config-announces-discord`=`Discord`、`config-announces-discord-web-hook`=`Webhook` 属品牌词，中英相同属正常。

> 若你的其它项目按“中文/英文固定文案”匹配，请特别注意以上两类：中文模式并不保证全部是中文。

---

## 4. 文案格式规则与占位符

1. 占位符替换顺序（`I18n.t` + `Player.sendMessage`）：
   * 先按 `MessageFormat.format(value, args)` 替换 `{0}`、`{1}`…；
   * 再把 `%prefix%` 替换为 `FishingBot.PREFIX`（仅在经过 `Player.sendMessage` 时；控制台日志路径不会替换 `%prefix%`，但走日志的文案本身不含 `%prefix%`）。
2. MessageFormat 注意：单引号 `'` 是转义字符，properties 中写作 `''`（如 `bot''s`）。若你构造包含 `{`、`}` 的匹配正则需自行转义。
3. 未找到 key → 输出字符串 `N/A`（英文、中文文件都缺失时）。
4. 换行 `\n`：
   * 控制台：`I18n.info/warning/severe` 会把整条文案按行拆开逐行打印，每行都有时间戳+级别。
   * 聊天：`Player.sendMessage` 按 `\n` 拆成多条聊天包依次发送。
5. 数值/字符串：很多占位符传入的是 `String.valueOf(...)`（如重连秒数、玩家数），匹配时按“任意整数/文本”处理。
6. 语言选择：`misc.language` 取值为枚举名（`CHINESE_SIMPLIFIED` 或 `ENGLISH`），非法值会回退英文并打印 `Could not find language ...`。

### 4.1 主要占位符一览

* `bot-automatic-reconnect`（占位符含义：`{0}` = 重连等待秒数（`auto.auto-reconnect-time`，默认 5））。
* `server-pinging`（占位符含义：`{0}`=主机, `{1}`=端口, `{2}`=默认协议版本名）。
* `network-received-pong`（占位符含义：`{0}`=MOTD, `{1}`=版本名, `{2}`=协议号, `{3}`=在线人数）。
* `network-changed-address` / `network-changed-port`（占位符含义：`{0}` = 新主机 / 新端口）。
* `module-fishing-caught-item`（占位符含义：`{0}` = 物品显示名）。
* `module-login-successful`（占位符含义：`{0}`=玩家名, `{1}`=UUID）。
* `module-login-failed`（占位符含义：`{0}` = 失败原因）。
* `module-client-disconnected`（占位符含义：`{0}` = 断开原因）。
* `announce-level-up`（占位符含义：`{0}` = 新等级）。
* `command-level`（占位符含义：`{0}` = 等级）。
* `command-look-executed`（占位符含义：`{0}`=yaw, `{1}`=pitch）。
* `command-droprod-item-count`（占位符含义：`{0}` = 丢弃鱼竿数量）。
* `command-wait-waited`（占位符含义：`{0}` = 等待秒数）。
* `command-empty-unknown-type` / `command-droprod-unknown-type`（占位符含义：`{0}` = 非法输入值）。
* `module-enabled` / `module-disabled`（占位符含义：`{0}` = 模块类名）。
* `config-loaded-from`（占位符含义：`{0}` = 配置文件绝对路径）。
* `auth-username`（占位符含义：`{0}` = 用户名）。
* `realms-connecting-to`（占位符含义：`{0}` = 领域接口返回 JSON）。
* `ui-tabs-loot`（占位符含义：`{0}` = 已钓获总数）。

---

## 5. 注意事项（对接必读）

1. 指令权限：没有鉴权。任何在聊天里发出 `机器人名, <指令>` 的玩家都能操控机器人（包括 `/bye` 关机、`/empty` 丢物品、`/droprod` 丢鱼竿）。请在受控服务器使用，或改代码加白名单。
2. 玩家触发的硬条件：聊天文本必须同时包含机器人用户名与英文逗号 `,`；且指令部分不能含逗号（会被 `split(",")` 吞掉）。解析失败无任何提示。
3. 控制台触发必须带 `/`：`/help` 是执行指令；`help`（无斜杠）会被机器人当作聊天内容发送到服务器。
4. 指令名大小写：Brigadier 字面量区分大小写，建议统一小写；别名同样小写。
5. “重连提示”来源：`bot-automatic-reconnect` 是控制台日志（不是游戏聊天），英文 `FishingBot will reconnect in {0} seconds...`、中文 `钓鱼机器人将在{0}秒内重新连接...`；`{0}` 取 `auto.auto-reconnect-time`（默认 `5`）。仅当 `auto.auto-reconnect=true` 且未 `preventReconnect` 时出现。
6. 聊天前缀会随版本变化：`PREFIX = "<name> v<version> - "` 由 `pom.xml` 决定（当前 `FishingBot v2.13.15 - `）。若你按前缀匹配钓获公告，升级版本会改变该前缀。
7. 语言不保证全中文：见 3.9，中文模式仍有 19 个 key 回退英文、约 26 个 key 值就是英文。
8. 中英文都可能出现的消息：若依赖某些关键消息（如重连、登录、钓获），请按关键字语义/正则同时支持中英，而不是只匹配中文。
9. 调试噪声：把 `logs.log-packets` / `logs.log-item-data` / `logs.log-entity-data` 设为 `true` 会产生大量 `Start reading...`、`[PLAY] |C| <<< S : ...`、`Read slot` 等英文日志，干扰消息匹配，生产环境保持 `false`。
10. GUI 相关键无实际输出：`ui-*`、`dialog-*`、`config-*` 标签为 GUI/配置界面用，而本仓库没有 GUI 实现，这些不会出现在控制台或聊天。
11. Discord 未接入：`discord-webhook-*`、`config-announces-discord*` 当前无代码引用，不会输出。
12. 日志文件：写入 `<logsdir>/log%g.log`（滚动），保留数量 `logs.log-count`（默认 15）。控制台与日志内容一致。
13. `-onlyCreateConfig`：只生成配置文件，不连接、不钓鱼（可用于初始化/校验配置）。
14. 配置热改无效：修改 `config.json` 需重启；语言同理（重启生效）。

### 5.1 消息匹配建议

* 优先匹配关键短语而非整句，例如：
  * 断线重连：`reconnect in (\d+) seconds` / `将在(\d+)秒内重新连接`，或直接匹配 `自动重新连接` 关键词（配置标签，不推荐）。
  * 钓获：`Caught "(.+)"` / `捕获到 "(.+)"`。
  * 升级：`Achieved level: (\d+)` / `已达到等级: (\d+)`。
  * 登录成功：`Login successful!` / `登录成功!`（后跟 `Name:`/`UUID:`）。
* 若你能控制启动参数/环境，可统一把 `misc.language` 固定为 `CHINESE_SIMPLIFIED` 或 `ENGLISH`，简化匹配。

---

## 附录 A：配置项默认值速查（`config.json` / `SettingsConfig`）

> 下表的“配置路径”是代码里的逻辑点号路径（`@Property.key`）。实际 `config.json` 是嵌套结构，例如 `server.ip` → `{"server":{"ip":...}}`、`auto.auto-eject.rules` → `{"auto":{"auto-eject":{"rules":[...]}}}`。

* `server.ip`（默认值：`127.0.0.1`）：服务器地址（含端口时会自动拆分）。
* `server.port`（默认值：`25565`）：服务器端口。
* `server.realm-id`（默认值：`-1`）：`-1`=不用领域；`0`=启动时列出领域；其他=指定领域。
* `server.realm-accept-tos`（默认值：`false`）：是否自动接受领域 ToS。
* `server.online-mode`（默认值：`true`）：在线（正版）模式。
* `server.default-protocol`（默认值：`AUTOMATIC`）：默认 MC 版本/协议。
* `server.spoof-forge`（默认值：`false`）：伪造 Forge 握手。
* `auto.auto-reconnect`（默认值：`true`）：断线自动重连。
* `auto.auto-reconnect-time`（默认值：`5`）：重连等待秒数（即重连消息里的 `{0}`）。
* `auto.auto-disconnect`（默认值：`false`）：人数达阈值自动断开。
* `auto.auto-disconnect-players-threshold`（默认值：`5`）：人数阈值。
* `auto.auto-sneak`（默认值：`false`）：自动潜行。
* `auto.auto-accept-code-of-conduct`（默认值：`false`）：自动接受服务器行为准则。
* `auto.auto-command-on-respawn.enabled`（默认值：`false`）：重生/进服自动执行命令。
* `auto.auto-command-on-respawn.delay`（默认值：`1000`）：延迟（ms）。
* `auto.auto-command-on-respawn.commands`（默认值：`["%prefix%I respawned", "/home fishing"]`）：命令/聊天。
* `auto.auto-command-before-death.enabled`（默认值：`false`）：濒死自动执行命令。
* `auto.auto-command-before-death.commands`（默认值：`["%prefix%I am about to die", "/home"]`）。
* `auto.auto-command-before-death.min-health-before-death`（默认值：`6.0`）：触发血量（2=1 颗心）。
* `auto.auto-quit-before-death.enabled`（默认值：`false`）：濒死自动退出。
* `auto.auto-quit-before-death.min-health-before-quit`（默认值：`6.0`）：触发血量。
* `auto.auto-eject.enabled`（默认值：`false`）：自动弹出战利品。
* `auto.auto-eject.rules`（默认值：fish/treasure/junk 三条 DROP 规则）：弹出规则（方向+白名单+类型）。
* `auto.timer.enabled`（默认值：`false`）：定时任务开关。
* `auto.timer.timers`（默认值：1 条示例（每 5 分钟））：定时执行命令。
* `account.mail`（默认值：`FishingBot`）：离线模式用户名。
* `logs.log-count`（默认值：`15`）：日志文件保留数量。
* `logs.log-packets`（默认值：`false`）：记录数据包（调试）。
* `logs.log-entity-data`（默认值：`false`）：记录实体数据（调试）。
* `logs.log-item-data`（默认值：`false`）：记录物品数据（调试）。
* `announces.announce-type-chat`（默认值：`ONLY_ENCHANTED`）：聊天公告类型（`ALL`/`ALL_BUT_FISH`/`ONLY_ENCHANTED`/`ONLY_BOOKS`/`NONE`）。
* `announces.announce-type-console`（默认值：`ALL`）：控制台公告类型。
* `announces.announce-lvl-up`（默认值：`true`）：是否在聊天宣布升级。
* `announces.announce-lvl-up-text`（默认值：`I am level %lvl% now`）：升级聊天文本（`%lvl%` 占位）。
* `start-text.enabled`（默认值：`true`）：进服自动执行启动动作。
* `start-text.text`（默认值：`["%prefix%Starting fishing", "/trigger Bot"]`）：启动文本/命令。
* `misc.stucking-fix-enabled`（默认值：`true`）：卡顿修复。
* `misc.prevent-rod-breaking`（默认值：`true`）：防止鱼竿耗尽。
* `misc.disable-rod-checking`（默认值：`false`）：禁用鱼竿可用性检查。
* `misc.language`（默认值：系统默认语言（回退英文））：语言。
* `misc.look-speed`（默认值：`16`）：视角转动速度。

## 附录 B：语言文件中已定义但当前代码未引用的键

`network-not-supported-server-version`、`rejected-code-of-conduct`、`discord-webhook-damage`、`discord-webhook-respawn`、`command-help-other-player-private`（仅供私聊分支，未启用），以及全部 `ui-*`、`dialog-*`、`config-*`（除少数被引用于日志的除外）等 GUI/配置界面文案。

> 这些键不会在运行日志/聊天中出现，接入时无需匹配；但你若修改语言文件也无副作用。

---

## 附录 C：主要源码定位（便于二次开发）

* 语言加载/取值（文件：`i18n/I18n.java`、`i18n/Language.java`、`resources/lang/fb_*.properties`）。
* 前缀/标题（文件：`FishingBot.java`（`PREFIX`/`TITLE`）、`resources/fishingbot.properties`、`pom.xml`）。
* 控制台指令读取（文件：`modules/ChatProxyModule.java`）。
* 玩家聊天指令解析（文件：`modules/command/ChatCommandModule.java`）。
* 指令注册/分发（文件：`modules/command/CommandRegistry.java`、`modules/command/commands/*`）。
* 指令执行者/回复去向（文件：`modules/command/executor/*.java`、`bot/Player.java#sendMessage`）。
* 断线重连（文件：`Bot.java`（`bot-automatic-reconnect`））。
* 钓鱼/钓获公告（文件：`modules/fishing/FishingModule.java`）。
* 升级/血量/重生（文件：`bot/Player.java`）。
* 网络/连接消息（文件：`network/ping/ServerPinger.java`、`network/protocol/NetworkHandler.java`）。
* Realms（文件：`network/mojangapi/MojangAPI.java`、`Bot.java`）。
* 日志格式（文件：`io/logging/LogFormatter.java`）。
* 配置默认值（文件：`io/config/SettingsConfig.java`）。
