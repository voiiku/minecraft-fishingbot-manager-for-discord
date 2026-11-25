# runtime：共享运行组件
[English](runtime-en_us.md) | **简体中文**

`src/runtime/` 包含会话、队列、配置、日志和收尾。机器人操作位于 `src/operations/`，权限检查位于 `src/operations/mod.rs`，报告位于 `src/text/report.rs`。
底层名称、命令片段、文件及时长逻辑位于 `src/utils/`，不依赖其他项目模块；配置参数由调用方传入。`runtime::utils` 仅供 runtime 内部使用，同样不依赖文案；`runtime::log_files` 将文件错误转换为本地化文案。`bot_log` 同时负责过滤后的机器人日志和原始日志准备。

## 1. 模块地图

[架构地图](../architecture/architecture-zh_cn.md) 按功能列出源码位置，下文介绍公共工具的行为。

进程级可变状态集中声明在 `src/lib.rs`，其初始化和业务规则仍由所属模块负责：

* `CONFIG`、`STARTUP_COMPLETE`：配置和首次初始化完成标记；语言保存在配置中。
* `PROCESS_LOGGER`、`STARTUP_QUEUE`、`WATCHDOG`：日志器、启动队列和看门狗。
* `ANTI_AFK`、`SCREEN_COMMAND_LOCKS`、`COMMAND_SYNC_LOCK`：反 AFK 任务、screen 指令锁和命令同步锁。

共八个声明；不含不可变语言对象、测试专用状态或依赖库内部状态，也不把容器内的每个锁单独计数。

## 2. `bot_dirs`：Token 状态与目录范围

* `BotTokenStatus::Ready` 与 `BotTokenStatus::Missing`：只判断 Token 文件有内容还是没内容，不联网校验
  有效性，因为有效性只有 FishingBot 自己启动时才知道。
* `BotDirEntry` 是目录条目的结构体：归属 Discord 用户 ID、机器人名、文件夹路径，以及构造时取到的
  Token 状态。路径不用传（由 `bots_dir/Discord 用户 ID/名字` 拼出来），两条构造入口表达归属从哪来：
  `BotDirEntry::new(原始名字, 归属 Discord 用户 ID)` 与 `BotDirEntry::find_by_name(原始名字)`（按名字
  在全服用户目录里找条目，不判权限，权限由业务命令判，见 [lib.md](../lib/lib-zh_cn.md)）。名字在构造里校验一次
  （口径唯一来源仍是 `ToCleanBotName::to_clean_bot_name`），非法名直接 `Err`；第三条内部入口
  `from_dir_name`（磁盘列表用）因此会把管理员手工建进来的非法名目录跳过，它们不出现在列表与报告里。
* 取哪批目录不设计过滤枚举，而是每个范围一个函数（函数名直接说明范围，调用处不必再查枚举含义）：
  * `all_bot_dir_entries()`：全部用户目录（`/startall`、`/restartall`、`/stopall`、进程收尾、
    `/show allbots`）；根目录还不存在时按空列表处理，只有真的读取失败才是错误；
  * `token_ready_bot_dir_entries()`：只看配好 Token 的（`/commandall`、`/show status`）；
  * `bot_dir_entries_of_owner(用户ID)`：只看某个 Discord 用户 ID 名下的（`/show mybots`）。

  上层不再自己拼 `read_dir` 循环（循环本身收进 `fs::list_subdirectory_names`），避免不同模块的过滤
  条件写得不一致。

## 3. `bot_sessions`：会话快照

批量状态分组每次调用 `screen -list` 一次，并使用该快照判断所有目标的会话状态：

