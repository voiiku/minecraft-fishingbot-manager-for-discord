# 常见故障提示与排查
[English](troubleshooting-en_us.md) | **简体中文**

一些常见的故障与提示。

## 1. 日志位置

本程序维护两类日志
* 程序日志：`<部署目录>/logs/年-月-日-时-分-秒.log`。每次启动时创建一份新日志，管理员手动清理。
* 钓鱼机器人日志（过滤后）：`<部署目录>/bots/<用户 ID>/<机器人名>/logs/年-月-日-时-分-秒.log`，`/log`读的就是它。

FishingBot.jar 自己的原始日志默认被指向 `/dev/null` 丢弃，所以默认情况下，钓鱼机器人的日志只保留过滤后的内容，你可以修改配置以改变此行为。

## 2. Discord 里输入 `/` 后看不到本项目的指令

1. 邀请链接缺少 `applications.commands` 这个 scope。用程序启动日志里打印的邀请链接重新邀请一次。
2. 指令是全局注册的，最长要等 1 小时才能传播完。
3. 程序不在运行。查看 `systemctl status minecraft-fishingbot-manager-for-discord` 或日志。

## 3. 指令交互失败

1. 机器人异常崩溃。看 `<部署目录>/logs/` 下的最新程序日志；钓鱼机器人的日志用 `/log <机器人名>` 看。
2. 机器人在该频道没有 Send Messages 权限。
3. 极少数情况是网络波动导致 3 秒内未应答指令，超时失败。

## 4. 程序日志文件中没有框架（serenity、poise）的日志

默认情况下：框架的日志只记录 `WARN`/`ERROR` 级别，因为它们按心跳节奏打印大量重复的 `INFO`，把配置里
`[log] debug` 改成 `true` 后重启本程序生效（级别只看这一行配置，没有环境变量可以覆盖它，也无法临时放宽）。

## 5. 程序日志显示 `Discord 初始化失败: Missing Access`，进程连上 Discord 后立即退出

机器人可能没有被邀请进任何服务器，或邀请时缺少 `applications.commands` 权限，所以在注册斜杠指令时 Discord
返回 `50001 Missing Access`。用程序启动日志里打印的邀请链接重新邀请一次机器人。

## 6. `/start` 成功发送，但机器人并不存在

用 `/log <机器人名>` 查看机器人最新日志。常见原因：

1. `refreshToken` 失效。
2. `runtime.java_opts` 与 jar 不匹配、内存不足被杀。

## 7. 日志显示 `NeverReady`（启动后一直没握手成功）

* 网络测试：`curl -sS -o /dev/null -w '%{http_code}\n' https://discord.com/api/v10/gateway`，状态为 `200`
  说明网络正常。
* 再检查配置中的 `[discord] token` 无误。
* 使用代理时请注意：WebSocket 不会被 HTTP 代理，建议使用 TUN 模式。

## 8. 日志显示 `Unreachable`

可能是断网、代理节点错误、Token 被 Reset 等原因。

