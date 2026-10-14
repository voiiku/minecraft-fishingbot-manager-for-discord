# watchdog：网关看门狗
[English](watchdog-en_us.md) | **简体中文**

* 代码：`src/watchdog.rs`（`Watchdog` 本体：时序字段、握手/探活记账、掉线记账，以及进程级单例 `WATCHDOG`）
  与它的两个子模块：`gateway`（握手与探活判定，自带探活任务）、`lifecycle`（掉线记账、重连信号、等一次尝试结束
  与进程收尾：停机、退出、重连通报）；进程入口 `src/main.rs` 只管接线与重连循环（用 `WATCHDOG.get_or_init()` 显式
  取那一份单例，首次调用按配置建；事件回调只收函数指针，走的是同一个入口）。
* 目的：判定本进程是否已经跟 Discord 失去联系。判定不通过就把原因交给进程入口（`request_reconnect()`），
  由入口停掉全部机器人并重建连接；进程不退出，等一个判定窗口后在同一个进程里重新装配 `Client` 再连（见第 4 节）。

## 1. 为什么必须有它

连不上 Discord 时我们既控制不了机器人，机器人却还占着游戏账号：FishingBot 掉线后会自动重连，账号却
始终在线，真人玩家就上不了线。所以宁可先停干净，也不能挂着一个没人能指挥的钓鱼机。

看门狗使用本实现能观测的两项信号：网关握手与 REST 可达性。判定掉线后，进程会停掉机器人、保持运行并自行
重连，而不是依赖进程管理器反复重启；这样可以避免每次重试都冷启动并新建程序日志文件。

## 2. 两条判据

* 握手：本次尝试开始后 `first_handshake_timeout` 内必须收到过一次 Discord 的 `READY` 或 `RESUMED`
  （`main.rs` 的 `event_handler` 调 `watchdog.record_handshake_success()`）。不满足时请求重建连接，原因是
  `NeverReady`。
* 可达：每 `probe_interval` 用 REST 取一次自身信息（`watchdog.probe()`，每次装配前
  拉起一个，参数是那次连接的 `Http` 句柄）：取到就调 `watchdog.clear_probe_failures()`，失败就调
  `watchdog.record_probe_failure_start()`。连续失败累计满 `window` 时请求重建连接，原因是 `Unreachable`。

细节：

* 三项时序都来自配置、各自独立：判定窗口 `timing.watchdog_window_ms`、探活间隔 `timing.probe_interval_ms`
  （示例配置里取窗口的 1/3）、首次握手超时 `timing.first_handshake_timeout_ms`（示例配置里取窗口的 2 倍）。
  三项都必填（代码里没有默认值）。入口把它们装进 `Watchdog`（就是它的三个字段），
  判定与探活用的是同一份。探活间隔要小于或等于判定窗口，比它慢的话
  第一次探活失败就已经满一个窗口、等于一次失败就判掉线，启动时会拦下这个配置。
* 探活走 REST：`GET /api/v10/gateway` 收到 HTTP 响应只证明 REST 端点可达，不代表 Token 有效；认证错误由
  Discord 客户端连接流程报告。超时或连接错误才记为探活失败。探活打的是 `discord.com`，网关实际连的
  是 `gateway.discord.gg`；若出现 API 可达但网关不可达的部署，再考虑增加网关域名的 TCP 或 TLS 探活。
* 探活任务在 `client.start()` 之前就拉起：网关从头到尾连不上时 `READY` 不会来，必须在 client 之外也
  能判；提前拉起还能让"启动就断网"在首次握手超时内就退出。
* 判定可逆：探活恢复、或又收到 `READY` 或 `RESUMED`，就回到 `Alive`，不是判一次就回不来。
* 判定是纯计时：四态 `Alive`、`Waiting`、`NeverReady`、`Unreachable`（看门狗内部，不对外暴露），判定时刻由调用方传入
  （`watchdog.state(now)`），所以每条规则都有单元测试（含只记第一次失败、恢复即复活）。

## 3. 刻意不读框架状态

serenity 0.12.5 的 `ShardMessenger`（也就是 `ctx.shard`）只暴露 `shutdown_clean`、`set_presence` 这类
动作，没有 `stage()` 与 `latency()`；`Shard` 又在 shard runner 内部拿不到。而网关侧那种连着但收不到
东西的半死状态，由 serenity 自己的心跳与 ACK 机制发现并自动重连（协议保证），我们不需要也不能重复
实现，只兜上面那两种根本联系不上的情况。

## 4. 掉线之后：停机器人，等一个窗口，重建连接

判定在 `gateway.rs`，收尾与重建在 `src/watchdog/lifecycle.rs`：判定侧只负责判定与叫醒，看得见的动作
（停机、记账、写日志、退进程）都在进程入口那一侧做——入口那侧的动作是 `wait_for_attempt()`（等这一次尝试的
结束，并做掉线收尾）、`wait_before_reconnect()`（等一个判定窗口）与 `exit_on_shutdown_signal()`（收到退出信号后
停机并以退出码 0 退出）。

1. 判定侧：判定掉线 → `request_reconnect(reason)`，把原因放在看门狗身上并叫醒进程入口（一个 `tokio::Notify`）；
   它本身不动机器人。
