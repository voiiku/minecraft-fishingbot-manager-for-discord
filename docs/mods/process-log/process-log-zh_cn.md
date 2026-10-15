# process_log：程序日志
[English](process-log-en_us.md) | **简体中文**

* 代码：`src/process_log.rs`（+ `process_log/levels.rs` 级别判定、`process_log/format.rs` 记录渲染，
  两份纯函数各自带测试）
* 对外 API：`ProcessLog::get_or_init()`（`ProcessLog` 从 crate 根重导出；首次调用创建本次的日志文件、把它
  装上，并写下日志位置、配置路径与进程号三条启动事实）。

## 1. 为什么自己装 logger

serenity 与 poise 自己的日志走 `tracing`，默认只往终端打，而本项目还需要能事后翻查的文件
（`journalctl` 会轮转），所以自己实现 `log::Log`，直接依赖 `log` 与 `chrono`。装好之后框架与本项目
自己的 `log::*` 全部落到这里（poise 依赖 `tracing` 时开着它的 `log` 特性，所以框架日志会转成 `log`
记录进来，已由实际运行确认：程序日志里能看到 serenity 的记录）。本项目不再用 `println!` 打诊断信息，
因此终端与文件内容一致；例外只有几条直写终端的说明：日志这条线自己出问题、或重复初始化时保留已装的
那一份（见第 4 节与第 5 节）。

## 2. 两条输出

* 终端（stderr）：前台输出，或 `journalctl -u minecraft-fishingbot-manager-for-discord`。用途是实时看，
  systemd 托管时也能追溯。
* 文件：`<部署目录>/logs/年-月-日-时-分-秒.log`。用途是事后翻查（写一行刷一行，`kill -9` 不丢内容）。

每条记录形如
`[2026-09-28 15:30:12] [INFO] 已加载配置: /fishingbots/minecraft-fishingbot-manager-for-discord.conf.toml`，
纯文本、不带颜色；文件名复用 `utils::create_timestamped_log_file`（与机器人日志同一套命名，时间戳格式
`%Y-%m-%d-%H-%M-%S`），首行固定是 `--- 正在启动 Minecraft FishingBot Manager for Discord 进程 ---`。

记什么：

* 本项目自己的事件：配置加载、启动、关停、看门狗、机器人目录与房间、启动与停止与重启与暂停的明细、
  警告与错误。
* 每一次回复（`LogText::CommandReplied`）：谁调的（Discord 用户 ID）、哪条指令、回了什么。命令层的
  回复全部从 `commands::say_and_log` 与 `log_reply` 出去（见 [commands-zh_cn.md](../commands/commands-zh_cn.md)），所以
  "用户说他收到的是什么"能在日志里逐条对上。
* 第三方 crate（serenity、poise）的日志默认只到 `WARN`（见下一节），因为它们按心跳节奏打 `INFO`，会把
  上面两类淹掉。

## 3. 级别

分两档（实现见 `process_log/levels.rs` 的 `LogLevels::from_config`，只由配置里的 `[log] debug` 决定）：

* 本项目（target 以 `minecraft_fishingbot_manager_for_discord` 开头，lib 与 bin 都是）：`[log] debug = false`
  时上限 `info`，`true` 时 `debug`。启动、状态、报告这些有用信息平时就够看。
* 第三方 crate（`serenity`、`poise`、`tungstenite` 等）：`false` 时上限 `warn`，`true` 时 `debug`。
  它们按心跳节奏打 `recv; do_heartbeat; recv_event;`（启动时更多），默认会把上面那几行淹没。

一句话：`debug` 开着就是连框架的话一起听（两边都到 `DEBUG`），关着时框架只在真出问题时（`WARN`）出声。

没有环境变量、也没有第二个开关（本项目历史上认过 `RUST_LOG`，现已取消）：级别只看这一行配置，别处
没有来源。要更细的 `TRACE`（字节级流水）或按模块调试（`serenity=debug` 这类 per-target directive）都
不支持 —— 本项目按级别调试，不按模块调试。

## 4. 失败时怎么办

* 部署目录不可写（例如二进制在 `/usr/local/bin`、服务以普通用户跑）：不致命。退化成只写终端并写一条
  告警，机器人照常运行。这条告警走 logger（因此带 `[时间] [WARN]` 前缀落到 stderr），而两档上限里更宽
  的那一档恒不严于 `WARN`，所以它不会被级别过滤掉 ——"日志没落盘"不能只在文件里说，那时文件正是没有
  的那一份。
* logger 已经被装过：`ProcessLog::get_or_init` 自己先看一眼装没装，装过就保留已装的那一份、打印一条
  说明并直接返回，不再建日志文件（建文件发生在装 logger 之前，先判断才不会在 `logs/` 里留下一个只有
  首行的空日志文件）。

理由：日志是运维问题，不该演变成游戏账号上不了线。

所以 `ProcessLog::get_or_init` 不返回 `Result`：装完 logger 之后运行期的写失败（磁盘写满、文件被删）同样
无处上报（`log::*` 的签名就是 `()`），返回 `Result` 只盖住装之前那半步，还会让 `Ok(())` 看起来像日志
系统健康；把降级判断交给调用方，也只是让每个调用点重写一遍这套"写条告警、继续跑"。

## 5. 调用时机

必须在装配 poise `Framework` 与任何日志之前调用，顺序见 `src/main.rs`（`config::Config::get_or_init()`、
`ProcessLog::get_or_init`、装配 `Framework`/`Client` 并 `client.start()`）；级别由它自己从配置里读
`[log] debug`，所以入口不传参。配置没加载成功时还没有日志文件，那几条提示只出现在终端。不经 logger 的
直写只剩三处，都在 stdout：`src/config.rs` 的模板提示，与 `src/process_log.rs` 的两条说明（logger 装不
上、重复初始化时保留已装的那一份）。

## 6. 为什么不限制大小、也不清理旧文件

一次运行一份、名字是启动时刻，写完就不动它；进程不轮转、不删旧文件：

* 没有任何服务程序化地读这份日志，它只在排查时被人翻开，所以单份多大、留几份不是程序要管的事。
* 真需要切分时（`[log] debug = true` 那种连框架一起刷屏的排查）人工来：`split`、`logrotate`、直接
  `> 文件` 都行，比在程序里再造一套轮转策略简单。
* 会堆文件的是进程被 systemd 每 10 秒拉起一次那种循环（网关连不上、代理掉了），那件事由"进程常驻加
  掉线重连"从根上解决（见 [watchdog.md](../watchdog/watchdog-zh_cn.md)），不靠清理旧文件掩盖。

## 7. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 两条日志线（机器人日志与程序日志）的口径：[utils-zh_cn.md](../utils/utils-zh_cn.md) 第 7 节。
* 掉线判定与重连会在日志里留下什么：[watchdog-zh_cn.md](../watchdog/watchdog-zh_cn.md)。
