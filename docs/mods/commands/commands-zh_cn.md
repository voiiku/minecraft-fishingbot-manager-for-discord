# commands：命令层与全部指令
[English](commands-en_us.md) | **简体中文**

* `src/discord_commands/` 接收 Discord 交互并发送回复。`src/operations/` 根据调用者和目标统一鉴权。`/sync-commands` 由 `operations::begin_command_sync` 检查管理员并取得同步锁，Discord API 操作留在适配器。

## 1. 命令层的规矩

### 1.1 为什么每个指令都先 defer

Discord 要求交互在 3 秒内应答，否则用户看到的是"交互失败"；而我们要做的事（起 JVM、跑 `screen -list`、
排队等待批次冷却、跑 `screen -list`、按台数等待、读日志）远超 3 秒。`ctx.defer()` 先把应答做掉，之后有 15 分钟窗口
（交互令牌的有效期）。

以下情况不先 defer：

* `/help` 与 `/whoami` 是纯本地文案，不 defer 直接答；
* `/token` 用 Modal 收集输入，而弹窗必须是交互的第一个响应，所以它也不能先 defer。
* `/sync-commands` 未取得同步许可时直接回复，不调用 Discord 注册 API。

### 1.2 长指令为什么发两条消息

`defer` 之后 Discord 有个兼容行为：第一条回复会替换那条"正在思考"占位，第二条起才是新消息。所以长耗时
指令（六个 `*all` 与单机器人的 `start`、`stop`、`restart`）按两条发：

1. 启动类指令（`/start`、`/restart`、`/startall`、`/restartall`）先
   `ctx.say("已接受 /startall 请求；本次启动 N 台，其中启动队列中已有 M 台在等待；按当前分批设置，预计最长耗时 X 分钟，等待启动结果的超时时间为 Y 分钟；完成后将返回完整结果")`
   （N 取自本次准备阶段筛选出的、有 Token 且未运行的账号；M 是队列里现有的台数
   （`StartupQueue::pending_bot_count()`，含别的指令排在前面的那些）；X 与 Y 按同一份节奏取值算：
   `StartupQueue::slowest_start_duration()` 与 `StartupQueue::result_timeout()`，都取整到分钟）；
   其他长指令回 `已开始执行 …，执行完毕后会返回完整结果`。
2. 跑完再 `ctx.say(<Report::render() 的结果>)`，这是真正的新消息。

查询类指令（`/show status`、`/show ...`、`/log`）只发第一条：结果直接替换掉占位。

#### 1.2.1 共享回执入口

* `run_control_with_notice`：先 defer，在阻塞线程池调用业务准备函数。鉴权或准备失败只发送错误；成功后先发送进度消息，再执行一次并发送最终报告。进度消息发送失败时不执行。`/startall` 的估时和入队使用同一份候选快照。
* `/bot add|del|rename` 和 `/show` 的文本查询共用 `run_blocking_and_reply`：先确认交互，再在阻塞线程池执行命令指定的业务函数，最后只发送一条结果。`run_async_and_reply` 处理异步操作。
* `/log`、`/show botconfig`、`/bot login` 和 `/token` 保留专用的附件、私密回复或 Modal 流程。

### 1.3 敏感内容：ephemeral 与 Modal

* ephemeral（只有调用者可见）：`/log` 的结果用它，因此日志内容不会显示给整个频道。
* Modal（弹窗）：`/token` 用它收集 Token。原因是斜杠指令的每次调用都会在频道里生成一条消息
  （Discord 的 `CHAT_INPUT_COMMAND` 消息类型就是它），参数值就在那条消息里；`ephemeral` 只管回复、
  管不了调用本身。弹窗的输入走 `MODAL` 回调，不进频道。

`/token` 的完整链路：发弹窗（`TokenModal::execute`）→ 等提交 → poise 替我们确认提交（关掉弹窗）→
解析成结构体 → `spawn_blocking` 写文件 → 用 `CreateReply::ephemeral(true)` 回结果。弹窗占用了首次
响应的位置，所以最后那一步是 followup 消息。

`/bot login` 使用两条私密回复：第一条（登录链接 + 设备码）要等 JAR 打印出来才
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

同步业务操作涉及文件、`screen` 和 `sleep`，直接在 async 中执行会占用 Tokio 工作线程。命令层在线程池
边界使用 `tokio::task::spawn_blocking(...)`，将所需的自有参数移入闭包；借用的游戏指令片段先复制，
再在线程池中传给业务路由。登录、日志和配置附件保留独立的响应流程。

### 1.5 注册与上下文

* 指令集合由 `discord_commands::all_commands()` 给出。启动时仅在配置范围内补建缺失的 `/sync-commands`，
  不覆盖同范围的其他命令；目标服务器的入口创建失败会记日志，不阻止其他服务器初始化。
* `discord.guild_ids` 为空或省略时，管理员执行 `/sync-commands` 会注册全局指令，然后立即清理机器人所在服务器的
  guild 指令；全局注册失败则不清理。传播最长约一小时，期间部分指令可能不可见。
* `discord.guild_ids` 非空时，先逐个更新指定服务器的指令；只要有一个更新失败，保留旧全局与其他服务器指令。
  全部成功后清理旧全局指令，再清理机器人所在的非目标服务器指令。清理失败逐项报告，需排除错误后手动重试。
  服务器级指令通常立即可用，但旧全局指令删除后的缓存可能延迟。
