# utils：公共工具层
[English](utils-en_us.md) | **简体中文**

* 代码：`src/utils.rs`（只管模块声明与再导出）与 `src/utils/*.rs`（一个主题一个文件）
* 命名约定是动作加对象（`raw_bot_name.to_clean_bot_name()`、`reply_text.utf16_len()`、
  `is_bot_name_taken`、`read_last_lines`）；返回 `Result<_, String>` 的地方，`Err` 一律是可以直接给
  用户看的文案。
* `utils.rs` 顶部那份 `pub use` 清单就是对外 API：业务层只能从这里拿东西；子模块里其余的 `pub` 项只
  服务 utils 内部（或只给流程内部用）。比如 `bot_status` 基本是内部实现细节（`running_bots` 拿它判定
  状态），只对外给业务层两样：`PauseCommand` 与 `confirm_pause_command`（`BotPauseStatus` 只服务
  utils 内部，不必导出）。
* utils 只返回原始信息，不拼最终报告：流程函数给的是明细（停止的 `StopOutcome`、启动的
  `LaunchOutcome`、重启的 `RestartOutcome`），它们自带 `into_report()` 与 `to_failure_text()` 这类
  方法，但要由上层显式调用。utils 不会替你返回拼好的 `Report`，见 [lib.md](../lib/lib-zh_cn.md)。

## 1. 模块地图

* `bot_name`：机器人名的规范化与合法性（非法名字报错，不过滤）。入口
  `ToCleanBotName::to_clean_bot_name`（`str` 扩展 trait，`&str` 与 `String` 都能直接调）。
* `bot_command`：游戏内指令的拼接口径（`/command` 与 `/commandall` 共用，只写一处）。入口
  `ToCleanBotCommand::to_clean_bot_command`（`[&str]` 的扩展方法，空指令报错）。
* `bot_dirs`：目录定位、遍历、创建、Token 状态与"取哪批目录"。入口 `BotDirEntry`（构造入口
  `BotDirEntry::new` 给原始名字与归属 Discord 用户 ID；`BotDirEntry::find_by_name` 只给名字、不判
  权限）、`all_bot_dir_entries`、`token_ready_bot_dir_entries`、`bot_dir_entries_of_owner`、
  `is_bot_name_taken`、`split_bot_entries_by_token_status`、`create_bot_dir`（成功返回条目）、
  `BotTokenStatus`（判据 `BotTokenStatus::from_dir`）、`token_file_path`（Token 文件路径口径）。
* `screen`：`screen` 会话的快照、探测与指令注入（注入是不做校验的原语，等待与强杀判定在 `shutdown`）。
  入口 `active_screen_sessions`（会话名 → `ScreenSession`，含 PID 与"是不是 `(Dead ...)` 残壳"，残壳也
  算在跑，好让 `/stop` 能清掉它）、`is_bot_screen_running`、`session_name`、
  `send_command_to_single_bot`、`send_stop_signal_to_session`、`force_quit_session`。
* `running_bots`：谁在跑的唯一口径。按运行状态分组、运行中的名字、在跑机器人的状态快照（含 `/show status`
  的表头与数据行）、运行中拒绝文案。入口 `split_bot_entries_by_is_running`、`all_running_bot_names`、
  `all_running_bot_infos`、`RunningBotInfo`（`into_row`）、`status_table_header`、`refusal_if_running`。
* `startup`：启动（四步显式流程；缺 Token 的目标不拉起；只返回原始明细 `LaunchOutcome`，它自带
  `into_report()` 与 `to_failure_text()`）。入口 `start_bot_sessions` 与 `prepare_and_launch_bot`
  （`/bot login` 用）；拉起步骤本身（`launch_bot_session`、`launch_command_line` 与 `LaunchCommandConfig`）是模块内部实现。
* `restart`：重启。`/restart` 与 `/restartall` 共用的"先停后起"编排（只返回原始明细
  `RestartOutcome`，含停止与启动两份明细）。入口 `restart_bot_sessions`。
* `shutdown`：停止会话（只返回原始明细 `StopOutcome`，它自带 `into_report()` 与
  `to_failure_text()`）。入口 `stop_single_bot_session`、`stop_all_bot_sessions`。
* `bot_log_files`：机器人日志的创建与挑选（FishingBot 原始日志的处置在 `startup`）。入口
  `create_bot_log_file`、`create_timestamped_log_file`、`target_logs_dir`、`latest_log_path`。
