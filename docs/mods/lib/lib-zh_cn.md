# lib：指令入口层（路由层）
[English](lib-en_us.md) | **简体中文**

* 代码：`src/lib.rs`（指令入口）；autocomplete 的候选查询 `bot_name_candidate_states(caller_id)` 在命令层，实现在 `src/commands/autocomplete.rs`
* 定位：把聊天指令翻译成动作函数的那一层（架构文档里叫指令入口层，简称路由层，见
  [architecture.md](../architecture/architecture-zh_cn.md) 第 1 节的术语说明），并统一处理错误文案与回复格式。
* 上层是命令层（解析文本、数参数个数），下层是 `bot_control`、`bot_manage`、`show`、`all_bots_control`
  （干活）。
* 权限全部在本层判一次（`has_permission`），底层被调用的函数不校验：每条指令都声明一个要求
  （`utils::RequiredPermission`，见第 3 节）；单机器人指令的要求是目标机器人归调用者或调用者是管理员，
  归属要先解析出目标条目才知道，所以本层用 `BotDirEntry::find_by_name` 把条目找出来、判过之后再交给
  业务函数。
* 报告由调用方拼装：utils 只返回原始明细（例如停止给的是 `StopOutcome`，它自带 `into_report()` 与
  `to_failure_text()`），`into_report()` 这一步必须在调用方显式调用，utils 不会替你返回拼好的 `Report`。
  批量停止由 `all_bots_control::stopall::stop_all_bots()` 显式拼，进程收尾走同一份拼装的
  `stop_all_bots_for_shutdown()`（只有标题不同）。
* 可见性，每个模块一个对外面：crate 的对外面只有 `lib.rs`（`pub fn handle_*`、`pub use` 的两个视图
  类型 `BotLogView` 与 `FishingbotConfigView`、`pub mod config`、`pub mod text`）；下层的五个业务模块与
  `utils` 是同一形状，子模块是私有 `mod`，父模块顶部一份精选的 `pub(crate) use` 就是它的面。于是
  `lib.rs` 只认识 `crate::bot_control::start_bot` 这种路径，而"某个函数能不能从外面调"只有一个答案：
  看那个父模块顶部的清单（`utils.rs`、`bot_control.rs`、`bot_manage.rs`、`general.rs`、`show.rs`、
  `all_bots_control.rs` 六份）。
* 参数也不在这一层清洗：命令层只切分、原样透传（动作枚举里带着 `raw_*`），谁真正要用就由谁清洗一次
  （机器人名在 `utils::ToCleanBotName::to_clean_bot_name`，指令片段在 `utils::ToCleanBotCommand`）。

## 1. 命名约定

* `handle_*`：入口。接指令、分发动作、把错误转成给用户看的文案（全员指令与批量指令的权限在这里查）。
* `bot_*_reply`：把一个动作枚举与原始参数收敛成回执文本的中间层，定义在命令层，不在本层。
* `*Action` 枚举：指令的动作维度，是解析产物（参数由命令层原样透传），只在这里被解释。

四个动作枚举与对应入口：

* `BotControlAction` 由 `handle_bot_control` 处理，覆盖 `/start`、`/stop`、`/restart`、`/pause`、
  `/unpause`、`/command`。