* 同步会整体替换目标范围内本应用的指令，并删除旧范围内本应用的全部指令，包括手动添加的指令；不影响其他应用。
  不存在后台定时或自动重试。修改配置后须重启并执行同步；首次部署需先配置管理员 ID。
* 指令及选项说明来自中英文文案；`/log` 的默认行数动态填入，由 `all_commands()` 在注册前校验，见第 1.7 节。
* 上下文别名：`Context<'a>`（普通指令）与 `ApplicationContext<'a>`（要用 Modal 的指令，Modal 只存在
  于斜杠指令的上下文里）。
* 只用 `slash_command`，不做 `!前缀` 指令，因为那需要 `MESSAGE_CONTENT` 特权意图，等于在开发者面板
  多一步操作。
* 父指令与子指令：`/bot` 与 `/show` 是 `subcommands("...")` 的父指令，本身不动作。

### 1.6 错误怎么处理

* 业务层返回的 `Err` 文案会被当成给用户看的话，命令层直接发出去（`operations` 已经转换好）。
* 命令函数自己返回的 `Err`（真正的意外）交给 `main.rs` 的 `on_error`：记 `log::error!`，再由命令层
  发送仅调用者可见的通用错误提示；内部错误详情不发给用户。命令 panic 也走同一回复路径。
* 框架级错误（`FrameworkError::Setup`，例如 Token 被拒）按成因分流：配置写错（Token 无效、机器人没有被
  邀请进服务器或缺 `applications.commands` 权限）时把成因写进日志并以退出码 1 退出（交给 systemd，单元里配
  `RestartPreventExitStatus=1` 就不重拉）；其余当掉线处理，停全部机器人并重建连接。

### 1.7 指令说明、选项说明与弹窗标签也是多语言的

命令层里的文字分两拨，来路不同、时机也不同：

* 指令说明与选项说明：来自 `config::Config::get().text().command_texts()` 那张表（注册前校验并填入），生效
  于启动与注册期。`/log` 行数说明由语言方法结合 `Config::get().constants.default_log_lines` 生成。
* 进度回执、用法提示、附件正文、回复记账：同一条 `config::Config::get().text()`（`command_started_notice`、
  `command_ping_port_usage`、`command_log_attachment_body` 等），每次调用时取。
* 弹窗标题、字段标签、占位提示：在 `TokenModal` 的 `create()` 里现取（手写 `impl poise::Modal`），
  每次弹窗时取。

poise 宏中的字面量无法随配置语言切换，因此指令定义不写说明，由文案表提供：

* 指令说明与选项说明：`discord_commands::all_commands()` 建完指令树后，按
  [`minecraft_fishingbot_manager_for_discord::text`] 的 `command_texts()` 表逐条填充 `Command.description`
  与 `CommandParameter.description`（这两个字段在 poise 里是公开的，可以直接改写）。
* `/token` 弹窗的标题、字段标签与占位提示：不用 `#[derive(Modal)]`，改成手写 `impl poise::Modal`
  （`create()` 里现取文案，`parse()` 按 `custom_id` 取值；形状与 derive 生成的完全一致）。

文字本体在语言文件里（`src/text/languages/zh_cn.rs` 与 `src/text/languages/en_us.rs` 的 `COMMAND_TEXTS`），`name_path`
必须与这里指令名逐字一致（子指令写 `"bot add"` 这种形式）。加改指令时要同步两种语言。

#### 1.7.1 文案表校验

命令树与当前语言的表必须一一对应：缺项、重复、未知命令或选项、全空白说明，以及超出 1–100 个 UTF-16
码元的说明都会在框架创建时报告错误并退出，不注册占位文案。测试还会对两种语言检查完整性、实际注册结构与长度。
命令宏不再保存英文说明；`autocomplete`、`rename` 和选项取值仍由命令定义。

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
* `/anti-afk`：必选 `command`（`on` 或 `off`），`defer`，回一条，仅管理员可用。

数字是命令层唯一的转换：Discord 的整数 option 只有有符号 64 位，所以 `/log` 的行数与 `/ping` 的端口
都声明成 `i64`，在本层校验（大于 0，或 1 到 65535）之后再交给业务层（`usize`、`u16`）；校验不过当场
回用法提示，不往下传。

`/command` 与 `/commandall` 的自由文本用单个 `String` 选项：含空格直接写（例如
`/command duck /home fishing`），不用加引号，因为 Discord 的文本选项在它自己的输入框里就是吃空格的。
唯一例外是把整行一次粘贴或手打：那时客户端按空格切词，多出来的词会去匹配别的选项而报错（客户端行为，
绕不过）。我们不做任何加工（不剥引号、不合并空格、不 `trim`），用户输入什么就注入什么；拼接与"不能
为空"的校验在业务层（`utils::console_command::ToCleanBotCommand`）做一次。

### 2.1 指令选项的动态候选（autocomplete）

`name` 这类机器人名选项会在用户输入时给候选（Discord 原生能力）：本命令层的
`operations/queries/list_bot_candidates.rs` 按权限取得候选机器人及状态；`discord_commands/autocomplete.rs` 按命令 scope 筛选候选、组装显示名并转换成 Discord 响应。
候选按指令语义分，不是一律"运行中"：

* `/stop`：有权限的全部。排队等待启动的也能被它取消（此时回一句"已取消排队启动"），所以候选里也要有
  排队中的；显示名带状态后缀。