* `fishingbot_config`：游戏侧配置（FishingBot 的 `conf.json`）的生成：先删旧文件，再用随仓库发布的模板
  `fishingbot-config.example.json` 当底、覆盖 `[fishingbot]` 那几个键（`misc.language` 直接写
  `[i18n] language` 解析出来的语言，它的取值本来就是 jar 的常量名）。写出来的是完整配置，不依赖 jar 补字段。
  入口 `regenerate_fishingbot_config`（只在进程启动时调一次，见
  [config.md 第 3.2 节](../config/config-zh_cn.md#32-游戏侧配置confjson是生成物)）：它只读配置，干活那半
  `write_fishingbot_config`（路径与取值都由调用方给全）在临时目录里可单测。
* `fs`：文件与目录操作原语，只回答怎么碰文件（尾部读行、列子目录名、删一个可能不存在的目录）；碰哪个
  与失败怎么说都归调用方。入口 `read_lines_backwards_until`、`read_last_lines`、
  `list_subdirectory_names`、`remove_dir_all_if_exists`。
* `bot_status`：机器人状态。暂停状态在进程内记着（确认到命令回显才改），在线状态读一次就现取一次；
  `/pause` 与 `/unpause` 的确认回显也在这里。入口是条目上的 `BotDirEntry::connection_status()`（每次
  现读；三种读不到都报错；只给"在线/重连/初始中"）、`ConnectionStatus`（四种正常状态，`from_line` 是
  单行判定；其中的"进程已结束"由 `running_bots` 按会话残壳给出，不来自日志）、
  `BotPauseStatusMemory`（挂在机器人名上：`to_bot_pause_status`、`update_bot_pause_status`、
  `forget_bot_pause_status`）、`BotPauseStatus`（`pause_status`、`set_paused`、`set_fishing`）、
  `PauseCommand`（`SetPause` 与 `SetUnpause`，各带指令名、回显关键词 `echo_keyword`（从配置
  `[command_echo]` 取）与要改的状态）、`confirm_pause_command`。
* `uptime`：运行时长。读进程已运行秒数并格式化。入口 `process_elapsed_seconds`、`format_uptime`。
* `reply`：回复的组装与渲染，即报告容器与长度收口。入口 `Report::new`（唯一构造口，出来全空）、
  `set_title`、`set_counts`、`render`，以及 `IntoReplyText`。
* `permission`：权限判定的唯一出口。每条指令声明一个要求，入口
  `has_permission(caller_id, RequiredPermission)`，要求有 `Everyone`、`MustBeAdmin`、
  `OwnerOrAdmin(归属用户ID)`。

## 2. `bot_dirs`：Token 状态与取哪批目录

* `BotTokenStatus::Ready` 与 `BotTokenStatus::Missing`：只判断 Token 文件有内容还是没内容，不联网校验
  有效性，因为有效性只有 FishingBot 自己启动时才知道。
* `BotDirEntry` 是目录条目的结构体：归属 Discord 用户 ID、机器人名、文件夹路径，以及构造时取到的
  Token 状态。路径不用传（由 `bots_dir/Discord 用户 ID/名字` 拼出来），两条构造入口表达归属从哪来：
  `BotDirEntry::new(原始名字, 归属 Discord 用户 ID)` 与 `BotDirEntry::find_by_name(原始名字)`（按名字
  在全服用户目录里找条目，不判权限，权限由路由层判，见 [lib.md](../lib/lib-zh_cn.md)）。名字在构造里校验一次
  （口径唯一来源仍是 `ToCleanBotName::to_clean_bot_name`），非法名直接 `Err`；第三条内部入口
  `from_dir_name`（磁盘列表用）因此会把管理员手工建进来的非法名目录跳过，它们不出现在列表与报告里。
* 取哪批目录不做成过滤枚举，而是一个口径一个函数（名字直接说明范围，调用处不必再查枚举含义）：
  * `all_bot_dir_entries()`：全部用户目录（`/startall`、`/restartall`、`/stopall`、进程收尾、
    `/show allbots`）；根目录还不存在时按空列表处理，只有真的读取失败才是错误；
  * `token_ready_bot_dir_entries()`：只看配好 Token 的（`/commandall`、`/show status`）；
  * `bot_dir_entries_of_owner(用户ID)`：只看某个 Discord 用户 ID 名下的（`/show mybots`）。

  上层不再自己拼 `read_dir` 循环（循环本身收进 `fs::list_subdirectory_names`），避免不同模块的过滤
  条件写得不一致。

## 3. `running_bots`：谁在跑的唯一口径

`screen -list` 只调用一次并缓存成快照，之后所有在不在运行的判断都基于同一份快照。目录与会话的连接
（join）只在本模块做，别处不要再遍历目录去匹配会话名：

* `/show status` 那张表：快照 `RunningBotInfo`（含 `all_running_bot_infos()`）在本模块，表头
  （`status_table_header()`）与数据行（`into_row(self)`）也在本模块。两个紧挨着的四元素数组，列序对着
  看；两个状态都是本次调用的快照，字段私有、只能 `into_row(self)` 消费一次，渲完即丢（见
  [commands-zh_cn.md 第 6.2 节](../commands/commands-zh_cn.md#62-show-status)）。
* `split_bot_entries_by_is_running(&[BotDirEntry])` 返回在运行与未运行两组；取哪批目录由调用方决定
  （`all_bot_dir_entries()` 或 `token_ready_bot_dir_entries()`），见
  [commands-zh_cn.md 第 5.2 节](../commands/commands-zh_cn.md#52-名单口径)。
* `all_running_bot_names()` 返回所有在跑的机器人的名字（`/commandall` 用）：范围与
  `all_running_bot_infos()` 一致（所有配好 Token 的），只查一次 `screen -list`，不读日志。

一次快照、多处复用是有意的：批量流程里若每次都重新探测，前面刚启动的会话会立刻被后面算成已在运行，
统计口径就会漂。

## 4. `startup` / `shutdown` 的等待参数

* 所有涉及启动的入口（`/start`、`/startall`、`/restart`、`/restartall`）共用进程级 FIFO 启动队列；
  调度器长期驻留，空闲时阻塞等待任务。每个命令请求按机器人列表切成批次，每批最多启动
  `[startup_queue] max_batch_size` 台；该请求完成后才处理下一个 FIFO 请求。两次登录之间至少隔
  `[startup_queue] login_interval_ms`：批内相邻两台按它错峰，批间等的时间 = 它减去本批的复核等待
  （`timing.settle_per_bot_ms` × 2 × 本批台数）。队列空闲时未满批次会关闭，但"上一次登录时刻"跨命令保留，
  连续命令也绕不过限流；进程重启时未执行的任务不持久化，会丢失。`/bot login` 不走这个队列。
* `start_bot_sessions` 负责批内启动与错峰，队列调度器负责批间等待，`settle_per_bot_ms` 仍按批次台数计算复核等待
  （两次会话快照间隔相同）。登录频率才是被限制的东西，所以批内与批间用同一个间隔值：批内台数只影响复核窗口的
  长度与 JVM 启动时的内存峰值，不影响总耗时（总耗时 ≈ 台数 × 间隔）。见
  [commands-zh_cn.md 第 5.5 节](../commands/commands-zh_cn.md#55-失败排查)。
* 停止流程的两种等待都不吃参数、由流程自己读配置：`stop_grace_ms` 在 `shutdown::stop_bot_sessions`
  里读（先发信号，等不到才强杀），`stop_settle_ms` 也在 `stop_bot_sessions` 里读（强杀后、复核前的
  注销缓冲）。
* 停止流程先发退出信号，等 `stop_grace_ms` 再不成就强杀，强杀后再等 `stop_settle_ms` 让
  旧会话彻底注销，因为立刻复核会读到还没消失的旧会话。
* 复核统一走 `running_bots`，所以拉起成功的定义是两遍复核后会话仍在（`screen -list` 快照），而不是
  命令已发出；但会话出现也不等于已经上线（登录、进服由 JVM 自己跑），报告与回复因此只写"启动命令已
  发送，会话已出现"。

## 5. `reply`：回执格式只写一处

* 字段即小节：`basics`（基础信息与列表行，列表行直接 push）、`succeeded`、`skipped`、`failed`
  （明细）、`content`（整块内容）。`render()` 会自动省略没填的部分，所以空节不会留下空标题。
* `set_counts(&[(标签, 数量)])` 覆盖基础信息行，统一生成 `标签: N 个`；非计数行（表头、名单、说明）
  直接往 `basics` 里 push。
* `new()` 与 `set_title()`：构造只有这一条路（出来的是全空报告），标题一律由 `set_title()` 补（有的
  标题要等流程跑完才知道）。
* 成败判定与一行错误提示不在这里，那是 utils 返回的明细自己的方法（`LaunchOutcome`、
  `RestartOutcome`、`StopOutcome` 的 `has_failed_lines()` 与 `to_failure_text()`）。
* `render()` 会按 `runtime.max_reply_chars` 收口长度：超长时从末尾按行丢，并在结尾注明省略了多少行。
  Discord 对单条消息有硬上限（2000 字符），超长会直接发送失败，用户那边看起来什么都没发生；按行截断
  则保证标题与计数行一定还在。长度口径是 `str::utf16_len()`（UTF-16 码元数；`.len()` 是字节、
  `chars()` 是码点，都不对）。

计数要等流程跑完才知道的指令，先跑流程、再用 `set_counts` 整体覆盖，而不是让流程往里追加，否则会同时
出现占位信息与真实结果两套数字。

## 6. 配置取值约定

`utils` 里没有"接收整份配置再自己挑字段"的函数，每个函数只碰它真正用到的那一两个字段，分两类写法：

* 吃参数（值由调用方给，函数不碰配置）：用在逻辑重、值得单测的地方。例如 `launch_command_line` 与
  `LaunchCommandConfig`（启动命令拼接）、`target_logs_dir`（纯拼串拼路径）。
* 只读它需要的字段（`config::Config::get().表.字段`）：用在只有一两行逻辑的便利函数。例如 `session_name` 读
  `runtime.screen_prefix`；`create_bot_log_file` 与 `latest_log_path` 读 `runtime.logs_dir`；
  `BotTokenStatus::from_dir` 读 `runtime.token_file`；`BotDirEntry::find_by_name` 读 `paths.bots_dir`；
  `stop_bot_sessions` 读 `timing.stop_grace_ms` 与 `timing.stop_settle_ms`；`Report::render`
  读 `runtime.max_reply_chars`。

配置是启动读一次、之后常驻内存（见 [config.md 第 1 节](../config/config-zh_cn.md)），所以便利函数读某一个字段没有
额外成本；而逻辑函数吃参数换来的是：启动命令、等待时间这些容易出错的地方都有单测（见
`utils::bot_dirs`、`utils::startup`、`utils::reply` 的测试）。

例外一个是状态判定 `BotDirEntry::connection_status()`：它逻辑重，但关键词自己去读，一半来自配置
`[keywords]`（各部署不同、只能自己填的那两项），一半来自语言表 `text` 的 `jar_*` 方法（JAR 自己打印的
断线重连提示，按 `[i18n] language` 取）。两边都是全局固定的一份，没必要让每个调用方传一遍，所以挂在
条目上（挑最新那份日志这一步在方法里做，调用方不必自己拼）。代价是它没法在单测里注入关键词，所以测试
落在吃参数的实现（`connection_status_of_log(bot_name, log_path)`，单行判定是吃关键词的
`ConnectionStatus::from_line`），而"往回读、读到结论就停"那一半由 `fs` 的测试
（`read_lines_backwards_until`）覆盖。它不产出"进程已结束"：那个状态看会话残壳，由 `running_bots`
直接给出（见 [`super::screen`] 的 `ScreenSession`）。

同一手法还有回复截断（就在 `Report::render` 里）：它自己读 `runtime.max_reply_chars`；代价一样，上限
只能来自配置，所以那段截断逻辑没有单测（要用测试兜住就得把上限做成参数，多一层）。

`bot_status` 里住着一份进程内状态（`BotPauseStatus`：按机器人名存，`update_bot_pause_status` 是唯一
写入口）。只有暂停状态进结构体，在线状态不存：

* 暂停状态是我们自己发 `/pause` 与 `/unpause` 的结果，进程内记着（初始是"钓鱼中"），确认到 JAR 回显
  才改（`BotPauseStatus::set_paused` 与 `set_fishing`）；读它（`pause_status`）就是读这份记忆。同一台
  机器人的 `/pause` 与 `/unpause` 几乎同时到达时，状态按回显窗口里最后那条记
  （`PauseCommand::last_confirmed_pause_command`），不是谁最后写谁赢；而我这条有没有生效也按同一口径
  判：最新的是另一条时，调用方按失败回一句，并把当前状态报出来。
* 在线状态只能由日志确认（只有 JVM 知道它连没连上、有没有掉线），存下来的副本立刻会过期，所以它不跨
  调用保存：`bot_dir_entry.connection_status()` 每次调用都当场重新取一次，调用者拿到的永远是最新的
  （`/show status` 里那份 `RunningBotInfo.connection_status` 只是本次渲染的快照，渲完即丢）。

这样 `/show status`（高频命令）就不必为了找一个"这个会话从没出现过"的 pause 关键词把日志扫到文件开头。
会话开始与结束时都会把这份状态忘掉（`forget_bot_pause_status`），所以它永远只描述这个会话。

## 7. 日志有两条线

### 7.1 机器人日志：只有一份

`<机器人目录>/<runtime.logs_dir>/` 里就这两样东西：

* `年-月-日-时-分-秒.log`：我们的日志。拉起会话前由 `create_bot_log_file` 建好，会话把 JVM 输出经
  `grep -v -E` 过滤后 `tee -a` 追加（命中 `keywords.filter_regex` 的行被丢掉）。`/log` 读它的尾部，
  `/show status` 从末尾往回扫它判状态。
* `[raw_game_logs] dir_name` 下的 `file_name`（默认 `raw/log0.log`）：FishingBot 的日志，一个指向
  `discard_path`（默认 `/dev/null`）的符号链接，写进去的内容由内核丢弃（每次拉起会话前整个目录清空
  重建，所以磁盘上只留过滤后的那一份）。名字与丢弃目标都在 `[raw_game_logs]` 里配。

`/show status` 为什么要倒着扫（`fs::read_lines_backwards_until` 加 `BotDirEntry::connection_status`）：连接
状态只看最后一次出现的关键词，所以从最新一行往回读、有结论就停。长跑账号的日志可能有几十 MB，整份读
进来既慢又占内存。暂停状态不扫日志，它是进程内记着的（见上一节），这就省掉了为了找不到 pause 关键词而
一路扫到文件开头那一趟（实测 14 MiB 日志 72 ms，每个机器人每次）。`/log` 用的 `read_last_lines` 是同一
份实现的薄封装（数够 N 行就停），所以它同样只读文件尾部，不会为了只想看最后 100 行把整份日志流一遍。
两条读法本身是通用的文件原语，在 `utils::fs`。

为什么还给 FishingBot 留个文件：它建不出日志文件会直接拒绝启动（`Bot.java` 里 `FileHandler` 抛
`IOException` 就 `setRunning(false)`），所以只能给它一个能写但什么都不存的目标。磁盘上因此只有一份真正
的日志，也不会再有两份互相干扰。

挑文件的口径只在 `bot_log_files::latest_log_path` 一处，两步：先只认时间戳命名（`is_bot_log_file`，名字
由 `create_timestamped_log_file` 生成），再按名字取最大。名字是定宽零填充的 `年-月-日-时-分-秒`，字典序
就等于时间序，所以它挑到的是最后启动的那一轮，而不是最后被写入的那个文件（后者会被还在刷屏的旧会话带
偏）。两步都不能省：早先少了名字过滤，而当时 FishingBot 的 `log0.log` 就在同一层、又一直在刷屏，于是
"最新"系统性指向它，`/log` 读回了未过滤的刷屏行；而 `log9.log` 这类名字在字典序里排在 `2026-…` 前面
（`l` 大于 `2`），所以那层过滤到今天仍然必要。

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
* 本项目自己的诊断信息也走 `log`（不再用 `println!`），所以终端与日志文件是同一份完整内容。例外只有
  logger 装不上那一条说明（那时日志系统还不能用，只能直写终端）。
* 日志文件建不出来（部署目录不可写）不致命：只在终端留一条告警，不会让机器人跑不起来。

## 8. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 谁在调用这些原语：[lib-zh_cn.md](../lib/lib-zh_cn.md)（指令入口层）与 [architecture-zh_cn.md](../architecture/architecture-zh_cn.md) 第 1 节（分层）。
* 两条日志线的完整口径：[process-log-zh_cn.md](../process-log/process-log-zh_cn.md)。