* `BotManageAction` 由 `handle_bot_manage` 处理，覆盖 `/bot add`、`/bot del`、`/bot rename`、`/token`。
* `/bot login` 不走动作枚举，它是**两段式**的两个入口：`handle_bot_login`（第一段：判权限、拉起会话、等
  设备码）与 `handle_bot_login_await`（第二段：等凭据落盘或超时）。为什么只有它拆成两段，见
  [commands-zh_cn.md 第 4.6 节](../commands/commands-zh_cn.md#46-bot-login)。
* `ShowAction` 由 `handle_show` 处理，覆盖 `/show mybots|allbots|admins`；`/show botconfig` 不走动作枚举
  （它返回的是附件而不是文本），走 `handle_fishingbot_config`。
* `AllBotsControlAction` 由 `handle_all_bots_control` 处理，覆盖 `/startall`、`/stopall`、
  `/restartall`、`/commandall`。

另一个入口不是指令执行：`bot_name_candidate_states(caller_id)` 返回调用者有权查看的机器人名及运行状态；缺少机器人目录时是空列表，明确的目录读取错误以 `Err` 返回。具体的 autocomplete 功能属于命令层，实现在 `src/commands/autocomplete.rs`：`BotNameScope`、前缀筛选、状态标签及候选数量上限都由这里管理，再转为 Discord 响应。它只提供建议，不执行机器人操作；各指令用哪种 scope 见 [commands-zh_cn.md](../commands/commands-zh_cn.md) 的"指令选项的动态候选"一节。

## 2. 两种错误口径

* 权限不足有两种来源，文案都由产生它的那一层给：
  * 批量指令：路由层 `handle_all_bots_control` 直接返回一句
    `Discord 用户 ID X 不是管理员，无权执行此操作！`（不进入业务函数，也不经 `Report`）；
  * 单机器人或账号管理指令：本层 `permitted_bot_dir_entry` 返回 `Err`，文案用的是
    `BotDirEntry::find_by_name` 里那句 `未找到机器人 'X' 对应的目录，或你没有权限访问`（刻意不区分
    不存在与没权限，见 [commands-zh_cn.md 第 4 节](../commands/commands-zh_cn.md)）。
* 参数或业务错误：业务函数返回 `Result<String, String>`，`Err` 里是给用户看的文案；入口层只负责把它当
  回复返回，不改写措辞（避免同一件事出现两种说法）。

## 3. 权限规则

每条指令声明一个要求，校验一律走 `has_permission(caller_id, 要求)`：

* `Everyone`：`/help`、`/whoami`、`/show status`、`/ping`、`/checkserver`、`/show *`（含
  `/show botconfig`），由本层的各 `handle_*` 判一次。
* `MustBeAdmin`：`/startall`、`/stopall`、`/restartall`、`/commandall`，由本层的
  `handle_all_bots_control` 判一次（所以 `all_bots_control/*` 一律不带 `caller_id`）。
* `OwnerOrAdmin(归属用户ID)`：单机器人指令（`/start`、`/stop`、`/log` 等）与账号管理指令
  （`/bot del`、`/bot rename`、`/token`），由本层 `permitted_bot_dir_entry` 判一次（先 `find_by_name`
  解析条目，再按归属判）。

另外三条：

* 普通用户只能碰自己用户目录里的机器人；管理员（`[admins]`）一律放行。
* 找不到与没权限回同一句话（刻意不区分，见 [commands-zh_cn.md 第 4 节](../commands/commands-zh_cn.md)）。
* `/bot add` 建在调用者自己的用户目录下，归属由路径决定，所以要求就是 `Everyone`。

## 4. 进程级生命周期

进程级生命周期在 `src/watchdog/lifecycle.rs`，是 `Watchdog` 对象上的方法（路由层只是替 `main.rs` 再导出）：

* `WATCHDOG.get_or_init()`：取进程内唯一的那一份看门狗（首次调用按配置 `[timing]` 建，之后拿到的都是它；
  进程入口与事件回调都走这一行）。要在 `config::get_or_init()` 之后调：首次调用会读配置。
  `Watchdog::new(...)` 是显式给时序的构造，仅供测试。
* `watchdog.shutdown_all_bots()`：停止所有运行中的机器人会话（不退出进程）。
* `watchdog.exit_on_shutdown_signal()`：收到 SIGTERM / Ctrl+C 后的收尾（入口收到信号后调它）：记日志、停全部
  机器人、以退出码 0 结束进程（systemd 不会重拉）。
* `watchdog.exit_on_permanent_failure(reason)`：停止全部机器人并以退出码 1 退出进程（`reason` 会写进本进程
  日志）。只给配置写错用（Token 无效、机器人没被邀请进服务器）：退出码 1 是本项目的主动失败专用码，
  systemd 单元里配 `RestartPreventExitStatus=1` 即可让它不重拉。
* `watchdog.request_reconnect(reason)`：判定掉线的一方（看门狗判定侧、入口的装配错误出口）用它把掉线原因交给
  进程入口并叫醒它：收尾与重建连接都由入口做，不退出进程（见 [watchdog.md 第 4 节](../watchdog/watchdog-zh_cn.md)）。
* `watchdog.wait_for_attempt(client)`：入口用它等这一次尝试结束并按结束方式收尾——`client.start()` 返回就调私有的
  `stop_all_bots_on_disconnect()`（停全部机器人 + 记原因与时刻）收尾，看门狗判定掉线就取来判定侧交的原因后同样
  收尾，收到退出信号就退出进程。
* `watchdog.wait_before_reconnect()`：等一个判定窗口再重建连接；期间收到退出信号就直接退出进程。
* `watchdog.notify_reconnect_success(ctx)`：掉线过又回来了（`Ready` / `Resume` 再次到来）就向本机器人所在的
  所有服务器补报一次；没掉线过就什么都不做。

装配失败怎么归类（哪种成因记哪条日志、算不算永久错）在文案层：`text::setup_failure_log_text` 与
`text::failure_is_permanent`（见 [i18n.md](../i18n/i18n-zh_cn.md)）。`main.rs` 的装配错误出口靠它决定记哪条日志、
要不要调 `exit_on_permanent_failure`。

## 5. 相关文档

* 指令怎么被注册、解析与分发：[commands-zh_cn.md](../commands/commands-zh_cn.md)（命令层）、
  [architecture.md](../architecture/architecture-zh_cn.md) 第 3 节（一条指令的完整路径）。
* 每条指令的说明：[commands-zh_cn.md](../commands/commands-zh_cn.md) 第 3 节到第 7 节。
