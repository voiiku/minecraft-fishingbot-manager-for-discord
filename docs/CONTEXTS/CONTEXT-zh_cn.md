# CONTEXT：项目概况与维护约束
[English](CONTEXT-en_us.md) | **简体中文**

本文件为后续维护者与自动化会话提供最小必要上下文：首节说明项目定位，随后列出必须遵守的硬性约束。
修改代码前必须阅读本文件；出现新约定时必须同步更新。

记号与代码注释一致（定义见 [architecture-zh_cn.md 第 5 节](../mods/architecture/architecture-zh_cn.md)）：以 `?` 开头的行表示尚待真实运行确认；确认后删除记号并保留结论。
第 8 节另列真实部署检查、暂缓事项与批量操作回复方式评估。

入门材料：[README-zh_cn.md](../README/README-zh_cn.md)、[tutorial-zh_cn.md](../tutorial/tutorial-zh_cn.md) 与
[troubleshooting-zh_cn.md](../troubleshooting/troubleshooting-zh_cn.md)。模块说明位于 `mods/`，上游 JAR 对照表位于
`refs/`，项目约束与待办位于 `CONTEXTS/`。仓库根目录的 `README.md` 为英文默认 README；
`docs/README/README-en_us.md` 是指向它的符号链接，集成测试会检查该关系。

## 1. 项目概况

* 展示名：**Minecraft FishingBot Manager for Discord**。Cargo 包名与可执行文件名均为
  `minecraft-fishingbot-manager-for-discord`。本项目是非官方 Rust 控制器，用于管理外部 FishingBot 实例；
  仅单独安装的 `FishingBot.jar` 需要 Java。
* 许可：MIT，见根目录的 `LICENSE`。发布二进制时必须附上 `LICENSE`，并针对该版本的依赖与发布目标重新生成、
  核对第三方声明；SBOM 可选。生成策略与模板分别位于 `about.toml`、`about.hbs`，每次发布均须重新生成并检查。
  FishingBot 不得打包：它是 GPL-3.0 独立程序，本项目仅通过 `java -jar` 启动，不修改、不链接；若将 JAR 与二进制
  放入同一分发包，该分发包中的 JAR 部分必须按 GPL-3.0 处理。
* 上游（`MrKinau/FishingBot`，GPL-3.0）已计划支持 `/pause`、`/unpause` 两条游戏内指令。在所用 JAR 包含这两条指令之前，
  `/pause`、`/unpause` 仅回复"命令已发送"，暂停状态不会更新。两条指令的回显仍由配置 `[command_echo]` 的
  `pause_echo`、`unpause_echo` 提供，因此换用包含它们的 JAR 时无需重新编译；这两个键暂时保留，待上游支持后可归入
  语言表并按 `[i18n] language` 派生。