如果你的网络延迟较高或经常抖动，把配置里的 `timing.watchdog_window_ms` 调大即可：探活间隔（它的 1/3）与
首次握手超时（它的 2 倍）跟着它一起变。判定细节见 [watchdog.md 第 5 节](../mods/watchdog/watchdog-zh_cn.md#5-常见排查)。

## 9. 钓鱼机器人的日志内容很少/有些信息没被记录

这是预期结果：过滤规则在生效。被过滤的行默认会被丢弃，你可以更改配置文件以修改行为。
你可以尝试修改配置文件的 `keywords.filter_regex` 中对应的表达式后重启进程。

## 10. 钓鱼机器人显示状态与实际情况不匹配

日志关键词与游戏服务器的实际消息不匹配。查看配置文件中的 `[keywords]`。
* 如果是"重连"或 `/pause` 认不出来，先看 `[i18n] language` 是不是你的 JAR 实际输出的语言。
* 重连提示，`/pause` 与 `/unpause` 的回显是 jar 的固定文案，随语言设置而变化，无法指定配置。
机制见 [i18n.md](../mods/i18n/i18n-zh_cn.md)。

## 11. `/startall` 有一些机器人启动失败

* 微软按出口 IP 限制登录频率。`/start`、`/startall`、`/restart` 与 `/restartall` 共用全局启动队列：
  随仓库发布的示例配置是两次登录至少间隔 20 秒（`[startup_queue] bot_login_interval_ms`），可自行调整
  `max_batch_size` 与 `bot_login_interval_ms`；机制、真实登录间隔的算法与推荐取值见第 12 节。
* 批量启动的瞬时内存峰值可能超过服务器可用内存，被杀掉的进程不会出现在 `screen -list` 里，所以结果里会
  显示"启动失败(常见原因是微软拒绝频繁登录或内存不足)"。缓解办法：把配置中的 `runtime.java_opts` 内存
  配额调小，或把 `timing.check_grace_ms` 与 `[startup_queue] bot_login_interval_ms` 调大，或稍后重试（JVM
  内存占用会随时间下降）。

## 12. 每次启动都固定失败同一台机器人（微软登录限流）

现象：`/startall` 的报告把某台机器人算成"启动命令已发送，会话已出现"，但随后的 `/show status` 里没有它；
与账号无关，**固定是登录次序靠后的那一台**（通常就是最后一台），而同一个账号稍后单独 `/start` 又能起来。

原因：XSTS 换 Minecraft 令牌那一步（`POST https://api.minecraftservices.com/authentication/login_with_xbox`）
按**出口 IP** 限流，超限返回 `HTTP 429`。FishingBot 不检查状态码、直接取响应里的 `access_token`，于是抛异常，
进程在启动后 2~5 秒自己退出。它会不会被算成失败，取决于复核窗口：

* 复核 = 两次会话快照（拉起后立即一次、等满 `timing.check_grace_ms` 一次），**整批拉起完**就进入复核。
  台数越少窗口越贴近启动瞬间：`max_batch_size = 1`（每台单独一批）与收尾的不足一批都只有约
  一个 `timing.check_grace_ms`，所以"2~5 秒后被 429 拒"常常来不及看见，被报成成功。

真实登录间隔（限流看的就是它）由 `[startup_queue]` 的两个间隔键决定：

* 批内相邻两台：`bot_login_interval_ms`；
* 上一批最后一台 → 下一批第一台：`batch_login_interval_ms` 减去本批从出队到最后一台拉起已过去的时间（复核在命令侧并行进行，不占这段等待）。

同一出口 IP 上的经验值：**≥20 秒一次（约 3 次/分）稳定**；12 秒一次（5 次/分）会掉最后一次；几秒内连发会直接
触发惩罚窗口，之后几十秒内的登录全部被拒。同一出口 IP 上其它登录（官方启动器、另一个实例、手动登录）也占额度。

处置：

1. 把 `[startup_queue] bot_login_interval_ms` 设到 18~20 秒（示例配置给的是 20000）。`max_batch_size` 取多少不影响
   节奏，它只改变复核窗口的长度与 JVM 启动时的内存峰值。
2. 失败的那台等 3~5 分钟再补（`/start <机器人名>` 或 `/restart <机器人名>`），不要立刻重试。
3. 要真正提速只有两条路：用多个出口 IP；或让 JAR 复用 Minecraft access token（有效期 24 小时），
   不要每次启动都重新登录。
4. 与"内存不足"（第 11 节的第二种原因）的区别：限流的进程在启动后 2~5 秒自己退出，日志里有认证相关报错；
   被 OOM 杀掉的是 0.3 秒内就没了，日志为空或写着 `Killed`，`dmesg` 里能看到。
5. 配置项细节见 [config-zh_cn.md](../mods/config/config-zh_cn.md)。

## 13. 我改了配置，正在跑的机器人会受影响吗

不会。配置只在启动时读一次并加载进内存，运行中改动（甚至删除）配置文件都不影响已跑的进程；修改配置要重启本程序才生效。

## 14. 怎么确认僵尸机器人

```bash
screen -ls | grep fb_                 # 根据你配置文件中定义的前缀
pgrep -af "java .*-refreshToken"      # 根据你配置文件中定义的 Token 文件名
```

如果有僵尸机器人就使用 `/stopall`（停止所有钓鱼机器人），或 `pkill` 掉那些 JVM，或重启本程序。

## 15. 升级版本后，启动时配置字段报错

本程序会在启动时校验配置合法性，并拒绝任何未知字段。
你可以尝试删掉旧配置文件，然后运行本程序，它会重新生成一份最新的配置模板。

本次精简动过的字段（示例配置模板已同步）：

* `[startup_queue] batch_cooldown_ms` 改名为 `bot_login_interval_ms`：含义从"批间冷却"变成"两次登录之间的
  最小间隔"（批内相邻两台按它错峰），原来的 `[timing] stagger_ms` 被它取代。
* `[timing]` 删除了 `probe_interval_ms` 与 `first_handshake_timeout_ms`：它们由 `watchdog_window_ms`
  派生（探活间隔是窗口的 1/3，首次握手超时是窗口的 2 倍）。

## 16. 同一条指令被回复多次

某些指令无法立即确认，所以会稍后再追加回复一些消息。
如果一模一样的回复重复出现，说明你在服务器上同时运行了多个机器人，尝试全部关停，并再次启动一个机器人。
* 一台机器上同时运行多个机器人是合法的，你应该为他们配置不同的工作目录，并确保配置不存在冲突（尤其是会话名前缀），此时他们可以各自作为独立的服务，服务不同游戏服务器的自动钓鱼群聊。

## 17. 相关文档

* [tutorial/tutorial-zh_cn.md](../tutorial/tutorial-zh_cn.md)：从零开始的部署教程（本文件的多数结论在那边有上下文）；
* [config-zh_cn.md 第 3.2 节](../mods/config/config-zh_cn.md#32-游戏侧配置confjson是生成物)：游戏侧 `conf.json` 是生成物这条规则；
* [i18n.md](../mods/i18n/i18n-zh_cn.md)：`[i18n] language` 怎么同时决定我们说哪种语言、按哪种语言认 jar 的重连提示；
* [watchdog.md 第 5 节](../mods/watchdog/watchdog-zh_cn.md#5-常见排查)：`NeverReady`、`Unreachable` 的判定与排查。
