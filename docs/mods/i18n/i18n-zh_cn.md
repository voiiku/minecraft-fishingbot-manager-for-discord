# i18n：程序运行使用的语言
[English](i18n-en_us.md) | **简体中文**

* 机制：`src/text/mod.rs` 定义 `Language`、`Text`、`LogMessages` 和 `WatchdogText`；
  `src/text/languages/` 存放中英文实现。`bootstrap` 提供启动专用英文文案，`report` 负责报告格式。
  `text` 不依赖其他项目模块：语言来自 `Config::get().text()`，启动错误的语言边界由 crate 根的
  `bootstrap_error_text()` 和 `complete_startup()` 管理，报告的语言和长度上限由调用方提供。
* 配置：`[i18n] language = "ENGLISH"`（示例配置里写的就是它；给中文部署时改成 `CHINESE_SIMPLIFIED`）。
  一个进程一种语言，启动时读一次，改了要重启；取值就是 FishingBot 自己 `i18n.Language` 枚举的常量名
  —— 本项目只实现了 `ENGLISH`（English）与 `CHINESE_SIMPLIFIED`（简体中文）两种，上游还有的
  `CHINESE_TRADITIONAL`、`FRENCH`、`GERMAN` 等取值在启动时会被拒掉（我们没写那些语言的文案）。
* 换语言不需要重新编译：两种语言的实现都已经编进二进制，用户只改配置这一行并重启进程。只有新增第三种
  语言才要动代码（见第 2 节）。
* 没进 i18n 的东西是有意不进的，见下一节。

## 1. 边界：Discord 文案与日志匹配用的固定文案

纳入：

* 指令回复、报错（业务层返回的 `Err` 与 `String`）。
* `/show status` 表格列名和状态文案。
* `Report` 的小节标题与"内容过长"的说明。
* 程序日志和运行期错误跟随 `[i18n] language`。配置及其他启动错误在首次框架初始化成功前使用英文；
  原有非错误启动日志仍使用配置语言。
* JAR 打印的 `/status` 回显前缀和不可用提示。

刻意不纳入：

* 机器人日志的内容本身，那是 FishingBot 打印的，我们改不了。只有它自己打印的固定文案才进来，见下。
* 内部不变量消息：`expect(...)` 与 `panic!` 这类代码写错了才会出现的提示，以及 `cargo test` 的断言
  文案。除看门狗锁中毒的 14 条外，它们不进语言表（不实现 `Text`）；必须固定英文的收在 `bootstrap`，见第 4 节。
  看门狗文案是例外：首次初始化成功前的错误使用英文，之后跟随 `[i18n] language`。
