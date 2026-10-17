# 配置文件结构说明
[English](config-en_us.md) | **简体中文**

* 代码：`src/config.rs`（运行时 `Config` 与取值接口）与 `src/config/{schema,parse,validate,template}.rs`
  （各配置表与 `[paths]` 的文件形态、解析、校验、模板）
* 文件：可执行文件同级目录下的 `minecraft-fishingbot-manager-for-discord.conf.toml`（文件名由
  `CONFIG_FILE_NAME` 固定，不可改名）
* 刻意不看工作目录、不读环境变量：部署时把二进制与配置放在一起就不会错，systemd 之类的托管也不需要
  额外设置。
* 文件里有 Bot Token，建议 `chmod 600`。

## 1. 加载流程（启动最早期）

1. `config::Config::get_or_init()` 定位配置文件路径。
2. 文件不存在时，按仓库里的 `minecraft-fishingbot-manager-for-discord.conf.example.toml` 生成一份：写模板与
   打印提示都在这一步里做，然后就地以退出码 0 结束进程，不进 Discord。文件已存在则在终端提示找到的那份
   配置文件路径再往下走（这一步日志系统还没装，所以证据只在终端；程序日志里那句"已加载配置: …"由
   `ProcessLog::get_or_init` 补）。
3. 文件存在时读文本，`Config::from_toml_text` 先把 TOML 解析成值树、递归去掉所有字符串值两头的空白，再用
   Serde 的 `Deserialize` 实现直接映射成运行时 `Config`（这是全链路唯一一次清洗）；其中 `[paths]` 那三个键
   由 `Paths` 自己的反序列化实现展开成以 `base_dir` 为根的路径。这一步只负责格式解析与结构转换，不负责所有配置规则。
4. `validate` 先校验 `i18n.language`：失败信息固定英文、不调用 `text::get()`；合法就安装语言，再检查其余字段的取值、范围与运行环境（例如必填项、端口范围、目录和 JAR 是否存在）。
5. 任何一步失败都以"配置项 + 字段名 + 要求"的形式报错退出，不带着错值跑起来。
6. 通过后写入 `OnceLock`，只在启动时读一次并常驻内存。运行中改配置文件不影响已经在跑的进程，想让新
   配置生效就重启进程。

字符串值两头的空白统一在 `Config::from_toml_text` 里去掉：配置文件里多写的空格、缩进与换行都不会带进代码，
因此读取配置的入口（`main.rs`、业务层）直接取原值，不再各自 `trim()`。需要保留两头的空白时写转义
（TOML 里的 `"\u0020"`）。

反序列化依赖 `Config` 及其字段类型上的 `#[derive(Deserialize)]`：Serde 按 TOML 表名与键名填充字段，类型不符、
缺少必需字段或出现未知字段时返回解析错误。代码里**没有任何默认值**：每张表、每个字段都必须在文件里显式写出，
缺项或拼错都在启动时报错，不会被静默兜底。路径字段由 `Paths` 自己的 `Deserialize` 实现转为以 `base_dir` 为根的
运行时路径（`bots_dir` 由代码派生），业务取值规则仍由后续的 `validate` 负责。

配置读完后、执行其他步骤之前，进程入口会按 `[fishingbot]` **生成游戏侧配置**（FishingBot 的 `conf.json`，
路径来自 `paths.fishingbot_config_file_name`）：先删掉旧文件，再写一份新的。这一步失败按"配置写错"的同一口径
处理（记 ERROR + 退出码 1）。为什么是生成物、生成哪些内容，见 3.2 节。

## 2. 对外接口

都是 `Config` 上的关联函数（模块里没有游离的取值函数；运行时的 `Config` 就定义在本文件），进程入口因此只有一行
`let settings = config::Config::get_or_init();`：

* `Config::get_or_init()`：取配置，也是进程入口的唯一入口。首次调用完成定位、读取、解析与校验，之后返回已装载的
  `&'static Config`。模板已生成（提示已打印）时，函数就地以退出码 0 结束进程；配置不合法时，报错并以退出码 1
  结束进程。两条失败路径都在函数内结束进程，因此返回值一定是可用配置，调用方不必再判结果。
* `Config::get()`：取已加载的 `Config`。
* `Config::config_file_path`：本次启动使用的配置文件路径（装载时按可执行文件位置算好，存在结构体字段里）。
* `Config::admins()`：取管理员名单（`Config::get().admins` 的快捷方式）。
* `Config::install_dir()`：取部署目录（可执行文件所在目录）。配置与程序日志都在这里。

