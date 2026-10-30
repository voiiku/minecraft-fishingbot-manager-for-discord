# watchdog：网关看门狗
[English](watchdog-en_us.md) | **简体中文**

* 代码：`src/watchdog.rs`（`Watchdog` 本体：时序字段、握手/探活记录、掉线记录，以及进程级单例 `WATCHDOG`）
  与其子模块 `gateway`（握手与探活判定、探活任务）、`lifecycle`（掉线记录、重连信号、等待一次尝试结束、
  进程收尾）；`src/main.rs` 负责接线与重连循环（用 `WATCHDOG.get_or_init()` 获取单例，事件回调只接收函数
  指针，也走同一入口）。
* 目的：判定本进程是否已与 Discord 失去联系。判定失败时把原因交给进程入口（`request_reconnect()`），由入口
  停止全部机器人并重建连接；进程不退出，等待一个判定窗口后在同一进程内新建 `Client` 重连（见第 4 节）。

## 1. 为什么需要看门狗

Discord 不可达时，我们既无法控制机器人，机器人却仍占用游戏账号：FishingBot 掉线后会自动重连，账号始终在线，
真人玩家因此无法进入。此时应先将机器人干净地停止，而不是留下无人能控制的钓鱼机。

看门狗使用本实现能够观测的两项信号：网关握手与 REST 可达性。判定掉线后，进程会停止机器人、保持运行并自行
重连，而不依赖进程管理器反复重启；这样既避免每次重试都重新启动进程，也不会每次都新建一份程序日志。

## 2. 判定条件

* 握手：本次连接尝试开始后，必须在 `first_handshake_timeout` 内收到一次 Discord 的 `READY` 或 `RESUMED`
  （`main.rs` 的事件处理器调用 `watchdog.record_handshake_success()`）。未满足时请求重建连接，原因为
  `NeverReady`。
* 可达性：每隔 `probe_interval` 通过 REST 调用 `Http::get_current_user()` 获取自身信息。成功时调用
  `watchdog.clear_probe_failures()`，失败时调用 `watchdog.record_probe_failure_start()`；从首次失败起持续满
  `window` 后请求重建连接，原因为 `Unreachable`。

细节：

* 时序来自配置中的判定窗口 `timing.watchdog_window`；探活间隔（窗口的 1/3）与首次握手超时（窗口的 2 倍）
  由它派生，不再作为独立配置项——这两个值只有按窗口比例取值才有意义。入口将三项装入 `Watchdog`（对应三个
  字段），判定与探活共用同一份。派生关系保证探活间隔始终小于判定窗口，因此“探活比窗口还慢、首次失败就判
  掉线”的组合无法再被配置出来。
* 探活使用 REST：`GET /api/v10/users/@me` 收到 2xx 只说明 REST 端点可达且当前 Token 可用，不代表网关可达；
  超时、连接失败以及 HTTP 401/5xx 等错误都会计为一次探活失败。探活请求发往 `discord.com`，网关实际连接的是
  `gateway.discord.gg`；如果出现 API 可达但网关不可达的部署，再考虑增加网关域名的 TCP 或 TLS 探活。
* 探活任务在 `client.start()` 之前启动：网关完全连不上时 `READY` 不会到来，判定必须在 client 之外也能工作；
  提前启动还能让“启动时无网络”在首次握手超时后结束本轮尝试。
* 判定可逆：探活恢复，或再次收到 `READY`/`RESUMED`，都会回到 `Alive`，不会因一次失败而永久失效。
* 判定只依赖时间与已记录状态：四态 `Alive`、`Waiting`、`NeverReady`、`Unreachable`（看门狗内部状态，不对外
  暴露），判定时刻由调用方传入（`watchdog.state(now)`），因此每条规则都有单元测试（包括“只记录第一次失败”
  与“恢复后重新存活”）。

## 3. 不读取框架内部状态

serenity 0.12.5 的 `ShardMessenger`（即 `ctx.shard`）只暴露 `shutdown_clean`、`set_presence` 等动作，没有
`stage()` 与 `latency()`；`Shard` 也无法从 shard runner 内部获取。对于“网关仍连接但收不到数据”的半死状态，
serenity 自带的心跳与 ACK 机制会发现并重连（协议保证），我们既不需要也无法重复实现，因此只覆盖上面两种
完全无法联系 Discord 的情况。

## 4. 掉线后的处理：停止机器人、等待窗口、重建连接

判定在 `gateway.rs`；`lifecycle.rs` 提供收尾与等待的辅助函数；重建连接的重连循环在 `src/main.rs`。职责划分是：
判定侧只判定并唤醒，所有可见动作（停止、记录、写日志、退出）都在入口侧执行。入口侧使用
`wait_for_attempt()`（等待本轮尝试结束并完成掉线收尾）、`wait_before_reconnect()`（等待一个判定窗口）与
`exit_on_shutdown_signal()`（收到退出信号后就地生成与 `/stopall` 同口径的收尾明细，写入程序日志并发送到各
服务器的通报频道，标题为“收到退出信号，收尾结果”，随后以退出码 0 退出）。

1. 判定侧：判定掉线后调用 `request_reconnect(reason)`，将原因保存在看门狗中并唤醒进程入口（`tokio::Notify`）；
   它本身不操作机器人。
2. 入口侧收尾（`wait_for_attempt()` 的“看门狗判定掉线”分支）：取出判定侧交来的原因，调用私有的
   `stop_all_bots_on_disconnect(reason)` 停止全部机器人（`shutdown_all_bots()`，与 `/stopall` 同一套流程），
   记录 WARN、掉线原因与掉线时刻。若取不到原因，说明这是迟到的唤醒，本轮已经完成收尾，不再操作机器人。
   必须停止机器人的原因是：Discord 不可达时无人能够指挥它们，而 FishingBot 的自动重连会持续占用游戏账号，
   与真人玩家争抢登录名额。掉线期间不会自动恢复；重连成功后，需要由人到 Discord 中发送 `/startall` 或
   `/start 名字`。