* 功能：通过 Discord 斜杠指令批量管理多个 Minecraft 账号的
  [FishingBot](https://github.com/MrKinau/FishingBot) 自动钓鱼实例。
* 技术栈：Rust（edition 2024）、[serenity](https://github.com/serenity-rs/serenity) 0.12.5、
  [poise](https://github.com/serenity-rs/poise) 0.7.0、`screen`、JVM。默认目标为
  `x86_64-unknown-linux-musl`，仅支持 64 位 Linux 部署，不支持 32 位。
* 无额外服务端组件：进程使用配置中的 Bot Token 直接连接 Discord Gateway，无需 NapCat 等协议端，也不向部署目录写入状态文件。
* 进程模型：一个机器人对应一个 `screen` 会话（会话名为 `runtime.screen_prefix` 加机器人名）。一个进程对应一个
  Discord 应用身份；指令仅使用斜杠指令，不使用 `!前缀` 指令（原因见第 7 节）。
* 无法连接 Discord 时停止机器人并自行重连，此为刻意保留的行为：判定本进程已与 Discord 失去联系后，停止全部机器人，
  等待一个判定窗口，然后在同一进程内重建连接，不退出进程。理由见第 3 节：无法连接 Discord 时既无法控制机器人，
  机器人又仍占用游戏账号，导致真人玩家无法登录。

## 2. 全局硬性约束（修改前必须确认合规）

### 2.1 编码规则

1. 框架边界：使用框架的连接与指令能力，不使用其日志与配置。使用 serenity 的 `Client`、Gateway 与
   `Http`，以及 poise 的 `Framework`、`#[poise::command]`、`FrameworkOptions`、`ctx.say` 与 `ctx.defer*`。
   不使用框架自身的日志通道；日志统一写入本项目的 `process_log.rs`（自行实现 `log::Log`）。poise 依赖
   `tracing` 时启用其 `log` 特性，框架日志会转为 `log` 记录并进入该通道；实机运行已确认日志写入程序日志。
   配置仅使用 `minecraft-fishingbot-manager-for-discord.conf.toml` 一份，不使用环境变量。
2. 尽可能避免向部署目录之外写文件。所有生成文件均位于部署目录（可执行文件所在目录：配置、程序日志
   `logs/`）或机器人目录（`paths.base_dir/bots/Discord用户ID/机器人名`）。当前无例外：框架不写状态文件，也不依赖
   工作目录。
3. 文档先行。先在本文件或 `docs/` 里写明预期行为与对外 API，再实现；实现必须与文档逐条对上，不允许
   文档写了代码没做，或代码做了文档没写。
4. 一个文件只负责一项职责。文件名必须直接说明所属模块与功能类别；单个函数不得同时承担"创建资源"与"启动进程"
   等两项职责，步骤应按第 4 节的启动管道显式拆分。仅有一个调用者且函数体仅为一句话的薄包装，按可读性决定内联
   或保留：仅复述调用处语义的名称应内联；命名了领域概念，或封装互斥锁与 panic 说明、判断与枚举映射的薄包装
   应保留。按表或按节组织的薄函数同样保留，例如配置校验采用一表一函数，`validate()` 仅按顺序调用（见 config
   文档第 5 节）。

5. 命名：函数使用动词短语（`start_bot`、`create_bot_log_file`）；取值函数使用名词且不加 `get_` 前缀
   （`latest_log_path`、`all_running_bot_names`）；判断使用 `is_` 与 `has_`；转换使用 `to_`，可能失败的转换
   返回 `Result`，其 `Err` 必须可直接作为用户回复（`raw_bot_name.to_clean_bot_name()`、
   `from_dir_name_to_owner_id`）；构造类型时，已知事实使用 `new`，需要查找使用 `find_*`
   （`BotDirEntry::new` 与 `BotDirEntry::find_by_name`）。变量名必须包含上下文与形态：列表使用 `*_names`、
   `*_entries`、`*_paths`，计数使用 `*_count`，不得出现裸 `count`、`list`、`data`、`arr`，不得使用自造缩写。
   允许的常规写法包括单位后缀 `_ms` 与 `_secs`、单行闭包中的 `|e|`（错误）以及 `bot_dir_path` 这类复合词。
   ID 统一使用 `*_id`：调用者为 `caller_id`（`u64`，Discord 用户 ID），目录归属为 `owner_id`；不使用旧平台
   专属的 ID 命名。
6. 注释：模块头 `//!` 说明文件职责与设计依据；函数 `///` 说明对外行为与边界；行内 `//` 仅解释原因，不复述代码。
   注释不得包含空泛内容或"待补充"。风格范本见 `src/utils/bot_dirs.rs`：模块头用一至两行说明职责与关键约定；
   函数注释使用简洁陈述句，说明行为、调用方与边界，并尽量引用具体函数名（如"供 `/start`、`/stop` 调用"）。
   存疑或存在更优实现时就地标注，`?` 表示待定或存疑，`!` 表示需要修改。不得写成大段设计说明。示例配置
   `minecraft-fishingbot-manager-for-discord.conf.example.toml` 是唯一例外：其注释使用英文（面向按模板修改
   配置的部署者），仅"取值随语言变化、中文部署必须修改"的项额外增加一行 `# CHINESE_SIMPLIFIED:` 提示，见
   [docs/mods/i18n/i18n-zh_cn.md 第 1 节](../mods/i18n/i18n-zh_cn.md)。会生成可见文案的行同样使用英文，即命令
   `///` 第一段与选项的 `#[description]`；poise 宏将其作为发送给 Discord 的说明（宏仅接受字面量，无法在运行期
   替换），因此这两处必须与 `en_us.rs` 表逐字一致（缺项时的英文副本，不是语言回落，见
   [docs/mods/commands/commands-zh_cn.md 第 8.1 节](../mods/commands/commands-zh_cn.md)）；第一段之后的段落仍使用
   中文，仅供维护者阅读，不发送给用户。涉及校验责任或职责归属时，必须明确到具体层级（命令层、指令入口层、
   业务层、utils 原语），例如"权限由业务层校验：本函数由 [`crate::handle_all_bots_control`] 调用，后者先判定
   `has_permission(caller_id, RequiredPermission::MustBeAdmin)`"；仅写"由 X 统一校验"无法体现分层。
7. 参数清洗的归属：Discord 提供结构化 option，但其值仍是用户输入的原始文本。命令层仅负责取值、数字校验与分发，
   原样传递（参数名以 `raw_` 开头）；实际使用方负责清洗一次：机器人名使用
   `ToCleanBotName::to_clean_bot_name`（`BotDirEntry` 构造条目时调用；非法名称返回 `Err`，不作过滤），游戏内
   指令片段使用 `utils::ToCleanBotCommand`。同一份文本在全链路中只清洗一次，第二处不得再出现 `trim()`。配置侧
   同理：字符串值由 `config::Config::from_toml_text` 统一 trim 一次，读取配置的代码接收原值。两条固定规则：
   机器人名口径的唯一来源是 `utils::bot_name`；需要业务状态才能判定的检查（Token 是否存在、会话是否运行、
   权限与目录归属、`max_reply_log_lines` 上限）一律留在业务层。数字是命令层唯一负责转换的输入类型：`/log` 的
   行数在命令层解析并校验后转换为 `usize`，`/ping` 的端口同理；校验失败时立即返回用法提示（Discord 的整数
   option 仅有有符号 64 位，因此 option 声明为 `i64`）。
8. 配置按需读取，且只读取所需字段：逻辑复杂且值得单测的函数通过参数接收依赖（`launch_command_line` 的全部取值
   由参数传入）；仅含少量逻辑的便捷函数直接读取 `config::Config::get().表.字段`（例如 `session_name` 仅读取
   `runtime.screen_prefix`，`BotTokenStatus::from_dir` 仅读取 `runtime.token_file`）；状态判定
   `BotDirEntry::connection_status()` 是该模式的复杂实现，关键词是全局固定的一份（一部分位于配置的 `[keywords]`
   与 `[command_echo]`，另一部分位于语言表 `text` 的 `jar_*` 方法），因此它直接读取配置与 `text::get()`，不通过
   参数传入（由条目自身选择最新日志）。任何函数都不得接收整份 `Config` 后再自行选择字段。配置在启动时读取一次
   并常驻内存，因此便捷函数读取单个字段无额外开销；逻辑函数通过参数接收依赖，从而获得可测试性。详见
   [docs/mods/utils/utils-zh_cn.md 第 6 节](../mods/utils/utils-zh_cn.md) 与
   [docs/mods/config/config-zh_cn.md 第 7 节](../mods/config/config-zh_cn.md)。

### 2.2 不变量清单（修改前逐项确认）

* 停止必须完整：`/stop` 经 `utils::shutdown::stop_single_bot_session`，`/stopall` 与进程退出收尾经
  `utils::shutdown::stop_all_bot_sessions`，重启的停止阶段直接调用 `utils::shutdown::stop_bot_sessions`；
  三条路径最终都复用 `stop_bot_sessions`（Ctrl+C、等待、强制终止、复核），不得留下占用游戏账号的孤儿 JVM。
  systemd 侧由 `KillMode=control-group` 作为最终保障。
* 日志分为两类，磁盘上仅保留一份机器人日志：
  * 机器人日志：`<机器人目录>/<runtime.bot_logs_dir>/年-月-日-时-分-秒.log`，在拉起会话前创建
    （`create_bot_log_file`），会话内经 `grep -v -E` 过滤后由 `tee -a` 追加；`/log` 与 `/show status` 读取该日志；
  * FishingBot 自身日志：同层 `raw/log0.log`，一个指向 `/dev/null` 的符号链接。FishingBot 无法创建日志文件时
    拒绝启动，因此为其提供可写但不保存内容的目标。不得改为保留原始日志；
  * 程序日志：`<部署目录>/logs/年-月-日-时-分-秒.log`，终端同时保留一份；每次运行一份，不轮转、不清理，见
    [docs/mods/process-log/process-log-zh_cn.md](../mods/process-log/process-log-zh_cn.md)；
  * 日志文件名的唯一来源是 `bot_logs::create_timestamped_log_file`，不得在其他位置拼接时间戳文件名。
* 机器人状态有两个来源：连接状态仅由日志确认（只有 JVM 知道是否连接或掉线），因此不跨调用保存，每次调用时重新读取
  （`BotDirEntry::connection_status`，调用者始终获得最新值）；暂停状态以本程序确认的命令回显为准，在进程内按机器人名
  保存（`bot_status::BotPauseStatus`，`update_bot_pause_status` 是唯一写入口），会话开始与结束时重置
  （`forget_bot_pause_status`）。因此 `/show status` 不必为查找从未出现的 pause 关键词而扫描到文件开头。暂停状态仅由
  `/pause` 与 `/unpause` 的确认路径写入，其他位置只读。例外为"进程已结束"：该状态不读取日志，当会话仅剩
  `(Dead ...)` 残壳时由 `running_bots` 直接给出（JVM 已不存在，日志最后一行可能仍为旧的"在线"）。
* `/show status` 的表格没有专门类型：数据行由 `utils::running_bots::all_running_bot_status_rows()` 即时拼接
  （四格 + `" | "`），表头由 `show/status.rs` 即时拼接；两处列顺序必须一致（列名位于 `text`，中英文各一份）。
  完整实现见 [docs/mods/commands/commands-zh_cn.md 第 6.2 节](../mods/commands/commands-zh_cn.md#62-show-status)。
* 回复内容不得超出可验证范围：`/start` 使用启动复核结果，按"已确认进服"、"会话已存在但尚未确认状态"或"启动失败"
  回答；`/restart` 的停止阶段可确认，启动阶段仍仅回复"已发送重启命令"；`/stop` 的停止流程当场复核，因此可以回复
  "已成功停止"；`/pause` 与 `/unpause` 发送后等待配置中的 `command_echo.wait_ms`（默认 300 毫秒，
  `utils::bot_status::confirm_pause_command`）在日志末尾查找回显，确认回显后回复"已确认暂停"或"已恢复"，未确认时
  仅回复"已发送"。新增指令的措辞必须遵循同一原则：只陈述已验证的状态。
* 交互必须在 3 秒内应答，否则用户会看到"交互失败"。本项目的操作（启动 JVM、等待批次冷却、执行 `screen -list`、
  按台数等待、读取日志）通常明显超过 3 秒，因此：
  * 会产生进程操作或外部探测的指令（`/start`、`/stop`、`/restart`、`/pause`、`/unpause`、`/command`、
    四个 `*all`、`/ping`、`/checkserver`、`/log`）均先调用 `ctx.defer()`；其中 `/log` 使用
    `ctx.defer_ephemeral()`，并且每条回复都显式设置 `.ephemeral(true)`（日志属于敏感内容，仅调用者可见；
    不能仅依赖 defer 的原因见 `docs/mods/commands/commands-zh_cn.md` 第 3 节）；
  * 纯本地文本指令（`/help`、`/whoami`）不调用 defer，直接回复；
  * defer 后的响应包含两条消息（Discord 的兼容行为，官方文档已说明）：第一条替换"正在思考"占位，第二条及以后
    为新消息。因此长指令（四个 `*all` 与单机器人 `start`、`stop`、`restart`）分两条消息发送：启动类指令先确认
    请求已加入全局启动队列；其他长指令先报告已开始执行；任务完成后发送 `Report::render()` 的完整结果。第二条
    消息按 Discord defer 规则作为新消息发送；修改该行为后必须在真实部署中回归验证。
  * 交互令牌有效期为 15 分钟：所有 defer 后的回复必须在该窗口内发出。批量启动耗时超过该窗口时（台数多或
    `[startup_queue] bot_login_interval_ms` 大），followup 必定失败，因此长指令的最终报告经
    `commands::say_final_report` 处理：无法发送时改发普通频道消息（普通消息不受交互令牌约束），并记录一条
    `LogText::ReportPostedToChannel` 日志说明回退原因与失败原因。批量操作回复方式的评估见第 8.1 节；当前行为
    未改变。
* 敏感输入必须通过 Modal 收集，不得作为指令参数：每次斜杠指令调用都会在频道生成一条消息（Discord 的
  `CHAT_INPUT_COMMAND` 消息类型），参数值包含在该消息中；`ephemeral` 仅作用于回复，不作用于调用消息本身。因此
  `/token` 使用 `#[derive(poise::Modal)]` 弹窗收集（输入通过 `MODAL` 回调传递，不进入频道），提交后通过带
  `ephemeral` 的 followup 返回结果。Modal 必须是交互的第一个响应，因此该指令不得先调用 `defer`（先 defer 会使
  弹窗失败）。新增需要用户填写敏感内容的指令时，必须遵循此模式。
* 回复必须容纳于一条消息：Discord 单条消息上限为 2000 字符。面向用户的长文本一律使用 `Report`（`render()` 按
  `runtime.max_reply_chars` 渲染，默认 2000，从末尾按行截断并注明省略行数）；超长消息会直接发送失败，用户侧无
  可见反馈，因此不得拼接无长度上限的字符串后直接发送。计数口径为 `str::utf16_len()`
  （`encode_utf16().count()`），`Report::render` 按该口径收口，不使用 `chars()`（码点）：非 BMP 字符（emoji）
  在 UTF-16 中占 2 个，采用更严格的计数口径才能保证 2000 字符可发送。按 UTF-16 单元计数比按 Unicode 码点更
  严格；`/log` 不受该限制，使用附件发送。
* 每次回复均记录一行程序日志：命令层不得直接调用 `ctx.say`，面向用户的回复一律通过
  `commands::say_and_log`（业务层通过 `commands::log_reply`）写入 `LogText::CommandReplied`，日志格式为
  `回复 /指令 (调用者 用户ID): 内容`。该记录固定了调用者、时间与回复内容，排查问题时以该日志为准。
* `/log` 使用附件发送，不截断：读取最后 N 行并作为附件上传（文件名为日志文件名），正文包含标题、行数与文件名；
  使用 `defer_ephemeral()` 加带附件的回复，并显式设置 `.ephemeral(true)`，其他用户不可见。附件大小上限为
  `MAX_ATTACHMENT_BYTES`（8 MiB；Discord 普通上限为 10 MiB，保留余量。该值为平台限制，不是用户偏好，因此作为
  常量，不进入配置）；超过上限时按行截断并在正文中说明。
* `keywords.filter_regex` 不得包含单引号（该值会内联到 `bash -c` 中的 `grep -v -E '...'`）。
* 配置仅在启动时读取一次：运行期间修改配置不生效，重启进程后生效（刻意不实现热重载）。
* `bot_dirs` 是目录约定的唯一来源：`base_dir/bots/Discord用户ID/机器人名`，权限边界位于用户 ID 层。
* 涉及运行中机器人的操作必须先检查会话：`/bot del`、`/bot rename`、`/token`、`/bot login` 在执行前均调用
  `is_bot_screen_running`（文案统一由 `refusal_if_not_stopped` 提供，运行中直接拒绝）。原因如下：修改运行中机器人的
  Token 不会生效；删除运行中的目录会破坏数据；重命名运行中的目录会使后续日志写入位置失效；登录需要先独占凭据文件。
  重启流程更严格：缺少 Token 的目标在停止前剔除（重启先停止再启动，无意义的停止会使仍在钓鱼的账号下线）。
* 凭据仅由 JAR 写入，本项目不实现微软登录：`/bot login` 拉起的会话即该机器人平时的会话，该指令将 JAR 打印的设备码
  （登录网址与代码）转发给调用者。凭据**以 `refreshToken` 文件出现为准**（不解析"授权成功"消息）：新凭据出现即表示
  认证成功，该步骤同时删除已移开的旧凭据（`<凭据文件名>.renewing`）。操作前必须移开已有凭据，否则 JAR 会直接使用
  已有凭据而不打印设备码，导致该指令没有任何可见动作。已观察到的两条 JAR 行为：**没有有效凭据时仅等待登录，不连接
  游戏服务器**；请求被微软频繁拒绝（返回空 token）时，JAR 会自行结束 JVM，因此第二段将"会话消失"直接报告为
  "启动后退出"。详见 [docs/mods/commands/commands-zh_cn.md 第 4.6 节](../mods/commands/commands-zh_cn.md#46-bot-login)。
* 游戏侧配置（FishingBot 的 `conf.json`）是**生成物**：进程启动时先删除旧文件，再以随仓库发布的完整模板
  （`fishingbot-config.example.json`）为基础，覆盖 `[fishingbot]` 中的对应键（`utils/fishingbot_config.rs`）。生成的
  文件必定是完整配置，因此不依赖 JAR 补齐字段。实测结果（v2.13.15，`-onlyCreateConfig`）：将生成文件提供给 JAR 后，
  JAR 既不打印 `Your configuration was incomplete...`（极简配置会打印），也不修改该文件的任何内容；语言按写入值生效
  （`Using language: CHINESE_SIMPLIFIED`）。手工修改该 JSON 不会保留到下次启动；模板是上游变更配置结构时唯一需要
  同步的对象（更换模板后运行 `cargo test`；托管路径缺失时测试会失败）。`misc.language` 写入的就是
  `[i18n] language` 的取值（该值即 JAR 的常量名，不做映射），不提供手工填写入口，从而避免"按哪种语言的 JAR 日志
  识别重连"不一致。生成失败按配置错误处理（退出码 1）。详见
  [docs/mods/config/config-zh_cn.md 第 3.2 节](../mods/config/config-zh_cn.md)。
* 各指令自行确定目录范围：`utils::split_bot_entries_by_is_running(&[BotDirEntry])`（位于 `running_bots`）仅负责分组，
  目录范围由调用方选择函数决定：`all_bot_dir_entries()`（全量，用于 `/startall`、`/restartall`、`/stopall` 与进程
  收尾，缺少 Token 的孤儿进程也必须能被停止）、`token_ready_bot_dir_entries()`（仅包含已配置 Token 的机器人，用于
  `/commandall` 与 `/show status`）、`bot_dir_entries_of_owner(owner_id)`（仅包含指定账号的机器人，用于
  `/show mybots`）。目录与会话的关联只允许在 `running_bots` 一处实现。
* 每条指令声明一个权限要求，判定仅有一个出口：要求写为 `utils::permission::RequiredPermission`
  （`Everyone`、`MustBeAdmin`、`OwnerOrAdmin(归属用户ID)`），校验统一调用
  `has_permission(caller_id, 要求)`，管理员名单来自 `[admins]`。
  * 所有用户可用的指令（`/help`、`/whoami`、`/show status`、`/ping`、`/checkserver`、`/show *`）声明 `Everyone`，
    在 `lib::handle_*` 中判定一次（该层是它们唯一的入口）；
  * 批量指令（`/startall` 等）声明 `MustBeAdmin`，同样由 `lib::handle_all_bots_control` 判定一次；
    `all_bots_control/*` 因此不接收 `caller_id`，也不自行校验；
  * 单机器人指令与账号管理指令声明 `OwnerOrAdmin(目标机器人的归属)`：归属必须先解析目标才能确定，因此路由层先调用
    `BotDirEntry::find_by_name` 查找条目，再执行判定（普通用户仅能操作自己的目录，管理员可跨目录；找不到与无权限
    返回相同提示）。底层函数不校验，也不接收 `caller_id`。
  命令层仅负责取值与参数个数校验，不检查权限。新增指令必须遵循此约定：命令层必须将结果交给 `lib::handle_*`，不得
  自行组装回复（`/whoami` 同样如此）。不存在注册表或框架钩子提供保障，因此该约定与评审是唯一保障。
  * 选项候选不单独定义权限：`src/commands/autocomplete.rs` 查询有权限查看的机器人目录，按运行状态与指令语义筛选
    机器人名候选（管理员为全部机器人，其他用户为 `bots/<用户ID>/` 下的机器人，与 `OwnerOrAdmin` 同一口径），并
    负责 Discord interaction 适配及 `AutocompleteChoice` 转换。候选仅为建议：用户仍可手动输入任意名称，归属与会话
    状态仍由各 `handle_*` 判定。
* utils 不组装报告，只返回原始信息：流程函数返回明细（例如停止的 `StopOutcome`，自带 `into_report()` 与
  `to_failure_text()`），组装必须由上层显式完成。批量停止由 `all_bots_control::stopall::stop_all_bots()` 显式组装；
  进程收尾在 `watchdog::Watchdog::exit_on_shutdown_signal()` 中就地组装同一份明细（仅标题不同："收到退出信号，
  收尾结果"；该报告除写入日志外，还会发送到各服务器的通报频道（发送总时限为 30 秒，超时则跳过且不补发）；掉线与
  永久错误的收尾仅写入日志）。
* 可见性：crate 的对外入口位于 `lib.rs`，包括 `pub fn handle_*`、公开视图类型，以及 `pub mod config`、
  `pub mod text`、`pub mod utils`。`utils` 的子模块为私有 `mod`，父模块的 `pub use` 构成公开工具面；五个业务模块
  同样保持子模块私有，父模块顶部的 `pub(crate) use` 构成 crate 内部面。因此 `lib.rs` 仅通过
  `crate::bot_control::start_bot` 这类路径访问业务模块；是否可从 crate 外调用，以 `lib.rs` 与 `utils.rs` 的导出清单
  为准。
* 网关看门狗是唯一判定掉线的位置：判定规则见第 3 节，收尾共用库侧的 `stop_all_bots_on_disconnect`（私有；停止
  机器人并记录原因与时刻）。判定侧仅调用 `request_reconnect` 将掉线原因交给入口并唤醒入口；停机与重建连接均在入口
  的 `wait_for_attempt()` 中完成。只有配置错误才走 `exit_on_permanent_failure`。不得依赖框架内部状态判定：serenity
  0.12.5 的 `ShardMessenger`（即 `ctx.shard`）不暴露连接阶段与延迟（实测仅有 `shutdown_clean`、`set_presence`、
  `chunk_guild` 等），`Shard` 位于 shard runner 内部，无法获取；网关侧的部分失效由 serenity 自身的心跳与 ACK 机制
  发现并自动重连，本项目仅处理"无法连接"与"网络中断"两类情况。
* 当前未实现并发互斥：多个用户可同时触发指令，对同一批机器人同时执行 `/startall` 或 `/stop` 时可能相互干扰。若
  实际使用表明有必要，可在命令层增加按机器人名加锁的约定，并记录于本节。
* 不实现单实例锁，依赖"一台机器一份配置一个实例"：serenity 是 Gateway 客户端，不监听端口，两个实例均可连接
  Discord 并接收交互，表现为同一指令回复两次；本进程不持有权威状态（状态位于 `screen` 会话与磁盘），重复实例不会
  破坏状态一致性。机器级锁会阻止"更换 `base_dir` 并行运行一份"这一正常用法。
* 面向 Discord 的文案必须通过 `text::get()`（机制见 [docs/mods/i18n/i18n-zh_cn.md](../mods/i18n/i18n-zh_cn.md)）：
  业务层与 utils 不得写入面向用户的中文字面量。新增语言需要在 `src/text/` 新增一个实现 `Text` 的文件（包含
  `command_texts()` 指令说明表）并在 `Language` 中登记：遗漏方法会导致编译失败，遗漏指令说明会被覆盖测试拦截；
  合法取值仅在 `Language::supported_names()` 中定义。当前机制与自查方法见
  [docs/mods/i18n/i18n-zh_cn.md 第 6 节](../mods/i18n/i18n-zh_cn.md)：语言机制不仅覆盖回复文案，也覆盖程序日志、
  启动期控制台提示与配置报错（`LogText` 枚举加 `log_text()`，两个语言文件穷尽 `match`）；指令说明与 `/token`
  弹窗标签通过注册前改写 poise 生成的指令树加手写 `impl poise::Modal` 实现；JAR 自身打印的断线重连提示同样位于
  语言表（`jar_*` 方法）。因此 `[i18n] language` 同时决定"本程序使用哪种语言"与"按哪种语言的 JAR 日志识别重连"；
  `/pause` 与 `/unpause` 的回显由配置 `[command_echo]` 提供（仅当所用 JAR 支持这两条指令时存在，见第 2 节第一条）。
  唯一固定为英文的是配置尚未读取时的提示（`text::bootstrap`：配置文件无法读取或解析、生成模板、发现已有配置文件、
  可执行文件路径、`language` 取值未知，以及 `config::Config::get()` 的 `CONFIG_NOT_LOADED` 与语言未安装时的
  `TEXT_NOT_INSTALLED` 不变量消息），因为此时语言尚未确定。`text::get()` 在语言未安装时，生产构建直接 panic
  （进程入口调用顺序错误），测试构建固定返回英语。`Language` 不实现 `Default`，配置中的 `language` 不受支持时
  启动校验直接报错并退出。示例配置的 `language = "ENGLISH"` 仅决定新部署运行哪种语言，中文部署应改为
  `CHINESE_SIMPLIFIED`。
  文案以中文为基准：修改文案时先修改 `zh_cn.rs`（内容与排版原稿），再将 `en_us.rs` 逐条对齐，最后同步
  `commands/*.rs` 中与英文表逐字一致的备用副本（顺序同时写在两个语言文件的模块头中）。
  排版约定：面向用户与部署者的文案符号随语言变化（中文全角、英文半角），句尾不留句号（叹号保留）；补充说明统一
  使用圆括号 `()`；例外为配置表名（`[admins]`）、`Report` 的小节标签（`[成功明细]`、`[Success]`）与用法提示中的
  参数记号（可选写 `[行数]`、必填写 `<机器人名>`）；错误回复统一以 `错误:`（英文 `Error: `）开头，见
  [docs/mods/i18n/i18n-zh_cn.md 第 5 节](../mods/i18n/i18n-zh_cn.md)。
* 关键词的归属由决定字符串的一方确定（`[keywords]` 限定为两项的依据）：仅将各部署不同且必须自行填写的项放入配置，
  即 `online`（游戏服务器自身的进服提示）与 `filter_regex`（当前服务器需要过滤的刷屏噪声）；`/pause` 与 `/unpause`
  的回显放入 `[command_echo]`（`pause_echo`、`unpause_echo`），原因是不同 JAR 构建可能提供不同回显，就地修改比
  重新编译更实际；JAR 自身打印的断线重连提示放入 i18n 的 `jar_*` 方法（见上一条），要求部署者重复录入只会增加
  出错点：标点错误会使 `/show status` 始终显示"初始中"。**"进程已结束"不进入配置，也不需要关键词**：旧版外壳脚本
  写入日志的结束标记（`fishingbots` 脚本：`java … | tee bot.log; echo 标记 >> bot.log`）是该脚本的产物，本项目自行
  拉起会话且不写入该标记，判定依据为 `screen` 会话状态（`(Dead ...)` 残壳），因此不受 JAR 版本与脚本写法影响。

## 3. 网关看门狗（唯一判定掉线的位置）

目的：判定本进程已与 Discord 失去联系后，不再假设机器人仍可被控制。无法连接 Discord 时既无法控制机器人，机器人
又仍占用游戏账号（FishingBot 的自动重连会与真人玩家争用账号），因此必须停止全部机器人；随后进程不退出，等待一个
判定窗口后重建连接（细节见 [docs/mods/watchdog/watchdog-zh_cn.md 第 4 节](../mods/watchdog/watchdog-zh_cn.md)）。

两条判据均位于 `src/watchdog/gateway.rs`（纯计时、不依赖框架、可单测）：

* 握手：本次尝试开始后 `first_handshake_timeout` 内必须至少收到一次 `Ready` 或 `Resume`（bin 侧
  `event_handler` 调用 `watchdog.record_handshake_success()`）。未满足时请求重建连接，原因为 `NeverReady`。
* 可达：每个 `probe_interval` 使用 `Http` 主动请求一次 `GET /gateway`；收到任何 HTTP 响应均视为可达（包括
  401 与 403），仅超时或连接错误视为不健康；连续不健康累计达到 `window` 时请求重建连接，原因为 `Unreachable`。

细节：

* 时序来自配置中的判定窗口 `timing.watchdog_window_ms`；探活间隔（窗口的 1/3）与首次握手超时（窗口的 2 倍）
  由该值派生（`TimingConfig::probe_interval` / `first_handshake_timeout`），不再作为配置项，因为仅按窗口比例取值
  才有意义。入口将三项写入 `Watchdog`，判定与探活共用同一份；派生规则同时排除了"探活间隔大于窗口、单次失败即判定
  掉线"的配置。
* 探活仅检查网络可达性，不检查凭据：Token 错误会使 `client.start()` 自行报错退出，不归看门狗处理。?探活请求发往
  `discord.com`，网关连接 `gateway.discord.gg`；若部署中出现 API 可达但网关不可达的情况，再增加针对网关域名的
  TCP 或 TLS 探活。
* 探活任务在 `client.start()` 之前启动（位于 `main.rs`）：网关始终无法连接时 `Ready` 不会到达，因此必须在 client
  之外也能判定；提前启动还可使"启动时断网"在首次握手超时内退出。
* 判定可逆：探活恢复或再次收到 `Ready` 与 `Resume` 时，状态回到 `Alive`，不会在单次判定后永久保持失败状态。
* 四种状态为 `Alive`、`Waiting`、`NeverReady`、`Unreachable`（看门狗内部状态，不向调用方公开）；判定时刻由调用方
  传入（`state(now)`），因此每条规则均可由单元测试覆盖。
* 收到第一个 `Ready` 时输出一行
  `网关看门狗已就绪：判定窗口 N 秒、探活间隔 N 秒、首次握手超时 N 秒`；排查时以该行为准（它反映实际生效的三项参数）。

判定失败时执行库侧的 `request_reconnect`：停止全部机器人，记录原因与掉线时刻，并唤醒进程入口重建连接（等待一个判定
窗口后再次装配 `Client`）。进程不退出，因为掉线属于常态；交由 systemd 重启只会每 10 秒生成一份日志并承担冷启动
开销。仅配置错误（Token 无效、机器人未被邀请）走 `exit_on_permanent_failure`（退出码 1，本项目用于主动失败的专用
退出码；systemd 单元配置 `RestartPreventExitStatus=1` 可阻止重启）。

## 4. 启动管道（步骤显式化的示例）

启动由启动队列分四个阶段完成，命令侧仅涉及第一阶段：

1. `StartupQueue::start_bots(bots)`：命令侧入口，创建账本 → `add_bots_to_queue` 入队 → `wait_until_done()` 等待这些
   条目均被拉起一次，取回结果并交给 `verify_batch`。
2. 启动线程 `run_starter`：等待队列非空 → 等待 `[startup_queue] batch_login_interval_ms`（从上一批出队时刻计算）→
   `pop_bots_from_head(max_batch_size)` 取出一批。
3. `start_single_batch` 逐台调用 `launch_one_bot` → `add_bot_for_login`：创建本次日志文件（失败则不拉起会话）→ 准备
   FishingBot 的原始日志目录（`<bot_dir>/<bot_logs_dir>/raw`，`raw/log0.log` 指向 `/dev/null`）→ 使用 `screen -dmS`
   拉起会话，命令形如 `java … | grep -v -E '<filter_regex>' | tee -a '<log_path>'`；相邻两台之间等待
   `bot_login_interval_ms`。
4. `verify_batch`：拉起后立即使用 `screen -list` 快照判定一次，等待 `timing.check_grace_ms` 后再次判定；仅第二次仍
   存在时才算拉起成功，日志仅用于说明是否已确认进服。

新增流程必须沿用该风格：每一步对应一个函数，且每一步均可独立理解。

## 5. 构建与验证（提交前全部执行）

```bash
cargo fmt --check                      # 格式
cargo test --all-targets               # 库、二进制与集成测试
cargo check --all-targets              # 真实目标：x86_64-unknown-linux-musl（静态）
cargo doc --no-deps                    # 文档链接：必须零警告（仅运行 cargo check 无法发现断链）
cargo clippy --all-targets -- -D warnings
```

测试分层：

* 单元测试与代码位于同一文件（`src/*.rs` 的 `#[cfg(test)] mod tests`）：覆盖私有逻辑与需要访问内部数据的场景；能够通过
  公开接口验证的行为应优先放入 `tests/`。
* 集成测试位于 `tests/`：将库作为外部依赖使用，覆盖公开行为与对外契约；能够通过公开 API 验证的测试应优先放在这里。
* 公共测试助手：单元测试使用 `src/test_support.rs`（仅在 `cfg(test)` 下编译）；集成测试使用 `tests/common/mod.rs`，
  提供临时目录、英语语言初始化与示例配置读取。
* 迁移原则：测试不依赖私有函数、私有类型或二进制内部状态时，应移至 `tests/`；依赖这些实现的测试保留在源码内，避免
  仅为测试扩大公开 API。

另外几条：

* 改动配置字段必须同步 `minecraft-fishingbot-manager-for-discord.conf.example.toml` 与
  [docs/mods/config/config-zh_cn.md](../mods/config/config-zh_cn.md)，否则 `cargo test` 会失败（有测试同时校验这两件事）。
* 注释里反引号中的本 crate 代码项写成可跳转链接（`` [`logger`] `` 这种），措辞用书面语，不用粗体标记、
  破折号与书名号引号，段头注释统一成 `============ X ============`。
* 文档注释里，公开项的文档不引用私有项（rustdoc 链不到、读者也看不到）：改成直述其含义，能链的补成
  链接。私有项自己的文档里可以正常链接私有项。

## 6. 代码地图（按名称可识别的顺序）

```
src/main.rs                 进程入口：配置 → 程序日志 → 显式创建看门狗（WATCHDOG.get_or_init()） → 生成游戏侧配置 → REST 探活任务 → poise 装配 → 运行 → 掉线重连 / 信号退出
src/lib.rs                  指令入口层：动作枚举 + handle_* + 退出与重连入口
src/commands.rs             命令层：poise 指令（一个业务模块一个文件，见下）
src/commands/               bot_control / bot_manage / token / log / all_bots /
                            show / general（`/help`、`/whoami` 位于此处）/ registration（全局注册与残留清理）
src/watchdog.rs             网关看门狗主体：时序字段 + 握手/探活/掉线记录 + 进程级单例 WATCHDOG（第 3 节）
src/watchdog/               gateway（握手与探活判定 + 探活任务）/ lifecycle（进程收尾）
src/process_log.rs          程序日志：终端 + <部署目录>/logs/时间戳.log
src/config.rs               外置配置：运行时结构 Config 与 Config::get_or_init / Config::get / Config::config_file_path / Config::admins / Config::install_dir
src/config/                 schema / parse / validate / template
src/text.rs                 文案入口：一个进程一种语言（`[i18n] language` 启动时选定一次）
src/text/                   en_us（英语）/ zh_cn（两份一一对应，仅字面量不同）
                            + bootstrap（引导期英文）/ log_text / requirement（数据型枚举）
                            + setup_failure（装配失败：按成因选择文案 + 判定是否为永久错误）
                            + watchdog（看门狗互斥锁中毒的内部不变量文案；随语言切换）
src/bot_control/            单机器人业务（一指令一文件）
src/bot_manage/             账号管理业务（一指令一文件）
src/all_bots_control/       全服批量业务（仅管理员，一指令一文件）
src/show/                   只读查询业务（一指令一文件）
src/general/                通用业务（一指令一文件）
src/utils/                  原语层：bot_command / bot_dirs / bot_name / bot_logs /
                            bot_status / fs / fishingbot_config / permission / reply / restart /
                            running_bots / screen / shutdown / startup / uptime
```

命令层按业务模块分组：`src/commands/` 下的 7 个业务文件（`all_bots`、`bot_control`、`bot_manage`、`general`、
`log`、`show`、`token`）与 `lib.rs` 的动作枚举对齐，另有 `autocomplete.rs`（交互候选适配）与 `registration.rs`
（全局指令注册与残留清理）。该拆分使文件名同时说明模块与功能，并避免将每条指令拆成仅十余行的文件。业务层仍为
一指令一文件。

## 7. 已确定的技术选择（不再重复讨论）

1. 机器人日志仅保留一份：写入前过滤（`grep -v -E` 加 `tee -a`），FishingBot 自身日志文件指向 `/dev/null`。不采用
   "原始日志写入 `/tmp`、过滤后写入机器人目录"的双份方案，因为该方案会增加磁盘（或 tmpfs 内存）占用以及清理与
   权限逻辑；历史上两份日志位于同一层目录，曾导致 `/log` 读取错误文件。排查过滤规则时，可临时清空
   `keywords.filter_regex` 并重启进程，以查看全部日志行。
2. 模块采用新版风格（`src/utils.rs` 加 `src/utils/*.rs`），不使用 `mod.rs`。两种方式对编译器完全等价；新版风格可在
   编辑器标签页与搜索结果中直接看到模块名，且全项目不含 `mod.rs`，风格统一。
3. 仅使用斜杠指令，不使用 `!前缀` 指令：前缀指令需要读取消息内容，因而需要 `MESSAGE_CONTENT` 特权意图（开发者面板
   需额外配置，大型服务器还需审核）；斜杠指令不需要任何特权意图，无需开启开发者面板开关，并提供参数提示与补全。
   该选择同时决定命令层不解析文本。
4. `/log` 使用附件而非截断：2000 字符无法容纳 100 行日志，截断会造成显著信息损失；附件可完整提供内容，并自然携带
   文件名。私密性由 `defer_ephemeral()` 与每条回复显式设置的 `.ephemeral(true)` 保证。
5. 长指令分两条消息回复（`*all` 与单机器人 `start`、`stop`、`restart`）：先返回一条回执（启动类回复"已接受 … 请求"，
   `/startall` 还会给出本次启动数量、队列中已有数量、预估耗时与结果超时，见 `lib::handle_startall_notice()`；其余回复
   "已开始执行 …"），完成后返回完整报告。该行为依据 Discord 官方文档中的 defer 规则（第一条替换占位，第二条及以后
   为新消息）。批量操作最终回复方式的评估见第 8.1 节。
6. 游戏侧配置由本程序生成，模板随仓库发布：FishingBot 仅接受 JSON，且没有逐项覆盖的命令行参数，因此
   `[fishingbot]` 是唯一真源；启动时删除旧文件，以模板为基础覆盖对应键，生成的文件必定是完整配置（不依赖 JAR 自动
   补齐）。模板为 `fishingbot-config.example.json`（完整 FishingBot 配置，默认值按本项目需求调整）；必须保持其与
   FishingBot 配置结构同步，修改模板后运行 `cargo test`。

## 8. 当前状态与待办

当前状态：项目为 Discord 实现，已在 Linux 环境运行。代码与文档同步维护；文档清单见
[docs 文档索引](../index-zh_cn.md)。修改后必须运行自动化检查；下列事项需在真实部署中端到端验证。

构建环境前置条件：rustls 的加密后端需要 C 编译器，musl 静态构建还需要 `musl-gcc`（Arch 安装 `musl`，Debian 与
Ubuntu 安装 `musl-tools`）。安装完成后，`cargo build --release`（默认目标 `x86_64-unknown-linux-musl`）可直接生成
静态二进制。

### 8.1 批量操作回复方式评估（未实施）

评估目标：将四个 `*all` 指令的最终结果改为普通频道消息，以规避交互令牌的 15 分钟限制；消息应说明回复对象
（调用者、调用时间、指令），并尽可能使用原生引用。

当前实现：`all_bots_manage` 调用 `run_and_reply_with_progress`，流程为 `ctx.defer()` → 进度 followup → 业务执行 →
`say_final_report`。`say_final_report` 先尝试 `ctx.say`；失败时改发普通频道消息，并记录
`LogText::ReportPostedToChannel`。单机器人指令共用同一入口。

评估结论：

1. 最终报告固定为普通频道消息可行。普通消息不受交互令牌限制，但仍受 2000 字符上限与频道发送权限约束。`ctx.defer()`
   与进度回执必须保留；批量入口必须与单机器人入口分离，避免改变单机器人指令行为。
2. 斜杠指令的原生引用不可行。`CommandInteraction` 不包含原始消息 ID；`Interaction::message` 仅适用于组件与 Modal
   交互；本项目仅订阅 `GatewayIntents::GUILDS`，无法获取 `CHAT_INPUT_COMMAND` 消息事件。`MessageReference` 必须
   指向真实消息 ID，且原生回复需要 `READ_MESSAGE_HISTORY`。因此不能引用用户的 `/startall` 调用本身。
3. 回复对象信息可获取：`ctx.author()`、`ctx.created_at()`（由 interaction ID 派生）与
   `ctx.command().qualified_name`。调用时间可使用 Discord 时间戳 `<t:…:f>` 显示；相关文案应新增 `Text` 方法，
   中英文各一份。
4. 若必须显示原生引用卡片，可引用机器人自身的进度消息：保存进度消息的 `MessageId`，最终普通频道消息通过
   `CreateMessage::reference_message((channel_id, progress_message_id))` 引用。该卡片引用的是进度消息，不是用户
   命令；需要 `READ_MESSAGE_HISTORY`，引用失败时应回退为不带引用的普通消息，并在真实部署中验证。
5. 实施前还需处理：最终消息与引用前缀的合计长度上限、频道发送失败日志、`ReportPostedToChannel` 语义调整（固定
   频道发送后不再表示交互令牌过期），以及单机器人入口隔离。实现可复用现有 `all_bots_manage`、`Report` 与
   `say_final_report` 路径，无需新增结构体；批量入口仍需与单机器人入口分离。

建议：第一阶段采用"最终报告固定为普通频道消息 + 文本引用说明"；原生引用卡片单独评估，先验证权限与引用行为。

### 8.2 真实部署检查

1. 端到端验证 Discord 交互：`/log` 与 `/show botconfig` 附件、defer 后第二条回复，以及 `/token` 弹窗提交后的
   followup。弹窗流程见[指令说明第 1.3 节](../mods/commands/commands-zh_cn.md#13-敏感内容ephemeral-与-modal)。
2. 实际验证 `/bot login`：确认登录链接与代码送达、凭据写入机器人目录、已有凭据成功刷新后
   `refreshToken.renewing` 被删除。见[指令说明第 4.6 节](../mods/commands/commands-zh_cn.md#46-bot-login)。
3. 在真实游戏服验证生成的 FishingBot `conf.json`、机器人连接，以及切换 `[i18n] language` 后 `/pause`、`/unpause`
   的对应语言回显。模板完整性已用真实 JAR 验证；见[配置说明第 3.2 节](../mods/config/config-zh_cn.md)。

### 8.3 暂缓事项

以下事项不阻塞发布：为 `/start`、`/stop`、`/restart` 增加多机器人参数（现有 `*all` 指令仍可用）；程序日志自动
轮转；命令层并发互斥；`/bot login` 并发上限。应根据实际使用需求重新评估；登录并发的取舍见
[指令说明第 4.6 节](../mods/commands/commands-zh_cn.md#46-bot-login)。

## 9. 相关文档

* 全部文档见 [docs 文档索引](../index-zh_cn.md)。
* 按主题查阅：[tutorial/tutorial-zh_cn.md](../tutorial/tutorial-zh_cn.md)（部署）、
  [architecture-zh_cn.md](../mods/architecture/architecture-zh_cn.md)（分层与目录结构）。
* 模块技术文档：`mods` 目录下每个模块一份（见索引第 3 节）。