`Config::get()` 在尚未装载时 panic，文案取自 `text::bootstrap::CONFIG_NOT_LOADED`（内部不变量消息，那时
语言还没装，固定英文）：进程入口第一件事就是 `Config::get_or_init()`，走到这里还 panic 说明调用顺序写错了。
配置文件路径是 `Config` 的一个字段（装载时算一次），"是否已装载"仍只看那一个 `OnceLock<Config>`。

## 3. 表结构

顺序与示例配置一致：新部署需要改动的表排在前面 —— 凭据、路径、游戏侧配置（`[fishingbot]`），其次是按游戏服务器
或 jar 补充的三张表（关键词、回显、语言），然后是通常保持默认的运行参数与时序，最后是纯可选的开关与展示文本。
示例配置的说明注释以英文为主；`[text].help` 上方另有完整中文原文注释，供中文用户参考。少数以
`# CHINESE_SIMPLIFIED:` 开头的行是"中文部署该填什么值"的提示（只有 `[command_echo]` 的两条回显与
`[i18n] language` 需要），见 [i18n.md 第 1 节](../i18n/i18n-zh_cn.md)。

* `[discord]`：`token`。Bot Token。唯一凭据，必填。
* `[paths]`：`base_dir`（绝对路径）、`jar`、`fishingbot_config_file_name`（相对路径）。数据根目录与共享文件。
* `[fishingbot]`：`server_ip`、`server_port`、`default_protocol`、`auto_reconnect`、
  `auto_reconnect_time`、`start_text`、`announce_type_console`、`announce_type_chat`。游戏侧配置
  （FishingBot 的 `conf.json`）里**由本项目开放**的那些键：启动时据此生成那份文件（见 3.2 节）。
  整张表必填；取值照抄随仓库发布的示例配置即可。
* `[admins]`：`"Discord用户ID" = "名字"`。管理员名单；键必须是纯数字的用户 ID（snowflake）。
* `[keywords]`：`online`、`filter_regex`。只有各部署不同、必须自己填的日志关键词：游戏服的进服提示与
  写盘前要丢掉刷屏行的正则（`grep -v -E`）。这里**没有**"进程已结束"的关键词：那种标记是旧版外壳脚本
  自己写进日志的（`>>> 机器人进程已结束或被手动中止 <<<`），本项目不写它，进程是否结束看 screen 会话
  状态（`(Dead ...)` 残壳），所以不需要配置。为什么只剩两项见 3.1 节。
* `[command_echo]`：`pause_echo`、`unpause_echo`、`wait_ms`、`scan_lines`。确认 `/pause` 与
  `/unpause` 真的执行了：前两项是 JAR 收到指令后打印的那行回显（上游消息 `command-pause-executed` 与
  `command-unpause-executed`，按所用 jar 的语言填，中文 jar 是 `已暂停钓鱼` / `已恢复钓鱼`）；发完等
  `wait_ms` 毫秒，再到最新一份日志的末尾（只看最后 `scan_lines` 行）按这两句判断。
* `[i18n]`：`language`。程序运行使用的语言，影响 Discord 文案、程序日志与报错；取值就是 FishingBot 自己
  `i18n.Language` 的常量名，它也是写进 conf.json 的 `misc.language`（同一个值，不是映射过去的），所以
  不用手工对齐两处。本项目实现了 `ENGLISH`（默认）与 `CHINESE_SIMPLIFIED`；改配置加重启即生效，不需要
  重新编译。见 3.1 节。
