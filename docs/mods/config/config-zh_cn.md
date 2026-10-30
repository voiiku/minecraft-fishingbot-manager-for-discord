# 配置文件说明
[English](config-en_us.md) | **简体中文**

* 代码：`src/config.rs`（运行时 `Config` 与取值接口）与 `src/config/{schema,parse,validate,template}.rs`
  （各配置表与 `[paths]` 的文件形态、解析、校验、模板）
* 文件：可执行文件同级目录下的 `minecraft-fishingbot-manager-for-discord.conf.toml`（文件名由
  `CONFIG_FILE_NAME` 固定，不可改名）
* 不依赖工作目录，也不读取环境变量：部署时将二进制与配置放在同一目录即可，systemd 等托管方式无需额外设置。
* 文件中包含 Bot Token，建议设置 `chmod 600`。

## 1. 加载流程（启动最早期）

1. `config::Config::get_or_init()` 定位配置文件路径。
2. 文件不存在时，按仓库中的 `minecraft-fishingbot-manager-for-discord.conf.example.toml` 生成一份：模板写入与提示打印都在这一步完成，随后以退出码 0 结束进程，不连接 Discord。文件已存在时，先在终端输出找到的配置文件路径，再继续（此时日志系统尚未安装，提示只出现在终端；程序日志中的“已加载配置: …”由 `ProcessLog::get_or_init` 记录）。
3. 文件存在时读取文本。`Config::from_toml_text` 先将 TOML 解析为值树，递归去除所有字符串值首尾的空白，再通过 Serde 的 `Deserialize` 实现映射为运行时 `Config`（这是全链路唯一一次清洗）。`[paths]` 的三个键由 `Paths` 自己的反序列化实现展开为以 `base_dir` 为根的路径。这一步只负责格式解析与结构转换，不处理取值规则。
4. `validate` 首先校验 `i18n.language`：失败信息固定为英文，不调用 `text::get()`；语言合法后立即安装，再检查其余字段的取值、范围与运行环境（如必填项、端口范围、目录与 JAR 是否存在）。
5. 配置读取、解析或校验失败时，错误以“配置项 + 字段名 + 要求”的形式报告并退出，不会带着错误取值运行。
6. 通过后写入 `OnceLock`，只在启动时读取一次并常驻内存。运行中修改配置文件不会影响已运行的进程；新配置需重启进程才会生效。

字符串值首尾的空白统一由 `Config::from_toml_text` 去除：配置文件中的多余空格、缩进与换行不会进入代码，因此配置读取方（`main.rs`、业务层）直接使用原值，不再各自调用 `trim()`。TOML 转义在去除空白前解码，因此 `"\u0020"` 也不能用于保留首尾空格。

反序列化依赖 `Config` 及其字段类型上的 `#[derive(Deserialize)]`：Serde 按 TOML 表名与键名填充字段，类型不符、缺少必需字段或出现未知字段时返回解析错误。代码中**没有默认值**：每张表与每个字段都必须显式写出，缺项或拼写错误都会在启动时报错，不会静默使用默认值。路径字段由 `Paths` 自己的 `Deserialize` 实现转换为以 `base_dir` 为根的运行时路径（`bots_dir` 由代码派生），其余取值规则由后续的 `validate` 负责。

配置中的时间统一使用秒，键名不带单位后缀。原毫秒字段接受整数或小数秒，必须是 `0.001` 秒的整数倍；例如 `300` 毫秒写成 `0.3`，内部仍按毫秒计算。反 AFK 的 `execution_interval` 为整数秒，`input_delay` 保持原有浮点秒精度。升级时须删除旧键并换算数值：`[command_echo] wait_ms` → `wait`、`[ping] timeout_ms` → `timeout`；`[startup_queue]` 的 `bot_login_interval_ms`、`batch_login_interval_ms`，以及 `[timing]` 的 `check_grace_ms`、`stop_grace_ms`、`stop_settle_ms`、`watchdog_window_ms`，都去掉 `_ms` 并将数值除以 1000。`[anti_afk]` 的 `execution_interval_seconds` 与 `input_delay_seconds` 分别改为 `execution_interval`、`input_delay`，数值不变。旧键不再支持。

