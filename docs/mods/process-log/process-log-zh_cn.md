# process_log：程序日志
[English](process-log-en_us.md) | **简体中文**

* 代码：`src/process_log.rs`（`process_log/levels.rs` 负责级别判定，`process_log/format.rs` 负责记录渲染；
  两部分都是纯函数，各自带测试）
* 对外 API：`ProcessLog::get_or_init()`（`ProcessLog` 从 crate 根重导出）。首次调用会尝试创建本次日志文件、安装
  logger，并记录配置路径与进程号（日志文件可用时也记录其路径）。

## 1. 为什么自行安装 logger

serenity 与 poise 通过 `tracing` 记录日志，默认只写终端；本项目还需要一份可事后查阅的文件（`journalctl`
会轮转）。因此这里自行实现 `log::Log`，只依赖 `log` 与 `chrono`。安装后，框架与本项目的 `log::*` 都汇入
这一处（poise 的 `tracing` 启用了 `log` 特性，框架记录会转换为 `log` 记录；实际运行已确认能看到 serenity
的记录）。本项目不再用 `println!` 输出诊断信息，因此终端与文件内容一致；例外只出现在启动最早期，见第 4、5 节。

## 2. 输出与格式

* 终端（stderr）：前台输出，或 `journalctl -u minecraft-fishingbot-manager-for-discord`。用于实时观察；
  systemd 托管时也可追溯。
* 文件：`<部署目录>/logs/年-月-日-时-分-秒.log`。用于事后查阅；每写一行立即刷新，`kill -9` 不会丢失已写入内容。

每条记录形如
`[2026-09-28 15:30:12] [INFO] 已加载配置: /fishingbots/minecraft-fishingbot-manager-for-discord.conf.toml`，
纯文本、无颜色。文件名由 `utils::create_timestamped_log_file` 生成（与机器人日志同一套命名，时间戳格式为
`%Y-%m-%d-%H-%M-%S`）；文件首行是本次启动的说明，文案随 `[i18n] language` 变化。

记录内容：

* 本项目自身的事件：配置加载、启动、关停、看门狗、机器人目录与会话、启动/停止/重启/暂停明细，
  以及警告与错误。
* 每一次回复（`LogText::CommandReplied`）：调用者（Discord 用户 ID）、指令与回复内容。命令层的回复统一
  经由 `commands::say_and_log` 与 `log_reply` 输出（见 [commands-zh_cn.md](../commands/commands-zh_cn.md)），
  因此“用户声称收到的内容”可以逐条与日志核对。
* 第三方 crate（serenity、poise）默认只记录到 `WARN`（见下一节），因为它们按心跳持续输出 `INFO`，会使
  上面两类记录难以查找。

## 3. 日志级别

`process_log/levels.rs` 的 `LogLevels::from_config` 只根据 `[log] debug` 计算两档上限：

* 本项目（target 以 `minecraft_fishingbot_manager_for_discord` 开头，lib 与 bin 都是）：`false` 时为
  `INFO`，`true` 时为 `DEBUG`。启动、状态与报告等信息在 `INFO` 下已足够阅读。
* 第三方 crate（`serenity`、`poise`、`tungstenite` 等）：`false` 时为 `WARN`，`true` 时为 `DEBUG`。
  它们会按心跳节奏记录 `recv; do_heartbeat; recv_event;`（启动时更多），默认设置下会使上面的关键行难以查找。

全局 `max_level` 取两档中较宽的一档，具体记录再由 target 前缀选择上限。`debug` 开启时两边都到 `DEBUG`；
关闭时框架只在真正出现问题时输出 `WARN`。

没有环境变量，也没有第二个开关（本项目早期支持过 `RUST_LOG`，现已移除）：级别只由这一行配置决定。更细的
`TRACE`（字节级流水）与按模块调试（`serenity=debug` 这类 per-target directive）均不支持；本项目按级别
调试，不按模块调试。

## 4. 失败时的降级

* 部署目录不可写（例如二进制位于 `/usr/local/bin`、服务以普通用户运行）：不影响运行。日志退化为只写终端，
  并记录一条告警。该告警通过 logger 输出（因此带 `[时间] [WARN]` 前缀写入 stderr）；全局上限在两种配置下
  都不低于 `WARN`，所以不会被过滤。日志未落盘这件事不能只写在文件里——此时文件正是缺失的那一份。
* logger 已安装：`ProcessLog::get_or_init` 先检查安装状态，已安装时保留现有 logger、打印一条说明并直接返回，
  不再创建日志文件（文件创建发生在安装 logger 之前，先检查才不会在 `logs/` 留下只有首行的空文件）。

这样处理的原因是：日志属于运维问题，不应导致游戏账号无法上线。

`ProcessLog::get_or_init` 因此不返回 `Result`：logger 安装后的运行期写失败（磁盘写满、文件被删除）同样无处
上报（`log::*` 的签名就是 `()`）；返回 `Result` 只能覆盖安装前的一步，还会让 `Ok(())` 看起来像日志系统
健康。把降级判断交给调用方，也只会让每个调用点重复“记录告警、继续运行”。

## 5. 调用时机

必须在装配 poise `Framework`、创建 `Client` 以及任何日志输出之前调用。顺序见 `src/main.rs`：
`config::Config::get_or_init()`、`ProcessLog::get_or_init`、装配 `Framework`/`Client` 并调用 `client.start()`。
级别由它自行从配置读取 `[log] debug`，因此入口不传参数。

配置加载失败时进程尚未创建日志，相关提示只会出现在终端。不经 logger 的直写仅剩启动早期几处：
`src/config.rs` 的模板/配置路径提示（stdout）与配置错误（stderr），以及 `src/process_log.rs` 的两条说明
（logger 安装失败、重复初始化时保留现有 logger，stdout）。

## 6. 文件保留策略

一次运行一份文件，以启动时刻命名，写入后不再改动；进程不轮转，也不删除旧文件：

* 没有任何程序会读取这份日志。它只在排查时由人打开，因此单份大小与保留数量不属于程序职责。
* 确实需要切分时（例如 `[log] debug = true` 且框架日志大量输出），由人工处理：`split`、`logrotate` 或直接
  `> 文件` 均可，比在程序内实现一套轮转策略更简单。
* 会堆积文件的是 systemd 每 10 秒拉起一次进程这类循环（网关不可达、代理失效）。这应由“进程常驻并自行
  重连”从根源解决（见 [watchdog-zh_cn.md](../watchdog/watchdog-zh_cn.md)），而不是靠清理旧文件掩盖。

## 7. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 两条日志线（机器人日志与程序日志）的口径：[utils-zh_cn.md](../utils/utils-zh_cn.md) 第 7 节。
* 掉线判定与重连会在日志里留下什么：[watchdog-zh_cn.md](../watchdog/watchdog-zh_cn.md)。