3. 随后关闭旧连接（`shard_manager.shutdown_all()`，否则新旧两个连接会争抢同一个 bot 会话，导致 Discord 反复
   踢下线），调用 `wait_before_reconnect()` 等待一个判定窗口（复用 `timing.watchdog_window`，不新增配置项），
   再重新装配 `Client` 并连接；如果仍然失败，就按同样流程继续重试。

客户端自行返回（`client.start()` 返回：客户端被关停，或 token / Intent 非法等致命错误；普通掉线不会让它返回）也走
`wait_for_attempt()`：使用同一个 `stop_all_bots_on_disconnect()`，区别只是原因由入口写入、且无需唤醒自身，
因此两条路径的收尾与恢复通报完全一致。

两个关键约束：

* 每次尝试都必须调用 `watchdog.begin_connection_attempt()`，让首次握手超时从本轮开始重新计时。否则重连一开始
  就会被上一轮的旧状态判为 `NeverReady`，形成“每个窗口重连一次、却始终连不上”的循环。
* 探活任务必须可以停止（`stop` 是每轮一个的 `AtomicBool`，在探活前与请求返回后各检查一次）。上一轮的探活任务
  必须退出，否则两个任务会同时探活、同时记录失败并一起触发掉线。使用标志位而非通知，是因为标志位表示状态，
  不会在无人等待时丢失；`notify_waiters()` 这类事件在没有等待者时会丢失。

只有重试无法解决的错误才退出进程（`exit_on_permanent_failure`，退出码 1；原因按 `ERROR` 记录，便于与网络抖动
这类可重试错误（`WARN`）区分）：Token 无效、机器人未被邀请进入服务器（`50001 Missing Access`）、装配被拒等。
这些都属于配置错误，重试再多次结果也相同。退出码 1 是本项目主动失败的专用码（配置加载失败、校验失败同样以 1
退出 `main`），因此单元文件建议配置 `RestartPreventExitStatus=1`（见
[部署教程第 12 节](../../tutorial/tutorial-zh_cn.md#12-配置-systemd-服务可选推荐)）：这类错误退出一次后即停止，
进入 `failed` 状态等待人工处理。崩溃使用其他退出码（panic 101、被 OOM 或 kill 137 等），仍由
`Restart=on-failure` 重新拉起。

### 重连成功后的通报

`Ready` 或 `Resume` 再次到来时，`watchdog.notify_reconnect_success()`（入口把 `serenity::Context` 交给它）会向
本机器人所在的所有服务器各发送一条消息：“我在 `<时刻>` 起与 Discord 失去联系，直到刚才才恢复（掉线 `<N>`
秒）；掉线期间已停止全部游戏机器人；要让它们继续运行，请发送 `/startall`”。消息发送到该服务器的系统频道；
若不存在系统频道，则使用第一个文字频道。一个服务器只发送一条；某个服务器发送失败只会留下一条告警，不影响
其他服务器。该通报同时写入程序日志（`ReconnectedAfterOffline`：发送到几个服务器、失败几处）。

### 收到退出信号时的收尾报告

`exit_on_shutdown_signal()` 就地生成的明细与 `/stopall` 同口径：先写入程序日志，再按相同落点规则（系统频道，
不存在则使用第一个文字频道）发送到各服务器（`ShutdownReportNotified`：发送到几个服务器、失败几处；本次连接
未成功握手时只写日志，记 `ShutdownReportNoContext`）。掉线与永久错误的即时收尾不会向频道发送消息：此时连接
已经不可用，发送也无法成功。

发送有 30 秒总时限（`SHUTDOWN_REPORT_TIMEOUT`）：到时仍未完成的发送会被取消（记 `ShutdownReportTimeout`），
进程照常退出；已发送的内容不受影响。该报告不会另行持久化：除写入本次程序日志并尝试发送一次外，不保留待发送
副本，下次启动也不会补发。

## 5. 常见排查

* 日志反复出现 `NeverReady` 的重连（进程未退出）：可能是出网被拦截、Token 错误或 Discord 侧故障。先在部署机
  上验证 REST 连通性：`curl -sS -o /dev/null -w '%{http_code}\n' https://discord.com/api/v10/gateway`
  应返回 200。
* 日志出现 `Unreachable`：可能是断网、代理失效或 Token 失效；查看上一行 `REST 探活失败:` 给出的具体原因。
* 网络偶发抖动导致一次重连：可将 `timing.watchdog_window` 调大（例如 60）；探活间隔与首次握手超时会随之
  变化，窗口内恢复不会被误判为掉线。
* 日志反复出现“重建连接失败”但进程未退出：这是正常重试（掉线不会退出进程）。若属于 `Missing Access`、`401`
  这类配置错误，进程会以退出码 1 退出；单元中配置 `RestartPreventExitStatus=1` 后不会重拉（退出一次后进入
  `failed`），`StartLimitBurst` 则用于限制崩溃循环。

收到第一个 `READY` 时会打印一行
`网关看门狗已就绪：判定窗口 N 秒、探活间隔 N 秒、首次握手超时 N 秒`。排查时以这一行为准，它反映实际生效的
三项参数。

## 6. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 为什么必须由它判定掉线：[CONTEXT-zh_cn.md](../../CONTEXTS/CONTEXT-zh_cn.md) 第 3 节。
* 探活与重连记在哪：[process-log-zh_cn.md](../process-log/process-log-zh_cn.md)。
