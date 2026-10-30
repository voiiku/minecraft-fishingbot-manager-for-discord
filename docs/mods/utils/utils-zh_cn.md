# utils：公共工具层
[English](utils-en_us.md) | **简体中文**

* 代码：`src/utils.rs`（只负责模块声明与再导出）与 `src/utils/*.rs`（一个主题一个文件）
* 命名约定为“动作 + 对象”（`raw_bot_name.to_clean_bot_name()`、`reply_text.utf16_len()`、
  `is_bot_name_taken`、`read_last_lines`）；返回 `Result<_, String>` 时，`Err` 一律是可以直接展示给
  用户的文案。
* `utils.rs` 顶部的 `pub use` 清单就是对外 API：业务层只能从这里取用；子模块中其余 `pub` 项仅供 utils
  内部（或流程内部）使用。例如 `bot_status` 基本属于内部实现（`running_bots` 用它判定状态），只向业务层
  暴露 `PauseCommand` 与 `confirm_pause_command`（`BotPauseStatus` 仅供 utils 内部使用，无需导出）。
* utils 只返回原始信息，不拼装最终报告：流程函数返回明细（停止的 `StopOutcome`、启动的
  `LaunchOutcome`、重启的 `RestartOutcome`），它们自带 `into_report()` 与 `to_failure_text()` 等方法，
  但必须由上层显式调用。utils 不返回拼好的 `Report`，见 [lib-zh_cn.md](../lib/lib-zh_cn.md)。

## 1. 模块地图

* `bot_name`：机器人名的规范化与合法性（非法名字报错，不过滤）。入口
  `ToCleanBotName::to_clean_bot_name`（`str` 扩展 trait，`&str` 与 `String` 都能直接调）。
* `bot_command`：游戏内指令的拼接口径（`/command` 与 `/commandall` 共用，只写一处）。入口
  `ToCleanBotCommand::to_clean_bot_command`（`[&str]` 的扩展方法，空指令报错）。
* `bot_dirs`：目录定位、遍历、创建、Token 状态与按范围取目录。入口 `BotDirEntry`（构造入口
  `BotDirEntry::new` 给原始名字与归属 Discord 用户 ID；`BotDirEntry::find_by_name` 只给名字、不判
  权限）、`all_bot_dir_entries`、`token_ready_bot_dir_entries`、`bot_dir_entries_of_owner`、
  `is_bot_name_taken`、`split_bot_entries_by_token_status`、`create_bot_dir`（成功返回条目）、
  `BotTokenStatus`（判据 `BotTokenStatus::from_dir`）、`token_file_path`（Token 文件路径口径）。
  重名检查在目录不存在时返回未占用，读取失败时返回错误，不把查询失败当作未重名。
* `screen`：`screen` 会话的快照、探测与指令注入（注入是不做校验的原语，等待与强杀判定在 `shutdown`）。
  入口 `screen_session_snapshot`（会话名 → `ScreenSession`，含 PID 与"是不是 `(Dead ...)` 残壳"，残壳也
  算在跑，以便 `/stop` 能够结束它）、`is_bot_screen_running`、`session_name`、
  `send_command_to_single_bot`、`send_stop_signal_to_session`、`force_quit_session`。
  会话查询返回 `Result`：GNU screen 明确报告无会话时返回空快照，执行失败或输出异常时返回错误。
* `running_bots`：机器人的状态口径（是否启动看会话、是否排队看队列）。按运行状态分组、运行中的名字、
  `/show status` 的数据行、运行中与排队中拒绝文案。入口 `split_bot_entries_by_is_running`、
  `all_running_bot_names`、`status_row`、`refusal_if_not_stopped`。
  启动、停止、重启及复核会传播查询错误，不将未知状态当作未运行或已停止。
