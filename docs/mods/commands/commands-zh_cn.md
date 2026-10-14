# commands：命令层与全部指令
[English](commands-en_us.md) | **简体中文**

* 代码：`src/commands.rs`、`src/commands/autocomplete.rs` 与 `src/commands/{general,show,bot_manage,token,log,bot_control,all_bots}.rs`
* 职责：取 option、校验数字、`defer`、把 `handle_*` 的结果发出去。权限、参数清洗、业务状态判断都在
  下一层（`src/lib.rs` 与各业务模块）。
* 硬约定：每条指令都必须落到 `lib::handle_*`（那一层负责声明权限要求并判一次），不许在本层自己拼回复，
  连 `/whoami` 这种只取作者信息的指令也走 `handle_whoami`。漏掉这一步就等于漏掉权限校验（没有注册表或
  框架钩子兜底，靠这条约定与评审）。
* 与业务层的关系：`commands/bot_control.rs`（命令层）只声明有哪些指令、参数长什么样、怎么回，真正的
  动作在 `bot_control/start.rs`（业务层）。两边文件名刻意同名，便于对照。

## 1. 命令层的规矩

### 1.1 为什么每个指令都先 defer

Discord 要求交互在 3 秒内应答，否则用户看到的是"交互失败"；而我们要做的事（起 JVM、跑 `screen -list`、
排队等待批次冷却、跑 `screen -list`、按台数等待、读日志）远超 3 秒。`ctx.defer()` 先把应答做掉，之后有 15 分钟窗口
（交互令牌的有效期）。

只有两条例外：

* `/help` 与 `/whoami` 是纯本地文案，不 defer 直接答；
* `/token` 用 Modal 收集输入，而弹窗必须是交互的第一个响应，所以它也不能先 defer。

### 1.2 长指令为什么发两条消息

`defer` 之后 Discord 有个兼容行为：第一条回复会替换那条"正在思考"占位，第二条起才是新消息。所以长耗时
指令（四个 `*all` 与单机器人的 `start`、`stop`、`restart`）按两条发：

1. 启动类指令（`/start`、`/restart`、`/startall`、`/restartall`）先
   `ctx.say("已接受 /startall 请求；涉及启动的操作将由全局启动队列执行，完成后会返回完整结果")`；其他长指令回
   `已开始执行 …，执行完毕后会返回完整结果`。
2. 跑完再 `ctx.say(<Report::render() 的结果>)`，这是真正的新消息。

查询类指令（`/show status`、`/show ...`、`/log`）只发第一条：结果直接替换掉占位。

#### 1.2.1 回复收口五个入口（按指令的逻辑分类命名，不按回复条数）

每个指令文件只从下面五个入口里挑一个，挑哪个由指令属于哪一类决定。`defer`、进度回执、
`spawn_blocking` 这些机制收在 `src/commands.rs` 的两个私有函数 `run_and_reply`（跑完回结果）与
`run_and_reply_with_progress`（先回进度回执、再回完整结果）里，改一次就全线生效。有名字的是"哪类
指令"，不是"发了几条"。每条回复都会记进程序日志（`say_and_log` 与 `log_reply` 走
`LogText::CommandReplied`：谁调的、哪条指令、回了什么），排查"它到底回了什么"以日志为准，见
[process-log.md](../process-log/process-log-zh_cn.md)。

* `single_bot_control`：覆盖 `/start`、`/stop`、`/restart`、`/pause`、`/unpause`、`/command`。
  用户先收到进度回执，跑完再收到完整结果。
* `single_bot_manage`：覆盖 `/bot add`、`/bot del`、`/bot rename`。用户只收到最终结果。
* `all_bots_manage`：覆盖 `/startall`、`/stopall`、`/restartall`、`/commandall`。用户先收到进度
  回执，跑完再收到完整结果。
* `read_only_query`：覆盖 `/show status`、`/show *`。用户只收到最终结果。
* `server_probe`：覆盖 `/ping`、`/checkserver`。用户只收到最终结果（异步探测，不进阻塞线程池）。

三处不入这五类，各有各的理由：`/help` 与 `/whoami` 是纯本地文案（不 defer）；`/log` 与
`/show botconfig` 要自己发附件；`/token` 的弹窗必须是交互的第一个响应。

### 1.3 敏感内容：ephemeral 与 Modal

* ephemeral（只有调用者可见）：`/log` 的结果用它，因此日志内容不会显示给整个频道。
* Modal（弹窗）：`/token` 用它收集 Token。原因是斜杠指令的每次调用都会在频道里生成一条消息
  （Discord 的 `CHAT_INPUT_COMMAND` 消息类型就是它），参数值就在那条消息里；`ephemeral` 只管回复、
  管不了调用本身。弹窗的输入走 `MODAL` 回调，不进频道。

`/token` 的完整链路：发弹窗（`TokenModal::execute`）→ 等提交 → poise 替我们确认提交（关掉弹窗）→
解析成结构体 → `spawn_blocking` 写文件 → 用 `CreateReply::ephemeral(true)` 回结果。弹窗占用了首次
响应的位置，所以最后那一步是 followup 消息。

