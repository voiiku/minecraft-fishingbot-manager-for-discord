# 常见故障与排查
[English](troubleshooting-en_us.md) | **简体中文**

以下按现象列出常见故障及处理方式。

## 1. 日志位置

本程序维护两类日志：

* 程序日志：`<部署目录>/logs/年-月-日-时-分-秒.log`。每次启动创建一份新日志，由管理员手动清理。
* 钓鱼机器人日志（过滤后）：`<部署目录>/bots/<用户 ID>/<机器人名>/logs/年-月-日-时-分-秒.log`，`/log` 读取的就是它。

FishingBot.jar 自身的原始日志默认写入 `/dev/null` 并丢弃，因此磁盘上只保留过滤后的机器人日志。该行为可通过 `[raw_game_logs]` 修改。

## 2. Discord 中输入 `/` 后看不到本项目的指令

1. 邀请链接缺少 `applications.commands` scope。使用程序启动日志中打印的邀请链接重新邀请。
2. 斜杠指令全局注册，最长需要 1 小时传播。
3. 程序未运行。检查 `systemctl status minecraft-fishingbot-manager-for-discord` 或程序日志。

## 3. 指令执行失败或无响应

1. 机器人异常退出。查看 `<部署目录>/logs/` 下最新的程序日志；机器人日志用 `/log <机器人名>` 查看。
2. 机器人在该频道缺少 Send Messages 权限。
3. 网络波动导致 Discord 在 3 秒内未收到应答，交互超时（少见）。

### `/ping` 拒绝探测本地或私网地址

显式指定主机时，程序使用运行主机的系统解析结果，并要求所有解析出的地址都是公网地址；本地、私网及保留网段会被拒绝。这样可以避免通过 `/ping` 探测内部服务。配置中的默认主机是管理员提供的可信目标，不受此检查限制。

若公网域名仍被拒绝，检查运行本程序的机器（或容器）上的解析结果，例如将以下域名替换为实际目标：

```bash
getent ahosts play.example.com
getent hosts play.example.com
```

代理的 Fake-IP DNS 可能返回 `198.18.0.0/15` 内的虚拟地址，而不是游戏服务器的真实地址；该网段被程序作为保留网段拒绝。对 Clash/Mihomo 等代理，可在其 DNS 配置的 `fake-ip-filter` 中排除此域名，让它返回真实地址。例如，在 `fake-ip-filter-mode` 为 `blacklist` 时，将下列项**追加到已有列表**，不要覆盖其他规则：

```yaml
fake-ip-filter:
  - play.example.com
```

这是代理的 YAML 配置，不是本程序 TOML 的 `[ping]` 选项。按代理工具的方式保存并重新加载配置、清理或等待 DNS 缓存过期后，重新检查解析结果并重试 `/ping`。如果代理的 TUN 模式接管 DNS，仅更换系统 DNS 服务器可能无效。真实结果若仍包含本地、私网或保留地址，拒绝是预期的保护行为；不要仅为绕过检查而信任未知目标。

## 4. 程序日志中没有框架（serenity、poise）的日志

默认情况下，框架日志只记录 `WARN` 与 `ERROR`：serenity、poise 会按心跳持续输出大量重复的 `INFO`。将配置中的 `[log] debug` 设为 `true` 并重启程序即可记录；级别只由该配置项决定，没有环境变量或临时开关可以覆盖。

## 5. 程序日志显示 `Discord 初始化失败: Missing Access`，进程连上 Discord 后立即退出

机器人可能未被邀请加入任何服务器，或邀请时缺少 `applications.commands` scope，导致注册斜杠指令时 Discord 返回 `50001 Missing Access`。使用启动日志中打印的邀请链接重新邀请。

## 6. `/start` 报告成功，但机器人没有运行

使用 `/log <机器人名>` 查看机器人最新日志。常见原因：

1. `refreshToken` 已失效。
2. `runtime.java_opts` 与 JAR 不匹配，或内存不足导致进程被杀。