配置装载完成后，进程入口会在启动流程中按 `[fishingbot]` **生成游戏侧配置**（FishingBot 的 `conf.json`，路径来自 `paths.fishingbot_config_file_name`）：先删除旧文件，再写入新文件。该步骤失败按配置错误处理（记录 ERROR 并以退出码 1 结束）。生成原因与内容见 3.2 节。

## 2. 对外接口

除 `config_file_path` 字段外，其余都是 `Config` 的关联函数（模块中没有独立的取值函数；运行时 `Config` 定义在本文件），进程入口因此只需一行 `let settings = config::Config::get_or_init();`：

* `Config::get_or_init()`：获取配置，也是进程入口访问配置的唯一入口。首次调用完成定位、读取、解析与校验，之后返回已装载的 `&'static Config`。文件不存在并生成模板时，函数打印提示后以退出码 0 结束；配置不合法时，报错并以退出码 1 结束。两条失败路径都在函数内结束进程，因此返回值一定是可用配置，调用方无需判断结果。
* `Config::get()`：获取已加载的 `Config`。
* `Config::admins()`：获取管理员名单（`Config::get().admins` 的快捷方式）。
* `Config::install_dir()`：获取部署目录（可执行文件所在目录）；无法取得路径时返回错误文本。配置与程序日志都位于此处。
* `config_file_path` 字段：本次启动使用的配置文件路径（装载时按可执行文件位置计算并写入结构体）。

`Config::get()` 在尚未装载时 panic，文案取自 `text::bootstrap::CONFIG_NOT_LOADED`（内部不变量消息；此时语言尚未安装，固定为英文）。进程入口的第一件事就是 `Config::get_or_init()`，此处仍 panic 说明调用顺序错误。配置文件路径是 `Config` 的字段（装载时计算一次），是否已装载仍只取决于那一个 `OnceLock<Config>`。

## 3. 表结构

顺序与示例配置一致：新部署通常需要修改的表排在前面（凭据、路径、游戏侧配置 `[fishingbot]`），随后是需按游戏服务器或 JAR 调整的三张表（关键词、回显、语言），然后是运行参数、启动队列与时序，最后是原始日志、`/ping` 默认值、日志开关与展示文本。示例配置的说明注释以英文为主；`[text].help` 上方另有完整中文原文注释，供中文用户参考。少数以 `# CHINESE_SIMPLIFIED:` 开头的行提示中文部署应填的值（只有 `[command_echo]` 的两条回显与 `[i18n] language` 需要），见 [i18n.md 第 1 节](../i18n/i18n-zh_cn.md)。