2. 入口侧的收尾（`wait_for_attempt()` 的"看门狗判定掉线"那一路）：取来判定侧交的原因，调私有的
   `stop_all_bots_on_disconnect(reason)` 停掉全部机器人（`shutdown_all_bots()`，与 `/stopall` 同一套流程）、
   写一条 WARN、记下原因与掉线时刻（取不到原因的是迟到的唤醒，那一轮已经收过尾，就不再动机器人）。
   为什么必须停：连不上 Discord 时谁也指挥不动它们，而 FishingBot 的自动重连会一直占着游戏账号跟真人玩家
   抢号。掉线期间不会自动恢复，重连成功后由人到 Discord 里发 `/startall` 或 `/start 名字`。
3. 然后关掉旧连接（`shard_manager.shutdown_all()`，否则新旧两个连接会抢同一个 bot 的会话、Discord 来回踢）、
   调 `wait_before_reconnect()` 等一个判定窗口（复用 `timing.watchdog_window_ms`，不新增配置项）、重新装配
   `Client` 再连一次；连不上就一直这样重试。

客户端自己返回（`client.start()` 返回：被关停，或 shard 启动失败与 token / Intent 非法这类致命错误；普通掉线
不会让它返回）也走 `wait_for_attempt()`：同一个 `stop_all_bots_on_disconnect()`，只是原因由入口自己写、也不必
叫醒自己，于是两条路径的收尾与恢复通报完全一致。

两条容易踩的坑，都已经钉在单测里：

* 每次尝试都要调 `watchdog.begin_connection_attempt()`，让首次握手超时从这一次尝试的开始重新计时。
  不重置的话重连一上来就被判 `NeverReady`（那是上一轮的旧账），于是每窗口重连一次、永远连不上。
* 探活任务要能被叫停（`stop` 是一个一轮一个的 `AtomicBool`），上一轮的探活必须退场，否则两个任务一起
  探、一起记账、一起喊掉线。用标志位而不是通知，是因为置位是状态、不会被错过，而 `notify_waiters()`
  那种事件在没人等时就丢了。

只有重试也没用的错误才退出进程（`exit_on_permanent_failure`，退出码 1；成因按 `ERROR` 记日志，它是
配置写错，与可重试的网络抖动（`WARN`）在日志里能一眼分开）：Token 无效、机器人没有被邀请进服务器
（`50001 Missing Access`）、装配被拒。这些是配置写错，重试一万次也一样。
退出码 1 是本项目的主动失败专用码（配置加载失败、校验失败也从 `main` 以 1 退出），所以单元
文件里建议配 `RestartPreventExitStatus=1`（见 [部署教程第 12 节](../../tutorial/tutorial-zh_cn.md#12-配置-systemd-服务可选推荐)）：这类错误退一次
就停、进 `failed` 等人处理。崩溃是别的码（panic 101、被 OOM 或 kill 137 等），照常按
`Restart=on-failure` 重拉。

### 重连成功后的通报

`Ready` 或 `Resume` 再次到来时，`watchdog.notify_reconnect_success()`（入口把 `serenity::Context` 交给它）会向本机器人
所在的所有服务器各发一条消息："我在 `<时刻>`
起与 Discord 失去联系，直到刚才才恢复（掉线 `<N>` 秒）；掉线期间已停止全部游戏机器人；要让它们继续跑
就发 `/startall`"。落点是该服务器的系统频道，没有就用它的第一个文字频道；一个服务器只发一条，某个
服务器发不出去只留一条告警，不影响其它服务器。这条通报同时也写进程序日志
（`ReconnectedAfterOffline`：发往几个服务器、失败几处）。

## 5. 常见排查

* 日志反复出现 `NeverReady` 的重连（进程没退出）：出网被拦、Token 不对或 Discord 侧故障。先在部署机
  上验：`curl -sS -o /dev/null -w '%{http_code}\n' https://discord.com/api/v10/gateway` 应当拿到 200。
* 日志说 `Unreachable`：断网、代理掉了、Token 失效；看上一行 `REST 探活失败:` 给的具体原因。
* 网络偶尔抖动就会重连一次：把 `timing.watchdog_window_ms` 调大（例如 60000，必要时连
  `timing.first_handshake_timeout_ms` 一起调大）；窗口内恢复不会误杀。
* 日志里反复出现"重建连接失败"，但进程没退出：这是正常的重试（掉线不退出进程）。若是
  `Missing Access`、`401` 这类配置写错，进程会以退出码 1 退出；单元里配 `RestartPreventExitStatus=1`
  就不重拉（退一次就停、进 `failed`），配 `StartLimitBurst` 则挡的是崩溃循环。

收到第一个 `READY` 时会打印一行
`网关看门狗已就绪：判定窗口 N 秒、探活间隔 N 秒、首次握手超时 N 秒`，排查时以这一行为准（它反映实际
生效的三项参数）。

## 6. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 为什么必须由它判定掉线：[CONTEXT-zh_cn.md](../../CONTEXTS/CONTEXT-zh_cn.md) 第 3 节。
* 探活与重连记在哪：[process-log-zh_cn.md](../process-log/process-log-zh_cn.md)。