* `/pause`、`/unpause`、`/command`、`/restart`：有权限且运行中。对没在跑的目标这些操作没
  意义（`/restart` 会直接拒绝、`/stop` 只会回"无需停止"）。
* `/start`、`/bot del`、`/bot rename`、`/bot login`：有权限且既没在跑、也不在启动队列里。这几个的前提
  就是它已经停下（`/start` 对在跑或排队中的目标会被拒；`/bot login` 同样如此，连"上次登录的会话还开着"
  它也一起拒）。
* `/log`：有权限的全部。历史日志对两者都有意义，显示名带"（运行中）"、"（排队中）"或"（已停止）"后缀。
* `/bot add`、`/token`：没有候选。`/bot add` 要的是一个尚不存在的名字（列不出来）；`/token` 是
  Modal，Discord 不支持它带候选。

细节：权限口径与服务端一致（管理员是全部机器人，其他人是自己 `bots/<用户ID>/` 下的）；状态取一次
`screen -list` 快照与一次启动队列查询（每次按键都会收到一次 autocomplete 交互，不能按台数反复查）；最多
`runtime.max_autocomplete_choices` 条（默认 25，Discord 硬上限，由 `src/discord_commands/autocomplete.rs` 中的候选逻辑
限制），在跑的排前面、其次是排队中的。候选只是建议：用户仍可手打任意名字，归属与会话是否在跑照旧由各
`operations` 判。

## 3. 单机器人控制：`/start`、`/stop`、`/restart`、`/pause`、`/unpause`、`/command`、`/log`

单台操作只由 `operations` 解析目标并检查归属；`/token` 在 Modal 提交后、写入凭据前鉴权。自动补全只提供该用户可见的候选，不授予权限。

### 3.1 指令一览

* `/start <机器人名>`：`single_bot_control::start_single_bot(name, caller_id)`。拉起会话并复核两遍（第二遍还在
  才算起来，见第 3.2 节），按 `/status` 回复已进服、当前未进服或异常。
* `/stop <机器人名>`：`single_bot_control::stop_single_bot(name, caller_id)`。停止会话（释放游戏账号）；停止当场
  复核，回复"已成功停止"。
* `/restart <机器人名>`：`single_bot_control::restart_single_bot(name, caller_id)`。停止并重新启动；旧会话不存在时拒绝。
  重启后按会话和状态查询结果回复“已确认进服”、“当前未进服”或失败原因。
* `/pause <机器人名>`、`/unpause <机器人名>`：鉴权后直接调用 `runtime::bot_instances::fishing_pause::set_fishing_pause`。
  两条指令分别要求暂停或恢复，共用发送指令和通过 `/status` 确认状态的逻辑。
* `/command <机器人名> <指令>`：`single_bot_control::send_single_bot_command(name, args, caller_id)`。
  转发游戏内指令。
* `/log <机器人名> <行数>`：`log::bot_log_view(&bot_dir_entry, line_count)`。回复用
  ephemeral（只有调用者可见）。

`name` 现在是**一个名字**：`/start duck duck2` 这种一次收多台还没做（评估结论与改法记在
[CONTEXT 第 8 节](../../CONTEXTS/CONTEXT-zh_cn.md)）。

`/pause` 与 `/unpause` 发送后再向 FishingBot 发送 `/status`，只检查发送前记录的日志偏移之后的新回显。
同一机器人的管理器指令与查询串行执行；查询最多等待 2 秒并检查新增日志末尾 100 行。日志被截短或替换时报告错误，
无有效回显时报告查询超时，JAR 报告不可用时作为异常报告。管理器不保存状态。

确认结果仅表示查询时的暂停状态，不证明状态由本次指令造成。“未暂停”也不代表已经连接或正在钓鱼。
需要支持 `/status`、`/pause` 与 `/unpause` 的 JAR，且日志过滤规则不得过滤状态回显。
上游回显不含请求标识；如果先前查询超时后的迟到回显或外部手动查询与本次请求交错，无法完全区分来源。

### 3.2 三个动作函数的固定步骤

`start_single_bot`、`stop_single_bot`、`restart_single_bot` 的结构一致，读代码时可以按这四步看：

1. 各业务命令通过 `permitted_bot_dir_entry` 解析目标并检查一次 所有者或管理员权限。拿到的条目里带着归属 Discord 用户
   ID、机器人名（清洗后）、路径与 Token 状态。
2. 针对动作的前置检查：
   * `/start`：条目里的 `bot_token_status == BotTokenStatus::Missing` 直接拒绝（没有 Token 必然登录
     失败）；已在运行时拒绝（重复拉起会顶掉旧会话）。
   * `/stop`：会话不在运行时直接回复"无需停止"，不必走停止流程。
   * `/restart`：不在运行的直接拒绝（回"机器人 X 未运行，无需重启"），因为 `/restart` 的语义是重启
     正在跑的，替用户启动会掩盖"它是什么时候没的"这条信息；缺 Token 的目标由重启流程在停止之前剔掉
     并写进失败明细，因为重启是先停再起，缺 Token 起不来，无谓的停止会让仍在钓鱼的账号下线。