* `/show status` 先读取一次机器人目录和 screen 快照，再复用它们生成状态行与排队名单。
  `src/operations/queries/show_status.rs` 按同一列序生成表头和 `status_row`（见
  [commands-zh_cn.md 第 6.2 节](../commands/commands-zh_cn.md#62-show-status)）。
* `split_bot_entries_by_is_running(&[BotDirEntry])` 返回在运行与未运行两组；取哪批目录由调用方决定
  （`all_bot_dir_entries()` 或 `token_ready_bot_dir_entries()`），见
  [commands-zh_cn.md 第 5.2 节](../commands/commands-zh_cn.md#52-名单口径)。
* `token_ready_bot_names_with_screen_sessions()` 返回已配置 Token 且有 screen 会话的机器人名（`/commandall` 用）：
  范围与 `/show status` 一致，包含死残壳，只查一次 `screen -list`，不读日志。

一次快照、多处复用是有意设计：批量流程中若每次都重新探测，前面刚启动的会话会立刻被后面算成已在运行，
统计结果就会前后不一致。

## 4. 启动与停止中的等待

* 所有涉及启动的入口（`/start`、`/startall`、`/restart`、`/restartall`）共用进程级 FIFO 启动队列；调度器长期驻留，空闲时阻塞等待任务。每个命令请求按机器人列表切成批次，每批最多启动 `[startup_queue] max_batch_size` 台；该请求的条目全部出队后才处理下一个 FIFO 请求。批内相邻两台至少隔 `[startup_queue] bot_login_interval`；批与批之间至少隔 `[startup_queue] batch_login_interval`，从上一批出队的时刻算起（复核在命令侧并行进行，不计入这段等待）。队列空闲时未满批次会关闭，但"上一次出队时刻"跨命令保留，连续命令也绕不过限流；进程重启时未执行的任务不持久化，会丢失。`/bot login` 不走这个队列。
* `StartupQueue::start_bots()` 是命令侧唯一入口（建账本 → 入队 → 等这些条目都被拉起 → 取结果并复核）；启动线程负责批内错峰与逐台拉起，复核按每次启动请求的 `check_grace` 观察会话存活（拉起后判定一次，等满宽限期再判定一次）。登录频率是受限项，批内与批间分别由 `bot_login_interval` 与 `batch_login_interval` 控制；批内台数只影响 JVM 启动时的内存峰值，每次启动请求复核只等一次。见 [commands-zh_cn.md 第 5.5 节](../commands/commands-zh_cn.md#55-失败排查)。
* 停止流程的两种等待不接收参数，由流程自己读取配置并执行：先发送退出信号，等待 `stop_grace`，超时后强杀；再等待
  `stop_settle`，让旧会话彻底注销，最后复核（立即复核仍会读到尚未消失的旧会话）。
* 复核统一走 `bot_startup::verify_start_results`：第二遍检查时会话必须仍存在且不是死残壳；进服状态根据日志另行确认。

## 5. `report`：回执格式只写一处

* 字段即小节：`basics`（基础信息与列表行，列表行直接 push）、`succeeded`、`skipped`、`queued`、`failed`
  （明细）、`content`（整块内容）。`render()` 会自动省略没填的部分，所以空节不会留下空标题。
  `succeeded` 显示为“完成明细”，可包含未确认状态；`failed` 显示为“异常明细”，包含失败与超时。
* `set_counts(&[(标签, 数量)])` 覆盖基础信息行，统一生成 `标签: N 个`；非计数行（表头、名单、说明）
  直接往 `basics` 里 push。
* `new(language, max_utf16_units)` 与 `set_title()`：调用者提供语言和长度上限，出来的是全空报告，标题由 `set_title()` 补（有的
  标题要等流程跑完才知道）。
* 报告只负责展示。启动队列返回每台机器人的复核状态或异常，单台停止返回已确认退出或错误原因。
* `render()` 以调用方传入的 `runtime.max_reply_chars` 为截断目标：超长时从末尾按整行删除，并在结尾注明省略了多少行。
  这是有意保留的边界：不会拆分过长的单行；上限太小时标题、计数甚至所有正文都可能被省略，省略说明仍会输出，
  因此结果可能超过配置上限。长度以 UTF-16 码元计数（`.len()` 是字节、`chars()` 是码点）。
* 配置只要求 `max_reply_chars` 大于 0，不限制其最大值，也不会自动调整到 Discord 的 2000 UTF-16 码元限制。
  部署者应设置适当的值；超过 Discord 限制的消息可能发送失败。该截断仅用于通过 `Report::render()` 渲染的报告，
  不保证其他普通回复或错误消息也按此配置截断。

计数要等流程跑完才知道的指令，先跑流程、再用 `set_counts` 整体覆盖，而不是让流程往里追加，否则会同时
出现占位信息与真实结果两套数字。

## 6. 配置取值约定

共享操作只读取真正用到的配置字段，分两类写法：

* 接收参数（值由调用方提供，函数不读取配置）：用于逻辑较多、值得单测的地方，例如 `launch_command_line`
  （启动命令拼接）。
* 只读取所需字段（`config::Config::get().表.字段`）：用于只有一两行逻辑的便利函数。例如 `session_name` 读取
  `runtime.screen_prefix`；`create_bot_log_file` 与 `latest_log_path` 读取 `runtime.bot_logs_dir`；
  `BotTokenStatus::from_dir` 读取 `runtime.token_file`；`BotDirEntry::find_by_name` 读取 `paths.bots_dir`；
  `stop_bot_sessions` 读取 `timing.stop_grace` 与 `timing.stop_settle`。

配置在启动时读取一次，之后常驻内存（见 [config-zh_cn.md 第 1 节](../config/config-zh_cn.md)），因此便利函数读取单个
字段没有额外成本；接收参数的函数则换来了单测覆盖：启动命令、等待时间等容易出错的地方都有测试（见
`runtime::bot_instances::bot_dirs`、`runtime::bot_startup`、`text::report` 的测试）。

状态查询由 `query_bot_status` 负责，解析函数接收语言表以便分别测试中英文回显；screen 会话仍独立用于进程管理。

回复截断位于 `Report::render`：报告构造时由调用方传入配置的 `runtime.max_reply_chars`，因此报告模块不依赖配置。

连接与钓鱼状态均由 `/status` 的新回显构造 `BotStatus`，不保存在管理程序中；启动、暂停与恢复使用同一查询。
状态不可用时报告异常但保留会话；死会话直接显示进程结束。状态可能在查询后变化。

## 7. 两类日志

### 7.1 机器人日志：只有一份

`<机器人目录>/<runtime.bot_logs_dir>/` 中包含两类文件：

* `年-月-日-时-分-秒.log`：我们的日志。拉起会话前由 `create_bot_log_file` 建好，会话把 JVM 输出经
  `grep -v -E` 过滤后 `tee -a` 追加（命中 `keywords.filter_regex` 的行被丢掉）。`/log` 读它的尾部，
  `/show status` 检查请求之后新增的状态回显。
* `[raw_game_logs] dir_name` 下的 `file_name`（默认 `raw/log0.log`）：FishingBot 的日志，一个指向
  `discard_path`（默认 `/dev/null`）的符号链接，写进去的内容由内核丢弃（每次拉起会话前整个目录清空
  重建，所以磁盘上只留过滤后的那一份）。名字与丢弃目标都在 `[raw_game_logs]` 里配。

`/status` 查询只检查请求发送后的新增日志，最多扫描末尾 100 行；`/log` 的 `read_last_lines` 也只读取文件尾部。

为何仍为 FishingBot 保留日志文件：它无法创建日志文件时会直接拒绝启动（`Bot.java` 中的 `FileHandler` 抛出
`IOException` 即 `setRunning(false)`），因此只能提供一个可写但不保存内容的目标。磁盘上因此只有一份真正的
日志，也不会出现两份日志互相干扰。

选择日志文件的规则只在 `bot_log::latest_log_path` 一处：先按内置时间戳格式解析并过滤文件名
（由 `create_timestamped_log_file` 生成），再选择时间戳最大的文件。默认格式是定宽零填充的
`年-月-日-时-分-秒`，选中的是最后启动的那一轮，而不是最后被写入的文件（后者会被仍在刷屏的旧会话带偏）。
两步都不可省略：早期版本缺少文件名过滤，而当时 FishingBot 的 `log0.log` 就在同一层且持续刷屏，于是
“最新”总是指向它，`/log` 读回了未过滤的刷屏行；此外，`log9.log` 这类名字在字典序中排在 `2026-…` 之前
（`l` 大于 `2`），因此该过滤仍然必要。

### 7.2 程序日志：终端与文件各一份

`src/runtime/process_log/mod.rs` 装的 logger（`ProcessLog::get_or_init`，进程入口在装配框架之前调用）把框架与库里的
`log::*` 同时写两处：

* 终端（stderr）：`journalctl -u minecraft-fishingbot-manager-for-discord`，或前台输出。用途是实时看，
  systemd 托管时也能追溯。
* 文件：`<部署目录>/logs/年-月-日-时-分-秒.log`。用途是事后翻查（`journal` 会轮转）。

细节：

* 命名与机器人日志同一套（都出自 `create_timestamped_log_file`），每条记录形如
  `[2026-09-28 15:30:12] [INFO] …`，纯文本、不带颜色，所以文件干净。
* 级别只看配置的 `[log] debug`（`false` 时本项目 `info` 起、框架 `warn` 起；`true` 时两边都 `debug`），
  没有环境变量、也没有第二个开关。框架自带的过滤支持 `模块=级别` 这类 directive，本项目刻意只保留一个
  级别的口径。
* 本项目自身的诊断信息也通过 `log` 输出（不再使用 `println!`），因此终端与日志文件内容一致。唯一例外是
  logger 安装失败时的提示（此时日志系统尚不可用，只能直接写终端）。
* 日志文件无法创建（部署目录不可写）不影响运行：仅在终端输出一条警告，不会阻止机器人启动。

## 8. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 谁在调用这些原语：[lib-zh_cn.md](../lib/lib-zh_cn.md)（指令入口层）与 [architecture-zh_cn.md](../architecture/architecture-zh_cn.md) 第 1 节（分层）。
* 两条日志线的完整口径：[process-log-zh_cn.md](../process-log/process-log-zh_cn.md)。
