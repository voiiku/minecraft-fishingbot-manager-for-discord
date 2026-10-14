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
`[log] debug` 改成 `true` 后重启本程序生效。
或临时用 `RUST_LOG=debug ./minecraft-fishingbot-manager-for-discord` 启动。
`RUST_LOG` 优先于配置设置。

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

如果你的网络延迟较高或经常抖动，考虑把配置中的 `timing.watchdog_window_ms` 调大，必要时连
`timing.first_handshake_timeout_ms` 一起调大。判定细节见 [watchdog.md 第 5 节](../mods/watchdog/watchdog-zh_cn.md#5-常见排查)。

## 9. 钓鱼机器人的日志内容很少/有些信息没被记录

这是预期结果：过滤规则在生效。被过滤的行默认会被丢弃，你可以更改配置文件以修改行为。
你可以尝试修改配置文件的 `keywords.filter_regex` 中对应的表达式后重启进程。

## 10. 钓鱼机器人显示状态与实际情况不匹配

日志关键词与游戏服务器的实际消息不匹配。查看配置文件中的 `[keywords]`。
* 如果是"重连"或 `/pause` 认不出来，先看 `[i18n] language` 是不是你的 JAR 实际输出的语言。
* 重连提示，`/pause` 与 `/unpause` 的回显是 jar 的固定文案，随语言设置而变化，无法指定配置。
机制见 [i18n.md](../mods/i18n/i18n-zh_cn.md)。

## 11. `/startall` 有一些机器人启动失败

* 微软可能限制短时间内的重复登录请求。`/start`、`/startall`、`/restart` 与 `/restartall` 共用全局启动队列：
  随仓库发布的示例配置是每批最多启动 3 个、批间等待 12 秒；可在 `[startup_queue]` 中调整 `max_batch_size` 与
  `batch_cooldown_ms`。这能降低请求密度，但不保证微软一定接受每次登录。
* 批量启动的瞬时内存峰值可能超过服务器可用内存，被杀掉的进程不会出现在 `screen -list` 里，所以结果里会
  显示"启动指令已发送，但进程不存在（通常是内存不足）"。缓解办法：把配置中的 `runtime.java_opts` 内存
  配额调小，或把 `timing.stagger_ms` 与 `timing.settle_per_bot_ms` 调大，或稍后重试（JVM 内存占用会
  随时间下降）。

## 12. 我改了配置，正在跑的机器人会受影响吗

不会。配置只在启动时读一次并加载进内存，运行中改动（甚至删除）配置文件都不影响已跑的进程；修改配置要重启本程序才生效。

## 13. 怎么确认僵尸机器人

```bash
screen -ls | grep fb_                 # 根据你配置文件中定义的前缀
pgrep -af "java .*-refreshToken"      # 根据你配置文件中定义的 Token 文件名
```

如果有僵尸机器人就使用 `/stopall`（停止所有钓鱼机器人），或 `pkill` 掉那些 JVM，或重启本程序。

## 14. 升级版本后，启动时配置字段报错

本程序会在启动时校验配置合法性，并拒绝任何未知字段。
你可以尝试删掉旧配置文件，然后运行本程序，它会重新生成一份最新的配置模板。

## 15. 同一条指令被回复多次

某些指令无法立即确认，所以会稍后再追加回复一些消息。
如果一模一样的回复重复出现，说明你在服务器上同时运行了多个机器人，尝试全部关停，并再次启动一个机器人。
* 一台机器上同时运行多个机器人是合法的，你应该为他们配置不同的工作目录，并确保配置不存在冲突（尤其是会话名前缀），此时他们可以各自作为独立的服务，服务不同游戏服务器的自动钓鱼群聊。

## 16. 相关文档

* [tutorial/tutorial-zh_cn.md](../tutorial/tutorial-zh_cn.md)：从零开始的部署教程（本文件的多数结论在那边有上下文）；
* [config-zh_cn.md 第 3.2 节](../mods/config/config-zh_cn.md#32-游戏侧配置confjson是生成物)：游戏侧 `conf.json` 是生成物这条规则；
* [i18n.md](../mods/i18n/i18n-zh_cn.md)：`[i18n] language` 怎么同时决定我们说哪种语言、按哪种语言认 jar 的重连提示；
* [watchdog.md 第 5 节](../mods/watchdog/watchdog-zh_cn.md#5-常见排查)：`NeverReady`、`Unreachable` 的判定与排查。