`/bot login` 是唯一"两条回复都由业务层给内容"的指令：第一条（登录链接 + 设备码）要等 JAR 打印出来才
知道内容，所以命令层不能像其它指令那样先发一句固定进度回执。它的形状是：`defer_ephemeral` → 跑第一段
（拉起会话、等设备码）→ 发第一条 → 跑第二段（等凭据落盘或超时）→ 发第二条。细节见
[第 4.6 节](#46-bot-login)。

一条容易踩的细节：`defer_ephemeral()` 只定住那条"正在思考…"占位响应。defer 之后的回复在 poise 里是
**followup**，它的 ephemeral 取自**指令属性**（`#[poise::command(..., ephemeral)]`，不写就是 `false`），
不是取自 defer。所以"只有调用者可见"必须在 `CreateReply` 上显式 `.ephemeral(true)` 声明一次（`/token`
与 `/bot login` 都这么做）。

`/log` 原本只靠 `defer_ephemeral`（回复没显式带 ephemeral），现已改成每条回复都显式
`.ephemeral(true)`：与 `/token`、`/bot login` 同一口径，不再依赖框架的默认值。

### 1.4 为什么业务调用都包 spawn_blocking

业务层全是同步阻塞（文件、`screen`、`sleep`），直接在 async 里跑会占住 tokio 的工作线程。命令层统一用
`tokio::task::spawn_blocking(...)`，而闭包必须是 `Send + 'static`（不能借用 `ctx`），所以每个指令都是
先把要用的值取出来（`caller_id`、`String` 参数），再 move 进闭包。

### 1.5 注册与上下文

* 指令集合由 `commands::all_commands()` 给出；`setup` 里用 `poise::builtins::register_globally` 注册一次：
  一律全局注册（Discord 端最长 1 小时才传播完）。注册成功后还会顺手清掉各服务器上遗留的 guild 指令——
  以前注册到某个服务器的那份不会自己消失，客户端会把两份一起列出来（同一条指令出现两次）。清理有硬前提：
  先确认这一套全局指令**在本轮启动之前就已经稳定存在满 1 小时**（名字一一对应，且最"年轻"的指令 Id 也
  够老），否则会出现"两份都看不到"的空窗；判定靠指令 Id 里的创建时间，不落任何状态文件。
* 指令描述来自函数的第一段文档注释（poise 的规矩：宏不接受 `description = "..."`，只认
  `description_localized`）。第一段写英文（等于默认语言表里那句），细节放到后面的段落里。为什么、
  长度怎么算、兜底语言是什么，见第 1.7 节。
* 上下文别名：`Context<'a>`（普通指令）与 `ApplicationContext<'a>`（要用 Modal 的指令，Modal 只存在
  于斜杠指令的上下文里）。
* 只用 `slash_command`，不做 `!前缀` 指令，因为那需要 `MESSAGE_CONTENT` 特权意图，等于在开发者面板
  多一步操作。
* 父指令与子指令：`/bot` 与 `/show` 是 `subcommands("...")` 的父指令，本身不动作。

### 1.6 错误怎么处理

* 业务层返回的 `Err` 文案会被当成给用户看的话，命令层直接发出去（`handle_*` 已经转换好）。
* 命令函数自己返回的 `Err`（真正的意外）交给 `main.rs` 的 `on_error`：记 `log::error!`，再由命令层
  发送仅调用者可见的通用错误提示；内部错误详情不发给用户。命令 panic 也走同一回复路径。
* 框架级错误（`FrameworkError::Setup`，例如 Token 被拒）按成因分流：配置写错（Token 无效、机器人没有被
  邀请进服务器或缺 `applications.commands` 权限）时把成因写进日志并以退出码 1 退出（交给 systemd，单元里配
  `RestartPreventExitStatus=1` 就不重拉）；其余当掉线处理，停全部机器人并重建连接。

### 1.7 指令说明、选项说明与弹窗标签也是多语言的

命令层里的文字分两拨，来路不同、时机也不同：

* 指令说明与选项说明：来自 `text::get().command_texts()` 那张表（注册之前覆盖宏生成的那一份），生效
  于启动与注册期。
* 进度回执、用法提示、附件正文、回复记账：同一条 `text::get()`（`command_started_notice`、
  `command_ping_port_usage`、`command_log_attachment_body` 等），每次调用时取。
* 弹窗标题、字段标签、占位提示：在 `TokenModal` 的 `create()` 里现取（手写 `impl poise::Modal`），
  每次弹窗时取。

为什么说明文字不能直接写在 `src/commands/*.rs` 里：poise 的 `#[poise::command]` 与
`#[derive(Modal)]` 宏只认字面量，会把说明在编译期烘进二进制，运行期读配置换不掉。所以在注册之前把
文字改一遍（只做一次）：

* 指令说明与选项说明：`commands::all_commands()` 建完指令树后，按
  [`minecraft_fishingbot_manager_for_discord::text`] 的 `command_texts()` 表逐条覆盖 `Command.description`
  与 `CommandParameter.description`（这两个字段在 poise 里是公开的，可以直接改写）。
* `/token` 弹窗的标题、字段标签与占位提示：不用 `#[derive(Modal)]`，改成手写 `impl poise::Modal`
  （`create()` 里现取文案，`parse()` 按 `custom_id` 取值；形状与 derive 生成的完全一致）。

文字本体在语言文件里（`src/text/zh_cn.rs` 与 `src/text/en_us.rs` 的 `COMMAND_TEXTS`），`name_path`
必须与这里指令名逐字一致（子指令写 `"bot add"` 这种形式）。加改指令时要同步两种语言，下面的覆盖测试
会把漏掉的那条列出来。

#### 1.7.1 宏里那一份算什么：英文兜底

`///` 的第一段与选项的 `#[description = "..."]` 是表里缺项时才会露出来的兜底（宏只认字面量，换不
掉）。规矩是一律写英文，并与 `en_us.rs` 表里那句逐字相同，于是兜底永远不可能是另一种语言，而且不必
多维护一份文案（那份英文就是默认语言表里的那一句，默认语言见 [i18n.md 第 4 节](../i18n/i18n-zh_cn.md)）。

* 为什么是英文：默认语言就是英语（`Language::default()`），兜底与它一致时，漏一条也不会语言错乱。
* 第一段之后的段落仍是中文：那是给维护者的说明，不会发给用户（poise 只把第一段当 `description`，
  其余段落成为本地 `Command::help_text`）。
* 三条测试把这件事钉住（都在 `src/commands.rs`）：
  * `every_command_and_option_has_text_in_every_language`：表齐备（缺哪条就列出来）。
  * `fallback_texts_match_the_default_language_table`：宏里的兜底等于默认语言表里那句（逐字）。
  * `every_description_fits_discords_100_character_limit`：说明不超过 100 字符（Discord 上限；选项
    说明没有编译期检查）。

poise 的具体规矩（读 `poise_macros` 0.7 的 `src/command/mod.rs` 得到的，不是猜的）：

* 文档注释按空行切段，第一段进 `description`（段内换行拼成一个空格），其余段落进 `help_text`。
* 命令描述超过 100 字符是编译错误（`slash command description too long`）；选项描述没有编译期检查，
  poise 源码那行注释写着留给 fluent 之类的翻译框架在运行期补，本项目正是这么用的，所以长度得自己数
  （`/command` 与 `/commandall` 的选项说明曾涨到 103 字符，就是被上面第三条测试逼回 100 以内的）。
* 描述里写 `[...]` 这类补充说明没问题（两种语言统一用 `[]`），只要别超长。

不用 Discord 的 `description_localized`：那是让 Discord 按客户端语言挑显示哪一份，与本项目"一个进程
一种语言"是两套机制，混用会出现说明英文、回复中文。

## 2. 指令清单与选项

* `/help`、`/whoami`：没有选项，不 defer，回一条。
* `/show status`、`/checkserver`：没有选项，`defer`，回一条。
* `/ping`：选项 `host?` 与 `port?`（`i64`），`defer`，回一条。
* `/show botconfig|mybots|allbots|admins`：没有选项，`defer`，回一条。
* `/bot add`、`/bot del`：选项 `name`，`defer`，回一条。
* `/bot rename`：选项 `old_name`、`new_name`，`defer`，回一条。
* `/bot login`：选项 `name`，`defer_ephemeral`，回两条（第一条是登录链接与设备码，第二条是结果）。
* `/token`：选项是弹窗字段 `bot_name` 与 `token`，不 defer（弹窗优先），回一条并标记 `ephemeral`。
* `/log`：选项 `name`、`lines?`（`i64`），`defer_ephemeral`，回一条加附件。
* `/start`、`/stop`、`/restart`、`/pause`、`/unpause`：选项 `name`，`defer`，回两条。
* `/command`：选项 `name`、`command`，`defer`，回两条。
* `/startall`、`/stopall`、`/restartall`：没有选项，`defer`，回两条。
* `/commandall`：选项 `command`，`defer`，回两条。

数字是命令层唯一的转换：Discord 的整数 option 只有有符号 64 位，所以 `/log` 的行数与 `/ping` 的端口
都声明成 `i64`，在本层校验（大于 0，或 1 到 65535）之后再交给业务层（`usize`、`u16`）；校验不过当场
回用法提示，不往下传。

`/command` 与 `/commandall` 的自由文本用单个 `String` 选项：含空格直接写（例如
`/command duck /home fishing`），不用加引号，因为 Discord 的文本选项在它自己的输入框里就是吃空格的。
唯一例外是把整行一次粘贴或手打：那时客户端按空格切词，多出来的词会去匹配别的选项而报错（客户端行为，
绕不过）。我们不做任何加工（不剥引号、不合并空格、不 `trim`），用户输入什么就注入什么；拼接与"不能
为空"的校验在业务层（`utils::ToCleanBotCommand`）做一次。

### 2.1 指令选项的动态候选（autocomplete）

`name` 这类机器人名选项会在用户输入时给候选（Discord 原生能力）：本命令层的
`commands/autocomplete.rs` 负责按权限取得候选机器人目录、查询运行状态、按命令 scope 筛选候选、组装显示名并转换成 Discord 响应。
候选按指令语义分，不是一律"运行中"：

* `/stop`、`/pause`、`/unpause`、`/command`、`/restart`：有权限且运行中。对没在跑的目标这些操作没
  意义（`/restart` 会直接拒绝、`/stop` 只会回"无需停止"）。
* `/start`、`/bot del`、`/bot rename`、`/bot login`：有权限且未运行。这几个的前提就是它已经停下
  （`/start` 对在跑的目标会被拒；`/bot login` 同样如此，连"上次登录的会话还开着"它也一起拒）。
* `/log`：有权限的全部。历史日志对两者都有意义，显示名带"（运行中）"或"（已停止）"后缀。
* `/bot add`、`/token`：没有候选。`/bot add` 要的是一个尚不存在的名字（列不出来）；`/token` 是
  Modal，Discord 不支持它带候选。

细节：权限口径与服务端一致（管理员是全部机器人，其他人是自己 `bots/<用户ID>/` 下的）；运行状态取一次
`screen -list` 快照（每次按键都会收到一次 autocomplete 交互，不能按台数反复查）；最多 `runtime.max_autocomplete_choices` 条
（默认 25，Discord 硬上限，由 `src/commands/autocomplete.rs` 中的候选逻辑限制），运行中的排前面。候选只是建议：用户仍可手打任意名字，
归属与会话是否在跑照旧由各 `handle_*` 判。

## 3. 单机器人控制：`/start`、`/stop`、`/restart`、`/pause`、`/unpause`、`/command`、`/log`

* 代码：`src/bot_control.rs`（模块文档）与 `src/bot_control/*.rs`（一条指令一个文件）
* 入口：`lib::handle_bot_control`（动作枚举 `BotControlAction`）
* 会话操作全部复用 `utils` 里的原语，本层只做校验、结果转换与回复。
* 权限不在本模块：`/start` 这类单机器人指令的要求是
  `RequiredPermission::OwnerOrAdmin(目标机器人的归属)`，由路由层 `handle_bot_control` 与
  `handle_bot_log` 先解析目标条目（`BotDirEntry::find_by_name`）再判一次。批量的 `/startall` 那批的
  要求是 `RequiredPermission::MustBeAdmin`，同样在路由层判。
* `name` 选项的候选（autocomplete）属于命令层，不在本业务模块：`src/commands/autocomplete.rs` 按指令语义挑一批
  （`/start`、`/bot del`、`/bot rename` 只看未运行的；`/stop`、`/pause`、`/unpause`、`/command`、
  `/restart` 只看运行中的；`/log` 给全部），见 [第 2 节](#2-指令清单与选项)。

### 3.1 指令一览

* `/start <机器人名>`：`start::start_bot(&bot_dir_entry)`。拉起会话并复核两遍（第二遍还在
  才算起来，见第 3.2 节），回复"已发送启动命令"。
* `/stop <机器人名>`：`stop::stop_bot(&bot_dir_entry)`。停止会话（释放游戏账号）；停止当场
  复核，回复"已成功停止"。
* `/restart <机器人名>`：`restart::restart_bot(&bot_dir_entry)`。停止加拉起；不在运行的会被
  拒绝（要启动请用 `/start`），回复"已发送重启命令"。
* `/pause <机器人名>`：`pause::pause_fishing(&bot_dir_entry)`。游戏内 `/pause`，不下线；发完
  等 `command_echo.wait_ms`（默认 300 毫秒）确认日志回显，回复"已确认暂停"或"已发送"。
* `/unpause <机器人名>`：`unpause::unpause_fishing(&bot_dir_entry)`。游戏内 `/unpause`；
  确认与措辞同上。
* `/command <机器人名> <指令>`：`command::send_command_to_bot(&bot_dir_entry, raw_bot_command_args)`。
  转发游戏内指令。
* `/log <机器人名> <行数>`：`log::bot_log_view(&bot_dir_entry, line_count)`。回复用
  ephemeral（只有调用者可见）。

`name` 现在是**一个名字**：`/start duck duck2` 这种一次收多台还没做（评估结论与改法记在
[CONTEXT 第 8 节](../../CONTEXTS/CONTEXT-zh_cn.md)）。

`/pause` 与 `/unpause` 不是独立实现：注入走的是 `/command` 同一条路
（`utils::bot_status::confirm_pause_command` 收下注入这一步，由本层把
`command::send_command_to_bot` 传进去，所以校验，也就是目录归属与会话在跑，与 `/command` 完全一致）。
区别只在这两条指令发完之后还要确认生效，而这正是"确认加记忆"这套逻辑放在 `utils/bot_status.rs`
（而不是 `command.rs`）的原因：别的指令发什么由用户决定，成功回显无法预测，确认不了。

确认生效的步骤：

1. 等配置里 `command_echo.wait_ms`（默认 300 毫秒）。JAR 收到指令会同步打一行回显，得等它落进日志。
2. 在最新一份日志的最后 `command_echo.scan_lines`（默认 50）行里找回显关键词。只看末尾，不整份往回
   扫，因为长跑日志整份扫很贵。
3. 看到回显才说"已确认暂停"或"已恢复钓鱼"，没看到就只按"已发送"说：能验证到哪一步就只说到哪一步
   （见 [docs/CONTEXTS/CONTEXT-zh_cn.md](../../CONTEXTS/CONTEXT-zh_cn.md) 的"回复口径不许夸大"）。最新一份日志都找不到（会话在跑但日志被
   挪走或还没建出来）时不当"没回显"处理，而是直接报"找不到日志文件"，因为那说明确认这一步根本做
   不了，不是指令没生效。
4. 确认到回显时 `confirm_pause_command` 会同时把状态记下来（`BotPauseStatus::set_paused` 与
   `set_fishing`），`/show status` 之后直接用这个记忆，不必再为了找 pause 关键词把日志扫到文件开头。用
   `/command` 手发 `/pause`、或人进 `screen` 会话里敲，不会更新它，那种情况下 `/show status` 可能显示旧
   值，下一次 `/pause` 或 `/unpause` 会把它纠正回来。

前提是这份 JAR 得有这两条指令。上游 FishingBot 的原版没有 `/pause`、`/unpause`，本项目用的构建打了
补丁补上它们（补丁同时负责打印下面要找的那两句回显）。用原版 jar 时这两条指令只会回"命令已发送"，
`/show status` 的钓鱼状态永远是"钓鱼中"。

要找的回显关键词是配置 `[command_echo]` 里的 `pause_echo` 与 `unpause_echo`（上游消息
`command-pause-executed` 与 `command-unpause-executed`），不由调用方传：`PauseCommand::echo_keyword`
按命令取其中一项。它们只存在于带补丁的构建里（上游原版没有这两条指令），所以就地写在配置上，换 jar
构建时改配置即可：默认值按英文 jar，中文 jar 填 `已暂停钓鱼` / `已恢复钓鱼`。两条不能相同、也不能互相
包含，启动时会校验，否则日志里那行回显认不出属于哪条指令。

### 3.2 三个动作函数的固定步骤

`start_bot`、`stop_bot`、`restart_bot` 的结构一致，读代码时可以按这四步看：

1. 权限与目标解析都在路由层做完：`lib::handle_bot_control` 先 `permitted_bot_dir_entry` 判一次
   `OwnerOrAdmin`，把条目传下来，所以本函数只吃 `&BotDirEntry`。拿到的条目里带着归属 Discord 用户
   ID、机器人名（清洗后）、路径与 Token 状态。
2. 针对动作的前置检查：
   * `/start`：条目里的 `bot_token_status == BotTokenStatus::Missing` 直接拒绝（没有 Token 必然登录
     失败）；已在运行时拒绝（重复拉起会顶掉旧会话）。
   * `/stop`：会话不在运行时直接回复"无需停止"，不必走停止流程。
   * `/restart`：不在运行的直接拒绝（回"机器人 X 未运行，无需重启"），因为 `/restart` 的语义是重启
     正在跑的，替用户启动会掩盖"它是什么时候没的"这条信息；缺 Token 的目标由重启流程在停止之前剔掉
     并写进失败明细，因为重启是先停再起，缺 Token 起不来，白停一次会把还在钓鱼的账号弄下线。
3. 调 `utils` 的流程函数（等待与错峰都不用传参数，由 `utils` 按这一批有几台自己定）：
   * `/start` 走 `start_bot_sessions(std::slice::from_ref(bot_dir_entry))`；
   * `/stop` 走 `stop_single_bot_session(bot_dir_entry.clone())`（拿回明细，成败看它）；
   * `/restart` 走 `restart_bot_sessions(std::slice::from_ref(bot_dir_entry))`。

   三个流程函数吃的都是 `&[BotDirEntry]`，单机就是一个元素的切片 —— 这也是"以后要支持一次多台"
   不用重写它们的原因（见 [CONTEXT 第 8 节](../../CONTEXTS/CONTEXT-zh_cn.md)）。

   拉起相邻两台之间等 `timing.stagger_ms`，全部拉起后再等台数乘它复核两遍（中间再等同样久，第二遍
   还在才算起来，因为 JVM 可能在启动后一两秒就自己退，只查一遍会把"起来就没了"报成成功），见
   [utils.md 第 4 节](../utils/utils-zh_cn.md#4-startup--shutdown-的等待参数)。
4. 失败判定都看 utils 返回的明细自己的方法（`outcome.has_failed_lines()` 为真就把
   `outcome.to_failure_text()` 当错误返回；停止明细用的是同形的 `has_failed_lines()`）。明细由
   utils 返回，拼装方法必须在这里显式调用（见 [utils.md](../utils/utils-zh_cn.md)）。文案按能验证到哪一步就只说到
   哪一步的原则选词：
   * `/start` 回 `已发送机器人 'X' 的启动命令(启动状态请用 /show status 确认)`。能立刻
     确认的只有命令发出与会话出现，登录与进服还要看日志关键词。
   * `/stop` 回 `已成功停止机器人 'X'`。停止流程当场复核会话已消失（见 `utils::stop_bot_sessions`），
     所以这里说得起成功。
   * `/restart` 回 `已发送机器人 'X' 的重启命令(已确认成功停止，启动状态请用 /show status 确认)`。停止
     阶段可确认，拉起阶段与 `/start` 同样只能说已发送。

### 3.3 `/command` 的细节

* 指令片段由命令层原样传下来（不剥引号、不合并空格、不 `trim`，见 [第 2 节](#2-指令清单与选项)），
  本函数用 `to_clean_bot_command`（`ToCleanBotCommand`）拼成一条指令（单空格连接，空则回
  `错误:发送的指令不能为空！`），只拼一次。
* 回显用的就是这条拼好的指令。
* 权限由路由层判过（`OwnerOrAdmin`），这里不再看一眼归属；本函数只拼指令并要求会话在跑（Token 检查
  在 `/start` 里）。
* 会话不在运行时拒绝，因为指令不能塞进不存在的会话。
* 注入由 `utils::send_command_to_single_bot` 完成（screen 原语，自己不校验；末尾自动补换行，`screen`
  才会提交这一行）。

### 3.4 `/log` 的细节

* 行数必须大于 0，且被 `runtime.max_log_lines` 封顶（一条指令不能把整份大日志拉回来）。
* 只读最新那一份日志（`utils::bot_log_files::latest_log_path`，只认时间戳命名）：每个会话启动前
  `create_bot_log_file` 先建好文件，会话里 `grep -v -E` 过滤后 `tee -a` 追加。
* FishingBot 自己的日志写在同层 `raw/log0.log`（指向 `/dev/null` 的符号链接，内容丢弃），所以磁盘上
  只有一份日志，`/log` 不可能读回未过滤的刷屏内容。细节见
  [utils.md 第 7 节](../utils/utils-zh_cn.md#7-日志有两条线)。
* 读取用 `utils::read_last_lines` 的滑动窗口，日志再大也只保留最后 N 行。
* 日志原文放进 `Report::content`（整块内容），避免每行被加上列表前缀；标题里带上日志文件名。
* 回复走 `defer_ephemeral`，并且每条回复（用法提示、失败文案、带附件的正文）都显式 `.ephemeral(true)`：
  整条回复连附件只有调用者自己看得到。为什么要显式声明（不能只靠 defer）见
  [第 1.3 节](#13-敏感内容ephemeral-与-modal)。

## 4. 账号管理：`/bot add`、`/bot del`、`/bot rename`、`/bot login`、`/token`

* 代码：`src/bot_manage.rs` 与 `src/bot_manage/*.rs`
* 入口：`lib::handle_bot_manage`（动作枚举 `BotManageAction`）；`/bot login` 另有两个入口
  `lib::handle_bot_login`（第一段）与 `lib::handle_bot_login_await`（第二段），见第 4.6 节
* 权限不在命令层、也不在本模块：`/bot add` 落在调用者自己的用户目录下（要求 `Everyone`）；`del`、
  `rename`、`token`、`login` 的要求是 `RequiredPermission::OwnerOrAdmin(目标机器人的归属)`，由路由层
  `handle_bot_manage`（`login` 是 `handle_bot_login`）先解析目标条目（`BotDirEntry::find_by_name`）再判一次。
  命令层只取值、只数参数个数，不看权限。
* 共同点：目录名与后台日志一律使用规范化后的名字（`utils::ToCleanBotName::to_clean_bot_name`，名字
  非法时它返回 `Err`、由用户改名重试），这样回复、日志、磁盘目录名三者始终对得上。

### 4.1 指令一览

* `/bot add`：`add::add_bot(raw_bot_name, caller_id)`。所有人可用，只会创建在自己用户目录下。
* `/bot del`：`del::delete_bot(&bot_dir_entry)`。创建者或管理员。
* `/bot rename`：`rename::rename_bot(&bot_dir_entry, raw_new_bot_name)`。创建者或管理员，
  运行中不可改。
* `/bot login`：`login::begin_login(&bot_dir_entry)` 与 `login::await_login(prompt)`（两段式，分别由
  `handle_bot_login` 与 `handle_bot_login_await` 调用，见第 4.6 节）。创建者或管理员，运行中不可登录，
  两条回复都仅自己可见。
* `/token`：`token::update_bot_token_file(&bot_dir_entry, raw_token)`。创建者或管理员，
  回复仅自己可见，运行中不可改。

`/bot add`、`/bot del`、`/bot rename`、`/bot login` 是 Discord 的子指令（在客户端里点 `/bot` 会看到四个
子命令）；`/token` 是独立的顶层指令，用弹窗收集输入（见第 5 节）。

### 4.2 `/bot add`

* 名称合法性（`utils::ToCleanBotName::to_clean_bot_name`：非法字符直接报错，不再过滤）与全服重名校验
  都在 `utils::create_bot_dir` 里，它成功时返回建出来的条目（回复与后台日志都用条目里的 `bot_name`）。
* 目录始终落在调用者自己的用户目录下：路径由调用者用户 ID 推导，调用者写不到别人的目录。
* 回复里明确提醒"还需为其配置 Token 后才可启动"，并给出下一步：推荐 `/bot login`，`/token` 只作为它
  失败时的手动退路（这条口径也写进了示例配置的 `/help` 文本）。

### 4.3 `/bot del`

1. 路由层 `permitted_bot_dir_entry` 定位目录并判 `OwnerOrAdmin`（先 `find_by_name`，再看归属）；定位
   不到或没权限都按未找到回复，不暴露"该机器人在别人名下"这类信息。
2. 会话在运行时拒绝删除（`refusal_if_running`，统一文案见 `utils::running_bots`），否则会删掉它正在
   写入的数据目录。
3. 用 `fs::remove_dir_all` 删除目录，成功与失败都会写后台日志。

### 4.4 `/bot rename`

顺序是刻意安排的，不要调整：

1. 校验新旧名字都合法、且与旧名不同。
2. 路由层按旧名解析条目（`BotDirEntry::find_by_name`）并判 `OwnerOrAdmin`（口径是目录归属，管理员可
   跨目录）。
3. 运行中的会话直接拒绝（`refusal_if_running`，与 `/bot del`、`/token` 同一条规矩）：运行中的进程会
   继续往旧目录写数据，改名会把它接下来的日志写丢。要改名先 `/stop`。
4. 全服重名校验（`is_bot_name_taken`）：名字被占了就报错，不自动换一个。
5. 用 `fs::rename` 在同一个用户目录里换名字（新旧目录同属一个账号）。

### 4.5 `/token`

* 输入走弹窗（Modal），不是指令参数：斜杠指令的调用会出现在频道里（参数值也在里面），而 refreshToken
  是敏感信息。弹窗的输入走 `MODAL` 回调、不进频道；提交后业务层收到的仍是一段原始文本
  （`raw_token`），写入前不再改写。细节见
  [第 1.3 节](#13-敏感内容ephemeral-与-modal)。
* 只写文件、不校验 Token 有效性：写进去的内容由 FishingBot 自己判断。
* 运行中的账号禁止改 Token：正在跑的 JVM 只在启动时读一次 Token 文件，改它不会生效，却容易让人以为
  已经换好了。要换就先 `/stop`（与 `/bot del`、`/bot rename` 同一条规矩）。
* 文件名为 `runtime.token_file`，写在机器人目录下；路径口径由 `utils::token_file_path` 给，`fs::write`
  不存在时创建、存在时覆盖。
* Token 文件是能不能启动的判据：条目里的 `bot_token_status`（`BotTokenStatus::Ready` 或
  `BotTokenStatus::Missing`），判据本身只在 `BotTokenStatus::from_dir` 一处。
* 拿凭据的日常路径是 `/bot login`（第 6 节），`/token` 是它走不通时的退路。

### 4.6 `/bot login`

一句话：走 FishingBot.jar 自带的**微软设备码登录**，把 JAR 打印的登录链接与设备码转给调用者，凭据由 JAR
自己写进机器人目录。

为什么不让本项目自己实现微软登录：凭据文件是 JAR（MSAL）写的序列化格式，我们自己造的它不认。实测它的
行为是：`-refreshToken` 指向的文件存在就直接用它，没有文件才走设备码流程。

#### 4.6.1 流程（两段式，中间夹着用户的操作时间）

第一段（`login::begin_login`，几秒）：

1. 会话在跑就拒：有凭据时是平时那句"正在运行，请先停止"，没有凭据时是"登录已在进行中"（那次会话多半就是
   上一次还没结束的登录，别让用户去 `/stop` 掉自己的登录）。
2. 已有凭据先挪到 `<凭据文件名>.renewing`：不挪的话 JAR 看到它就直接用、不打印设备码，`/bot login` 会变成
   "什么都没发生"。顺带收拾上一次被打断留下的残留（两个都在 → 删残留；只有 `.renewing` → 放回去）。
3. 拉起会话：与 `/start` 走同一套（`utils::prepare_and_launch_bot`：建日志 → 准备 FishingBot 原始日志目录
   → `screen -dmS`），只是不按凭据状态拒绝 —— `/start` 的第零步正是拒掉无凭据的机器人，这也是这段流程必须
   另开一个入口的原因。失败就把旧凭据放回去再回错。
4. 读日志尾部等设备码那一行（上游消息 `auth-create-refresh-token`，中英两版语言文件里都是英文原文，逐字见
   [BOT_COMMANDS_AND_MESSAGES](../../refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md) 第 3.4 节），最长等 60 秒。等不到就停
   会话、放回旧凭据，并按"会话起来就退了"（复用 `/start` 那句结论）或"没等到登录提示"给结论。

没有有效凭据时 JAR 只是等着你登录，**不会去连游戏服务器** —— 所以登录期间既不占用游戏账号，也不会在游戏里
留下一次"进服又掉线"。

第二段（`login::await_login`，最多 10 分钟）：轮询凭据文件是否出现 —— **以文件为准**，不去解析"授权成功"
那一行（上游改了那句也不影响我们）。三种结束方式：

* 文件出现 → 成功：删掉 `.renewing` 残留、把凭据文件权限收紧到 `0600`（它等价于账号口令）、回一条"登录
  成功，它正继续进服"。**会话留在原地不动**：登录会话就是该机器人平时的会话，JAR 授权完会自己继续进服钓鱼，
  期间 `/show status`、`/stop` 全都照常有效。
* 读到 `auth-could-not-get-refresh-token` → 被微软拒绝：停会话（它会一直重试）、放回旧凭据、回一条失败并
  提示可以重试或用 `/token`。
* 会话提前消失 → 按"启动后退出"给结论（复用 `start_exited_soon`）。真机上的典型成因就是微软判定请求太频繁、
  返回空 token，JAR 自己结束 JVM。

超时（10 分钟）：先看一次凭据（用户可能压着最后几秒完成），没有就停会话 —— 设备码已经过期，那个 JVM 只会
一直问微软要授权；停完再看一次凭据（JAR 可能在收到 Ctrl+C 时刚好把它写完），仍没有才回超时，并把旧凭据放
回去。

#### 4.6.2 几个刻意的选择

* 两条回复：第一条（链接 + 代码）必须**马上**给用户，而凭据要等他授权完才落盘，所以命令层不能像其它指令
  那样先发一句固定进度回执。命令层的形状与 ephemeral 口径见
  [第 1.3 节](#13-敏感内容ephemeral-与-modal)。
* 等 10 分钟：微软设备码约 15 分钟有效，Discord 给一次交互的窗口也是 15 分钟，取 10 分钟留余量。
* 停会话走 `/stop` 那条路（先 Ctrl+C、复核，必要时强杀），不直接强杀：JAR 可能要到收到信号时才把凭据落盘。
* 设备码那一行会留在机器人日志里（有效期约 15 分钟、单次有效）：ephemeral 只保证别人在 Discord 里看不到，
  机器目录本身建议只给部署账号可读。
* 并发登录**不设上限**：同时登录多台会同时启动多个 JVM，微软可能限流（症状可能是"启动后一两秒就退出"）。
  目前由使用者错开登录时间；如果实际部署频繁遇到限流，再评估加入全局并发上限。

## 5. 批量指令：`/startall`、`/stopall`、`/restartall`、`/commandall`

* 代码：`src/all_bots_control.rs` 与 `src/all_bots_control/*.rs`
* 入口：`lib::handle_all_bots_control`（动作枚举 `AllBotsControlAction`），它同时是这批指令的路由层。
* 权限只在路由层校验一次（`handle_all_bots_control` 里的
  `has_permission(.., RequiredPermission::MustBeAdmin)`）：本模块的函数一律不接 `caller_id`、也不
  自查，避免四份重复文案与口径不一致。本模块只做调批量流程与拼报告。

### 5.1 指令一览

* `/startall`：`startall::start_all_bots()`，作用范围是所有已配 Token 且未运行的机器人。
* `/stopall`：`stopall::stop_all_bots()`，作用范围是所有运行中的机器人（含 Token 已失效的残留进程）。
* `/restartall`：`restartall::restart_all_bots()`，作用范围是所有已配 Token 且正在运行的机器人。
* `/commandall <指令>`：`commandall::send_command_to_all_bots(raw_bot_command_args)`，作用范围是所有运行中的
  机器人。

这些函数都不接 `caller_id` 参数：权限在路由层 `handle_all_bots_control` 就拦下了，能进到这里的一定
是管理员。标题也由它们自己取（`/stopall` 用指令标题；进程收尾那条另走
`stopall::stop_all_bots_for_shutdown()`，把标题换成日志标题，那份报告进的是程序日志）。对照：单机器
人指令的权限在各业务函数内部按目录归属判定，见 [第 3 节](#3-单机器人控制startstoprestartpauseunpausecommandlog)。

### 5.2 名单口径

取哪批目录由各指令自己决定（`all_bot_dir_entries()` 或 `token_ready_bot_dir_entries()`），再用
`utils::split_bot_entries_by_is_running(&[BotDirEntry])` 分组（一次 `screen -list` 快照）：

* `/startall`：用 `all_bot_dir_entries()`，先按 Token 就绪拆成两组。缺 Token 的账号不是失败，而要
  看得见（进跳过明细），所以取全部再分组。
* `/restartall`：用 `all_bot_dir_entries()`，只取在运行的那组。未运行的目标重启等于启动（那是
  `/startall` 的事）；缺 Token 的由重启流程在停止之前剔掉。
* `/stopall` 与进程退出收尾：用 `all_bot_dir_entries()`。Token 被删掉之后，之前拉起的进程可能还在跑
  并占着游戏账号，收尾时必须能扫到它们。
* `/commandall`：用 `token_ready_bot_dir_entries()`，只要名字。发指令不需要目录，口径与 `/show status`
  一致（只发给配好 Token 且在跑的）。

`/restartall` 刻意不按 Token 过滤：重启是先停再起，而缺 Token 的目标起不来（登录必然失败），白停一次
会把一个可能还在钓鱼的账号弄下线。过滤因此放在重启流程里、且放在停止之前，那些目标会以"缺少 Token
文件，已保持原状"出现在失败明细里，而不是静默消失。

### 5.3 各指令的流程

* `/startall`：先按 Token 就绪拆成两组（`split_bot_entries_by_token_status`），有效账号再按运行状态
  拆开（`split_bot_entries_by_is_running`）。一个有效账号都没有时回 `未配置任何有效账号，未启动。`
  （若还有缺 Token 的账号，会追一句提示）；全部在运行时回 `全部有效机器人均已运行`；否则调
  `utils::start_bot_sessions(&not_running_bot_entries)`，之后用 `Report::set_counts` 覆盖计数行
  （有效账号总数、已发送启动命令、跳过（已在运行）、跳过（缺少 Token）、启动失败）。没被启动的账号
  都进 `skipped` 明细（已在运行与缺 Token 两种原因各一条），不算失败。
* `/stopall`：`utils::stop_all_bot_sessions()` 拿原始明细（它自己就是 `all_bot_dir_entries()` 加分组
  加停止的组合），报告由本模块的 `stopall::stop_all_bots()` 拼；进程收尾走同一份拼装的
  `stopall::stop_all_bots_for_shutdown()`，只有标题不同（日志标题）。
* `/restartall`：`all_bot_dir_entries()` 里只取在运行的那一组（未运行的不算重启对象）；没有时直接回
  `没有运行的机器人实例，无需重启`；否则调 `utils::restart_bot_sessions`，再用 `Report::set_counts`
  写运行中账号、已发送重启命令、重启失败（缺 Token 的目标会在重启失败明细里说明原因）。
* `/commandall`：指令片段由命令层原样透传，这里用 `to_clean_bot_command` 拼成一条指令（与 `/command`
  同一口径），再逐条注入。`utils::all_running_bot_names()` 来自一次 `screen -list` 快照，命中即运行
  中，因此这里直接逐条注入，不再逐个机器人重复鉴权与探测会话；逐条统计成功数，失败的原文放进
  `Report::failed`。

### 5.4 报告约定

* 四条指令都输出 `Report`：固定为标题、计数行、明细行，计数行统一由 `Report::set_counts` 生成，格式
  是 `标签: N 个`。
* 回复长度有上限：`Report::render()` 会按 `runtime.max_reply_chars` 从末尾按行截断并注明省略行数。
  账号多时批量明细最容易触到上限（Discord 单条消息超长会直接发送失败）。
* 计数标签不许夸大：启动类标签是"已发送启动命令"与"已发送重启命令"，不是"成功启动"，因为批量启动只
  能确认命令发出与会话出现（与 `/start` 同一条口径，见
  [第 3.2 节](#32-三个动作函数的固定步骤)）；停止类才用"成功停止"（停止流程
  当场复核）。
* 基础信息要等流程跑完才知道的指令（`/startall`、`/restartall`）先跑流程，再用 `Report::set_counts(..)`
  覆盖计数行。这是显式覆盖，不会与流程写入的行叠加。

### 5.5 失败排查

批量启动的瞬时内存峰值常高于云服务器可用内存，被 OOM 杀掉的进程不会出现在 `screen -list` 里，因此
结果里会写"启动指令已发送，但进程不存在（通常是内存不足）"。处理方式：调小 `runtime.java_opts` 的
内存配额、调大 `timing.settle_per_bot_ms` 或 `timing.stagger_ms`（等得更久或错峰更宽），或稍后重复
一次命令（JVM 内存占用会随时间优化）。

## 6. 查询：`/show status` 与 `/show botconfig|mybots|allbots|admins`

* 代码：`src/show.rs` 与 `src/show/*.rs`
* 入口：`lib::handle_show`（动作枚举 `ShowAction::{MyBots, AllBots, Admins}`）与
  `lib::handle_fishingbot_config`（`/show botconfig` 单独一条，它返回的是附件而不是文本）。
* 全部是只读操作：不碰会话、也不改磁盘，因此不需要运行中检查。

### 6.1 指令一览

* `/show status`：`status::all_running_bots_status()`（内部走 `utils::running_bots::all_running_bot_infos()`）。
  数据来自 `screen -list` 与各机器人最新一份过滤后的日志，回复形式是文本（一张表）。
* `/show botconfig`：`botconfig::fishingbot_config_view()`。数据来自 `paths.fishingbot_config_file_name`（所有
  机器人共用那份），回复形式是附件（文件名与全文）。
* `/show mybots`：`mybots::my_bots_list(caller_id)`。数据是调用者自己用户目录下的机器人，文本回复。
* `/show allbots`：`allbots::all_bots_list()`。数据是全部用户目录下的机器人，文本回复。
* `/show admins`：`admins::admins_list()`。数据是配置里的 `[admins]`，文本回复。

### 6.2 `/show status`

* 名单是已配 Token 与会话存在的交集：先取 `utils::token_ready_bot_dir_entries()`（与 `/commandall`
  同一口径），再由 `utils::running_bots::all_running_bot_infos()` 用一次 `screen -list` 快照把不在跑
  的剔掉。所以出现在表里的都是配好 Token 且会话存在的机器人；Token 文件被删掉但进程还在跑的那种不在
  表里（`/stopall` 仍能收掉它）。
* 每个机器人的状态由 `utils::running_bots` 判定。连接状态分两种来源：会话是 `(Dead ...)` 残壳时不再读
  日志，直接显示"进程已结束"（JVM 已经被杀，日志最后一条可能还停在"在线"）；会话正常时从最新一份日志里
  读（`BotDirEntry::connection_status`：读一次就当场重新取一次，从末尾往回扫、有结论就停）。暂停状态取
  进程内记着的 `BotPauseStatus.pause_status()`（同一台机器人的 `/pause` 与 `/unpause` 几乎同时到达时，
  按回显窗口里最后那条记；确认到回显时更新，会话开始与结束时重置，见
  [utils.md 第 6 节](../utils/utils-zh_cn.md#6-配置取值约定)）。挑文件的口径见
  [utils.md 第 7 节](../utils/utils-zh_cn.md#7-日志有两条线)。措辞来自 `utils::bot_status`：服务器状态是"就绪/初始中"、
  "服务器在线"、"重连/本人在线"、"进程已结束"；三种读不到日志的情况（挑不到、读不出来、为空）在正常
  运行里不会出现，所以直接在这一格里显示报错原因；钓鱼状态是"钓鱼中"与"已暂停"。
* 运行时长由 `utils::format_uptime` 统一格式化成"天-时-分"，避免每个调用点各写一套。
* 输出逐台一行，方便肉眼扫。表头与数据行都由 `utils::running_bots` 生成（同一个文件里紧挨着）：
  * 表头是 `status_table_header()`，数据行是 `RunningBotInfo::into_row()`，两者都是四元素数组（列序
    对着看），共用同一个分隔符常量（` | `，两侧各一个空格）；
  * 加列、删列、调序就是改那两个数组，另补 `text` 里对应的列名（中英各一份）；
  * 快照字段私有、`into_row(self)` 吃掉自己，所以每份快照只可能被渲染一次，不会出现"表头四列、数据
    三列"这种错位。
  * 为什么放在 `running_bots` 而不是通用的 `reply`：这张表只服务 `/show status`（只对正在运行的机器人
    执行），而快照的字段是私有的，放在它身边就能直接读字段，不必为一个表格引入 trait 或 getter。
    测试 `status_table_header_matches_row_columns` 在同一模块里对着两边的格子逐一断言。

### 6.3 `/show botconfig`

* 发出去的是 `paths.fishingbot_config_file_name` 指向的钓鱼配置原文（FishingBot 自己的 `conf.json`），
  不是本项目的 `minecraft-fishingbot-manager-for-discord.conf.toml`：要看的是钓鱼参数现在是什么。这份文件由本程序
  在启动时按 `[fishingbot]` 生成（先删旧文件再写，见
  [config.md 第 3.2 节](../config/config-zh_cn.md#32-游戏侧配置confjson是生成物)），所以它既是"配置里写的"、
  也是"FishingBot 实际在读的"。
* 为什么用附件：`conf.json` 很容易超过 Discord 单条消息的 2000 字符上限，正文会被截断；附件既能
  给出全文，又天然带上文件名（就是 `conf.json`）。
* 内容不做截断：它是结构化文件，切一半反而会让人误读；超过附件上限（`utils::MAX_ATTACHMENT_BYTES`，
  8 MiB）时如实报错，让人直接上服务器看。
* 文件缺失或读不出来时回一条说明原因的错误文案（这份文件由本程序在启动时生成，所以先查 `base_dir`
  与写入权限），而不是静默返回空。

### 6.4 `/show mybots` 与 `/show allbots`

* 两者共用同一套渲染、同一份目录口径（都在 `utils::bot_dirs` 里，都按机器人名排序），差别只在取哪些
  目录：`my_bots_list` 走 `bot_dir_entries_of_owner(用户ID)`，`all_bots_list` 走
  `all_bot_dir_entries()`。
* 每台列出机器人名、归属用户 ID（`allbots` 才有意义）、Token 是否存在。Token 只报在与不在
  （`BotTokenStatus`），不报内容、也不联网校验真实性。
* `allbots` 是不分权限地展示全部账号，所以它是管理员视角的信息；它本身的权限要求是
  `RequiredPermission::Everyone`（谁都能用），只是普通用户通常只看自己的目录，用 `/show mybots` 即可。

### 6.5 `/show admins`

* 逐行输出 `名字 : 用户ID`，名字只来自配置，不做任何联网查询。
* 名单为空时给一句明确提示（在 `minecraft-fishingbot-manager-for-discord.conf.toml` 的 `[admins]` 里加一行
  `"用户ID" = "名字"`，用户 ID 用 `/whoami` 拿），避免看起来像出错。

## 7. 通用：`/help`、`/whoami`、`/ping`、`/checkserver`

* 代码：`src/general.rs` 与 `src/general/*.rs`
* 与本项目其它模块没有耦合：不读机器人目录、不碰 `screen` 会话，因此可以单独取用、单独测试。
* 四条指令的权限要求都是 `RequiredPermission::Everyone`（谁都能用），在 `lib.rs` 的 `handle_*` 里各判
  一次。

### 7.1 指令一览

* `/help`：`help::help()`，不发网络请求。
* `/whoami`：`whoami::whoami(caller_id)`，不发网络请求。
* `/ping [地址] [端口]`：`ping::ping_report(host, port)`，要发网络请求（MC 服务器握手）。
* `/checkserver`：`checkserver::checkserver_report()`，不发网络请求（读本机资源）。

`*_report` 后缀是按返回形态取的：这四个里除 `help` 与 `whoami` 这种整段文本之外，都返回
`Report::render()` 的结果，与 `my_bots_list`、`all_running_bots_status` 同一套命名规则，见
[architecture.md 第 5 节](../architecture/architecture-zh_cn.md)。

### 7.2 `/help`

* 直接返回配置里的 `text.help` 原文（三引号多行字符串），改文案不用重新编译。
* 任何频道都能用（不需要私聊）。
* 因此帮助菜单与实际支持的指令可能不同步：改了指令记得同步改配置，这一点在配置文件里也写明了。

### 7.3 `/whoami`

* 返回调用者自己的 Discord 用户 ID，并给出可以直接抄进 `[admins]` 的那一行。
* 存在的意义是解开先有鸡还是先有蛋：配置管理员要填自己的用户 ID，而常规拿法（开发者模式、右键头像、
  复制用户 ID）对不熟 Discord 的人不好找。
* 权限要求 `Everyone`，在 `lib::handle_whoami` 里判一次；文案由 `general/whoami.rs` 生成，命令层不再
  自己拼回复（见 [第 1 节](#1-命令层的规矩) 的硬约定）。

### 7.4 `/ping`

* 不带参数时用 `[ping]` 表的默认目标（`default_ping_host` 与 `default_ping_port`）；带参数时以聊天里
  给的为准（端口解析失败就退回默认逻辑）。
* 走 Minecraft 服务器列表握手（`craftping`），因此拿到的是游戏层的响应：在线人数与最大人数，以及
  "钓鱼服务器到游戏服务器"这一跳的延迟，而不是简单的 TCP 连通性。
* 输出固定三行：`状态: 在线`、`人数: 当前人数/上限`、`延迟: N ms(钓鱼服务器到游戏服务器的延迟)`；
  连不上、超时、协议解析失败各给一条可读文案。
* 连接超时取 `ping.timeout_ms`（配置校验保证它不为 0，否则永远等不到响应）。
* 地址或端口非法、连不上、超时都会各自给出可读文案，不 panic。

### 7.5 `/checkserver`

* 报告的是跑本程序的这台机器（钓鱼进程所在机器）的资源，不是游戏服务器。
* 数据来自 `sysinfo`：CPU 使用率、内存、磁盘、系统运行时长。
* CPU 使用率取两次采样（间隔很短）再计算差值：单次采样拿到的是相对开机以来的平均值，会长时间停留在
  高位、看起来像是满载。
* 输出是固定字段的多行文本，方便直接截图，不依赖日志文件。

## 8. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 动作枚举与权限校验在哪一层：[lib-zh_cn.md](../lib/lib-zh_cn.md)。
* 启动/停止的等待参数、两条日志线：[utils-zh_cn.md](../utils/utils-zh_cn.md) 第 4 节与第 7 节。
* 游戏侧配置这份附件：[config-zh_cn.md](../config/config-zh_cn.md) 第 3.2 节。