3. 调用对应流程：
   * `/start` 走 `StartupQueue::get_or_init().start_bots(std::slice::from_ref(&bot_dir_entry))`；
   * `/stop` 调用 `stop_single_bot_session(&bot_dir_entry)`，返回停止结果或具体错误；
   * `/restart` 走 `restart_bot_sessions(std::slice::from_ref(&bot_dir_entry))`。

   拉起相邻两台之间等 `[startup_queue] bot_login_interval`；拉起后先判定一次，等满 `timing.check_grace` 再判定一次
   （每次启动请求只等一次）。第二次复核时会话存在且不是死残壳才算启动成功；
   见 [runtime-zh_cn.md 第 4 节](../runtime/runtime-zh_cn.md#4-启动与停止中的等待)。
4. 启动复核返回各机器人的 `BotStatus` 或具体异常。单台直接生成回复；批量按原顺序汇总：
   * `/start` 按复核结果回复：`/status` 的 `joined` 为真时确认进服，为假时报告当前未进服。
     状态不可用、查询失败或会话消失时报告异常；状态异常不会自动停止会话。
   * `/stop` 回 `已成功停止机器人 'X'`。停止流程当场复核会话已消失（见 `runtime::bot_instances::stop::stop_bot_sessions`），
     所以这里说得起成功。
   * `/restart` 确认旧会话已结束后启动新会话，并根据 `/status` 报告已进服或当前未进服。
     停止失败、启动失败、取消或复核错误均返回相应原因。

### 3.3 `/command` 的细节

* 指令片段由命令层原样传下来（不剥引号、不合并空格、不 `trim`，见 [第 2 节](#2-指令清单与选项)），
  本函数用 `to_clean_bot_command`（`ToCleanBotCommand`）拼成一条指令（单空格连接，空则回
  `错误:发送的指令不能为空！`），只拼一次。
* 回显用的就是这条拼好的指令。
* `send_single_bot_command` 检查 所有者或管理员权限，拼接指令并要求会话在运行（Token 检查在 `/start` 里）。
* 会话不在运行时拒绝，因为指令不能塞进不存在的会话。
* 注入由 `runtime::bot_instances::screen::send_command_to_single_bot` 完成（screen 原语，自己不校验；末尾自动补换行，`screen`
  才会提交这一行）。

### 3.4 `/log` 的细节

* 行数必须大于 0，且被 `runtime.max_reply_log_lines` 封顶（一条指令不能把整份大日志拉回来）。
* 只读最新那一份日志（`runtime::bot_log::latest_log_path`，只认时间戳命名）：每个会话启动前
  `create_bot_log_file` 先建好文件，会话里 `grep -v -E` 过滤后 `tee -a` 追加。
* FishingBot 自己的日志写在同层 `raw/log0.log`（指向 `/dev/null` 的符号链接，内容丢弃），所以磁盘上
  只有一份日志，`/log` 不可能读回未过滤的刷屏内容。细节见
  [runtime-zh_cn.md 第 7 节](../runtime/runtime-zh_cn.md#7-两类日志)。
* 读取用 `runtime::log_files::read_last_lines` 的滑动窗口，日志再大也只保留最后 N 行。
* 日志原文放进 `Report::content`（整块内容），避免每行被加上列表前缀；标题里带上日志文件名。
* 回复走 `defer_ephemeral`，并且每条回复（用法提示、失败文案、带附件的正文）都显式 `.ephemeral(true)`：
  整条回复连附件只有调用者自己看得到。为什么要显式声明（不能只靠 defer）见
  [第 1.3 节](#13-敏感内容ephemeral-与-modal)。

## 4. 账号管理：`/bot add`、`/bot del`、`/bot rename`、`/bot login`、`/token`

入口为 `add_bot`、`delete_bot`、`rename_bot` 和 `update_bot_token`。新账号归调用者，已有账号须为本人或管理员。登录分 `begin_bot_login`、`finish_bot_login` 两段，以便先私密发送设备码。

### 4.1 指令一览

* `/bot add`：`accounts::add_bot(raw_bot_name, caller_id)`。所有人可用，只会创建在自己用户目录下。
* `/bot del`：`accounts::delete_bot(name, caller_id)`。创建者或管理员。
* `/bot rename`：`accounts::rename_bot(name, new_name, caller_id)`。创建者或管理员，
  运行中不可改。
* `/bot login`：`login::begin_login(&bot_dir_entry)` 与 `login::await_login(prompt)`（两段式，分别由
  `operations::accounts::begin_bot_login` 与 `operations::accounts::finish_bot_login` 调用，见第 4.6 节）。创建者或管理员，运行中不可登录，
  两条回复都仅自己可见。
* `/token`：`token::update_bot_token_file(&bot_dir_entry, raw_token)`。创建者或管理员，
  回复仅自己可见，运行中不可改。

`/bot add`、`/bot del`、`/bot rename`、`/bot login` 是 Discord 的子指令（在客户端里点 `/bot` 会看到四个
子命令）；`/token` 是独立的顶层指令，用弹窗收集输入（见第 5 节）。

### 4.2 `/bot add`

* 名称合法性（`utils::bot_name::ToCleanBotName::to_clean_bot_name`：非法字符直接报错，不再过滤）与全服重名校验
  都在 `runtime::bot_instances::bot_dirs::create_bot_dir` 里，它成功时返回建出来的条目（回复与后台日志都用条目里的 `bot_name`）。
* 目录始终落在调用者自己的用户目录下：路径由调用者用户 ID 推导，调用者写不到别人的目录。
* 回复里明确提醒"还需为其配置 Token 后才可启动"，并给出下一步：推荐 `/bot login`，`/token` 只作为它
  失败时的手动退路（这条口径也写进了示例配置的 `/help` 文本）。

### 4.3 `/bot del`

1. 业务命令通过 `permitted_bot_dir_entry` 定位目录并判 所有者或管理员权限（先 `find_by_name`，再看归属）；定位
   不到或没权限都按未找到回复，不暴露"该机器人在别人名下"这类信息。
2. 会话在运行时拒绝删除（`refusal_if_not_stopped`，统一文案见 `runtime::bot_instances::bot_sessions`），否则会删掉它正在
   写入的数据目录。
3. 用 `fs::remove_dir_all` 删除目录，成功与失败都会写后台日志。

### 4.4 `/bot rename`

顺序是刻意安排的，不要调整：

1. 通过 `permitted_bot_dir_entry` 按旧名解析条目并检查 所有者或管理员权限（管理员可跨目录）。
2. 校验新名字合法且与旧名不同。
3. 运行中的会话直接拒绝（`refusal_if_not_stopped`，与 `/bot del`、`/token` 同一条规矩）：运行中的进程会
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
* 文件名为 `runtime.token_file`，写在机器人目录下；路径口径由 `runtime::bot_instances::bot_dirs::token_file_path` 给，`fs::write`
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
3. 拉起会话：与 `/start` 走同一套（`runtime::bot_startup::StartupQueue::launch_bot_to_login`：建日志 → 准备 FishingBot 原始日志目录
   → `screen -dmS`），只是不按凭据状态拒绝 —— `/start` 的第零步正是拒掉无凭据的机器人，这也是这段流程必须
   另开一个入口的原因。失败就把旧凭据放回去再回错。
4. 读日志尾部等设备码那一行（上游消息 `auth-create-refresh-token`，中英两版语言文件里都是英文原文，逐字见
   [BOT_COMMANDS_AND_MESSAGES](../../refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md) 第 3.4 节），最长等 60 秒。等不到就停
   会话、放回旧凭据，并按"会话起来就退了"（复用 `/start` 那句结论）或"没等到登录提示"给结论。

没有有效凭据时 JAR 只等待登录，**不会去连游戏服务器** —— 所以登录期间既不占用游戏账号，也不会在游戏里
留下一次"进服又掉线"。

第二段（`login::await_login`，最多 10 分钟）：轮询凭据文件是否出现 —— **以文件为准**，不去解析"授权成功"
那一行（上游改了那句也不影响我们）。三种结束方式：

* 文件出现 → 成功：删掉 `.renewing` 残留、把凭据文件权限收紧到 `0600`（它等价于账号口令）、回一条"登录
  成功，它正继续进服"。**会话留在原地不动**：登录会话就是该机器人平时的会话，JAR 授权完会自己继续进服钓鱼，
  期间 `/show status`、`/stop` 全都照常有效。
* 读到 `auth-could-not-get-refresh-token` → 被微软拒绝：停会话（它会一直重试）、放回旧凭据、回一条失败并
  提示可以重试或用 `/token`。
* 会话提前消失 → 按"启动失败"给结论（复用 `start_failed`，与批量启动同一句话）。真机上的典型成因就是微软判定请求太频繁、
  返回空 token，JAR 自己结束 JVM。

超时（10 分钟）：先看一次凭据（用户可能压着最后几秒完成），没有就停会话 —— 设备码已经过期，那个 JVM 只会
一直问微软要授权；停完再看一次凭据（JAR 可能在收到 Ctrl+C 时刚好把它写完），仍没有才回超时，并把旧凭据放
回去。

登录期间若 screen 查询失败，直接返回错误并尝试停止会话；无法确认停止时保留 `.renewing` 旧凭据备份，避免覆盖仍可能运行的登录会话。停止失败会记录到程序日志。

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

## 5. 批量指令：`/startall`、`/stopall`、`/restartall`、`/pauseall`、`/unpauseall`、`/commandall`

每条批量指令直接调用对应的管理员操作，如 `/startall` → `operations::all_bots_control::start_all_bots`。进度回执不向非管理员披露账号或队列数量。

### 5.1 指令一览

* `/startall`：`all_bots_control::start_all_bots(caller_id)`，作用范围是所有已配 Token 且未运行的机器人。
* `/stopall`：`all_bots_control::stop_all_bots(caller_id)`，作用范围是所有运行中的机器人（含 Token 已失效的残留进程）。
* `/restartall`：`all_bots_control::restart_all_bots(caller_id)`，作用范围是所有已配 Token 且正在运行的机器人。
* `/pauseall`、`/unpauseall`：`all_bots_control::pause_report::set_all_bots_fishing_pause` 使用与 `/commandall` 相同的目标范围，
  逐台调用 `set_fishing_pause` 并汇总结果；查询不可用、超时或失败均报告异常。
* `/commandall <指令>`：`all_bots_control::send_all_bots_command(args, caller_id)`，作用范围是所有运行中的
  机器人。

`/anti-afk <on|off>` 使用独立入口 `operations::set_anti_afk`，同样仅管理员可用：`on` 启动，已运行时提示无需重复启动；`off` 停止并等待线程退出，未运行时提示无需停止。它使用 `[anti_afk]` 输入与间隔，匹配名称包含 `runtime.screen_prefix` 的所有 screen 会话（可能包含本项目以外的会话）。启动成功只代表线程已创建，不保证存在匹配会话或输入送达；`/show status` 可查看线程是否存活。程序重启后不自动开启。

各批量业务入口先检查管理员；不通过时不会生成进度回执、查询 screen 或执行操作。
标题也由它们自己取（`/stopall` 用指令标题；进程收到退出信号时收尾在
`runtime::shutdown::exit_on_shutdown_signal(notice_ctx)` 里就地拼同一份明细，只把标题换成"收到退出信号，收尾结果"；
那份报告进程序日志，此外还发到各服务器的通报频道）。对照：单机器
人指令的权限在各业务函数内部按目录归属判定，见 [第 3 节](#3-单机器人控制startstoprestartpauseunpausecommandlog)。

### 5.2 名单口径

取哪批目录由各指令自己决定（`all_bot_dir_entries()` 或 `token_ready_bot_dir_entries()`），再用
`runtime::bot_instances::bot_sessions::split_bot_entries_by_is_running(&[BotDirEntry])` 分组（一次 `screen -list` 快照）：

* `/startall`：用 `all_bot_dir_entries()`，先按 Token 就绪拆成两组。缺 Token 的账号不是失败，但要
  计入跳过计数，所以取全部再分组。
* `/restartall`：用 `all_bot_dir_entries()`，只取在运行的那组。未运行的目标重启等于启动（那是
  `/startall` 的事）；缺 Token 的由重启流程在停止之前剔掉。
* `/stopall` 与进程退出收尾：用 `all_bot_dir_entries()`。Token 被删掉之后，之前拉起的进程可能还在跑
  并占着游戏账号，收尾时必须能扫到它们。
* `/commandall`、`/pauseall`、`/unpauseall`：用 `token_ready_bot_dir_entries()`，口径与 `/show status`
  一致（只发给配好 Token 且在跑的）。

`/restartall` 刻意不按 Token 过滤：重启是先停再起，而缺 Token 的目标起不来（登录必然失败），白停一次
会让一个可能仍在钓鱼的账号下线。过滤因此放在重启流程里、且放在停止之前，那些目标会以"缺少 Token
文件，已保持原状"出现在失败明细里，而不是静默消失。

### 5.3 各指令的流程

* `/startall`：筛出有 Token 且无会话的目标，由启动队列复核。报告按账本顺序列出成功和失败明细；
  已在运行、缺 Token 及取消的机器人只计数，不生成逐台明细。
* `/stopall`：`runtime::bot_instances::stop::stop_all_bot_sessions()` 拿原始明细（它自己就是"请求启动线程停止拉起 + 清空启动队列 +
  `all_bot_dir_entries()` 加分组加停止 + 再扫一遍晚到的会话"的组合），报告由本模块的 `all_bots_control::stop_all_bots(caller_id)`
  拼，只报成功与失败（计数行只有"成功停止"与"停止失败"，原本未运行的机器人不进明细；等待列表里被取消的那些进
  `Report::skipped`，没有排队时它为空；已经取走、正在拉的那批由触发它的 `/startall`／`/restartall` 回复报出
  "启动被取消"；"扫描机器人目录"这个字段只由 `/show allbots` 输出）。进程收到退出信号时，收尾在
  `runtime::shutdown::exit_on_shutdown_signal(notice_ctx)` 里就地拼同一份明细，只是标题换成"收到退出信号，收尾结果"：
  报告先写程序日志，再发到各服务器的通报频道（发送有 30 秒总时限，超时就跳过并退出，下次启动不补发）。
* `/restartall`：`all_bot_dir_entries()` 里只取在运行的那一组（未运行的不算重启对象）；没有时直接回
  `没有运行的机器人实例，无需重启`；否则调 `runtime::bot_instances::restart::restart_bot_sessions`，再用 `Report::set_counts`
  写目标机器人、重启复核通过数、重启异常（缺 Token 的目标会在异常明细里说明原因）。
  被取消的启动列入跳过明细。若停止后、重新拉起前已有会话，则计为重启异常。
* `/commandall`：指令片段由命令层原样透传，这里用 `to_clean_bot_command` 拼成一条指令（与 `/command`
  同一口径），再逐条注入。`runtime::bot_instances::bot_sessions::token_ready_bot_names_with_screen_sessions()` 来自一次 `screen -list` 快照，包含死残壳；
  这里直接逐条注入，不再逐个机器人重复鉴权与探测会话；逐条统计成功数，失败的原文放进
  `Report::failed`。
* `/pauseall`、`/unpauseall`：复用相同的 Token 与会话筛选，逐台调用单机器人暂停或恢复逻辑。
  每台发送后通过 `/status` 查询，异常在报告中列出。

### 5.4 报告约定

* 批量指令输出 `Report`：固定为标题、计数行、明细行，计数行统一由 `Report::set_counts` 生成，格式
  是 `标签: N 个`。
* 回复长度有上限：`Report::render()` 会按 `runtime.max_reply_chars` 从末尾按行截断并注明省略行数。
  账号多时批量明细最容易触到上限（Discord 单条消息超长会直接发送失败）。
* 启动和重启复核通过表示会话存在且状态查询成功，不一定已进服；超时和失败计为异常。完成明细逐台区分已进服与当前未进服，
  异常明细保留错误原因。停止结果则以会话退出复核为准。
* 基础信息要等流程跑完才知道的指令（`/startall`、`/restartall`）先跑流程，再用 `Report::set_counts(..)`
  覆盖计数行。这是显式覆盖，不会与流程写入的行叠加。

### 5.5 失败排查

进程在启动期间退出时，结果会显示“启动失败”；具体原因需结合日志判断。若内存不足，可调小
`runtime.java_opts` 的堆上限；若登录频率受限，可增大 `[startup_queue] bot_login_interval`。
需要更长复核窗口时可增大 `timing.check_grace`。

## 6. 查询：`/show status` 与 `/show botconfig|mybots|allbots|admins`

公开查询直接调用 `show_status`、`show_my_bots`、`show_all_bots`、`show_admins` 或 `show_bot_config`。仅个人列表需要调用者 ID。

### 6.1 指令一览

* `/show status`：`show::show_status()`（复用 `show::show_status::status_row()`）。
  数据来自 `screen -list`、各机器人最新一份过滤后的日志，以及一次启动队列查询（排队中的在报告下方
  `[排队中]` 一栏），回复形式是文本（一张表）。
* `/show botconfig`：`botconfig::fishingbot_config_view()`。数据来自 `paths.fishingbot_config_file_name`（所有
  机器人共用那份），回复形式是附件（文件名与全文）。
* `/show mybots`：`mybots::my_bots_list(caller_id)`。数据是调用者自己用户目录下的机器人，文本回复。
* `/show allbots`：`allbots::all_bots_list()`。数据是全部用户目录下的机器人，文本回复。
* `/show admins`：`admins::admins_list()`。数据是配置里的 `[admins]`，文本回复。

### 6.2 `/show status`

* 报告首先显示反 AFK 线程是否运行，即使没有运行中或排队中的机器人也会显示；这不是输入送达状态。
* 名单是已配 Token 与会话存在的交集：读取一次机器人目录并按 Token 分组（与 `/commandall`
  同一口径），再由 `show::show_status::status_row()` 用一次 `screen -list` 快照把不在跑
  的剔掉。所以出现在表里的都是配好 Token 且会话存在的机器人；Token 文件被删掉但进程还在跑的那种不在
  表里（`/stopall` 仍能收掉它）。
* 排队与否由 `/show status` 自己问一次启动队列（`StartupQueue::bots_in_queue`，与 `/start`、`/stop`
  同一口径）：排队中的机器人还没有会话，进不了上面那张表（没有在线 / 暂停 / 时长可报），所以在报告
  下方新开一栏（`Report::queued`，小节标签是 `report_queued_label()`，中文 `[排队中]`）逐台列出。
* 存活会话通过 `/status` 获取 `BotStatus` 的五个字段：`joined`、`paused`、`no_rod_available`、
  `has_bobber`、`last_fish`。前两项显示为连接与暂停状态，布尔字段以 `true`/`false` 显示；
  `last_fish` 为上次钓鱼动作的 Unix 毫秒时间戳，不一定是钓获时间。状态不可用时报告会话异常，
  查询超时或日志错误单独报告；screen 死残壳直接显示进程结束。
* 运行时长由 `utils::uptime::format_uptime` 统一格式化为 `1d-1h-5m`（天、时、分），避免每个调用点各写一套。
* 输出逐台一行，方便肉眼扫。这张表没有专门的类型：数据行由 `show::show_status::status_row()` 当场拼
  成一行文本，表头由 `src/operations/queries/show_status.rs` 当场拼成一行文本，两处均有七列并使用 `join(" | ")`：
  * 列名（中英各一份）在 `text` 里：`status_column_bot_name()`、`status_column_joined()`、
    `status_column_pause()`、`status_column_no_rod()`、`status_column_bobber()`、
    `status_column_last_fish()`、`status_column_uptime()`；
  * 加列、删列、调序要同时改这两处；列序没有类型层面的强制，对齐靠两边读同一组列名、用同一个分隔符
    （` | `，两侧各一个空格）；
  * 行是渲完即弃的字符串，所以不存在"同一份快照被渲两次"的问题，也不必为一个表格引入 getter 或 trait：
    `status_row` 算完各列就直接拼行。
  * 为什么数据行放在 `show_status` 而不是通用的 `text::report`：它要读会话快照与日志（两者都属于运行状态的
    判断），当场拼行比先造一个快照类型再消费更简单。

### 6.3 `/show botconfig`

* 发出去的是 `paths.fishingbot_config_file_name` 指向的钓鱼配置原文（FishingBot 自己的 `config.json`），
  不是本项目的 `minecraft-fishingbot-manager-for-discord.conf.toml`：要看的是钓鱼参数现在是什么。这份文件由本程序
  在启动时按 `[fishingbot]` 生成（先删旧文件再写，见
  [config.md 第 3.2 节](../config/config-zh_cn.md#32-游戏侧配置configjson是生成物)），所以它既是"配置里写的"、
  也是"FishingBot 实际在读的"。
* 为什么用附件：`config.json` 很容易超过 Discord 单条消息的 2000 字符上限，正文会被截断；附件既能
  给出全文，又天然带上文件名（就是 `config.json`）。
* 内容不做截断：它是结构化文件，切一半反而会让人误读；超过附件上限（`Config::get().constants.max_attachment_bytes`，
  8 MiB）时如实报错，让人直接上服务器看。
* 文件缺失或读不出来时回一条说明原因的错误文案（这份文件由本程序在启动时生成，所以先查 `base_dir`
  与写入权限），而不是静默返回空。

### 6.4 `/show mybots` 与 `/show allbots`

* 两者共用同一套渲染、同一份目录口径（都在 `runtime::bot_instances::bot_dirs` 里，都按机器人名排序），差别只在取哪些
  目录：`my_bots_list` 走 `bot_dir_entries_of_owner(用户ID)`，`all_bots_list` 走
  `all_bot_dir_entries()`。
* 每台列出机器人名、归属用户 ID（`allbots` 才有意义）、Token 是否存在。Token 只报在与不在
  （`BotTokenStatus`），不报内容、也不联网校验真实性。
* `allbots` 的第一行是计数行「扫描机器人目录」（`count_scanned_dirs`，值是 `all_bot_dir_entries()` 的
  条数）：这个字段只由它输出，停止类报告与进程退出收尾都不带它。
* `allbots` 对所有人公开，不过滤归属；只查看自己的账号可用 `/show mybots`。

### 6.5 `/show admins`

* 逐行输出 `名字 : 用户ID`，名字只来自配置，不做任何联网查询。
* 名单为空时给一句明确提示（在 `minecraft-fishingbot-manager-for-discord.conf.toml` 的 `[admins]` 里加一行
  `"用户ID" = "名字"`，用户 ID 用 `/whoami` 拿），避免看起来像出错。

## 7. 通用：`/help`、`/whoami`、`/ping`、`/checkserver`

* 代码：`src/operations/queries/mod.rs` 与 `src/operations/queries/*.rs`
* 与本项目其它模块没有耦合：不读机器人目录、不碰 `screen` 会话，因此可以单独取用、单独测试。
* 这四条查询对所有人公开，无需鉴权。`/log` 等非公开查询仍会验证目标归属。

### 7.1 指令一览

* `/help`：`operations::queries::help()`，不发网络请求。
* `/whoami`：`operations::queries::whoami(caller_id)`，不发网络请求。
* `/ping [地址] [端口]`：`ping::ping(host, port)`，要发网络请求（MC 服务器握手）。
* `/checkserver`：`checkserver::checkserver()`，不发网络请求（读本机资源）。

`ping` 与 `checkserver` 返回 `Report::render()` 的结果；按命名规则，一次性探测、检查与整段文本使用裸名词，
见 [architecture.md 第 5 节](../architecture/architecture-zh_cn.md)。

### 7.2 `/help`

* 直接返回配置里的 `text.help` 原文（三引号多行字符串），改文案不用重新编译。
* 任何频道都能用（不需要私聊）。
* 因此帮助菜单与实际支持的指令可能不同步：改了指令记得同步改配置，这一点在配置文件里也写明了。

### 7.3 `/whoami`

* 返回调用者自己的 Discord 用户 ID，并给出可以直接抄进 `[admins]` 的那一行。
* 存在的意义是解开先有鸡还是先有蛋：配置管理员要填自己的用户 ID，而常规拿法（开发者模式、右键头像、
  复制用户 ID）对不熟 Discord 的人不好找。
* 无权限限制；文案由 `operations::queries::whoami` 生成，命令层不再
  自己拼回复（见 [第 1 节](#1-命令层的规矩) 的硬约定）。

### 7.4 `/ping`

* 不带参数时用 `[ping]` 表的默认目标（`default_ping_host` 与 `default_ping_port`）；带参数时以聊天里
  给的为准（端口非法时返回用法错误）。
* 显式指定的主机必须全部解析为公网地址；配置中的默认主机视为可信目标。代理 Fake-IP 可能造成误判，
  处理方法见[故障排查](../../troubleshooting/troubleshooting-zh_cn.md#ping-拒绝探测本地或私网地址)。
* 走 Minecraft 服务器列表握手（`craftping`），因此拿到的是游戏层的响应：在线人数与最大人数，以及
  "钓鱼服务器到游戏服务器"这一跳的延迟，而不是简单的 TCP 连通性。
* 输出固定三行：`状态: 在线`、`人数: 当前人数/上限`、`延迟: N ms(钓鱼服务器到游戏服务器的延迟)`；
  连不上、超时、协议解析失败各给一条可读文案。
* 解析、连接及握手共用 `ping.timeout` 超时（配置校验保证它不为 0）。
* 地址或端口非法、连不上、超时都会各自给出可读文案，不 panic。

### 7.5 `/checkserver`

* 报告的是跑本程序的这台机器（钓鱼进程所在机器）的资源，不是游戏服务器。
* 数据来自 `sysinfo`：CPU 使用率、已用与总内存（按总量自动用 MB 或 GB）、内存占用率、磁盘、系统运行时长；本次运行时长读 `/proc`，系统时间与时区取自本机的 `chrono::Local`。
* 系统信息初始化、磁盘信息或系统运行时长查询失败时，返回相应错误及原因。
* CPU 使用率取两次采样（间隔很短）再计算差值：单次采样拿到的是相对开机以来的平均值，会长时间停留在
  高位、看起来像是满载。
* 输出是固定字段的多行文本，方便直接截图，不依赖日志文件。

## 8. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 动作枚举与权限校验在哪一层：[lib-zh_cn.md](../lib/lib-zh_cn.md)。
* 启动/停止的等待参数、两条日志线：[runtime-zh_cn.md](../runtime/runtime-zh_cn.md) 第 4 节与第 7 节。
* 游戏侧配置这份附件：[config-zh_cn.md](../config/config-zh_cn.md) 第 3.2 节。
