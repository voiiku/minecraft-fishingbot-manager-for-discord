# i18n：程序运行使用的语言
[English](i18n-en_us.md) | **简体中文**

* 机制：`src/text.rs`（`Language`、`Text` trait、`install`、`get`）与 `src/text/*.rs`（每种语言一个
  文件：`en_us.rs` 是英语实现，`zh_cn.rs` 是简体中文；另有五个子模块：`bootstrap`（引导期英文）、
  `log_text`（`LogText`）、`requirement`（`ConfigRequirement`）、`setup_failure`（装配失败该用哪条
  文案、算不算永久错）与 `watchdog`（看门狗互斥锁中毒的内部不变量文案，随语言切换））。
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
* `/show status` 表格里的那些字（`ConnectionStatus` 与 `PauseStatus` 的 `Display`）。
* `Report` 的小节标题与"内容过长"的说明。
* 程序日志、启动期控制台提示与配置报错。它们给部署者看，但同样跟随 `[i18n] language`（做法见第 4
  节）；语言还没读出来时的提示是唯一例外，固定英文（`src/text/bootstrap.rs`）。
* JAR 自己打印的断线重连提示（`jar_reconnect_prefix`、`jar_reconnect_suffix`），用来在机器人日志里认出
  "重连了"。

刻意不纳入：

* 机器人日志的内容本身，那是 FishingBot 打印的，我们改不了。只有它自己打印的固定文案才进来，见下。
* 内部不变量消息：`expect(...)` 与 `panic!` 这类代码写错了才会出现的提示，以及 `cargo test` 的断言
  文案。除看门狗锁中毒的 14 条外，它们不进语言表（不实现 `Text`）；必须固定英文的收在 `bootstrap`，见第 4 节。
  看门狗文案是例外：它发生在配置装载之后，必须跟随 `[i18n] language`。