* 代码注释、`docs/`、指令名（Discord 只允许 ASCII 小写）。命令与选项说明只写在文案表中，
  注册前严格校验（见 [commands-zh_cn.md 第 1.7.1 节](../commands/commands-zh_cn.md#171-文案表校验)）。
* 示例配置的注释固定写英文，与代码注释分开。
* 需要过滤的刷屏噪声按部署配置在 `[keywords]` 中。

### 为什么 JAR 自己打印的固定文案要进来，而日志内容本身不进来

判据只有一条：这条字符串由谁决定。

* `/status` 的状态前缀和不可用提示由 JAR 语言决定，同样保存在语言表中。
* `filter_regex` 是部署者选择的日志过滤规则，留在 `[keywords]` 中。
* "进程已结束"既不是 jar 打的、也不进配置：它看 screen 会话状态（`(Dead ...)` 残壳，见
  [runtime.md](../runtime/runtime-zh_cn.md)）。日志里那种结束标记是旧版外壳脚本自己写进去的
  （`>>> 机器人进程已结束或被手动中止 <<<`），本项目不产生它，所以没有对应文案要翻译。

换游戏服或改插件时要动 `[keywords]`；换 JAR 的配置语言
（或换 JAR 版本后文案变了）时，把 `[i18n] language` 换成对应语言（新文案确实变了的，改
`src/text/languages/*.rs` 里那两句）。

## 2. 怎么加一种语言（三步，漏一步编译不过）

1. 在 `src/text/languages/` 下新建一个文件，实现 `Text` 和 `LogMessages` 的全部方法。用 trait 而不是 `HashMap<&str, &str>`，就是
   为了漏翻一条编译失败。
2. 在 `src/text/languages/mod.rs` 里加 `mod <文件名>;`，给 `Language` 加一个枚举项，并在
   `Language::from_name()`、`Language::name()` 和 `Language::implementation()` 中增加对应分支。
3. 把语言名加进 `Language::supported_names()`，并保持它与解析和名称返回值一致。取值照抄 FishingBot 自己 `i18n.Language` 枚举的常量名
   —— 它同时就是写进 `config.json` 的 `misc.language`，所以不要另起一套名字（那会多出一张映射表）。

不需要改业务层，也不需要改配置解析：业务层只调 `config::Config::get().text()`。

## 3. 为什么不用外部语言文件（`.ftl`、`gettext`、语言包 toml）

* 本项目要一个二进制加一份配置就能跑：外部语言包是第三个必须跟着走的文件，丢了就没有可用文案。
* 编译期齐备性检查比运行时查表安全：漏翻一条是编译错误，而不是用户在聊天里看到空串或 key。
* 少两个依赖（不引入 Fluent 或 `rust-i18n`）。

代价是新增一种语言要重新编译（切换已有语言不用，两种实现都在二进制里，改配置即可）。这与项目既有的
取舍一致：需要不重编就能改的大段文案（`/help`）本来就放在配置的 `text.help` 里，所以英文用户想让
`/help` 也是英文，把 `text.help` 换成英文文本就行（它本来就是给使用者自己写的）。

## 4. 读取文案

* 业务层与运行时模块通过 `config::Config::get().text().xxx(...)` 获取本地化文案，不依赖 Discord 框架。
* 日志与控制台文案直接调用 `LogMessages` 的命名方法，例如 `config::Config::get().text().process_log_path(path)`。
  `Text` 继承 `LogMessages`，两种语言漏实现方法都会编译失败。级别（`info!`、`warn!`）仍由调用点决定。
* 配置解析时将语言名转换为 `Language`，不支持的语言使用英文报错；整份配置校验通过后才发布。
  `config::Config::get().text()` 通过已发布配置选择文案，不再单独安装或存储语言。
  配置尚未加载时访问配置会 panic；测试应显式加载配置，或直接使用指定语言实现。
  同一语言名也写入游戏侧 `config.json` 的 `misc.language`（见 [config.md 第 3.2 节](../config/config-zh_cn.md)）。
* `text` 不依赖其他项目模块；报告创建时由调用方传入语言和回复长度上限。配置和游戏侧配置的启动专用错误由
  `text::bootstrap` 输出英文。
* 共享错误通过 crate 根的 `bootstrap_error_text()` 取文案：首次框架初始化成功前使用英文，之后使用配置语言。
  初始化在同步命令注册尝试后完成；单个服务器注册失败只产生告警，不阻止完成。首次成功之前的失败与重试
  继续使用英文，后续重连不会重置这一边界。外部错误细节保留原文。
  将本项目作为库嵌入其他程序时，应在自身初始化成功后调用 crate 根的 `complete_startup()`；否则共享错误
  会一直使用英文。
* 普通信息和指令说明仍使用 `config::Config::get().text()`，因此继续跟随配置语言。
* 看门狗互斥锁中毒文案由 `WatchdogText` 和 `Text::watchdog_text` 提供，遵循同一启动错误边界；
  静态字符串避免在每次锁操作时构造临时 `String`。
* 指令说明等注册期文字也从已加载的配置读取语言。

## 5. 排版约定：符号跟随语言，补充说明用圆括号

符号随语言走：

* 中文：标点用中文全角（`，`、`：`、`！`、`；`、`……`），句尾不留句号（`。`）。
* 英文：标点一律用英文半角（`.`、`,`、`:`、`!`、`;`、`...`），不出现 `…`、`—` 这类非 ASCII 字符，
  也不出现任何中文标点；句尾同样不留句号。

补充说明（括号里那句解释）两种语言统一写成 `()`，例如 `/log` 的选项说明
`读取最新多少行 (默认 50，上限见配置 runtime.max_reply_log_lines)` 与
`how many trailing lines to read (default 50, cap in runtime.max_reply_log_lines)`。三类例外是"看起来像
括号、并非补充说明"的记号，照旧用各自的写法：

* 配置表名照抄方括号：`[admins]`、`[keywords]`、`[raw_game_logs]`。
* `Report` 的小节标签：`[完成明细]`、`[异常明细]`、`[Completed]`、`[Issues]`。
* 用法提示里的参数记号：可选参数写 `[行数]`、必填参数写 `<机器人名>`，与
  [README-zh_cn.md](../../README/README-zh_cn.md) 的记法一致。

错误回复一律以 `错误:` 开头（英文 `Error: `），让人一眼分出"这是失败"与"这是结果"；纯结果行（`/ping`
的延迟、`/startall` 的明细）不加这个前缀。

理由：括号统一后，中英两版的排版骨架完全一致（只有字和标点不同），维护时容易逐条对照；半角圆括号在中英
混排时字距稳定，而 `[]` 与 `<>` 留给表名、小节标签与参数记号，读起来不会与补充说明混淆。命令说明表
（`command_texts()`）里的描述与 `/help` 的默认正文同样遵循这套约定。

## 6. 覆盖范围与核对方式

以下内容都由 `Text` 与相关枚举提供多语言实现：

* `report.rs`：`Report` 小节标签、"内容过长"说明、计数行 `count_line`。
* `runtime/bot_instances/bot_status.rs`：`/show status` 表里的文字，以及状态查询的 `jar_*` 固定文案。
* `text::bot_name_error`：将 `utils::bot_name` 的校验错误转换为机器人名文案。
* `runtime/bot_instances/bot_dirs.rs`（含 `BotTokenStatus` 的 `Display`）与 `runtime/bot_instances/bot_sessions.rs`（含 `/show status`
  列名）。
* `runtime/bot_instances/screen.rs`、`runtime/bot_startup/mod.rs`、`runtime/bot_instances/stop.rs`：启动与停止流程的明细行。
* `operations/`：业务操作和查询结果。
* `operations/mod.rs` 使用非管理员的拒绝文案；`discord_commands/` 使用进度提示、附件正文和用法提示。
* 指令元数据：指令说明、选项说明、`/token` 弹窗标签（注册前改写宏生成的指令树，做法见下）。
* 程序日志直接调用 `LogMessages` 的命名方法；启动专用配置错误位于 `bootstrap`。
* 看门狗锁操作走 `WatchdogText` 和 `watchdog_text()`，遵循上述启动错误边界。

按边界不做：内部不变量消息（`expect(...)` 与 `panic!`）与测试断言；看门狗锁中毒文案是例外。

两种语言必须实现全部 `Text` 和 `LogMessages` 方法，并穷尽 `WatchdogText`；遗漏会导致编译失败。
指令说明表也有测试校验。

措辞以中文为准：改文案先改 `zh_cn.rs`，再把 `en_us.rs` 逐条对齐。测试可直接选择英语实现，
措辞类断言因此以英语为基准；这不影响中文原稿的地位。

### 指令元数据（指令说明、选项说明、`/token` 弹窗）

这些文字跟随配置语言，不写在命令宏中：

* 指令说明与选项说明：`discord_commands::all_commands()` 建完指令树后，用 `Text::command_texts()`（表）逐条
  校验并填入 poise 的 `Command.description` 与 `CommandParameter.description`。
* `/token` 弹窗的标题、字段标签与占位提示：不用 `#[derive(Modal)]`，改成手写 `impl poise::Modal`，
  在 `create()` 里现取文案、`parse()` 按 `custom_id` 取值（形状与 derive 生成的完全一致）。

于是这些文字跟的也是配置里的语言（不是 Discord 客户端语言），与回复文案同一个开关。

指令说明表与指令树必须一一对应；框架创建时会检查缺项、重复项、未知项与说明长度，不合法时明确报错，
不会注册占位文案。测试会对两种语言检查完整性和实际注册结构。

与 Discord 自带 `description_localized` 的区别：那是让 Discord 按客户端语言挑显示哪一份，与"一个进程
一种语言"是两套机制。本项目不用它，否则会出现说明是英文、回复是中文的混搭。

## 7. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 指令说明、选项说明与弹窗标签的多语言来源：[commands-zh_cn.md](../commands/commands-zh_cn.md)。
* 文案与日志匹配的不变量：[CONTEXT-zh_cn.md](../../CONTEXTS/CONTEXT-zh_cn.md) 第 2.2 节。