* `startup`：启动。逐台拉起步骤为“查 Token → 建日志 → 准备原始日志目录 → `screen -dmS`”，缺 Token 的目标不拉起；
  命令方入口是 `StartupQueue::start_bots()`（建账本 → `add_bots_to_queue` 入队 → 等自己的条目都被拉起 → 取结果并
  复核 → `BotStartResults`，它自带 `into_report()` 与 `to_reply_text()`），`/start`、`/startall`、`/restart`
  与 `/restartall` 共用它；`/bot login` 单独走 `StartupQueue::add_bot_for_login`（不入队、不复核）；拉起步骤本身
  （`launch_bot_session`、`launch_command_line`）是模块内部实现。命令层读取队列的入口还有 `pending_bot_count()`（当前排队数量）、
  `slowest_start_duration(台数)`（按节奏计算的耗时预估，`/startall` 的第一条回执使用）与 `result_timeout()`
  （等待结果的兜底超时）。“停止全部机器人”的第一步是 `StartupQueue::request_stop()`：
  启动线程不再取新批，正在拉起的那一批将剩余条目记为 `BotStartOutcome::Cancelled`（不算失败，计入跳过明细），
  因此等待账本的调用方（`/start`、`/startall`）不会卡住。待启动条目本身也有兜底：若没有写回结果便离开作用域
  （清空队列、撤单、启动线程 panic 展开），`QueuedBot::drop` 会按“取消”写回账本；
  `wait_until_done()` 另有兜底超时，由队列按节奏参数计算（`StartupQueue::result_timeout()`，取“队列排满时最慢
  的一次启动 × 2”）；超时后，尚未返回结果的条目按“未确认”收尾，无需配置，也不会随节奏调整而失配。
* `restart`：重启。`/restart` 与 `/restartall` 共用的"先停后起"编排（只返回原始明细
  `RestartOutcome`，含停止与启动两份明细）。入口 `restart_bot_sessions`。
* `shutdown`：停止会话（只返回原始明细 `StopOutcome`，它自带 `into_report()` 与
  `to_failure_text()`）。入口 `stop_single_bot_session`、`stop_all_bot_sessions`；后者的语义是"全停"，
  顺序固定为"请求启动线程别再拉起新的 → 清空等待列表 → 关掉全部会话 → 再扫一遍会话"：最后那遍收掉
  第一次快照之后才出现的会话（启动线程正拉着的那一台拦不住，只能事后收）。
  （`/stopall`、看门狗的掉线收尾与退出收尾都走它。）
* `bot_logs`：机器人日志的创建与挑选（FishingBot 原始日志的处置在 `startup`）。入口
  `create_bot_log_file`、`create_timestamped_log_file`、`target_logs_dir`、`latest_log_path`。