## 7. 日志显示 `NeverReady`（启动后始终未完成握手）

* 网络连通性：`curl -sS -o /dev/null -w '%{http_code}\n' https://discord.com/api/v10/gateway`，返回 `200` 表示可以访问 Discord API。
* 检查 `[discord] token` 是否正确（多余空格或缺少片段都会导致失败）。
* 使用代理时注意：HTTP 代理通常无法转发 WebSocket，建议使用 TUN 模式。

## 8. 日志显示 `Unreachable`

可能原因包括网络中断、代理节点异常或 Token 被重置。

若网络延迟较高或经常抖动，可增大 `timing.watchdog_window`：探活间隔（其 1/3）与首次握手超时（其 2 倍）会随之变化。判定细节见 [watchdog.md 第 5 节](../mods/watchdog/watchdog-zh_cn.md#5-常见排查)。

## 9. 钓鱼机器人日志内容很少，部分信息未记录

这是预期行为：过滤规则正在生效。匹配 `keywords.filter_regex` 的行默认会被丢弃；删除对应表达式并重启进程后，新会话的日志中会重新出现这些行。

## 10. 钓鱼机器人显示状态与实际不符

日志关键词与游戏服务器实际输出的消息不一致。检查配置中的 `[keywords]`。

* 重连提示或 `/pause` 无法识别时，先确认 `[i18n] language` 与 JAR 实际输出的语言一致。
* 重连提示与 `/pause`、`/unpause` 的回显是 JAR 的固定文案，随语言设置变化，不能单独配置。机制见 [i18n.md](../mods/i18n/i18n-zh_cn.md)。

## 11. 使用 `/startall` 时部分机器人启动失败

* 微软按出口 IP 限制登录频率。`/start`、`/startall`、`/restart` 与 `/restartall` 共用全局启动队列。示例配置使用 `max_batch_size = 1` 与 `batch_login_interval = 20`，即两次登录至少间隔 20 秒；若增大批大小，还需把 `bot_login_interval` 设为 18–20 秒。机制、实际登录间隔的计算方式与推荐取值见第 12 节。
* 批量启动的瞬时内存峰值可能超过服务器可用内存。被系统杀掉的进程不会出现在 `screen -list` 中，因此报告会显示“启动失败（常见原因是微软拒绝频繁登录或内存不足）”。可减小 `runtime.java_opts` 的堆上限；若 `max_batch_size` 大于 1，可调小它，或增大 `bot_login_interval` 以错开登录；内存暂时紧张时可稍后重试（JVM 内存占用会随时间下降）。如需让复核覆盖更长的启动阶段，可增大 `timing.check_grace`。

## 12. 固定某台机器人启动失败（微软登录限流）

现象：`/startall` 报告将某台机器人计为“启动命令已发送，会话已出现”，但随后 `/show status` 中看不到它。失败与账号无关，始终是登录次序靠后的机器人（通常是最后一台）；同一账号稍后单独 `/start` 可以成功。

原因：XSTS 换取 Minecraft 令牌的请求（`POST https://api.minecraftservices.com/authentication/login_with_xbox`）按**出口 IP** 限流，超限返回 `HTTP 429`。FishingBot 不检查状态码，直接读取响应中的 `access_token`，随后抛出异常，进程在启动后 2–5 秒退出。是否被报告为失败取决于复核窗口：

* 复核通过两次会话快照完成：整批机器人拉起后立即一次，等满 `timing.check_grace` 后一次。批次越小，窗口越接近启动瞬间：`max_batch_size = 1`（每台单独一批）以及末尾不足一批时，只有约一个 `check_grace` 的观察时间，因此 2–5 秒后发生的 429 拒绝常常无法被发现，最终被报告为成功。

限流实际依据的登录间隔由 `[startup_queue]` 的两个间隔键决定：

* 批内相邻两台：`bot_login_interval`；
* 上一批最后一台 → 下一批第一台：`batch_login_interval` 减去本批从出队到最后一台拉起已过去的时间（复核在命令侧并行进行，不占这段等待）。

同一出口 IP 的经验值：每 20 秒一次（约 3 次/分）稳定；每 12 秒一次（5 次/分）会丢失最后一次；数秒内连续登录会触发惩罚窗口，随后数十秒内的登录全部被拒绝。同一出口 IP 上的其他登录（官方启动器、另一实例、手动登录）也占用同一份额度。

处置：

1. 确保实际登录间隔为 18–20 秒：示例配置使用 `max_batch_size = 1` 与 `batch_login_interval = 20`；若增大批大小，还需把 `bot_login_interval` 设为 18–20 秒。`max_batch_size` 本身不决定最小间隔，它决定批内与批间两个间隔键如何组合。
2. 失败的机器人等待 3–5 分钟后再启动（`/start <机器人名>` 或 `/restart <机器人名>`），不要立即重试。
3. 要显著提高启动速度，主要只有两种方式：使用多个出口 IP，或让 JAR 复用 Minecraft access token（有效期 24 小时），避免每次启动都重新登录。
4. 与内存不足的区别（第 11 节）：被限流的进程在启动后 2–5 秒自行退出，日志中有认证相关报错；被 OOM 杀掉的进程通常在 0.3 秒内消失，日志为空或包含 `Killed`，`dmesg` 中可见记录。
5. 配置项细节见 [config-zh_cn.md](../mods/config/config-zh_cn.md)。

## 13. 修改配置后，正在运行的机器人会受影响吗

不会。配置只在启动时读取一次并常驻内存。运行中修改甚至删除配置文件都不会影响已启动的进程；修改后需重启本程序才会生效。

## 14. 如何检查残留的机器人进程

```bash
screen -ls | grep fb_                 # 根据配置中的会话名前缀调整
pgrep -af "java .*-refreshToken"      # 根据配置中的 Token 文件名调整
```

若发现残留进程，可使用 `/stopall` 停止全部钓鱼机器人、用 `pkill` 终止对应 JVM，或重启本程序。

## 15. 升级后启动时提示配置字段错误

本程序会在启动时校验配置，并拒绝未知字段。升级后可先备份，再删除旧配置文件并重新运行，程序会生成最新模板。

本次配置调整涉及的字段（示例模板已同步更新）：

* `[startup_queue] batch_cooldown_ms` 改为 `bot_login_interval`：含义从“批间冷却”变为“两次登录之间的最小间隔”（批内机器人也按此间隔错开），并取代原 `[timing] stagger_ms`。
* `[timing]` 移除 `probe_interval_ms` 与 `first_handshake_timeout_ms`：两者由 `watchdog_window` 派生（探活间隔为其 1/3，首次握手超时为其 2 倍）。

## 16. 同一条指令收到多次回复

部分指令无法立即确认，因此会在稍后补充回复。若完全相同的回复出现两次，通常是有多个进程使用了同一个 Bot Token；先全部停止，再只启动一个实例。

* 同一台机器可以运行多个实例，但每个实例应使用独立的工作目录、配置与 Bot Token，并避免冲突（尤其是会话名前缀）。这样它们可以作为独立服务，分别管理不同游戏服务器上的机器人。

## 17. 相关文档

* [tutorial/tutorial-zh_cn.md](../tutorial/tutorial-zh_cn.md)：从零开始的部署教程（本文件的多数结论在那边有上下文）。
* [config-zh_cn.md 第 3.2 节](../mods/config/config-zh_cn.md#32-游戏侧配置confjson是生成物)：游戏侧 `conf.json` 是生成物的规则。
* [i18n.md](../mods/i18n/i18n-zh_cn.md)：`[i18n] language` 如何同时决定本程序与 FishingBot 使用的语言，以及关键词如何随语言变化。
* [watchdog.md 第 5 节](../mods/watchdog/watchdog-zh_cn.md#5-常见排查)：`NeverReady`、`Unreachable` 的判定与排查。
