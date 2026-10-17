# CONTEXT：项目概况与必须记住的事
[English](CONTEXT-en_us.md) | **简体中文**

本文件是给下一位接手的人（或 AI 会话）的最短上下文：开头几行说清项目是什么，后面列清必须关注、不能
被忘记的硬约束。改动代码前先读这里，改完如有新约定请更新这里。

记号与代码注释同一套（定义见 [architecture-zh_cn.md 第 5 节](../mods/architecture/architecture-zh_cn.md)）：以 `?` 开头的行是待实机运行确认的问题；确认之后删记号、保留结论。第 8 节另列真实部署检查与暂缓改进。

入门文档：[README-zh_cn.md](../README/README-zh_cn.md)、[tutorial-zh_cn.md](../tutorial/tutorial-zh_cn.md) 与
[troubleshooting-zh_cn.md](../troubleshooting/troubleshooting-zh_cn.md)。模块说明在 `mods/`，上游 JAR 对照表在
`refs/`，项目约束与待办在 `CONTEXTS/`。仓库根的 `README.md` 是英文版默认 README；
`docs/README/README-en_us.md` 是指向它的软链接，集成测试会检查链接关系。

## 1. 项目概况

* 展示名：**Minecraft FishingBot Manager for Discord**。Cargo 包名与可执行文件名均为
  `minecraft-fishingbot-manager-for-discord`。这是非官方 Rust 控制器，管理外部 FishingBot 实例；只有单独安装的
  `FishingBot.jar` 需要 Java。
* 许可：MIT，见根目录的 `LICENSE`。发布二进制时必须附上 `LICENSE`，并为该版本的依赖与发布目标重新生成、核对
  第三方声明；SBOM 可选。生成策略和模板分别在 `about.toml`、`about.hbs`，每次发布都要重新生成并检查。
  不要打包 FishingBot：它是 GPL-3.0 的独立程序，本项目
  只以 `java -jar` 拉起它、不修改不链接；真要把 jar 与二进制装进同一个分发包，那份分发包里的 jar 部分
  必须按 GPL-3.0 处理。