* `[discord]`：`token`。Bot Token，唯一凭据，必填。
* `[paths]`：`base_dir`（绝对路径）、`jar`、`fishingbot_config_file_name`（相对路径）。数据根目录，以及相对根目录解析的 JAR 与游戏侧配置文件名。
* `[fishingbot]`：`server_ip`、`server_port`、`default_protocol`、`auto_reconnect`、`auto_reconnect_time`、`start_text`、`announce_type_console`、`announce_type_chat`。游戏侧配置（FishingBot 的 `conf.json`）中**由本项目开放**的键；启动时据此生成该文件（见 3.2 节）。整张表必填，取值可直接沿用随仓库发布的示例配置。
* `[admins]`：`"Discord用户ID" = "名字"`。管理员名单；键必须是纯数字的 Discord 用户 ID（snowflake）。
* `[keywords]`：`online`、`filter_regex`。只有随部署变化、需要手工填写的日志关键词：游戏服务器的进服提示，以及写入日志前过滤刷屏行所用的正则（`grep -v -E`）。这里**没有**“进程已结束”关键词：该标记由旧版外壳脚本写入日志（`>>> 机器人进程已结束或被手动中止 <<<`），本项目不写该标记，进程是否结束改为读取 screen 会话状态（`(Dead ...)` 残壳），因此不需要配置。只保留两项的原因见 3.1 节。
* `[command_echo]`：`pause_echo`、`unpause_echo`、`wait`、`scan_lines`。用于确认 `/pause` 与 `/unpause` 是否真正执行：前两项是 JAR 收到指令后打印的回显（上游消息为 `command-pause-executed` 与 `command-unpause-executed`，按所用 JAR 的语言填写；中文 JAR 为 `已暂停钓鱼` / `已恢复钓鱼`）；发送后等待 `wait` 秒，再检查最新日志末尾（仅最后 `scan_lines` 行）是否出现这两句。
* `[i18n]`：`language`。程序运行语言，影响 Discord 文案、程序日志与报错。取值是 FishingBot 自身 `i18n.Language` 的常量名，同时写入 `conf.json` 的 `misc.language`（同一个值，不做映射），因此无需手工对齐两处。本项目实现 `ENGLISH` 与 `CHINESE_SIMPLIFIED`；修改配置并重启即可生效，无需重新编译。见 3.1 节。
* `[runtime]`：`java_opts`、`screen_prefix`、`token_file`、`bot_logs_dir`、`max_reply_log_lines`、`max_reply_chars`、`max_autocomplete_choices`。单个机器人的运行时参数。`bot_logs_dir` 只保存本项目日志（FishingBot 自身的原始日志位于同层的 `[raw_game_logs] dir_name` 下，文件名为 `[raw_game_logs] file_name`，并指向 `discard_path`，见 [utils-zh_cn.md 第 7 节](../utils/utils-zh_cn.md#7-两类日志)）；`max_reply_chars` 限制单条回复长度（Discord 上限为 2000 字符，超长会直接发送失败）；`max_autocomplete_choices` 限制机器人名选项的自动候选数量（范围 1–25，Discord 硬上限）。
* `[anti_afk]`：`command`、`execution_interval`、`input_delay`。反 AFK 的输入内容、每轮输入完成后的等待秒数，以及每个字符或按键后的延迟秒数。使用 [input-pulse](https://github.com/voiiku/input-pulse) 的输入动作语法，示例为 `!/!!/!!ENTER!`；目标是会话名**包含** `runtime.screen_prefix` 的所有 GNU screen 会话，不仅限于项目创建的会话。配置务必避免匹配无关会话。三项均必填：内容须非空且符合 screen 模式语法；执行间隔须大于 0；输入延迟必须是非负、有限且可表示的秒数（可以为 0）。启动时校验配置，管理员可通过 `/anti-afk` 开关功能，程序重启后不会自动启动。升级旧配置时须补充此表。
* `[startup_queue]`：`max_batch_size`、`bot_login_interval`、`batch_login_interval`、`max_pending_requests`。四项依次为全局 FIFO 启动队列的每批最多启动数量、批内相邻两台的登录间隔（秒）、批与批之间的间隔（秒）、等待调度的请求上限（不含正在执行的请求）。整张表与每个字段必填，且每个值必须大于 0。`bot_login_interval` 控制批内错峰：同一批中相邻两台按它隔开。`batch_login_interval` 控制批间：从“上一次取出一批”的时刻算起，至少间隔该值才取下一批（复核在此期间并行进行，不计入批间等待）；即使上一批只有一台也照此执行，因此长时间空闲后的第一条命令无需等待。微软按出口 IP 限制登录频率，取值依据见[故障提示第 12 节](../../troubleshooting/troubleshooting-zh_cn.md)。`/start`、`/startall`、`/restart` 与 `/restartall` 共用该队列；`/bot login` 不使用。队列最多暂存 `max_pending_requests` 台，超出部分立即退回调用方（回复中会说明）；程序重启会丢弃尚未执行的队列任务。启动结果超时无需配置：它由这张表与 `timing.check_grace` 计算（`StartupQueue::result_timeout`；后者只计一次）：`2 × (⌈max_pending_requests / max_batch_size⌉ × max(batch_login_interval, (max_batch_size - 1) × bot_login_interval) + check_grace)`。超时后，尚未返回结果的机器人按“未确认”写入回复（同时记录一条程序日志）；批内错峰越慢或队列容量越大，该超时自动延长，无需人工调整。
* `[timing]`：`check_grace`、`stop_grace`、`stop_settle`、`watchdog_window`。各流程等待时间，单位为秒。`check_grace` 是每次启动请求的全部机器人拉起后的会话存活宽限期（复核分两次：拉起后立即判定一次，等满该值再判定一次；每次启动请求只等待一次，不按台数累加）。`stop_grace` 是发送停止信号后等待 JVM 优雅退出的时间，超时后强杀；`stop_settle` 是强杀后、再次核对会话状态前的稳定等待。`stop_grace` 与 `stop_settle` 只被读取，没有数值校验。`watchdog_window` 是网关看门狗的判定窗口，掉线后也用作重建连接前的等待；REST 探活间隔（窗口的 1/3）与首次握手超时（窗口的 2 倍）由它派生，不再是独立配置项，见 [watchdog.md](../watchdog/watchdog-zh_cn.md)。
* `[raw_game_logs]`：`dir_name`、`file_name`、`discard_path`。FishingBot.jar 自身原始日志的约定（目录名、文件名、内容丢弃位置）。随 JAR 变化，整张表必填；示例配置中为 `raw`、`log0.log`、`/dev/null`。
* `[ping]`：`default_ping_host`、`default_ping_port`、`timeout`。`/ping` 不带参数时的默认目标与超时。显式指定的地址必须全部解析为公网地址；代理 Fake-IP 导致误判时见[故障排查](../../troubleshooting/troubleshooting-zh_cn.md#ping-拒绝探测本地或私网地址)。
* `[log]`：`debug`。程序日志的级别开关。为 `false` 时，本项目日志从 `INFO` 开始，第三方库（serenity、poise）从 `WARN` 开始；为 `true` 时两者都降到 `DEBUG`（全部开放；比 `DEBUG` 更细的 `TRACE` 仍不会记录）。这是唯一的级别开关（不再读取环境变量）；细节见 [process-log.md](../process-log/process-log-zh_cn.md)。
* `[text]`：`help`。`/help` 的完整输出（多行字符串，修改文案无需重新编译）。

### 3.1 关键词放哪里：`[keywords]`、`[command_echo]` 与 `[i18n]`

* 随部署变化、只能由部署者提供的字符串放入 `[keywords]`：游戏服务器自身的进服提示（`online`）与本服务器需要过滤的刷屏噪声（`filter_regex`）。
* `/pause` 与 `/unpause` 的回显放入 `[command_echo]`（`pause_echo`、`unpause_echo`）：回显同样由 JAR 打印，但只存在于包含这两条命令的构建中（上游原版没有）；放入配置后即可适配其他构建，无需重新编译。
* 断线重连提示的前后缀保留在程序内（`src/text/*.rs` 的 `jar_*` 方法），跟随 `[i18n] language`：它只随 JAR 语言变化，各部署一致，写入配置只会增加一个出错点。该语言也是生成 `conf.json` 时写入 `misc.language` 的值（3.2 节），因此 JAR 一定使用同一种语言。过去需要手工对齐，现在不会出现不一致。
* “进程已结束”不需要关键词：旧版外壳脚本使用 `java … | tee bot.log; echo 标记 >> bot.log` 写入结束标记，本项目自行拉起会话，不写该标记；进程是否结束改为读取 screen 会话状态（`(Dead ...)` 残壳，见 [utils.md](../utils/utils-zh_cn.md)）。这比复制标记更可靠，不受 JAR 版本、脚本写法与语言影响。

判断标准为：“这条字符串由谁决定，更换构建时是否需要随之修改”。由部署环境决定的写入 `[keywords]`，由 JAR 构建决定的写入 `[command_echo]`，由 JAR 语言决定的跟随 `[i18n]`；不属于字符串的判断（进程是否结束）直接读取会话状态。

### 3.2 游戏侧配置（`conf.json`）是生成物

FishingBot 只读取 JSON（`-config <path>`），**没有逐项覆盖配置的命令行参数**（启动参数仅有 `-nogui`、`-help`、`-logsdir`、`-config`、`-refreshToken`、`-onlyCreateConfig`）。因此，把 `conf.json` 合并进 TOML 只能采用**单向生成**：TOML 是唯一数据源，进程启动时写出 JSON（代码位于 `utils/fishingbot_config.rs`）。

生成方式：**以随仓库发布的模板为基础，只覆盖 `[fishingbot]` 中的对应键**。

* 模板是仓库根目录的 `fishingbot-config.example.json`，一份**完整**的 FishingBot 配置（取自上游生成的默认文件，并按本项目需要调整默认值：关闭控制台与聊天播报、将 `/home fishing` 作为默认抛竿命令、延长 `auto-reconnect-time` 等）。它通过 `include_str!` 编入二进制，部署时无需额外携带。
* 覆盖的键是 `utils/fishingbot_config.rs` 中的托管清单（十条点分路径），与 `[fishingbot]` 字段一一对应；其中 `misc.language` 直接写入 `[i18n] language` 的取值（该取值就是 JAR 的常量名，见 `text::Language::from_name` / `name`），不提供手工填写入口，也不存在两边对齐的问题。
* 其余键保持模板原样，因此输出**一定是完整配置**，不依赖 JAR 补充字段。（实测 v2.13.15 缺字段时它会补齐并回写文件，但补什么由 JAR 决定，而且每次进程启动都会在机器人日志中多出一行 `Your configuration was incomplete. The missing fields were added!`。）
* **先删除旧文件**：该文件是生成物，手工修改不会保留到下次启动；要修改请改 `[fishingbot]`，要修改默认值请改模板。

上游修改配置结构（新增、改名或删除键）时的处理：用新 JAR 生成的默认 `conf.json` 覆盖模板，然后运行 `cargo test`。`template_holds_every_managed_path` 会首先失败（模板中找不到托管路径），按提示同步 `managed_values` 与 `[fishingbot]` 即可。这是**唯一**需要跟随上游维护的地方。

`[fishingbot]` 是“期望的游戏侧配置”的唯一入口，磁盘上的 `conf.json` 是 FishingBot 实际读取的内容（`/show botconfig` 发送的就是它）。另需注意：更换 `[i18n] language` 等同于更换 JAR 的语言；`[command_echo]` 的两条回显（`pause_echo`、`unpause_echo`）由 JAR 打印，需要同步改为对应语言。

## 4. 字段规则

* 所有字段必填：代码中没有默认值；少写一个或拼错字段名（由 `deny_unknown_fields` 拦截）都会在启动时报错。示例配置已填好每个键，可直接作为起点。
* 路径规则（`paths`）：`base_dir` 必须是绝对路径；`jar` 与 `fishingbot_config_file_name` 必须是相对路径，并相对 `base_dir` 解析；`bots_dir` 由代码派生为 `base_dir/bots`，不写入配置。`base_dir` 为相对路径或 `jar` 为绝对路径都会在启动时报错。
* `[discord]`：`token` 是唯一凭据，为空时必然无法连接（启动时报错）；没有其他字段，斜杠指令一律全局注册（Discord 端最长需要 1 小时传播）。
* `[admins]` 允许为空（此时没有管理员，各人管理自己的机器人），但键必须是纯数字的 Discord 用户 ID，名字不能为空白。任一项写错都会使权限静默失效，因此在启动时拦截。可使用 `/whoami` 获取自己的用户 ID。
* `[i18n]`：`language` 必须是受支持的语言取值，即 FishingBot 自身 `i18n.Language` 的常量名（本项目实现 `ENGLISH` 与 `CHINESE_SIMPLIFIED`；上游的 `CHINESE_TRADITIONAL`、`FRENCH` 等会被拒绝，因为本项目未提供这些语言的文案）。可选值由 `text::Language` 统一定义，报错时会全部列出。取值必须完全一致（大小写敏感，与 JAR 枚举名一致；首尾空白会被忽略）。示例配置使用 `ENGLISH`（中文 JAR 部署时改为 `CHINESE_SIMPLIFIED`）；两种语言都编入二进制，修改后重启即可生效。
* `[log] debug` 决定是否同时收集框架日志。为 `false` 时，本项目日志从 `INFO` 开始，第三方库（serenity、poise）从 `WARN` 开始，因为它们按心跳持续输出 `INFO`，会淹没启动、指令、回复等关键行；为 `true` 时两者都降到 `DEBUG`（全部开放；`TRACE` 不会记录）。
* `[fishingbot]`：整张表必填（示例配置可直接作为起点），错误取值会在启动时报错并指出字段，因为错误取值在 JAR 中只会**静默按默认值运行**（实测：未知枚举值静默回落，版本号无法识别时由 JAR 自行探测），在启动时拦截更清晰。会被拦截的取值包括：`server_ip` 为空、`server_port` 为 0、`default_protocol` 既不是 `AUTOMATIC` 也不是版本号形式（`1.20` / `1.20.4`）、`start_text` 中含空字符串、两个 `announce_type_*` 不在上游的五个取值中（`ALL`、`ALL_BUT_FISH`、`ONLY_ENCHANTED`、`ONLY_BOOKS`、`NONE`）。在线模式、离线用户名、自动抛竿规则、领域服与 GUI 项**不开放**：它们使用模板中的值（本项目按在线模式运行，所有账号均通过微软登录）。
* `[command_echo]`：`wait` 是发送指令后等待多久再检查日志（过短会看不到回显）；`scan_lines` 是仅检查末尾多少行（数值越大容错越高，即使回显被少量无关日志挤出末尾范围也能找到，代价是多读几行）。两项都必须大于 0：等待为 0 等于发送后立即检查，回显来不及写入日志。

## 5. 校验（`validate`）

校验按表拆分：每张 TOML 表各有一个 `validate_<表名>` 函数，`validate()` 只包含这些调用，按启动顺序排列；`[log]` 只有布尔开关 `debug`，没有校验函数。

语言是第一个校验项：`validate()` 首先调用 `validate_i18n`。它只解析 `i18n.language`，失败信息固定为英文，不调用 `text::get()`；语言合法后立即调用 `install()`，随后按表拆分的 `validate_<表名>` 才使用配置语言。

覆盖以下三类基础问题（见 `validate.rs` 的模块文档）：

* 必填项不得为空：`ping.default_ping_host`、`discord.token`、`keywords.*`、`command_echo.pause_echo`、`command_echo.unpause_echo`、`text.help`、`runtime.screen_prefix`、`runtime.token_file`、`runtime.bot_logs_dir`（后三个还要求不含空格与斜杠，且不能为 `.` 或 `..`）、`raw_game_logs.discard_path`，以及 `raw_game_logs.dir_name` 与 `raw_game_logs.file_name`（这两个同样要求是纯名称）。
* 数值范围：`ping.default_ping_port` 不得为 0；`runtime.max_autocomplete_choices` 必须在 1 到 25 之间（Discord 对候选数量的硬上限）；`startup_queue.max_batch_size`、`startup_queue.bot_login_interval`、`startup_queue.batch_login_interval`、`startup_queue.max_pending_requests`、`ping.timeout`、`runtime.max_reply_log_lines`、`runtime.max_reply_chars`、`timing.watchdog_window`、`timing.check_grace`、`command_echo.wait`、`command_echo.scan_lines` 必须大于 0（看门狗判定窗口为 0 会使判定退化，无法证明仍然连接；探活间隔与首次握手超时由该窗口派生，因此不再单独设置“探活不能比窗口慢”的规则）。
* 会拼入命令或路径的字符串：`runtime.java_opts` 禁止 `'`、`"`、`` ` ``、`$`、`;`、`&`、`|`、`<`、`>` 与换行（未加引号内联）；`keywords.filter_regex` 不得包含单引号（会内联进 `grep -v -E '...'`）；`paths.jar` 与 `paths.fishingbot_config_file_name` 同样不得包含单引号。

此外，`command_echo` 的两条回显不能相同，也不能互相包含（包括完全相同）：否则无法判断日志中的回显属于哪条命令，判定会始终偏向先检查的那一句。

`[anti_afk]` 的字段取值与 screen 输入动作语法也在启动时校验；这不检查 screen 是否安装或目标会话是否存在。

另外两条属于“环境就绪”检查，同样在启动时报错：`paths.base_dir` 必须是目录，`paths.jar` 必须是文件。若写错，要到拉起会话时才会失败，报错也不如这里明确。

其余内容不校验：时间参数取值、地址选择、名字能否包含中文，均由使用者判断。`[discord]` 只有 token 一条规则（不得为空）；`[log]` 的 `debug` 是布尔开关，没有规则；`[timing]` 有两条（`check_grace` 大于 0、网关窗口大于 0）；`[command_echo]` 有两条（两条回显非空且不能互相包含，两项时间值都大于 0）。

## 6. 模板（`template`）

* 模板是仓库根目录的 `minecraft-fishingbot-manager-for-discord.conf.example.toml`，通过 `include_str!` 编入二进制，因此发布时无需额外携带文件。
* 生成的模板与示例文件逐字一致。单元测试会检查示例中的每张表与每个字段（缺少字段会使直接复制模板的用户启动失败），并检查每个键都已写入本文档。

## 7. 为什么采用分表结构（而不是一个扁平结构体）

代码中有一个 `Config`，每张 TOML 表对应一个结构体（`Config::ping`、`runtime`、`timing` 等）。这样做的目的不是增加一层抽象，而是让三个位置使用同一个名字：

* 配置文件中的键：`[runtime]` 表下的 `bot_logs_dir`；
* 代码中的读取方式：`config::Config::get().runtime.bot_logs_dir`；
* 启动校验的报错：`配置项 runtime.bot_logs_dir 不能为空`。

在将结构体扁平化为“只含具体字段的大结构体”之前，需要先决定 TOML 表的处理方式；三种方案都不理想：

* 同时扁平化文件（`runtime_logs_dir = "logs"`）：文件仍与结构体一一对应，但所有现有部署都必须重写配置（本项目启用 `deny_unknown_fields`，旧写法会直接启动失败），而且 `[admins]`（用户 ID 到名字）本身就是映射，无法扁平化为“每项一个具体配置”。字段名还会冲突（例如 `log.debug` 与 `runtime.debug`），需要加前缀区分，此后文件键名与字段名不再一致，校验报错也无法直接引用文件中的名字。
* 保留文件分表、扁平化结构体：serde 无法反向完成。`#[serde(flatten)]` 的含义是“把结构体展开到文件中”；要将 `[runtime]` 展开到一个扁平结构体，只能自行实现按路径取值的映射层，代码量只会增加，字段级报错也会退化为手写字符串，容易与文件不一致。
* 用宏生成“扁平视图”：可以同时保留文件与结构体，但逐字段的文档注释会变成宏参数，`cargo doc` 中不再显示当前说明；对“配置即文档”的项目而言并不合适。

结论是保持分表：`config::Config::get()` 之后多出的一层点号是可接受的成本，换来的是“文件键名 = 结构体路径 = 校验报错”三者同名。“启动时读取一次，之后直接取字段”同样成立：`Config::get_or_init()` 在启动最早期将配置写入 `OnceLock`，之后每次 `config::Config::get()` 都是内存读取，无 I/O。

## 8. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 游戏侧 `conf.json` 由谁生成、字段怎么取：[utils-zh_cn.md](../utils/utils-zh_cn.md) 的 `fishingbot_config` 一节。
* 配置项怎么被读进指令：[commands-zh_cn.md](../commands/commands-zh_cn.md)。
* 配置相关的不变量（启动校验、模板随仓库走）：[CONTEXT-zh_cn.md](../../CONTEXTS/CONTEXT-zh_cn.md) 第 2 节。