* `fishingbot_config`：游戏侧配置（FishingBot 的 `conf.json`）的生成：先删除旧文件，再以随仓库发布的模板
  `fishingbot-config.example.json` 为基础，覆盖 `[fishingbot]` 中的对应键（`misc.language` 直接写入
  `[i18n] language` 解析出的语言，该取值本身就是 JAR 的常量名）。输出为完整配置，不依赖 JAR 补充字段。
  入口 `regenerate_fishingbot_config`（只在进程启动时调用一次，见
  [config-zh_cn.md 第 3.2 节](../config/config-zh_cn.md#32-游戏侧配置confjson是生成物)）：它只负责读配置，实际写入部分
  `write_fishingbot_config`（路径与取值均由调用方提供）在临时目录中可单测。
* `fs`：文件与目录操作原语，只负责文件操作本身（尾部读行、列子目录名、删除可能不存在的目录）；操作哪个文件、
  失败如何表述由调用方决定。入口 `read_lines_backwards_until`、`read_last_lines`、
  `list_subdirectory_names`、`remove_dir_all_if_exists`。
* `bot_status`：机器人状态。暂停状态保存在进程内（确认到命令回显后才修改），在线状态每次读取时现取；
  `/pause` 与 `/unpause` 的确认回显也在这里。入口是条目上的 `BotDirEntry::connection_status()`（每次
  现读；三种读取失败均返回错误；只给出“在线/重连/初始中”）、`ConnectionStatus`（四种状态，`from_line` 负责
  单行判定；其中 `Stopped`“进程已结束”由 `running_bots` 根据会话残壳给出，不来自日志）、
  `BotPauseStatusMemory`（按机器人名保存：`to_bot_pause_status`、`update_bot_pause_status`、
  `forget_bot_pause_status`）、`BotPauseStatus`（`pause_status`、`set_paused`、`set_fishing`）、
  `PauseCommand`（`SetPause` 与 `SetUnpause`，各带指令名、回显关键词 `echo_keyword`（从配置
  `[command_echo]` 取）与要修改的状态）、`confirm_pause_command`。
* `uptime`：运行时长。读取进程已运行秒数并格式化。入口 `process_elapsed_seconds`、`format_uptime`。
* `reply`：回复的组装与渲染，包括报告容器与长度截断。入口 `Report::new`（唯一构造入口，初始为空）、
  `set_title`、`set_counts`、`render`，以及 `IntoReplyText`。
* `permission`：权限判定的唯一出口。每条指令声明一个要求，入口
  `has_permission(caller_id, RequiredPermission)`，要求有 `Everyone`、`MustBeAdmin`、
  `OwnerOrAdmin(归属用户ID)`。

## 2. `bot_dirs`：Token 状态与目录范围

* `BotTokenStatus::Ready` 与 `BotTokenStatus::Missing`：只判断 Token 文件有内容还是没内容，不联网校验
  有效性，因为有效性只有 FishingBot 自己启动时才知道。
* `BotDirEntry` 是目录条目的结构体：归属 Discord 用户 ID、机器人名、文件夹路径，以及构造时取到的
  Token 状态。路径不用传（由 `bots_dir/Discord 用户 ID/名字` 拼出来），两条构造入口表达归属从哪来：
  `BotDirEntry::new(原始名字, 归属 Discord 用户 ID)` 与 `BotDirEntry::find_by_name(原始名字)`（按名字
  在全服用户目录里找条目，不判权限，权限由路由层判，见 [lib.md](../lib/lib-zh_cn.md)）。名字在构造里校验一次
  （口径唯一来源仍是 `ToCleanBotName::to_clean_bot_name`），非法名直接 `Err`；第三条内部入口
  `from_dir_name`（磁盘列表用）因此会把管理员手工建进来的非法名目录跳过，它们不出现在列表与报告里。
* 取哪批目录不设计过滤枚举，而是每个范围一个函数（函数名直接说明范围，调用处不必再查枚举含义）：
  * `all_bot_dir_entries()`：全部用户目录（`/startall`、`/restartall`、`/stopall`、进程收尾、
    `/show allbots`）；根目录还不存在时按空列表处理，只有真的读取失败才是错误；
  * `token_ready_bot_dir_entries()`：只看配好 Token 的（`/commandall`、`/show status`）；
  * `bot_dir_entries_of_owner(用户ID)`：只看某个 Discord 用户 ID 名下的（`/show mybots`）。

  上层不再自己拼 `read_dir` 循环（循环本身收进 `fs::list_subdirectory_names`），避免不同模块的过滤
  条件写得不一致。

## 3. `running_bots`：运行状态的唯一来源

`screen -list` 只调用一次并缓存成快照，之后所有在不在运行的判断都基于同一份快照。目录与会话的连接
（join）只在本模块做，别处不要再遍历目录去匹配会话名：

* `/show status` 先读取一次机器人目录和 screen 快照，再复用它们生成状态行与排队名单。`status_row` 在本模块逐台
  算完四格并拼成一行文本；表头由 `show/status.rs` 自己拼，两处列序要对齐（见
  [commands-zh_cn.md 第 6.2 节](../commands/commands-zh_cn.md#62-show-status)）。
* `split_bot_entries_by_is_running(&[BotDirEntry])` 返回在运行与未运行两组；取哪批目录由调用方决定
  （`all_bot_dir_entries()` 或 `token_ready_bot_dir_entries()`），见
  [commands-zh_cn.md 第 5.2 节](../commands/commands-zh_cn.md#52-名单口径)。
* `all_running_bot_names()` 返回所有在跑的机器人的名字（`/commandall` 用）：范围与 `/show status`
  一致（所有配好 Token 的），只查一次 `screen -list`，不读日志。

一次快照、多处复用是有意设计：批量流程中若每次都重新探测，前面刚启动的会话会立刻被后面算成已在运行，
统计结果就会前后不一致。

## 4. 启动与停止中的等待

* 所有涉及启动的入口（`/start`、`/startall`、`/restart`、`/restartall`）共用进程级 FIFO 启动队列；调度器长期驻留，空闲时阻塞等待任务。每个命令请求按机器人列表切成批次，每批最多启动 `[startup_queue] max_batch_size` 台；该请求的条目全部出队后才处理下一个 FIFO 请求。批内相邻两台至少隔 `[startup_queue] bot_login_interval`；批与批之间至少隔 `[startup_queue] batch_login_interval`，从上一批出队的时刻算起（复核在命令侧并行进行，不计入这段等待）。队列空闲时未满批次会关闭，但"上一次出队时刻"跨命令保留，连续命令也绕不过限流；进程重启时未执行的任务不持久化，会丢失。`/bot login` 不走这个队列。
* `StartupQueue::start_bots()` 是命令侧唯一入口（建账本 → 入队 → 等这些条目都被拉起 → 取结果并复核）；启动线程负责批内错峰与逐台拉起，复核按每次启动请求的 `check_grace` 观察会话存活（拉起后判定一次，等满宽限期再判定一次）。登录频率是受限项，批内与批间分别由 `bot_login_interval` 与 `batch_login_interval` 控制；批内台数只影响 JVM 启动时的内存峰值，每次启动请求复核只等一次。见 [commands-zh_cn.md 第 5.5 节](../commands/commands-zh_cn.md#55-失败排查)。
* 停止流程的两种等待不接收参数，由流程自己读取配置并执行：先发送退出信号，等待 `stop_grace`，超时后强杀；再等待
  `stop_settle`，让旧会话彻底注销，最后复核（立即复核仍会读到尚未消失的旧会话）。
* 复核统一走 `startup::verify_batch`，所以"拉起成功"的定义是第二遍复核后会话仍在（`screen -list` 快照），而不是命令已发出；但会话出现也不等于已经上线（登录、进服由 JVM 自己跑），报告与回复因此只写"启动命令已发送，会话已出现"。

## 5. `reply`：回执格式只写一处

* 字段即小节：`basics`（基础信息与列表行，列表行直接 push）、`succeeded`、`skipped`、`queued`、`failed`
  （明细）、`content`（整块内容）。`render()` 会自动省略没填的部分，所以空节不会留下空标题。
* `set_counts(&[(标签, 数量)])` 覆盖基础信息行，统一生成 `标签: N 个`；非计数行（表头、名单、说明）
  直接往 `basics` 里 push。
* `new()` 与 `set_title()`：构造只有这一条路（出来的是全空报告），标题一律由 `set_title()` 补（有的
  标题要等流程跑完才知道）。
* 成败判定与一行错误提示不在这里，那是 utils 返回的明细自己的方法（`LaunchOutcome`、
  `RestartOutcome`、`StopOutcome` 的 `has_failed_lines()` 与 `to_failure_text()`）。
* `render()` 会按 `runtime.max_reply_chars` 截断长度：超长时从末尾按行删除，并在结尾注明省略了多少行。
  Discord 对单条消息有 2000 个 UTF-16 码元的硬上限，超长会直接发送失败；应将配置上限设为不超过 2000。
  按行截断优先保留开头，但上限太小时无法保证保留标题或计数，省略说明本身也可能超出配置上限。
  长度口径是 `str::utf16_len()`（UTF-16 码元数；`.len()` 是字节、`chars()` 是码点）。

计数要等流程跑完才知道的指令，先跑流程、再用 `set_counts` 整体覆盖，而不是让流程往里追加，否则会同时
出现占位信息与真实结果两套数字。

## 6. 配置取值约定

`utils` 中没有“接收整份配置再自行挑字段”的函数，每个函数只读取真正用到的一两个字段，分两类写法：

* 接收参数（值由调用方提供，函数不读取配置）：用于逻辑较多、值得单测的地方，例如 `launch_command_line`
  （启动命令拼接）与 `target_logs_dir`（纯字符串与路径拼接）。
* 只读取所需字段（`config::Config::get().表.字段`）：用于只有一两行逻辑的便利函数。例如 `session_name` 读取
  `runtime.screen_prefix`；`create_bot_log_file` 与 `latest_log_path` 读取 `runtime.bot_logs_dir`；
  `BotTokenStatus::from_dir` 读取 `runtime.token_file`；`BotDirEntry::find_by_name` 读取 `paths.bots_dir`；
  `stop_bot_sessions` 读取 `timing.stop_grace` 与 `timing.stop_settle`；`Report::render`
  读取 `runtime.max_reply_chars`。

配置在启动时读取一次，之后常驻内存（见 [config-zh_cn.md 第 1 节](../config/config-zh_cn.md)），因此便利函数读取单个
字段没有额外成本；接收参数的函数则换来了单测覆盖：启动命令、等待时间等容易出错的地方都有测试（见
`utils::bot_dirs`、`utils::startup`、`utils::reply` 的测试）。

例外是状态判定 `BotDirEntry::connection_status()`：它逻辑较多，但由自己读取关键词，一半来自配置
`[keywords]`（各部署不同、需要手工填写的两项），一半来自语言表 `text` 的 `jar_*` 方法（JAR 打印的
断线重连提示，按 `[i18n] language` 取）。两者都是全局固定的一份，没有必要让每个调用方传入，所以挂在
条目上（挑选最新日志这一步也在方法内部完成，调用方无需自行拼装）。代价是无法在单测中注入关键词，因此测试
落在接收参数的实现（`connection_status_of_log(bot_name, log_path)`，单行判定由
`ConnectionStatus::from_line` 负责），而“往回读、读到结论即停”的部分由 `fs` 的测试
（`read_lines_backwards_until`）覆盖。它不产出“进程已结束”：该状态读取会话残壳，由 `running_bots`
直接给出（见 [`super::screen`] 的 `ScreenSession`）。

同样的取舍也出现在回复截断（位于 `Report::render`）：它自行读取 `runtime.max_reply_chars`，上限只能来自配置，
因此该截断逻辑没有单测（要用测试覆盖，就需要把上限做成参数，增加一层抽象）。

`bot_status` 保存一份进程内状态（`BotPauseStatus`：按机器人名保存，`update_bot_pause_status` 是唯一
写入口）。只有暂停状态保存在结构体中，在线状态不保存：

* 暂停状态由本项目自己发送 `/pause` 与 `/unpause` 得到，保存在进程内（初始为“钓鱼中”），确认到 JAR 回显
  后才修改（`BotPauseStatus::set_paused` 与 `set_fishing`）；读取它（`pause_status`）即读取该状态。同一台
  机器人的 `/pause` 与 `/unpause` 几乎同时到达时，状态按回显窗口中的最后一条记录
  （`PauseCommand::last_confirmed_pause_command`），而不是由最后写入的一方决定；本次命令是否生效也按同一口径
  判断：若最新回显属于另一条命令，调用方按失败回复，并报告当前状态。
* 在线状态只能由日志确认（只有 JVM 知道是否连接成功、是否掉线），保存的副本会立即过期，因此不跨调用
  保存：`bot_dir_entry.connection_status()` 每次调用都重新读取，调用者总是拿到最新结果
  （`/show status` 中的该单元格只是本次渲染使用的字符串，渲染后即丢弃）。

这样 `/show status`（高频命令）不必为了查找一个“这个会话从未出现过”的 pause 关键词而扫描到文件开头。
会话启动与结束时都会清除这份状态（`forget_bot_pause_status`），因此它始终只描述当前会话。

## 7. 两类日志

### 7.1 机器人日志：只有一份

`<机器人目录>/<runtime.bot_logs_dir>/` 中包含两类文件：

* `年-月-日-时-分-秒.log`：我们的日志。拉起会话前由 `create_bot_log_file` 建好，会话把 JVM 输出经
  `grep -v -E` 过滤后 `tee -a` 追加（命中 `keywords.filter_regex` 的行被丢掉）。`/log` 读它的尾部，
  `/show status` 从末尾往回扫它判状态。
* `[raw_game_logs] dir_name` 下的 `file_name`（默认 `raw/log0.log`）：FishingBot 的日志，一个指向
  `discard_path`（默认 `/dev/null`）的符号链接，写进去的内容由内核丢弃（每次拉起会话前整个目录清空
  重建，所以磁盘上只留过滤后的那一份）。名字与丢弃目标都在 `[raw_game_logs]` 里配。

`/show status` 需要倒序扫描（`fs::read_lines_backwards_until` 加 `BotDirEntry::connection_status`）：连接
状态只取决于最后一次出现的关键词，因此从最新一行往回读取、得到结论即停止。长跑账号的日志可能有几十 MB，整份读取
既慢又占内存。暂停状态不扫描日志，而是保存在进程内（见上一节），从而省去了为了找不到 pause 关键词而
一路扫描到文件开头的过程（实测 14 MiB 日志耗时 72 ms，每个机器人每次）。`/log` 使用的 `read_last_lines` 是同一
实现的薄封装（数够 N 行即停止），因此也只读取文件尾部，不会为了显示最后 100 行而流式读取整份日志。
两种读取方式都是通用文件原语，位于 `utils::fs`。

为何仍为 FishingBot 保留日志文件：它无法创建日志文件时会直接拒绝启动（`Bot.java` 中的 `FileHandler` 抛出
`IOException` 即 `setRunning(false)`），因此只能提供一个可写但不保存内容的目标。磁盘上因此只有一份真正的
日志，也不会出现两份日志互相干扰。

选择日志文件的规则只在 `bot_logs::latest_log_path` 一处，分两步：先只接受时间戳命名的文件（`is_bot_log_file`，
文件名由 `create_timestamped_log_file` 生成），再按文件名取最大值。文件名是定宽零填充的 `年-月-日-时-分-秒`，
字典序即时间序，因此选中的是最后启动的那一轮，而不是最后被写入的文件（后者会被仍在刷屏的旧会话带偏）。
两步都不可省略：早期版本缺少文件名过滤，而当时 FishingBot 的 `log0.log` 就在同一层且持续刷屏，于是
“最新”总是指向它，`/log` 读回了未过滤的刷屏行；此外，`log9.log` 这类名字在字典序中排在 `2026-…` 之前
（`l` 大于 `2`），因此该过滤仍然必要。

### 7.2 程序日志：终端与文件各一份

`src/process_log.rs` 装的 logger（`ProcessLog::get_or_init`，进程入口在装配框架之前调用）把框架与库里的
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