* 上游（`MrKinau/FishingBot`，GPL-3.0）已计划支持 `/pause`、`/unpause` 两条游戏内指令。在所用 JAR 包含这两条指令之前，`/pause`、`/unpause` 只会回复“命令已发送”，暂停状态不会更新。两条指令的回显仍由配置 `[command_echo]` 的 `pause_echo`、`unpause_echo` 提供，因此换用包含它们的 JAR 时无需重新编译；这两个键暂时保留，等上游支持后可以收进语言表、按 `[i18n] language` 派生。
* 功能：用 Discord 斜杠指令批量管理多个 Minecraft 账号的
  [FishingBot](https://github.com/MrKinau/FishingBot) 自动钓鱼实例。
* 技术栈：Rust（edition 2024）、[serenity](https://github.com/serenity-rs/serenity) 0.12.5、
  [poise](https://github.com/serenity-rs/poise) 0.7.0、`screen`、JVM。默认目标
  `x86_64-unknown-linux-musl`，只面向 64 位 Linux 部署（不打算支持 32 位）。
* 没有任何额外服务端：进程用配置里的 Bot Token 直连 Discord 的 Gateway，不需要 NapCat 这类协议端，也
  不往部署目录写状态文件。
* 进程模型：一个机器人等于一个 `screen` 会话（会话名是 `runtime.screen_prefix` 加机器人名）。一个进程
  等于一个 Discord 应用身份；指令只有斜杠指令（不做 `!前缀` 指令，原因见第 7 节）。
* 连不上 Discord 就停机器人、自己重连（刻意保留的行为）：判定到本进程已经跟 Discord 失去联系就停止全部
  机器人、等一个判定窗口后在同一个进程里重建连接，不退出进程。理由见第 3 节：连不上 Discord 时我们既
  控制不了机器人，机器人却还占着游戏账号，真人玩家就上不了线。

## 2. 全局硬约束（改动前必须确认不违反）

### 2.1 写代码的红线

1. 框架的边界：用它的连接与指令框架，不用它的日志与配置。用：serenity 的 `Client`、Gateway 与
   `Http`，poise 的 `Framework`、`#[poise::command]`、`FrameworkOptions`、`ctx.say` 与 `ctx.defer*`。
   不用：框架自己的日志通道，日志一律走我们自己的 `process_log.rs`（自实现 `log::Log`）。poise 依赖
   `tracing` 时开着它的 `log` 特性，框架日志会转成 `log` 记录进到这条通道；实机运行已确认日志会写入程序日志。
   配置只有 `minecraft-fishingbot-manager-for-discord.conf.toml` 一份（完全不用环境变量）。
2. 尽可能避免向部署目录之外写文件。所有生成的文件都在部署目录（可执行文件所在目录：配置、程序日志
   `logs/`）或机器人目录（`paths.base_dir/bots/Discord用户ID/机器人名`）里。这一条现在没有例外：框架
   不写状态文件、也不看工作目录。
3. 文档先行。先在本文件或 `docs/` 里写明预期行为与对外 API，再实现；实现必须与文档逐条对上，不允许
   文档写了代码没做，或代码做了文档没写。
4. 一个文件一件事。文件名要能直接说明它是哪个模块的哪一类功能；一个函数不要同时干"建资源"与"起进程"
   这种两件事，步骤要显式拆开（见第 4 节的启动管道）。
   只有唯一调用者、函数体就是一句话的薄包装：内联还是保留，看哪种读起来更清楚。名字只是把调用处那句话
   再说一遍的就内联；命名了领域概念，或挡住互斥锁与 panic 说明、判断与枚举映射的，留着。按表/按节对齐结构
   的薄函数也属于"留着"：例如配置校验一表一函数，`validate()` 只按顺序调用（见 config 文档第 5 节）。

5. 命名：函数用动词短语（`start_bot`、`create_bot_log_file`）；取返回值用名词且不加 `get_` 前缀
   （`latest_log_path`、`all_running_bot_names`）；判断用 `is_` 与 `has_`；转换用 `to_`，可能失败的
   转换返回 `Result`，`Err` 是可以直接回复给用户的文案（`raw_bot_name.to_clean_bot_name()`、
   `from_dir_name_to_owner_id`）；构造一个类型时，已知事实用 `new`、需要去找用 `find_*`
   （`BotDirEntry::new` 与 `BotDirEntry::find_by_name`）。变量名要带上下文与形态：列表叫 `*_names`、
   `*_entries`、`*_paths`，计数叫 `*_count`（不允许出现裸 `count`、`list`、`data`、`arr`），不用自制
   简称。允许的常规写法：单位后缀 `_ms` 与 `_secs`、一行闭包里的 `|e|`（错误）、`bot_dir_path` 这类
   复合词。ID 一律叫 `*_id`：调用者是 `caller_id`（`u64`，Discord 用户 ID），目录归属是 `owner_id`；
   不使用旧平台专属的 ID 命名。
6. 注释：模块头 `//!` 说明这个文件负责什么、为什么这么写；函数 `///` 说明对外行为与边界；代码里的
   `//` 只解释为什么，不复述代码。不放废话，也不写"待补充"。风格范本见 `src/utils/bot_dirs.rs`：模块头
   一两行讲清负责什么与关键口径，函数注释直白短句、写清做什么、谁调用、边界，能点到具体函数名就点
   （如"供 `/start`、`/stop` 用"）；有疑问或有更好写法就地标出来，`?` 表示待定或存疑，`!` 表示该改。
   不要写成大段"为什么这么设计"的说明文。示例配置 `minecraft-fishingbot-manager-for-discord.conf.example.toml`
   是唯一的例外：它的注释写英文（面向"照着模板改配置"的人），只有"取值随语言变、中文部署必须改"的那
   几项额外带一行 `# CHINESE_SIMPLIFIED:` 提示，见 [docs/mods/i18n/i18n-zh_cn.md 第 1 节](../mods/i18n/i18n-zh_cn.md)。例外：会变成可见文案的那一
   行写英文，即命令 `///` 的第一段与
   选项的 `#[description]`，它们会被 poise 宏当成要发给 Discord 的说明（宏只认字面量、运行期换不掉），
   所以这两处写英文，且与 `en_us.rs` 表里那句逐字相同（缺项时的英文副本，不是语言回落，见
   [docs/mods/commands/commands-zh_cn.md 第 8.1 节](../mods/commands/commands-zh_cn.md)）；第一段之后的段落仍是中文（只给维护者看，不会发给
   用户）。凡是涉及谁校验、谁负责的地方，必须点名是哪一层（命令层、指令入口层、业务层、utils 原语），
   例如"权限在业务层校验：本函数由 [`crate::handle_all_bots_control`] 调用，它先判
   `has_permission(caller_id, RequiredPermission::MustBeAdmin)`"，只写"由 X 统一校验"看不出层次。
7. 参数清洗的归属：Discord 给的是结构化 option，但 option 的值仍然是用户输入的原始文本。命令层只做
   取值、校验数字、分发，原样透传（参数名以 `raw_` 起），谁真正要用就由谁清洗一次：机器人名走
   `ToCleanBotName::to_clean_bot_name`（`BotDirEntry` 构造条目时调用，非法名字返回 `Err` 而不是过
   滤），游戏内指令片段走 `utils::ToCleanBotCommand`。同一份文本全链路只清洗一次，第二处不该再出现
   `trim()`。配置这一侧同理：字符串值由 `config::Config::from_toml_text` 统一 trim 一次，读配置的人递原值。两条不变的
   规矩：机器人名的口径唯一来源是 `utils::bot_name`；需要业务状态才能判的检查
   （Token 是否存在、会话在不在跑、权限与目录归属、`max_reply_log_lines` 封顶）一律留在业务层。数字是命令层
   唯一的转换：`/log` 的行数在命令层解析并校验后转 `usize`，`/ping` 的端口同理，校验不过当场回用法提示
   （Discord 的整数 option 只有有符号 64 位，所以 option 声明成 `i64`）。
8. 配置只在需要它的地方读，而且只读需要的那几个字段：逻辑重、值得单测的函数吃参数
   （`launch_command_line` 的取值全靠参数传入）；只有一两行逻辑的便利函数直接读
   `config::Config::get().表.字段`（例如 `session_name` 只读 `runtime.screen_prefix`、
   `BotTokenStatus::from_dir` 只读 `runtime.token_file`）；状态判定
   `BotDirEntry::connection_status()` 是这类做法里的重逻辑版本，关键词是全局固定的一份（一半在配置的
   `[keywords]` 与 `[command_echo]`，一半在语言表 `text` 的 `jar_*` 方法），所以它自己读配置与
   `text::get()`、不通过参数传（挂在条目上，自己挑最新那份日志）。任何函数都不接收整份 `Config` 再自己
   挑字段。配置启动读一次、之后常驻内存，所以便利函数读一个字段没有额外成本，而逻辑函数吃参数换来了
   可单测。详见 [docs/mods/utils/utils-zh_cn.md 第 6 节](../mods/utils/utils-zh_cn.md) 与 [docs/mods/config/config-zh_cn.md 第 7 节](../mods/config/config-zh_cn.md)。

### 2.2 不变量清单（改动前逐条确认）

* 停止必须彻底：所有停止入口只走 `utils::stop_bot_sessions`（Ctrl+C、等待、强杀、复核），绝不允许留下占着
  游戏账号的孤儿 JVM；systemd 侧靠 `KillMode=control-group` 兜底。
* 日志两条线，磁盘上只有一份机器人日志：
  * 机器人日志：`<机器人目录>/<runtime.bot_logs_dir>/年-月-日-时-分-秒.log`，拉起会话前建好
    （`create_bot_log_file`），会话里 `grep -v -E` 过滤后 `tee -a` 追加；`/log` 与 `/show status` 读它；
  * FishingBot 自己的日志：同层 `raw/log0.log`，一个指向 `/dev/null` 的符号链接。它建不出日志文件会
    拒绝启动，所以给它一个能写但什么都不存的目标。不要改成保留一份原始日志；
  * 程序日志：`<部署目录>/logs/年-月-日-时-分-秒.log`，终端同时有一份，一次运行一份、不轮转也不
    清理，见 [docs/mods/process-log/process-log-zh_cn.md](../mods/process-log/process-log-zh_cn.md)；
  * 日志文件名的唯一来源是 `bot_logs::create_timestamped_log_file`，不要在别处拼时间戳文件名。
* 机器人状态的两个来源：连接状态只能由日志确认（只有 JVM 知道它连没连上、有没有掉线），所以它不跨调用
  保存，每调用一次就现取一次（`BotDirEntry::connection_status`，调用者拿到的永远是最新的）；暂停状态以
  自己确认到的命令回显为准，进程内记着（`bot_status::BotPauseStatus`，按机器人名存，
  `update_bot_pause_status` 是唯一写入口），会话开始与结束时重置（`forget_bot_pause_status`）。这样
  `/show status` 就不必为了找一个从没出现过的 pause 关键词扫到文件开头。写暂停状态的只有 `/pause` 与
  `/unpause` 那条确认路径，别处只读。例外是"进程已结束"：它不看日志，会话只剩 `(Dead ...)` 残壳时
  `running_bots` 直接给这个状态（JVM 已经没了，日志最后一条可能还是旧的"在线"）。
* `/show status` 的表格没有专门的类型：数据行由 `utils::running_bots::all_running_bot_status_rows()` 当场拼
  （四格 + `" | "`），表头由 `show/status.rs` 当场拼，两处列序要对齐（列名在 `text` 里，中英各一份）。
  完整写法见 [docs/mods/commands/commands-zh_cn.md 第 6.2 节](../mods/commands/commands-zh_cn.md#62-show-status)。
* 回复口径不许夸大：`/start` 与 `/restart` 只能确认命令已发送、会话已出现，因此回复写"已发送启动命令"；
  `/stop` 的停止流程当场复核，才可以回"已成功停止"；`/pause` 与 `/unpause` 发完等配置里
  `command_echo.wait_ms`（默认 300 毫秒，`utils::bot_status::confirm_pause_command`）到日志末尾找回显，
  看到回显才回"已确认暂停"或"已恢复"，没看到只说"已发送"。新增指令时按这条原则选词：能验证到哪一步，
  就只说到哪一步。
* 交互必须 3 秒内应答，否则用户看到的是"交互失败"。而我们要做的事（起 JVM、排队等待批次冷却、跑
  `screen -list`、按台数等待、读日志）常常远超 3 秒，所以：
  * 会产生进程操作或外部探测的指令（`/start`、`/stop`、`/restart`、`/pause`、`/unpause`、`/command`、
    四个 `*all`、`/ping`、`/checkserver`、`/log`）一律先 `ctx.defer()`；其中 `/log` 用
    `ctx.defer_ephemeral()` **并且每条回复都显式 `.ephemeral(true)`**（日志属于敏感内容，只有调用者
    看得见；为什么不能只靠 defer，见 `docs/mods/commands/commands-zh_cn.md` 第 3 节）；
  * 纯本地文案指令（`/help`、`/whoami`）不 defer，直接答；
  * defer 之后的回复有两条（Discord 的兼容行为，官方文档已写明）：第一条替换那条"正在思考"占位，
    第二条起才是新消息。因此长指令（四个 `*all` 与单机器人的 start、stop、restart）按两条消息发：
    启动指令先确认请求已加入全局启动队列；其他长指令先报告开始执行；任务完成后再发送
    `Report::render()` 的完整结果。
    第二条消息按 Discord defer 规则作为新消息发送；如改动该行为，需在真实部署中回归验证。
  * 交互令牌只有 15 分钟：所有 defer 后的回复必须在这个窗口内发出。批量启动跑得比它久时（台数多、
    `[startup_queue] bot_login_interval_ms` 大）followup 一定失败，所以长指令的最终报告走
    `commands::say_final_report`：发不出去就改发普通频道消息（普通消息不受交互令牌约束），并留一条
    `LogText::ReportPostedToChannel` 日志说明原因与失败原因。
* 敏感输入必须走 Modal，不能当指令参数：斜杠指令的每次调用都会在频道里生成一条消息（Discord 的
  `CHAT_INPUT_COMMAND` 消息类型就是它），参数值就在那条消息里，而 `ephemeral` 只管回复、管不了调用
  本身。所以 `/token` 用 `#[derive(poise::Modal)]` 弹窗收集（输入走 `MODAL` 回调、不进频道），提交后
  再用 followup 加 `ephemeral` 回结果。弹窗必须是交互的第一个响应，因此这条指令不能先 `defer`（先 defer
  会让弹窗失败）。新增"要用户填敏感内容"的指令时照这个来。
* 回复必须塞得进一条消息：Discord 单条消息硬上限 2000 字符。面向用户的长文本一律走 `Report`
  （`render()` 按 `runtime.max_reply_chars`，默认就是 2000，从末尾按行截断并注明省略行数）；超长会直接
  发送失败，用户那边看起来什么都没发生，不要自己拼无界字符串直接回。计数口径是 `str::utf16_len()`
  （`encode_utf16().count()`），`Report::render` 按它收口，不用 `chars()`（码点）：非 BMP 字符
  （emoji）在 UTF-16 里占 2 个，取更严的一侧才能保证 2000 发得出去。
   按 UTF-16 单元计数比按 Unicode 码点更严格；`/log` 不受这条限制，它走附件。
* 每次回复都留一行程序日志：命令层不直接 `ctx.say`，面向用户的回复一律走 `commands::say_and_log`
  （业务层经 `commands::log_reply`）到 `LogText::CommandReplied`，日志里留下
  `回复 /指令 (调用者 用户ID): 内容`。这条把谁在什么时候收到了什么固定在程序日志里，出问题时先查它。
* `/log` 用附件发，不再截断：读最后 N 行，作为附件上传（文件名就是日志文件名），正文写标题、行数、
  文件名；用 `defer_ephemeral()` 加带附件的回复并显式 `.ephemeral(true)`，别人看不到。附件大小上限取
  `MAX_ATTACHMENT_BYTES`（8 MiB，Discord 普通上限是 10 MiB，留余量；这是平台限制而不是用户偏好，
  所以是常量、不进配置）；超了按行截断并在正文里说明。
* `keywords.filter_regex` 不能含单引号（它会内联进 `bash -c` 里的 `grep -v -E '...'`）。
* 配置只在启动时读一次：运行中改配置不生效，重启进程才生效（刻意不做热重载）。
* `bot_dirs` 是目录口径的唯一来源：`base_dir/bots/Discord用户ID/机器人名`，权限边界在用户 ID 那一层。
* 运行中不许动要先查会话：`/bot del`、`/bot rename`、`/token`、`/bot login` 动手前都先
  `is_bot_screen_running`（`refusal_if_not_stopped` 一处出文案，运行中直接拒绝）：改成运行中的 Token 不会生效、
  删运行中的目录会毁掉数据、改运行中的目录名会把它接下来的日志写丢、登录要先独占凭据文件。重启流程更严：
  缺 Token 的目标在停止之前就剔掉（重启是先停再起，无谓的停止会让仍在钓鱼的账号下线）。
* 凭据只由 JAR 写，本项目不自己实现微软登录：`/bot login` 拉起的会话就是该机器人平时的会话，它把 JAR
  打印的设备码（登录网址 + 代码）转给调用者，凭据**以 `refreshToken` 文件出现为准**（不解析"授权成功"那
  条消息）：新凭据一出现就说明认证成功，这一步同时删除挪开的旧凭据（`<凭据文件名>.renewing`）。动手前要把
  已有凭据挪走，否则 JAR 看到它就直接用、不打印设备码，这条指令会变成"什么都没发生"。
  两条观察到的 JAR 行为：**没有有效凭据时它只等待登录、不会去连游戏服务器**；请求频繁被微软拒绝
  （返回空 token）时它会自己结束 JVM —— 所以第二段把"会话消失"直接当"启动后退出"报出来。
  详见 [docs/mods/commands/commands-zh_cn.md 第 4.6 节](../mods/commands/commands-zh_cn.md#46-bot-login)。
* 游戏侧配置（FishingBot 的 `conf.json`）是**生成物**：进程启动时先删旧文件、再用随仓库发布的完整模板
  （`fishingbot-config.example.json`）当底、覆盖 `[fishingbot]` 那几个键（`utils/fishingbot_config.rs`）。写出来的
  一定是完整配置，所以不依赖 jar 补字段。实测（v2.13.15，`-onlyCreateConfig`）：拿生成的文件喂 jar，它
  既不打印 `Your configuration was incomplete...`（极简配置会打印）、也**一字不改**我们的文件；语言按我们
  写的值生效（`Using language: CHINESE_SIMPLIFIED`）。手改那份 JSON 不会留到下次启动；模板是上游改配置
  结构时唯一要跟着换的东西（换了模板跑 `cargo test`，托管路径找不到就会红）。
  `misc.language` 写的就是 `[i18n] language` 的取值（取值就是 jar 的常量名，同一个值不是映射过去的），
  不给手填的机会，这样"按哪种语言的 JAR 日志认重连"不会再对不上。生成失败按配置写错处理（退出码 1）。
  详见 [docs/mods/config/config-zh_cn.md 第 3.2 节](../mods/config/config-zh_cn.md)。
* 取哪批目录是各指令的口径：`utils::split_bot_entries_by_is_running(&[BotDirEntry])`（在 `running_bots`
  里）只做分组，目录范围由调用方自己挑函数决定：`all_bot_dir_entries()`（全量，用于 `/startall`、
  `/restartall`、`/stopall` 与进程收尾，缺 Token 的孤儿进程也要能被收掉）、
  `token_ready_bot_dir_entries()`（只看配好 Token 的，用于 `/commandall` 与 `/show status`）、
  `bot_dir_entries_of_owner(owner_id)`（只看某个账号，用于 `/show mybots`）。目录与会话这条 join 只允许
  在 `running_bots` 一处实现。
* 每条指令都声明一个权限要求，判定只有一个出口：要求写成 `utils::permission::RequiredPermission`
  （`Everyone`、`MustBeAdmin`、`OwnerOrAdmin(归属用户ID)`），校验一律
  `has_permission(caller_id, 要求)`，名单来自 `[admins]`。
  * 全员指令（`/help`、`/whoami`、`/show status`、`/ping`、`/checkserver`、`/show *`）声明 `Everyone`，在
    `lib::handle_*` 里判一次（这一层是它们唯一的入口）；
  * 批量指令（`/startall` 等）声明 `MustBeAdmin`，也是 `lib::handle_all_bots_control` 判一次，
    `all_bots_control/*` 因此不接 `caller_id`、也不自查；
  * 单机器人指令与账号管理指令声明 `OwnerOrAdmin(目标机器的归属)`：归属要先把目标解析出来才知道，所以
    路由层先 `BotDirEntry::find_by_name` 找条目、再判（普通用户只能碰自己目录，管理员可跨目录；找不到与
    没权限回同一句话）。底层函数一律不校验，也不接 `caller_id`。

  命令层只取值与校验参数个数，不看权限。新增指令一律照这个来：命令层必须把结果交给 `lib::handle_*`、
  不许自己拼回复（`/whoami` 也一样），因为没有注册表或框架钩子兜底，只能依赖这条约定与评审。
  * 选项候选不另立一套权限：`src/commands/autocomplete.rs` 查询有权限查看的机器人目录、按运行状态和指令语义筛选
  机器人名候选（管理员是全部机器人，其他人是自己 `bots/<用户ID>/` 下的，与 `OwnerOrAdmin` 同一口径），并负责
  Discord interaction 适配及 `AutocompleteChoice` 转换。候选只是建议：用户仍可
  手打任意名字，归属与会话是否在跑照旧由各 `handle_*` 判。
* utils 不拼报告，只给原始信息：流程函数返回明细（例如停止的 `StopOutcome`，自带 `into_report()` 与
  `to_failure_text()`），拼装这一步必须由上层显式调用。批量停止由
  `all_bots_control::stopall::stop_all_bots()` 显式拼，进程收尾在
  `watchdog::Watchdog::exit_on_shutdown_signal()` 里就地拼同一份明细（只有标题不同："收到退出信号，收尾结果"；
  这份报告除了进日志，还会发到各服务器的通报频道（发送有 30 秒总时限，超时就跳过、不补发），而掉线与永久错误的收尾只进日志）。
* 可见性，每个模块一个对外面：crate 的对外面只有 `lib.rs`（`pub fn handle_*`、`pub use` 的两个视图类型、
  `pub mod config`、`pub mod text`）；`utils` 与五个业务模块是同一形状，子模块私有 `mod`，父模块顶部的
  一份精选 `pub(crate) use` 就是它的面，所以 `lib.rs` 只认识 `crate::bot_control::start_bot` 这种路径
  （能不能从外面调，看那份清单即可）。
* 网关看门狗是唯一判定掉线的地方：判定规则见第 3 节，收尾共用库侧的
  `stop_all_bots_on_disconnect`（私有；停机器人并记原因与时刻）：判定侧只调 `request_reconnect` 把掉线原因交给入口
  并叫醒它，停机与重建连接都在入口的 `wait_for_attempt()` 里做。只有配置写错才走
  `exit_on_permanent_failure`。不要试图从框架内部状态判：serenity 0.12.5 的
  `ShardMessenger`（也就是 `ctx.shard`）不暴露连接阶段与延迟（实测只有 `shutdown_clean`、
  `set_presence`、`chunk_guild` 等），`Shard` 又在 shard runner 内部拿不到；网关侧的半死由 serenity
  自己的心跳与 ACK 机制发现并自动重连，我们只兜"连都连不上"与"网络断了"两种。
* 当前没有并发互斥：Discord 里多个用户可以同时点指令，对同一批机器人同时发 `/startall` 或 `/stop` 可能互相
  干扰。若实际使用显示有需要，可在命令层增加按机器人名加锁的约定，并记录于本节。
* 不做单实例锁，靠"一台机器一份配置一个实例"：serenity 是 Gateway 客户端、不占端口，两个实例都能连上
  Discord 并收到交互，症状只是同一条指令回两次；本进程不持有权威状态（状态在 `screen` 会话与磁盘上），
  重复实例不会互相把状态搞乱。加锁只能是机器级作用域，会挡住"换个 `base_dir` 并行跑一份"这种正常用法。
* 面向 Discord 的文案必须走 `text::get()`（机制见 [docs/mods/i18n/i18n-zh_cn.md](../mods/i18n/i18n-zh_cn.md)）：业务层与 utils 里不许
  再写面向用户的中文字面量。加一种语言等于在 `src/text/` 加一个文件实现 `Text`（含 `command_texts()`
  那张指令说明表）并在 `Language` 里登记：漏实现一个方法编译不过，漏一条指令说明由覆盖测试拦下；合法
   取值只在 `Language::supported_names()` 一处。当前机制与自查方法见
   [docs/mods/i18n/i18n-zh_cn.md 第 6 节](../mods/i18n/i18n-zh_cn.md)：不只是回复文案，程序日志、启动期控制台提示、配置报错也走同一套语言
  （`LogText` 枚举加 `log_text()`，两个语言文件里穷尽 `match`）；指令说明与 `/token` 弹窗标签则靠注册前
  改写 poise 生成的指令树加手写 `impl poise::Modal`；JAR 自己打印的断线重连提示也在里面（`jar_*`
  方法），于是 `[i18n] language` 同时决定"我们说什么话"与"按哪种语言的 JAR 日志认重连"；`/pause` 与
  `/unpause` 的回显则由配置 `[command_echo]` 提供（所用 JAR 支持这两条指令时才有那两句，见 §2 第一条）。唯一固定英文的
  是配置还没读出来时的那些提示（`text::bootstrap`：配置文件
  读不出来或解析不了、生成模板、找到已存在的配置文件、可执行文件路径、`language` 写了个不认识的取值，还有 `config::Config::get()` 的
  `CONFIG_NOT_LOADED` 与语言未安装时的 `TEXT_NOT_INSTALLED` 不变量消息；它们都是英文），因为那时语言还没确定。`text::get()` 未安装语言时，
  生产构建直接 panic（进程入口的调用顺序错误）；测试构建固定返回英语。`Language` 不再实现
  `Default`，配置里的 `language` 不受支持时启动校验直接报错退出。示例配置写 `language = "ENGLISH"` 只决定新部署运行哪种语言，中文 jar 改成
  `CHINESE_SIMPLIFIED`。
  措辞以中文为准：改文案先改 `zh_cn.rs`（内容与排版的原稿），再把 `en_us.rs` 逐条对齐，最后同步
  `commands/*.rs` 里那份与英文表逐字相同的兜底（顺序也写在两个语言文件的模块头里）。
  排版约定：面向用户与部署者的文案符号随语言（中文全角、英文半角），句尾不留句号（叹号保留）；补充说明
  统一写圆括号 `()`；例外是配置表名（`[admins]`）、`Report` 的小节标签（`[成功明细]`、`[Success]`）与用法
  提示里的参数记号（可选写 `[行数]`、必填写 `<机器人名>`）；错误回复统一以 `错误:`（英文 `Error: `）开头，
  见 [docs/mods/i18n/i18n-zh_cn.md 第 5 节](../mods/i18n/i18n-zh_cn.md)。
* 关键词放哪儿，谁决定的字符串就放谁那儿（`[keywords]` 收敛到两项的依据）：只有各部署不同、必须
  自己填的才进配置，即 `online`（游戏服务器自己的进服提示）与 `filter_regex`（自己服务器要丢掉的刷屏
  噪声）；`/pause` 与 `/unpause` 的回显进 `[command_echo]`（`pause_echo`、`unpause_echo`），理由不是
  "由谁决定"而是"不同 JAR 构建可能提供不同回显"，就地能改比重新编译实际；JAR 自己打印的断线重连提示才进 i18n
  的 `jar_*` 方法（见上一条），让部署者再抄一遍只会多一个出错点：抄错一个标点，`/show status` 就永远显示
  "初始中"。**"进程已结束"不进配置也不需要关键词**：旧版外壳脚本往日志里写的那句结束标记
  （`fishingbots` 脚本：`java … | tee bot.log; echo 标记 >> bot.log`）是脚本的产物，本项目自己拉起
  会话、不写它，判定看 screen 会话状态（`(Dead ...)` 残壳），这样不受 jar 版本与脚本写法影响。

## 3. 网关看门狗（唯一判定掉线的地方）

目的：判定本进程已经跟 Discord 失去联系之后，别再假装它还被人指挥得动。连不上 Discord 时我们既控制不了
机器人，而机器人还占着游戏账号（FishingBot 的自动重连会跟真人玩家抢号），所以必须停干净；然后进程不退出，
等一个判定窗口重建连接（细节见 [docs/mods/watchdog/watchdog-zh_cn.md 第 4 节](../mods/watchdog/watchdog-zh_cn.md)）。

两条判据都写在 `src/watchdog/gateway.rs`（纯计时、不依赖框架、可单测）：

* 握手：本次尝试开始后 `first_handshake_timeout` 内必须收到过一次 `Ready` 或 `Resume`（bin 侧的
  `event_handler` 调 `watchdog.record_handshake_success()`）。不满足时请求重建连接，原因是 `NeverReady`。
* 可达：每 `probe_interval` 用 `Http` 主动打一次 `GET /gateway`，拿到任何 HTTP 响应都算可达（含
  401 与 403），只有超时或连接错误才算不健康；连续不健康累计满 `window` 时请求重建连接，原因是
  `Unreachable`。

细节：

* 时序来自配置里的判定窗口 `timing.watchdog_window_ms`；探活间隔（窗口的 1/3）与首次握手超时（窗口的 2 倍）
  由它派生（`TimingConfig::probe_interval` / `first_handshake_timeout`），不再是配置项——这两个值只有按窗口
  比例取才有意义。入口把三项装进 `Watchdog`，判定与探活共用同一份；派生同时保证了"探活比窗口还慢、一次
  失败就判掉线"这种组合配不出来。
* 探活只看网络可达性，不看凭据对错：Token 错会让 `client.start()` 自己报错退出，不归看门狗管。
  ?探活请求发往 `discord.com`，网关连接 `gateway.discord.gg`；如果部署中出现 API 可达但网关不可达，再增加
  针对网关域名的 TCP 或 TLS 探活。
* 探活任务在 `client.start()` 之前就拉起（在 `main.rs` 里）：网关从头到尾连不上时 `Ready` 永远不会来，
  必须在 client 之外也能判；提前拉起还能让"启动就断网"在首次握手超时内就退出。
* 判定是可逆的：探活恢复、或又收到 `Ready` 与 `Resume`，就回到 `Alive`，不是判一次就回不来。
* 四态为 `Alive`、`Waiting`、`NeverReady`、`Unreachable`（看门狗内部，不对调用方公开），判定时刻由调用方传入
  （`state(now)`），因此每条规则都能用单元测试覆盖。
* 收到第一个 `Ready` 时打印一行
  `网关看门狗已就绪：判定窗口 N 秒、探活间隔 N 秒、首次握手超时 N 秒`，排查时以这一行为准（它反映实际
  生效的三项参数）。

判定不通过就走库侧的 `request_reconnect`：停全部机器人，记下原因与掉线时刻，叫醒进程入口重建连接（等一个
判定窗口后再装配一次 `Client`）。进程不退出，因为掉线是常态，退出交给 systemd 重拉只会每 10 秒刷一份
日志、还得每次冷启动；只有配置写错（Token 无效、机器人没被邀请）才走 `exit_on_permanent_failure`
（退出码 1，本项目的主动失败专用码，systemd 单元里配 `RestartPreventExitStatus=1` 就不重拉）。

## 4. 启动管道（示范步骤显式的写法）

启动由启动队列分四段完成，命令侧只碰第一段：

1. `StartupQueue::start_bots(bots)`：命令侧入口，建账本 → `add_bots_to_queue` 入队 → `wait_until_done()` 等这些条目都被拉起过一遍，取回结果交给 `verify_batch`。
2. 启动线程 `run_starter`：等队列非空 → 等够 `[startup_queue] batch_login_interval_ms`（从上一批出队时刻算起）→ `pop_bots_from_head(max_batch_size)` 取一批。
3. `start_single_batch` 逐台调用 `launch_one_bot` → `add_bot_for_login`：建本次日志文件（失败就不拉会话）→ 准备 FishingBot 的 raw 日志目录（`<bot_dir>/<bot_logs_dir>/raw`，`raw/log0.log` 指向 `/dev/null`）→ 用 `screen -dmS` 拉起会话，命令形如 `java … | grep -v -E '<filter_regex>' | tee -a '<log_path>'`；相邻两台之间等 `bot_login_interval_ms`。
4. `verify_batch`：拉起后立即用 `screen -list` 快照判定一次，等满 `timing.check_grace_ms` 再判定一次；只有第二遍仍在才算拉起成功，日志只说明是否已确认进服。

新增流程请沿用这个风格：每一步一个函数、每步都能被单独读懂。

## 5. 构建与验证（提交前跑完）

```bash
cargo fmt --check                      # 格式
cargo test --lib                       # 库测试（看门狗判定、配置 schema 与校验、Report）：Linux 上默认目标 musl 直接跑
cargo test --bins                      # 命令层与进程入口的测试
cargo check --all-targets              # 真实目标：x86_64-unknown-linux-musl（静态）
cargo doc --no-deps                    # 文档链接：必须零警告（只用 cargo check 抓不到断链）
```

测试分两类（Rust 标准），不要互相搬：

* 单元测试与代码同文件（`src/*.rs` 里的 `#[cfg(test)] mod tests`）：测私有逻辑，改了私有函数就跟着改；
* 集成测试在 `tests/`：把库当外部依赖用，只打 `src/lib.rs` 里 `pub` 且能从 crate 根走到的项。它是对外
  契约的守门人（谁把公共项改私有、改签名，那里就编译不过）；
* 公共测试助手：单元测试放 `src/test_support.rs`（`#[cfg(test)]` 才编译），集成测试放
  `tests/common/mod.rs`（需要时再建）。

另外几条：

* 改动配置字段必须同步 `minecraft-fishingbot-manager-for-discord.conf.example.toml` 与
  [docs/mods/config/config-zh_cn.md](../mods/config/config-zh_cn.md)，否则 `cargo test` 会失败（有测试同时钉这两件事）。
* 注释里反引号中的本 crate 代码项写成可跳转链接（`` [`logger`] `` 这种），措辞用书面语，不用粗体标记、
  破折号与书名号引号，段头注释统一成 `============ X ============`。
* 文档注释里，公开项的文档不引用私有项（rustdoc 链不到、读者也看不到）：改成直述其含义，能链的补成
  链接。私有项自己的文档里可以正常链接私有项。
* 提交前 `cargo clippy --all-targets -- -D warnings` 也要零警告。

## 6. 代码地图（按看名字就知道的顺序）

```
src/main.rs                 进程入口：配置 → 程序日志 → 显式建看门狗（WATCHDOG.get_or_init()） → 生成游戏侧配置 → REST 探活任务 → poise 装配 → 运行 → 掉线重连 / 信号退出
src/lib.rs                  指令入口层：动作枚举 + handle_* + 退出与重连入口
src/commands.rs             命令层：poise 指令（一个业务模块一个文件，见下）
src/commands/               bot_control / bot_manage / token / log / all_bots /
                            show / general（`/help`、`/whoami` 在这里）/ registration（全局注册与残留清理）
src/watchdog.rs             网关看门狗本体：时序字段 + 握手/探活/掉线记账 + 进程级单例 WATCHDOG（第 3 节）
src/watchdog/               gateway（握手与探活判定 + 探活任务）/ lifecycle（进程收尾）
src/process_log.rs          程序日志：终端 + <部署目录>/logs/时间戳.log
src/config.rs               外置配置：运行时结构 Config 与 Config::get_or_init / Config::get / Config::config_file_path / Config::admins / Config::install_dir
src/config/                 schema / parse / validate / template
src/text.rs                 文案入口：一个进程一种语言（`[i18n] language` 启动时选定一次）
src/text/                   en_us（英语）/ zh_cn（两份一一对应，只换字面量）
                            + bootstrap（引导期英文）/ log_text / requirement（数据型枚举）
                            + setup_failure（装配失败：按成因选文案 + 判是不是永久错）
src/bot_control/            单机器人业务（一指令一文件）
src/bot_manage/             账号管理业务（一指令一文件）
src/all_bots_control/       全服批量业务（仅管理员，一指令一文件）
src/show/                   只读查询业务（一指令一文件）
src/general/                通用业务（一指令一文件）
src/utils/                  原语层：bot_command / bot_dirs / bot_name / bot_logs /
                            bot_status / fs / fishingbot_config / permission / reply / restart /
                            running_bots / screen / shutdown / startup / uptime
```

命令层按业务模块分组：`src/commands/` 下的 7 个文件与 `lib.rs` 的动作枚举一一对齐，既让文件名说明模块
与功能，也避免把每条指令拆成一个只有十几行的文件。业务层仍然是一指令一文件。

## 7. 已经定型的几个技术选择（讨论过，不要反复改）

1. 机器人日志只留一份：写入前过滤（`grep -v -E` 加 `tee -a`），FishingBot 自己的日志文件指向
   `/dev/null`。不采用"原始日志写 `/tmp`、过滤后写机器人目录"的双份方案，因为磁盘（或 tmpfs 内存）多一份、
   要额外的清理与权限逻辑，而且历史上正是两份日志落在同一层目录让 `/log` 读错文件。要排查过滤规则时，
   临时把 `keywords.filter_regex` 调空并重启进程即可看到全部行。
2. 模块用新版风格（`src/utils.rs` 加 `src/utils/*.rs`），不用 `mod.rs`。两者对编译器完全等价，但新版风格
   在编辑器标签页与搜索结果里能直接看到模块名，且全项目零 `mod.rs`，风格统一。
3. 只用斜杠指令，不做 `!前缀` 指令：前缀指令要读消息内容，需要 `MESSAGE_CONTENT` 特权意图（等于在开发
   者面板多一步，而且大服务器还要过审）；斜杠指令不需要任何特权意图，开发者面板一个开关都不用勾，还自带
   参数提示与补全。这条也决定了命令层不解析文本。
4. `/log` 用附件而不是截断：2000 字符装不下 100 行日志，截断后信息量损失太大；附件既能全量给出，又天然
   带上文件名。私密性由 `defer_ephemeral()` 加每条回复的显式 `.ephemeral(true)` 保证。
5. 长指令分两条消息回复（`*all` 与单机器人 start、stop、restart）：先回一条回执（启动类回"已接受 … 请求"，
   `/startall` 还会给出这次要启动几台、队列里已有几台、预估耗时与等结果的超时，见
   `lib::handle_startall_notice()`；其余回"已开始执行 …"），
   跑完再回完整报告。
   这是 Discord 官方文档写明的 defer 行为（第一条替换占位、第二条起是新消息）。
6. 游戏侧配置由本程序生成、模板随仓库走：FishingBot 只认 JSON、又没有逐项覆盖的命令行参数，所以
   `[fishingbot]` 当唯一真源，启动时"删旧文件 + 用模板当底覆盖我们那几个键"，写出来的一定是完整配置
   （不依赖 jar 自动补齐）。模板是 `fishingbot-config.example.json`（完整的一份 FishingBot 配置，按本项目
   口味调过默认值）；保持它与 FishingBot 的配置结构同步，改模板后运行 `cargo test`。

## 8. 当前状态与待办

当前状态：这是 Discord 实现，已在 Linux 环境运行。代码与文档同步维护；文档清单见 [docs 文档索引](../index-zh_cn.md)。
改动后应运行自动化检查；下列清单记录的是需要在真实部署中端到端验证的事项。

构建环境的一条前置条件：rustls 的加密后端需要一个 C 编译器，musl 静态构建还要 `musl-gcc`（Arch 装
`musl`，Debian 与 Ubuntu 装 `musl-tools`）；装齐后 `cargo build --release`（默认目标
`x86_64-unknown-linux-musl`）能直接出静态二进制。

待在真实部署上完成的检查：

1. 端到端走通 Discord 交互：`/log` 与 `/show botconfig` 附件、defer 后第二条回复，以及 `/token` 弹窗提交后的
   followup。弹窗流程见[指令说明第 1.3 节](../mods/commands/commands-zh_cn.md#13-敏感内容ephemeral-与-modal)。
2. 实际走通 `/bot login`：确认登录链接与代码送达、凭据写入机器人目录、已有凭据时成功刷新后
   `refreshToken.renewing` 被删除。见[指令说明第 4.6 节](../mods/commands/commands-zh_cn.md#46-bot-login)。
3. 用真实游戏服验证生成的 FishingBot `conf.json`、机器人连接，以及切换 `[i18n] language` 后
   `/pause`、`/unpause` 的对应语言回显。模板完整性已用真 JAR 验证；见[配置说明第 3.2 节](../mods/config/config-zh_cn.md)。

暂缓的改进（不是发布阻塞项）：为 `/start`、`/stop`、`/restart` 增加多机器人参数（现有 `*all` 指令仍可用）；
程序日志自动轮转；命令层并发互斥；`/bot login` 并发上限。按实际使用需求再评估；登录并发的取舍见
[指令说明第 4.6 节](../mods/commands/commands-zh_cn.md#46-bot-login)。

## 9. 相关文档

* 全部文档见 [docs 文档索引](../index-zh_cn.md)。
* 按主题查：[tutorial/tutorial-zh_cn.md](../tutorial/tutorial-zh_cn.md)（部署）、[architecture-zh_cn.md](../mods/architecture/architecture-zh_cn.md)（分层与目录结构）。
* 模块技术文档：[mods](../mods) 下每个模块一份（见索引第 3 节）。

---

## 附录 A：本轮"注释与文档整理"的工作状态（会话交接用）

> 本节由 AI 协作者在整理过程中写入，供下一次会话直接接续；它不属于项目不变量，整理完成后可删除。

### A.1 用户提出的要求（逐条照办）

1. **只改行文与注释**：不动键名、取值、占位符、代码结构与行为；改完必须 `cargo test` 全绿。
2. **不提交**：所有改动留在工作区，由用户一次性提交（常用聚合提交，如 `git commit -am "Improved comments"`）。
3. **行文标准**：自然、简洁、逻辑清晰；顺序为"是什么/做什么 → 为什么 → 怎么用"。
4. **一句话不拆成多行**：Markdown 里也不要把同一句拆到两行，示例配置的注释同样适用。
5. **避免口语化**：不写"清掉""也不用""白等""没过""顺手""搞定""干脆""其实"这类说法；不称呼读者（不用"你/我们"）；不拟人。
6. **中英成对同步**：`zh_cn` 与 `en_us`（以及 `docs/.../*-zh_cn.md` 与 `*-en_us.md`）逐句对应，术语一致（`JAR`、`Token`、`screen` 会话）。
7. **删除写死的无用测试**：逐字钉文案的测试一律删掉，也不要为新文案再补这类测试。
8. **简化逻辑**：能删的类型/函数就删。示例：`/show status` 去掉 `RunningBotInfo`、`status_table_header()`、`into_row()`，改为在调用处用数组 + `join(" | ")` 拼行。
9. **按顺序推进**：`docs/mods/architecture/` → `docs/mods/config/` → `docs/mods/lib/` → `docs/mods/commands/` → `docs/mods/utils/` → `docs/CONTEXTS/` → 外围（index、README、tutorial、refs、troubleshooting）。

### A.2 已完成的改动（全部未提交）

**代码与文案**

* `src/text.rs`：删除 `add_and_login_messages_are_exact_in_both_languages`（原 593–676 行）及其残留注释，共 -89 行；保留的文本测试均为结构性（语言名解析、英文实现、重连前后缀非空且中英不同、`log_text` 拼装、语言名可解析、未安装时返回英语实现）。
* `src/text/zh_cn.rs`（9 处）：156（"带斜杠是指令/也不用加引号" → "为指令/无需加引号"）、392（`错误:` 补空格、"JAR 参数不正确"）、655（"按当前分批设置，预计最长耗时…等待启动结果的超时时间"）、798（"微软对频繁登录限流"、`(` 前补空格）、807（全角括号改半角）、812（"重启会登录失败，因此不作改动 (如需停止请使用 /stop)"）、939（`服务器 (` 补空格）、1031（"启动复核未通过"）、1135（全角冒号改 `:`）。
* `src/text/en_us.rs`（6 处）：407（`Common causes:` 直陈罗列）、844 与 1092（`closed as unconfirmed` → `marked unconfirmed`）、850（`Microsoft throttles frequent logins, or because it ran out of memory`）、856（`no token file, so … ; left unchanged`）、1099（`did not pass the re-check`）。
* `/show status` 的简化已由用户提交为 `093d1a4`：删 `RunningBotInfo`、`status_table_header()`、`into_row()`，新增 `all_running_bot_status_rows()` 与私有 `status_row()`，表头在 `show/status.rs` 就地拼。

**文档**

* `README.md`（英文；`docs/README/README-en_us.md` 是它的符号链接）：按中文版重排为 §1–§8，补上"官方 `FishingBot.jar` 尚不支持 `/pause` `/unpause`"的警告与 `THIRD-PARTY-NOTICES.html` 一行，去掉英文版独有的"27 commands"计数与"Project structure"节；`cargo test --test docs_links` 为 6 passed。
* `docs/mods/architecture/architecture-zh_cn.md`（5 处）与 `architecture-en_us.md`（4 处）：`bot_log_files.rs` → `bot_logs.rs`、`running_bots` 不再生成表头、`utils::start_bot_sessions` → `StartupQueue::start_bots()`、拆开 §1 术语长句与 §5 单例长句。
* `docs/mods/config/config-zh_cn.md`（多处）：§1 三段长句拆分与去口语、§2 `get_or_init` 拆句、§3 排序句与 `[startup_queue]` 超长单行、§3.1 两句、§3.2 三处、§4 `[log]` 与 `[fishingbot]`、§5 把 `command_echo` 段移出列表、§7 首尾。
* `docs/mods/config/config-en_us.md`（3 处）：§2 `get_or_init` 拆句、§4 `[log]`、§5 `[log] debug`。
* `docs/mods/commands/commands-{zh_cn,en_us}.md`、`docs/mods/utils/utils-{zh_cn,en_us}.md`、`docs/CONTEXTS/CONTEXT-{zh_cn,en_us}.md`、`docs/mods/architecture/*`：清掉对已删 API 的引用（`status_table_header`、`RunningBotInfo`、`all_running_bot_infos`、`into_row`），并改写 `/show status` 表格的说明（表头在 `show/status.rs`、数据行在 `running_bots`，两处列序须一致，没有快照类型）。

**第二轮（本轮续做）**

* `docs/mods/config/config-en_us.md`：§1 的 trim、反序列化与生成 `conf.json` 三段改为短句；§3 表顺序与 `[startup_queue]` 段重写；§3.1 去口语并修正语言对齐的说法；§3.2 三处改直陈；§4 `[fishingbot]` 改为"整张表必填"并去掉"有默认值"的错误说法；§5 把 `command_echo` 段移到列表之后；§7 首尾改写。
* `docs/mods/config/config-{zh_cn,en_us}.md`：按代码修正 `StartupQueue::result_timeout` 公式（`check_grace_ms` 只计一次）、`[startup_queue]` 批间等待从出队时刻起算、`check_grace_ms` 为每次启动请求一次、`[fishingbot]` 整表必填。
* 失效名称：`docs/` 内 `bot_log_files` 全部改为 `bot_logs`；`start_bot_sessions` 改为 `StartupQueue::start_bots()`，`CONTEXT` 第 4 节的启动管道按当前 `start_bots` / `run_starter` / `start_single_batch` / `verify_batch` 重写。
* `docs/mods/lib/*`：对外面补全（`BotLoginPrompt`、`ProcessLog`、`StartupQueue`、`WATCHDOG`、`pub mod utils`），autocomplete 段落改为"命令层私有函数"。
* `docs/mods/commands/*` 与 `docs/mods/utils/*`：修正复核口径（拉起后立即判定一次、等满宽限期再判定一次，每次启动请求一次）、`/start` 与 `/startall` 的调用入口、`stop_single_bot_session` 的签名差异，复核归属改为 `startup::verify_batch`。
* `docs/CONTEXTS/CONTEXT-zh_cn.md`、`docs/index-*.md`、`docs/troubleshooting/*`：去除主要口语化表述，修正批间等待的算法；`docs/index-*.md` 去掉写死的指令条数。
* `minecraft-fishingbot-manager-for-discord.conf.example.toml` 与 `src/config/schema.rs`、`src/utils/startup*.rs`：修正 `batch_login_interval_ms`、`check_grace_ms` 的注释口径（从出队时刻起算、每次启动请求一次），只改注释。
* `src/text/zh_cn.rs`：全部 `错误:` 统一为 `错误: `；设备码缺失一句改为直陈原因（`。常见原因：`）。
* `src/**`：公开项的文档不再链接私有项（`MAX_AUTOCOMPLETE_CHOICES`、`result_timeout`、`create_bot_log_file`、`running_bots`、`ConnectionStatus::Stopped`、`PauseCommand::*`、`Report::push_list_block`、`Utf16Len::utf16_len`、`token_ready_bot_dir_entries`、`permitted_bot_dir_entry` 等改为普通代码字面量），`cargo doc --no-deps` 的 12 条警告清零。
* 验证：`cargo fmt --check`、`cargo test`（141 / 13 / 6 / 5 / 0，`failed=0`）、`cargo clippy --all-targets -- -D warnings`、`cargo check --all-targets`、`cargo doc --no-deps` 全部通过。

### A.3 状态

* `docs/mods/config/config-en_us.md` 的指定段落已处理。
* `lib`、`commands`、`utils`、`CONTEXT` 与外围文档已清理失效 API、修正批次与复核口径，并去除主要口语化表述。
* `src/text/zh_cn.rs` 与 `src/text/en_us.rs` 的长行已逐条复查；剩余超过阈值的都是单条格式化文案，保持整句。
* 未再发现阻塞项。可选的后续：`docs/mods/i18n/` 逐句复读（不在 A.1 的推进顺序内）。

### A.4 工作方法与已知的坑

* 终端工具常报 `exit 130` 且"抓不到输出"，但命令**实际已执行**。确认结果的办法：把输出重定向到 `/tmp/x.txt`，再用 `sed -n '/test result/p' /tmp/x.txt` 回读，或直接回读终端缓冲。
* **报告"拼写错误"前必须用 `read_files` 读原文**：窄终端缓冲会在长行折行处吞掉空格，本轮曾两次误报（`devicecode`、`wentto` 实际都不存在）。
* 扫描长行：`awk '/"/ && length($0)>80 {print NR": "$0}' src/text/zh_cn.rs`，英文阈值用 110。
* `docs/README/README-en_us.md` 是仓库根 `README.md` 的符号链接，改根文件即可；`cargo test --test docs_links` 校验链接与锚点。
* `minecraft-fishingbot-manager-for-discord.conf.example.toml` 由 `src/config/template.rs` 用 `include_str!` 内嵌，是用户首次启动所得配置的注释来源；改注释后必须保持 TOML 合法（`example_config_matches_schema` 校验），键名与取值一律不动。
* 测试基线：`cargo test` 为 `141 / 13 / 6 / 5 / 0`，`failed=0`。若失败集中出现在 `text::tests`，先检查是否又出现了"逐字比对"的测试——应当删除该测试，而不是改文案去迁就它。
