# 库接口与命令边界
[English](lib-en_us.md) | **简体中文**

## 1. 命名约定

`operations` 提供不依赖 Discord 的命名函数。`/start` 直接调用 `start_single_bot(name, caller_id)`，`/restartall` 调用 `restart_all_bots(caller_id)`。没有动作枚举或统一分发器。

## 2. 错误处理约定

普通命令返回本地化的结果文本；日志、配置附件和登录设备码使用类型化结果。`discord_commands` 负责 defer、阻塞线程、私密回复和附件，不执行机器人业务。

## 3. 权限规则

单台控制、读取日志、编辑账号和开始登录前检查归属或管理员身份；无权访问与目标不存在的回复相同。批量控制、反 AFK 和指令同步仅管理员可用。新增机器人归调用者；公开查询不要求管理员。

## 4. 进程级生命周期

连接生命周期由 `src/runtime/watchdog/lifecycle.rs` 的 `Watchdog` 方法处理；机器人停止位于 `src/runtime/bot_instances/stop.rs`，进程退出位于 `src/runtime/shutdown.rs`：

* `Watchdog::get_or_init()`：获取进程内唯一的看门狗（首次调用按配置 `[timing]` 创建，后续调用返回同一实例；
  进程入口与事件回调都通过它取用）。必须在 `config::Config::load()` 之后调用，因为首次调用会读取配置。
  `Watchdog::new(...)` 是显式给定时序的构造，仅供测试。
* `runtime::bot_instances::stop::shutdown_all_bots()`：停止所有运行中的机器人会话（不退出进程），失败时返回错误。
* `runtime::shutdown::exit_on_shutdown_signal(notice_ctx)`：收到 SIGTERM / Ctrl+C 后的收尾（入口收到信号后调用）：记录日志、停止全部
  机器人、把收尾报告发到各服务器的通报频道（本次连接尚未握手成功时只写日志），以退出码 0 结束进程
  （systemd 不会重启）。
* `runtime::shutdown::exit_on_permanent_failure(reason)`：配置已加载时报告错误并停止机器人，以退出码 1 结束；
  配置未加载时只向终端报告并退出；日志不可用时也回退到终端。部署示例使用 systemd 的 `KillMode=control-group` 清理残留进程，
  `RestartPreventExitStatus=1` 避免永久错误后重启。
* `watchdog.request_reconnect(reason)`：判定掉线的一方（看门狗判定侧、入口的装配错误出口）用它把掉线原因交给
  进程入口并唤醒它：收尾与重建连接都由入口完成，不退出进程（见 [watchdog.md 第 4 节](../watchdog/watchdog-zh_cn.md)）。
* `watchdog.wait_for_attempt(client)`：入口用它等待本次尝试结束并按结束方式收尾——`client.start()` 返回则调用私有的
  `stop_all_bots_on_disconnect()`（停止全部机器人并记录原因与时刻）收尾；看门狗判定掉线则取来判定侧交来的原因后
  同样收尾；收到退出信号则退出进程。
* `watchdog.wait_before_reconnect()`：等待一个判定窗口后重建连接；期间收到退出信号则直接退出进程。
* `watchdog.remember_notice_context(ctx)`：每次握手成功（`Ready` / `Resume`）由入口记录当前上下文，退出信号收尾
  时用它发送频道消息（掉线收尾不发频道消息，因此不需要它）。
* `watchdog.notify_reconnect_success(ctx)`：掉线后恢复（`Ready` / `Resume` 再次到来）时，向本机器人所在的
  所有服务器发送一次通知；未发生掉线时不执行任何操作。

`main.rs` 按原因分类装配失败并选择对应的本地化日志。缺少访问权限或 Token 无效属于永久错误，
调用 `exit_on_permanent_failure`；其他错误允许重试。

## 5. 相关文档

* 指令怎么被注册、解析与分发：[commands-zh_cn.md](../commands/commands-zh_cn.md)（命令层）、
  [architecture-zh_cn.md](../architecture/architecture-zh_cn.md) 第 3 节（一条指令的完整路径）。
* 每条指令的说明：[commands-zh_cn.md](../commands/commands-zh_cn.md) 第 3 节到第 7 节。