* 代码注释、`docs/`、指令名（Discord 只允许 ASCII 小写）。例外：命令 `///` 的第一段与选项的
  `#[description]` 会变成发给 Discord 的说明，那两处写英文（等于 `en_us.rs` 表里那句，见
  [commands-zh_cn.md 第 1.7.1 节](../commands/commands-zh_cn.md#171-宏里那一份算什么英文副本)）。
* 示例配置的注释固定写英文：它是给"照着模板改配置"的人看的，与中文的代码注释分开。只有"取值随语言变、
  中文部署必须改成另一个值"的那几项，额外带一行 `# CHINESE_SIMPLIFIED:` 提示（`command_echo` 的两条
  回显与 `[i18n] language`）—— 那不是第二份说明，而是取值提示，改值的人会同时看到它，因此不会漂移。
* 与游戏服务器或本机环境有关的关键词：服务器自己的进服提示、本机环境的结束标记、自己服务器要丢掉的
  刷屏噪声。它们各部署不同，留在配置的 `[keywords]` 里。
* `/pause` 与 `/unpause` 的回显：它由 JAR 打印；是否出现取决于所用 JAR 是否支持这两条指令，所以留在配置的
  `[command_echo]`（`pause_echo`、`unpause_echo`），换 JAR 时就地改，不必重新编译。

### 为什么 JAR 自己打印的固定文案要进来，而日志内容本身不进来

判据只有一条：这条字符串由谁决定。

* `bot-automatic-reconnect`（重连提示）由 JAR 自己的语言资源决定：中文 JAR 就是那句中文，英文 JAR 就
  是那句英文，与跑在哪个游戏服、哪台机器无关。让部署者去配置里再抄一遍，等于把程序本来就知道的事变成
  一个人为出错点（抄错一个标点，`/show status` 就永远显示"初始中"）。所以它跟着 `[i18n] language` 走，跑
  哪种语言的 JAR 就把 `language` 设成哪种。这两条字符串都写在 `src/text/zh_cn.rs` 与
  `src/text/en_us.rs` 里一一对应，测试
  `text::tests::jar_reconnect_messages_are_non_empty_and_distinct_in_both_languages` 盯着它们（中英
  都非空、两版互不相同）。
* `command-pause-executed`、`command-unpause-executed`（两条指令的回显）同样由 JAR 决定，但只在所用 JAR 支持
  `/pause`、`/unpause` 时才会打印（上游已计划加入这两条指令），所以它们落在配置 `[command_echo]` 的
  `pause_echo`、`unpause_echo` 上：换 JAR 时改配置即可，不必重新编译；启动校验要求非空、且两条不能互相包含
  （否则日志里那行认不出属于哪条指令）。
* 反过来，`online`（进服提示，来自游戏服务器或插件）与 `filter_regex`（自己服务器的刷屏噪声）只有
  部署者知道，JAR 里没有对应句子，它们留在 `[keywords]` 里，由启动校验保证非空。
* "进程已结束"既不是 jar 打的、也不进配置：它看 screen 会话状态（`(Dead ...)` 残壳，见
  [utils.md](../utils/utils-zh_cn.md)）。日志里那种结束标记是旧版外壳脚本自己写进去的
  （`>>> 机器人进程已结束或被手动中止 <<<`），本项目不产生它，所以没有对应文案要翻译。

换游戏服或改插件时要动 `[keywords]`；换 jar 构建时看 `[command_echo]` 的两条回显；换 JAR 的配置语言
（或换 JAR 版本后文案变了）时，把 `[i18n] language` 换成对应语言（新文案确实变了的，改
`src/text/*.rs` 里那两句）。

## 2. 怎么加一种语言（三步，漏一步编译不过）

1. 在 `src/text/` 下新建一个文件，实现 `Text` 的全部方法。用 trait 而不是 `HashMap<&str, &str>`，就是
   为了漏翻一条编译失败。
2. 在 `src/text.rs` 里加 `mod <文件名>;`，给 `Language` 加一个枚举项，并在
   `Language::implementation()` 的 `match` 里加一个分支（`install()` 的实现在那里；`get()` 未安装时生产
   构建 panic，测试构建直接使用英语实现）。
3. 把语言名加进 `Language::supported_names()`。这是合法取值的唯一来源：配置校验、报错文案、示例配置
   与本文档都取自它，不会出现两份名单漂开。取值照抄 FishingBot 自己 `i18n.Language` 枚举的常量名
   —— 它同时就是写进 `conf.json` 的 `misc.language`，所以不要另起一套名字（那会多出一张映射表）。

不需要改业务层，也不需要改配置解析：业务层只调 `text::get()`。

## 3. 为什么不用外部语言文件（`.ftl`、`gettext`、语言包 toml）

* 本项目要一个二进制加一份配置就能跑：外部语言包是第三个必须跟着走的文件，丢了就没有可用文案。
* 编译期齐备性检查比运行时查表安全：漏翻一条是编译错误，而不是用户在聊天里看到空串或 key。
* 少两个依赖（不引入 Fluent 或 `rust-i18n`）。

代价是新增一种语言要重新编译（切换已有语言不用，两种实现都在二进制里，改配置即可）。这与项目既有的
取舍一致：需要不重编就能改的大段文案（`/help`）本来就放在配置的 `text.help` 里，所以英文用户想让
`/help` 也是英文，把 `text.help` 换成英文文本就行（它本来就是给使用者自己写的）。

## 4. 读法与未安装时的行为

* 业务层与 utils 一律 `text::get().xxx(...)`（它们不碰框架，只碰这里）。
* 日志与控制台文案走 `text::get().log_text(LogText::…)`。这类文案数量多、结构单一，所以用"带数据的
  枚举加一个方法"收口：两个语言文件里的 `match` 是穷尽匹配，漏翻一条或加了变体只在一边实现都编译
  不过（与逐条方法同样的保证）。级别（`info!`、`warn!`）仍由调用点决定，因为那是语义而不是文案。
* `text::get()` 只读取安装过的实现。生产构建在未安装时直接 panic（入口调用顺序错误）；测试构建固定返回
  英语，免去每个测试都先 `install()`。`Language` 不再实现 `Default`；`validate()` 的第一个调用
  `validate_i18n` 会固定英文报错，配置入口打印到终端并以退出码 1 结束进程。测试里的文案断言因此分两类：
  1. 接线类（这句话来自哪张表、随语言变）：期望值从 `text::get()` 取（例如 JAR 回显、机器人名报
     错），换个测试语言也不会红；
  2. 措辞类（配置报错这类由字段、值、要求拼起来的整句）：钉英语实现的片段。

  示例配置里的 `language = "ENGLISH"` 只决定新部署运行哪种语言（中文部署改成 `CHINESE_SIMPLIFIED`），
  与测试构建未安装语言时使用英语无关。它同时是生成出来的 `conf.json` 里 `misc.language` 的取值（同一个值，
  不是映射过去的，见 [config.md 第 3.2 节](../config/config-zh_cn.md)），于是 jar 跟着说同一种话，
  两处不会再对不上。
* 装语言的时机决定了哪几条消息只能用引导期英文：
  1. `config::Config::get_or_init()` 解析成功之后立刻 `install()` 一次，于是配置校验报错也是配置语言；
  2. `main.rs` 不再重复装。语言在第 1 步就装好了，所以 `ProcessLog::get_or_init()` 起程序日志也是配置
     语言。入口那边刻意不写第二次 `install()`，因为写了也不生效，只会让"语言在哪选定"变得含糊；
  3. 第 1 步之前发出去的提示一律固定英文：配置文件不存在、读失败、解析失败，生成模板失败，可执行文件
     路径定位不到，`[i18n] language` 自己写错，以及配置没装载就调 `config::Config::get()` 时那句 `expect`
     （`bootstrap::CONFIG_NOT_LOADED`），还有语言没安装就取文案时的 `bootstrap::TEXT_NOT_INSTALLED`。那时语言还没确定，提示固定英文。它们统一收在
     `crate::text::bootstrap`（与语言表并列，但不实现 `Text`）。判断标准只有一条：这条提示是不是配置
     还没读成功的产物。
* 看门狗互斥锁中毒时的文案由 `crate::text::watchdog::WatchdogText` 与
  `Text::watchdog_text` 提供，随 `[i18n] language` 切换；读取静态字符串可避免在每次锁操作时构造临时 `String`。
* 指令说明这类注册期文字也读同一个 `get()`：`commands::all_commands()` 在建完指令树后调用一次（那时
  `install()` 已经跑过）。

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
* `Report` 的小节标签：`[成功明细]`、`[内容]`、`[Success]`、`[Content]`。
* 用法提示里的参数记号：可选参数写 `[行数]`、必填参数写 `<机器人名>`，与
  [README-zh_cn.md](../../README/README-zh_cn.md) 的记法一致。

错误回复一律以 `错误:` 开头（英文 `Error: `），让人一眼分出"这是失败"与"这是结果"；纯结果行（`/ping`
的延迟、`/startall` 的明细）不加这个前缀。

理由：括号统一后，中英两版的排版骨架完全一致（只有字和标点不同），维护时容易逐条对照；半角圆括号在中英
混排时字距稳定，而 `[]` 与 `<>` 留给表名、小节标签与参数记号，读起来不会与补充说明混淆。命令说明表
（`command_texts()`）里的描述与 `/help` 的默认正文同样遵循这套约定。

## 6. 覆盖范围与核对方式

以下内容都由 `Text` 与相关枚举提供多语言实现：

* `utils/reply.rs`：`Report` 小节标签、"内容过长"说明、计数行 `count_line`。
* `utils/bot_status.rs`：`/show status` 表里那些字，以及 `jar_*` 固定文案（断线重连提示的前后缀；`/pause`
  与 `/unpause` 的回显已按"换构建要跟着改"的理由挪到配置 `[command_echo]`，见第 1 节）。
* `utils/bot_name.rs`：机器人名报错。
* `utils/bot_dirs.rs`（含 `BotTokenStatus` 的 `Display`）与 `utils/running_bots.rs`（含 `/show status`
  列名）。
* `utils/screen.rs`、`utils/startup.rs`、`utils/shutdown.rs`：启动与停止流程的明细行。
* `bot_control/*`、`bot_manage/*`、`all_bots_control/*`、`show/*`、`general/*`。
* `lib.rs`（非管理员的拒绝文案）与 `commands/*` 的固定话术（"已开始执行"、附件正文、用法提示、
  `/whoami`）。
* 指令元数据：指令说明、选项说明、`/token` 弹窗标签（注册前改写宏生成的指令树，做法见下）。
* 程序日志（`log::*`）、启动期控制台提示、配置报错（`config.rs` 与 `config/*`）：走 `LogText` 枚举
  加 `log_text()`，两种语言里穷尽 `match`。
* 看门狗锁操作：走 `WatchdogText` 枚举加 `watchdog_text()`（14 条，随语言切换）。

按边界不做：内部不变量消息（`expect(...)` 与 `panic!`）与测试断言。唯一的例外是看门狗锁中毒文案，
它属于运行期信息，随 `[i18n] language` 切换。

`Text` 目前有 161 个方法（含 `log_text`）、27 条指令说明，以及 `LogText` 的 77 个变体、
`ConfigRequirement` 的 19 个变体与 `WatchdogText` 的 14 个变体，全部都有调用点。可以这样自查（第一条先用
`sed` 切出 `pub trait Text` 到它结尾的整段，再按 `fn` 名字去重，所以签名跨行书写也不会漏——
`command_log_attachment_body` 就是这么写的；第一条同时会列出"没有调用点的方法"，一切正常时它没有输出。
后三条分别数三个枚举的变体数，`match` 是穷尽的，所以语言文件里出现的名字数就是枚举的变体数）：

```bash
sed -n '/^pub trait Text/,/^}/p' src/text.rs | grep -oE 'fn [a-z_0-9]+' | sort -u | wc -l  # 161
sed -n '/^pub trait Text/,/^}/p' src/text.rs | grep -oE 'fn [a-z_0-9]+' | sed 's/fn //' | sort -u | \
while read -r m; do [ "$(grep -rn "\.$m(" src/ --include='*.rs' | grep -vc '^src/text')" -eq 0 ] && echo "未被调用: $m"; done
grep -oE 'LogText::[A-Za-z]+' src/text/en_us.rs | sort -u | wc -l            # 77
grep -oE 'ConfigRequirement::[A-Za-z]+' src/text/en_us.rs | sort -u | wc -l  # 19
grep -oE 'WatchdogText::[A-Za-z]+' src/text/en_us.rs | sort -u | wc -l       # 14
```

英文实现 `src/text/en_us.rs` 与中文一一齐备（161 个方法、27 条指令说明、77 与 19 个枚举变体，外加 14 条看门狗
文案，一个不少：漏方法或漏 `match` 分支都编译不过，漏指令说明测试失败）。要确认英文确实上得来，把配置改成
`language = "ENGLISH"` 跑一次；单测里有 `text::tests::english_impl_is_english` 直接按英文实现断言（它
不调 `install`，因为 `OnceLock` 是进程级的，装了会影响并发跑的其它用例）。

措辞以中文为准：改文案先改 `zh_cn.rs`（内容与排版的原稿），再把 `en_us.rs` 逐条对齐，最后同步
`commands/*.rs` 里那份与英文表逐字相同的文案副本。测试进程未安装语言时 `get()` 返回英语实现，措辞类断言
因此以英语为基准；这不影响中文原稿的地位。

### 指令元数据（指令说明、选项说明、`/token` 弹窗）

这些文字由 `#[poise::command]` 与 `#[derive(Modal)]` 宏在编译期写进二进制，宏只认字面量，运行期改
不了。所以做法是注册之前把文字改一遍（也只做一次）：

* 指令说明与选项说明：`commands::all_commands()` 建完指令树后，用 `Text::command_texts()`（表）逐条
  覆盖 poise 的 `Command.description` 与 `CommandParameter.description`（poise 这两个字段是公开的，
  能直接改写）。
* `/token` 弹窗的标题、字段标签与占位提示：不用 `#[derive(Modal)]`，改成手写 `impl poise::Modal`，
  在 `create()` 里现取文案、`parse()` 按 `custom_id` 取值（形状与 derive 生成的完全一致）。

于是这些文字跟的也是配置里的语言（不是 Discord 客户端语言），与回复文案同一个开关。

齐备性怎么保证：指令说明用"表加覆盖测试"而不是几十个 trait 方法，因为元数据与 `src/commands/*.rs`
里的名字一一对应，表更好对照。`commands` 里的两条测试把这事钉死：
`every_command_and_option_has_text_in_every_language` 逐语言检查每一条指令与每一个选项（漏一条就列出
缺哪条），`command_descriptions_come_from_the_language_table` 证明注册时真的改写生效（而不是只把表放
在那儿）。效果等同漏翻编译不过，只是发生在测试期。

与 Discord 自带 `description_localized` 的区别：那是让 Discord 按客户端语言挑显示哪一份，与"一个进程
一种语言"是两套机制。本项目不用它，否则会出现说明是英文、回复是中文的混搭。

## 7. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 指令说明、选项说明与弹窗标签的多语言来源：[commands-zh_cn.md](../commands/commands-zh_cn.md)。
* 文案与日志匹配的不变量：[CONTEXT-zh_cn.md](../../CONTEXTS/CONTEXT-zh_cn.md) 第 2.2 节。
