# process_log：程序日志
[English](process-log-en_us.md) | **简体中文**

* 代码：`src/process_log.rs`
* 对外 API：`init_process_log(debug: bool)`（从 crate 根重导出；它自己会写下配置路径与进程号两行启动
  事实）。

## 1. 为什么自己装 logger

serenity 与 poise 自己的日志走 `tracing`，默认只往终端打，而本项目还需要能事后翻查的文件
（`journalctl` 会轮转），所以自己实现 `log::Log`，直接依赖 `log` 与 `chrono`。装好之后框架与本项目
自己的 `log::*` 全部落到这里（poise 依赖 `tracing` 时开着它的 `log` 特性，所以框架日志会转成 `log`
记录进来，已由实际运行确认：程序日志里能看到 serenity 的记录）。本项目不再用 `println!` 打诊断信息，
因此终端与文件内容一致；例外只有一条，即日志自己出问题时的降级告警（见第 4 节与第 5 节）。

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

分两档（实现见 `process_log::effective_log_levels`）：

* 本项目（target 以 `minecraft_fishingbot_manager_for_discord` 开头，lib 与 bin 都是）：`[log] debug = false`
  时上限 `info`，`true` 时 `debug`。启动、状态、报告这些有用信息平时就够看。
* 第三方 crate（`serenity`、`poise`、`tungstenite` 等）：`false` 时上限 `warn`，`true` 时 `debug`。
  它们按心跳节奏打 `recv; do_heartbeat; recv_event;`（启动时更多），默认会把上面那几行淹没。

一句话：`debug` 开着就是连框架的话一起听（两边都到 `DEBUG`），关着时框架只在真出问题时（`WARN`）出声。

`RUST_LOG` 优先，但只认单个级别词（`off`、`error`、`warn`、`info`、`debug`、`trace`，大小写不敏感）：
显式设置了它，两档都用它给的那个级别，想单独看框架的 `TRACE`（字节级流水）只能靠它。没设置或为空时按
配置的 `[log] debug`；值认不出来（例如 `poise=debug` 这类 directive）按上面的默认两档处理，并写一条
告警说明。刻意只保留一个级别：本项目按级别调试，不按模块调试（不支持 `serenity=debug` 这种 per-target
directive）。

## 4. 失败时怎么办

* 部署目录不可写（例如二进制在 `/usr/local/bin`、服务以普通用户跑）：不致命。退化成只写终端并写一条
  告警，机器人照常运行。这条告警在全局级别严于 `WARN`（`RUST_LOG=error` 或 `off`）时直写终端，不会被
  级别过滤掉，因为"日志没落盘"不能只在文件里说，那时文件正是没有的那一份。
* logger 已经被装过（只会出现在调用两次时）：保留已装的 logger，打印一条说明。

理由：日志是运维问题，不该演变成游戏账号上不了线。

所以 `init_process_log` 不返回 `Result`：装完 logger 之后运行期的写失败（磁盘写满、文件被删）同样
无处上报（`log::*` 的签名就是 `()`），返回 `Result` 只盖住装之前那半步，还会让 `Ok(())` 看起来像日志
系统健康；把降级判断交给调用方，也只是让每个调用点重写一遍这套"写条告警、继续跑"。

## 5. 调用时机

必须在装配 poise `Framework` 与任何日志之前调用，顺序见 `src/main.rs`（`config::get_or_init()`、
`init_process_log`、装配 `Framework`/`Client` 并 `client.start()`）。配置没加载成功时还没有日志文件，那几条提示只出现在
终端。不经 logger 的直写只剩三处：`src/config.rs` 的模板提示（stdout），以及日志自己出问题时的两条告警
（`src/process_log.rs`：logger 装不上走 stdout；日志文件建不出来走 stderr，与日志同一个流，见第 4 节）。

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