* `[runtime]`：`java_opts`、`screen_prefix`、`token_file`、`bot_logs_dir`、`max_reply_log_lines`、
  `max_reply_chars`、`max_autocomplete_choices`。单个机器人的运行时参数。`bot_logs_dir` 里只有我们的日志（FishingBot 自己的原始日志在
  同层 `[raw_game_logs] dir_name` 下的 `[raw_game_logs] file_name`，指向 `discard_path`，见
  [utils.md 第 7 节](../utils/utils-zh_cn.md#7-日志有两条线)）；`max_reply_chars` 是单条回复的长度上限（Discord 硬
  上限 2000，超长会直接发送失败）；`max_autocomplete_choices` 是机器人名选项自动候选的条数上限（1-25，Discord 硬上限）。
* `[startup_queue]`：`max_batch_size`、`bot_login_interval_ms`、`batch_login_interval_ms`、`max_pending_requests`。
  四项依次是全局 FIFO 启动队列的每批最多启动数量、批内相邻两台的登录间隔（毫秒）、批与批之间的间隔（毫秒）、
  等待调度的请求上限（不含正在执行的请求）。整张表与每个字段都必填，且每个值必须大于 0。
  `bot_login_interval_ms` 管批内错峰：同一批里相邻两台按它隔开。`batch_login_interval_ms` 管批间：从"上一次取出一批"的时刻算起，
  至少隔它才取下一批（复核在此期间并行进行，不计入批间等待）；即使上一批只有一台也照此执行，因此空闲很久之后的第一条命令无需等待。
  微软按出口 IP 限制登录频率，取值理由见[故障提示第 12 节](../../troubleshooting/troubleshooting-zh_cn.md)。
  `/start`、`/startall`、`/restart` 与 `/restartall` 共用队列；`/bot login` 不使用它。队列最多暂存 `max_pending_requests` 台，
  装不下的那部分当场退回给调用方（回复里会说明）；程序重启会丢弃尚未执行的队列任务。
  等启动结果的兜底超时不用配置：它由这张表与 `timing.check_grace_ms` 算出来（`StartupQueue::result_timeout`；后者只计一次）：
  `2 × (⌈max_pending_requests / max_batch_size⌉ × max(batch_login_interval_ms, (max_batch_size - 1) × bot_login_interval_ms) + check_grace_ms)`。
  超时就把还没回结果的按"未确认"写进回复（同时记一条程序日志）；批内错峰越慢（微软限流）、队列容量越大，
  它自动越长，不需要人为跟着调。
* `[timing]`：`check_grace_ms`、`stop_grace_ms`、`stop_settle_ms`、`watchdog_window_ms`。各流程等待时间（毫秒）。
  `check_grace_ms` 是每次启动请求的全部机器人拉起后的会话存活宽限期（复核分两次：拉起后立即判定一次，等满它再判定一次；
  每次启动请求只等一次，不乘台数）；
  `watchdog_window_ms` 是网关看门狗的判定窗口，掉线后也用作重建连接前的等待，REST 探活间隔（它的 1/3）与首次
  握手超时（它的 2 倍）由它派生，不再是独立配置项，见 [watchdog.md](../watchdog/watchdog-zh_cn.md)。
* `[raw_game_logs]`：`dir_name`、`file_name`、`discard_path`。FishingBot.jar 自己那份原始日志的约定
  （目录名、文件名、内容丢到哪里）。跟着 JAR 走，整张表必填；示例配置里是 `raw`、`log0.log`、`/dev/null`。
* `[ping]`：`default_ping_host`、`default_ping_port`、`timeout_ms`。`/ping` 不带参数时的默认目标与超时。
* `[log]`：`debug`。程序日志的级别开关。`false`（默认）时本项目自己的记录 `INFO` 起、第三方库
  （serenity、poise）`WARN` 起；`true` 时两边都放到 `DEBUG`（即全部放开）。这是唯一的级别
  开关（不再认环境变量）；细节见 [process-log.md](../process-log/process-log-zh_cn.md)。
* `[text]`：`help`。`/help` 的完整输出（多行字符串，改文案不用重新编译）。

### 3.1 关键词放哪里：`[keywords]`、`[command_echo]` 与 `[i18n]`

* 各部署不同、只能由部署者提供的字符串放 `[keywords]`：游戏服务器自己的进服提示（`online`）、自己
  服务器上要丢掉的刷屏噪声（`filter_regex`）。
* `/pause` 与 `/unpause` 的回显放 `[command_echo]`（`pause_echo`、`unpause_echo`）：它同样由 jar 打印，
  但只存在于带这两条指令的构建里（上游原版没有），放进配置后即可适配其他构建，不必重新编译。
* 断线重连提示的前后缀留在程序里（`src/text/*.rs` 的 `jar_*` 方法），跟随 `[i18n] language`：它只随
  jar 的语言变化、各部署一致，抄进配置只会多一个出错点。这个语言也是生成 `conf.json` 时写进
  `misc.language` 的值（3.2 节），因此 jar 一定使用同一种语言 —— 以前要手工对齐两处，现在已不存在对不上的可能。
* "进程已结束"不需要关键词：旧版外壳脚本用 `java … | tee bot.log; echo 标记 >> bot.log` 往日志里写一句
  结束标记，本项目自己拉起会话、不写它，判定改看 screen 会话状态（`(Dead ...)` 残壳，见
  [utils.md](../utils/utils-zh_cn.md)）。这比抄一句标记更稳：不受 jar 版本、脚本写法与语言影响。

判断标准是"这条字符串由谁决定、换构建时要不要跟着改"：由部署环境决定的进 `[keywords]`，由 jar 的补丁
构建决定的进 `[command_echo]`，由 jar 的语言决定的跟随 `[i18n]`；不属于"字符串"的判断（进程结束与否）
直接看会话状态。

### 3.2 游戏侧配置（`conf.json`）是生成物

FishingBot 只认 JSON（`-config <path>`），**没有任何逐项覆盖配置的命令行参数**（启动参数只有 `-nogui`、
`-help`、`-logsdir`、`-config`、`-refreshToken`、`-onlyCreateConfig`），所以"把它那份 `conf.json` 合并
进 TOML"只能是**单向生成**：TOML 是唯一真源，进程启动时写出那份 JSON（代码在 `utils/fishingbot_config.rs`）。

生成方式：**拿随仓库发布的模板当底、只覆盖 `[fishingbot]` 里那几个键**。

* 模板是仓库根的 `fishingbot-config.example.json` —— 一份**完整**的 FishingBot 配置（取自上游生成的
  默认文件，再按本项目的需要调整过默认值：关掉控制台/聊天播报、把 `/home fishing` 当默认抛竿起手、
  `auto-reconnect-time` 拉长等）。它可以被 `include_str!` 编进二进制，所以部署时不用带着它。
* 覆盖的键就是 `utils/fishingbot_config.rs` 里那份托管清单（十条点分路径），与 `[fishingbot]` 的字段一一对应；
  其中 `misc.language` 直接写 `[i18n] language` 的取值（它的取值本来就是 jar 的常量名，见
  `text::Language::from_name` / `name`），因此不留手填的入口，也不存在"两边要对齐"的问题。
* 其余键保持模板原样，所以写出来的**一定是完整配置** —— 不依赖 jar 去补字段。（实测 v2.13.15 缺字段时
  它也会补齐并把文件回写，但那样"补什么"由 jar 决定，而且每次进程启动都会在机器人日志里多一行
  `Your configuration was incomplete. The missing fields were added!`。）
* **先删旧文件**：这份文件是生成物，手改的内容在下次启动后不会保留 —— 要改就改 `[fishingbot]`，要改默认值就
  改模板。

上游改了配置结构（加键、改名、删键）时怎么办：拿新 jar 生成的默认 `conf.json` 覆盖成模板，然后跑
`cargo test` —— `template_holds_every_managed_path` 会先红（托管路径在模板里找不到），照着提示同步
`managed_values` 与 `[fishingbot]` 即可。这是**唯一**需要跟着上游动的维护点。

`[fishingbot]` 是"期望的游戏侧配置"的唯一入口，磁盘上那份 `conf.json` 是"FishingBot 实际在读什么"
（`/show botconfig` 发的就是它）。另外注意：`[i18n] language` 换语言等于换 jar 的语言，`[command_echo]`
的两条回显（`pause_echo`、`unpause_echo`）是 jar 打印的，要跟着换成对应语言的那两句。

## 4. 字段规则

* 所有字段都必填：代码里没有默认值，少写一个（或拼错字段名，靠 `deny_unknown_fields` 拦下）都会在启动时报错。
  随仓库发布的示例配置已把每个键都填好，直接照着它改即可。
* 路径规则（`paths`）：`base_dir` 必须绝对路径；`jar` 与 `fishingbot_config_file_name` 必须相对路径、
  相对 `base_dir` 解析；`bots_dir` 由代码派生为 `base_dir/bots`，不写在配置里。相对路径的 `base_dir`、
  绝对路径的 `jar` 都会在启动时报错。
* `[discord]`：`token` 是唯一凭据，空了必然连不上（启动时报错）；没有别的字段，斜杠指令一律全局注册
  （Discord 端最长 1 小时才传播完）。
* `[admins]` 允许为空（那就没有管理员，各人管各人的机器人），但键必须是纯数字的 Discord 用户 ID、
  名字不能空白。这两项写错会让权限静默失效，所以直接拦下来。自己的用户 ID 用 `/whoami` 拿。
* `[i18n]`：`language` 必须是支持的语言取值 —— 也就是 FishingBot 自己 `i18n.Language` 的常量名
  （本项目实现 `ENGLISH` 与 `CHINESE_SIMPLIFIED`；上游还有的 `CHINESE_TRADITIONAL`、`FRENCH` 等会被拒，
  因为我们没写那些语言的文案）。可选值由 `text::Language` 一处定义，报错里会全部列出来。值要完全一致
  （大小写敏感，跟 jar 的枚举名一样；前后空白会被忽略）。示例配置里写的是 `ENGLISH`（给中文 jar 部署时
  改成 `CHINESE_SIMPLIFIED`），两种语言都编在二进制里，改这一行重启即生效。
* `[log] debug` 决定"是否连框架的日志一起收"。`false`（默认）时本项目自己打的日志 `INFO` 起、第三方库
  （serenity、poise）`WARN` 起，因为它们按心跳节奏打 `INFO`，会淹没启动、指令、回复这些关键行；
  `true` 时两边都放到 `DEBUG`（即全部放开；比 `DEBUG` 更细的 `TRACE` 仍然拿不到）。
* `[fishingbot]`：整张表必填（取值照抄随仓库发布的示例配置），写错的地方会在启动时报错并点名字段 ——
  因为错误取值在 jar 里只会**静默按默认值运行**（实测：未知枚举值静默回落，版本号认不出时它自行探测），
  在这里拦下更清楚。会被拦下的有：`server_ip` 空、`server_port` 是 0、`default_protocol` 既不是
  `AUTOMATIC` 也不是版本号形状（`1.20` / `1.20.4`）、`start_text` 里有空串、两个 `announce_type_*`
  不在上游的五个取值里（`ALL`、`ALL_BUT_FISH`、`ONLY_ENCHANTED`、`ONLY_BOOKS`、`NONE`）。
  在线模式、离线用户名、自动抛竿规则、领域服、GUI 项这些**不开放**：它们用模板里的值（本项目按在线模式
  使用，账号都走微软登录）。
* `[command_echo]`：`wait_ms` 是发完指令后等多久再去看日志（太短看不到回显）；`scan_lines` 是只看末尾
  多少行（越大容错率越高，回显被几行无关日志挤上去也能找到，代价只是多读几行）。两项都必须大于 0：
  等待为 0 等于发完立刻去看，回显来不及落进日志。

## 5. 校验（`validate`）

校验按表拆开：`Config` 的每个字段（即每张 TOML 表）一个 `validate_<表名>` 函数，`validate()` 里只有这些
调用，按启动时的先后顺序排下来；`[log]` 只有一个布尔开关 `debug`，没有校验函数。

语言是第一个校验项：`validate()` 先调用 `validate_language`，它只解析 `i18n.language`，失败信息固定英文、
不调用 `text::get()`；语言合法后立刻 `install()`，其余按表拆开的 `validate_<表名>` 才使用配置语言。

只覆盖"写错就无法启动"的三类问题（见 `validate.rs` 的模块文档）：

* 必填项不能为空：`ping.default_ping_host`、`discord.token`、`keywords.*`、`command_echo.pause_echo`、
  `command_echo.unpause_echo`、`text.help`、`runtime.screen_prefix`、`runtime.token_file`、
  `runtime.bot_logs_dir`（后三个还要求不含空格与斜杠、不能写成 `.` 或 `..`）、`raw_game_logs.discard_path`，
  以及 `raw_game_logs.dir_name` 与 `raw_game_logs.file_name`（这两个同样要求是纯名字）。
* 数值范围：`ping.default_ping_port` 不能是 0；`runtime.max_autocomplete_choices` 必须在 1 到 25 之间
  （Discord 对候选条数的硬上限）；`startup_queue.max_batch_size`、`startup_queue.bot_login_interval_ms`、
  `startup_queue.batch_login_interval_ms`、`startup_queue.max_pending_requests`、
  `ping.timeout_ms`、
  `runtime.max_reply_log_lines`、`runtime.max_reply_chars`、
  `timing.watchdog_window_ms`、`timing.check_grace_ms`、`command_echo.wait_ms`、`command_echo.scan_lines`
  必须大于 0（看门狗判定窗口为 0 会让判定退化，永远证明不了"还连着"；探活间隔与首次握手超时由窗口派生，
  所以没有再写一条"探活不能比窗口慢"的规则）。

* 会拼进命令或路径的字符串：`runtime.java_opts` 禁止 `'`、`"`、`` ` ``、`$`、`;`、`&`、`|`、`<`、
  `>` 与换行（未加引号内联）；`keywords.filter_regex` 不能含单引号（内联进 `grep -v -E '...'`）；
  `paths.jar` 与 `paths.fishingbot_config_file_name` 也不能含单引号。

`command_echo` 的两条回显不能相同，也不能互相包含（含完全相同）：否则日志里那条回显认不出属于哪条指令，
判定会永远偏向先检查的那句。

另外两条属于"环境就绪"检查，同样在启动时报错：`paths.base_dir` 必须是目录、`paths.jar` 必须是文件。
写错了会一直等到拉起会话才失败，报错也远不如这里清楚。

其余一律不校验：时间参数设多大、地址写哪个、名字里能否用中文，都属于使用者的判断。`[discord]`
只有 token 一条规则（不能为空）；`[log]` 的 `debug` 是布尔开关，没有规则；`[timing]` 有两条
（`check_grace_ms` 大于 0、网关窗口大于 0）；`[command_echo]` 有两条（两条回显
非空且不能互相包含、两项时间都大于 0）。

## 6. 模板（`template`）

* 模板就是仓库根目录的 `minecraft-fishingbot-manager-for-discord.conf.example.toml`，用 `include_str!` 内嵌
  进二进制，因此发布时不需要额外带文件。
* 生成的模板与示例文件逐字一致。单元测试会核对示例里出现的每张表与每个字段（缺少字段会让照抄模板的
  用户直接启动失败），并且核对每个键都写进了本文档。

## 7. 为什么是分表结构（而不是一个扁平结构体）

代码里是一个 `Config` 加每个 TOML 表一个结构体（`Config::ping`、`runtime`、`timing` 等）。这不是为了
多套一层，而是为了让三个地方的名字完全一样：

* 配置文件里的键：`[runtime]` 表下的 `bot_logs_dir`；
* 代码里读它的写法：`config::Config::get().runtime.bot_logs_dir`；
* 启动校验的报错：`配置项 runtime.bot_logs_dir 不能为空`。

把结构体拍平成"只有具体字段的大结构体"之前，要先决定 TOML 的表怎么处理；三条路都不划算：

* 连文件一起拍平（`runtime_logs_dir = "logs"`）：文件与结构体仍一一对应，但所有现有部署的配置都要
  重写（本项目开着 `deny_unknown_fields`，旧写法会直接启动失败），而且 `[admins]`（用户 ID 到名字）
  本来就是映射，拍不成"每项具体的配置"。字段还会出现重名（`log.debug` 与 `runtime.debug` 这类），
  只能加前缀区分，文件键名与字段名从此不再同名，校验报错也没法照抄文件里的名字。
* 文件保持分表、结构体拍平：serde 反方向做不到。`#[serde(flatten)]` 是"把结构体摊进文件"，要让
  `[runtime]` 摊进一个扁平结构就只能自己写一层"按路径取值"的映射，代码只多不少，字段级报错也会退化
  成手写字符串、容易和文件漂开。
* 用宏生成"扁平视图"：能同时保住文件和结构体，但逐字段的文档注释会变成宏参数，`cargo doc` 里读不到
  现在这种说明，对这个"配置即文档"的项目不划算。

结论是保持分表：`config::Config::get()` 之后那一层点号是可接受的成本，换来"文件键名 = 结构体路径 = 校验报错"
三者同名。"启动时读一次、之后直接取字段"也成立：`Config::get_or_init()` 在最早期把配置写进 `OnceLock`，
之后每次 `config::Config::get()` 都是内存取值，没有 IO。

## 8. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 游戏侧 `conf.json` 由谁生成、字段怎么取：[utils-zh_cn.md](../utils/utils-zh_cn.md) 的 `fishingbot_config` 一节。
* 配置项怎么被读进指令：[commands-zh_cn.md](../commands/commands-zh_cn.md)。
* 配置相关的不变量（启动校验、模板随仓库走）：[CONTEXT-zh_cn.md](../../CONTEXTS/CONTEXT-zh_cn.md) 第 2 节。
