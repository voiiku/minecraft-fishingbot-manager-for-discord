# 架构与模块地图
[English](architecture-en_us.md) | **简体中文**

本文说明代码分几层、每层负责什么，以及一条 Discord 斜杠指令如何落到 `screen` 会话。按主题查找文档，见
[docs 文档索引](../../index-zh_cn.md)。

## 1. 分层

```
Discord 斜杠指令（Gateway 推 INTERACTION_CREATE）
   │
   ▼
commands（命令层，poise）          取 option + 校验数字 + defer + 分发，不查权限、不清洗
   │  handle_*
   ▼
lib.rs（指令入口层）               枚举动作 + 权限校验（has_permission）+ Result → 回复文本
   │
   ▼
业务模块                            每条指令一个文件
bot_control / bot_manage / all_bots_control / show / general
   │
   ▼
utils（原语层）                     目录、会话、日志、报告等可复用能力
   │
   ▼
screen 会话里的 JVM（FishingBot）
```

先明确各层的叫法。`src/commands/` 是命令层：用 poise 的 `#[poise::command]`，只负责取值、校验参数个数、
把参数值转成数字、`defer`、把 `Err` 变成回复。`lib.rs` 的 `handle_*` 是指令入口层（下文简称路由层）：
负责把动作枚举落到业务函数，全员指令与批量指令的权限也只在这一层查。业务层指 `bot_control`、`bot_manage`、
`all_bots_control`、`show` 与 `general`；其下是 utils 原语层。

参数怎么往下传：Discord 交给命令层的是结构化 option，但 option 的值仍是用户输入的原始文本。命令层只负责取值、
校验数字并透传（参数名以 `raw_` 起），真正使用该值的层再清洗一次：机器人名走
`ToCleanBotName::to_clean_bot_name`（`BotDirEntry` 构造条目时调用），游戏内指令片段走
`utils::ToCleanBotCommand`。数字转换（`/log` 的行数、`/ping` 的端口）是命令层唯一的转换；校验失败时当场回复
用法提示。

另外两条与指令无关的链路：

* `main.rs`：进程生命周期。读取配置、初始化程序日志、准备 REST 探活句柄、装配 poise `Framework` 与
  serenity `Client`、注册指令、启动连接循环。正常运行时只有收到 SIGTERM 或 Ctrl+C 才以退出码 0 退出；配置错误等
  永久失败以退出码 1 退出。掉线与 `client.start()` 返回都不会结束进程，而是停止全部机器人、等待一个判定窗口后
  重建连接。
* `watchdog`：网关看门狗。判定本进程是否已经跟 Discord 失去联系（判定是纯计时，重建连接的请求由进程入口发出）。
  它是唯一判定掉线的地方，规则见
  [CONTEXT 第 3 节](../../CONTEXTS/CONTEXT-zh_cn.md#3-网关看门狗唯一判定掉线的位置)。

## 2. 目录结构

```
src/
├── main.rs                     进程入口：配置 → 日志 → 装配 poise（探活任务由看门狗拉起）→ 运行 → 掉线重连 / 信号退出
├── lib.rs                      指令入口层：动作枚举 + handle_*（+ 给 bin 的再导出）
├── commands.rs                 命令层：poise 指令集合（子模块见下）
│   ├── autocomplete.rs             `name` 选项的候选查询（autocomplete）
│   ├── bot_control.rs              /start /stop /restart /pause /unpause /command
│   ├── bot_manage.rs               /bot add|del|rename|login（父指令 + 四个子指令）
│   ├── token.rs                    /token（ephemeral）
│   ├── log.rs                      /log（ephemeral + 附件）
│   ├── all_bots.rs                 /startall /stopall /restartall /commandall
│   ├── show.rs                     /show status|botconfig|mybots|allbots|admins
│   └── general.rs                  /help /ping /checkserver /whoami
├── text.rs                     文案入口：一个进程一种语言（`[i18n] language`）+ 指令元数据改写
├── text/                       en_us（英语）/ zh_cn（两份一一对应，只换字面量）
│                               + bootstrap（引导期英文）/ log_text / requirement（两个数据型枚举）
│                               + setup_failure（装配失败归到哪条文案、算不算永久错）
├── watchdog.rs                 网关看门狗（子模块见下）
│   ├── gateway.rs              握手与探活判定 + REST 探活任务（纯计时；每轮尝试前重置计时）
│   └── lifecycle.rs            掉线记账、重连信号、等一次尝试结束、停机退出与永久失败退出
├── process_log.rs              程序日志：终端（stderr）+ <部署目录>/logs/时间戳.log（子模块见下）
│   ├── levels.rs               级别判定（本项目与第三方两档上限）
│   └── format.rs               记录渲染（时间戳、级别前缀、续行缩进）
├── config.rs                   外置配置的加载、校验与访问（子模块见下）
│   ├── schema.rs               配置结构体（与 TOML 一一对应）
│   ├── parse.rs                TOML 反序列化 + 路径展开
│   ├── validate.rs             启动校验
│   └── template.rs             首次启动生成配置模板
├── bot_control.rs              单机器人控制指令
│   ├── start.rs  stop.rs  restart.rs
│   └── pause.rs  unpause.rs  command.rs  log.rs
├── bot_manage.rs               账号管理指令
│   └── add.rs  del.rs  rename.rs  token.rs  login.rs
├── all_bots_control.rs         全服批量指令（仅管理员）
│   └── startall.rs  stopall.rs  restartall.rs  commandall.rs
├── show.rs                     只读查询指令
│   └── status.rs  botconfig.rs  mybots.rs  allbots.rs  admins.rs
├── general.rs                  通用指令
│   └── help.rs  ping.rs  checkserver.rs  whoami.rs
├── test_support.rs             只给测试用的公共助手（`#[cfg(test)]` 才编译）
└── utils.rs                    原语层
    ├── bot_command.rs          游戏内指令的拼接口径（`/command` 与 `/commandall` 共用，只写一处）
    ├── bot_dirs.rs             机器人目录：路径、查找、遍历、创建（条目 `BotDirEntry` 内含 Token 状态）
    ├── bot_name.rs             机器人名规范化
    ├── bot_logs.rs              机器人日志：创建 / 挑选（raw 日志目录的准备也在本文件，由启动流程调用）
    ├── fishingbot_config.rs          游戏侧配置（FishingBot 的 conf.json）生成：先删旧文件，再用仓库模板
    │                           （fishingbot-config.example.json）当底覆盖 [fishingbot] 那几个键
    ├── fs.rs                   文件/目录操作原语：尾部读行、列子目录名、删目录（不存在也算成功）
    ├── bot_status.rs           机器人状态：暂停记忆 + 确认回显，在线实时读日志
    ├── running_bots.rs         运行中机器人的状态汇总（`/show status` 的数据行；表头在 `show/status.rs`）
    ├── screen.rs               会话检测与指令注入
    ├── shutdown.rs             停止会话（`/stop`、`/stopall`、重启的停止阶段、进程收尾）
    ├── startup.rs              启动（四步显式流程）
    ├── restart.rs              重启：先停后起（停止 + 启动的组合编排）
    ├── uptime.rs               运行时长：从 /proc 读进程已运行秒数 + 格式化成天/时/分
    ├── reply.rs                回复的组装与渲染（Report + 长度收口）
    └── permission.rs           管理员判断
```

业务层与命令层是两套文件：`commands/bot_control.rs`（命令层）只声明有哪些指令、参数长什么样、怎么
`defer`、把结果怎么发出去，真正的动作在 `bot_control/start.rs`（业务层）。两边的文件名刻意同名，便于
对着看。

## 3. 一条指令的完整路径

以 `/start 机器人名:duck` 为例（调用者 `caller_id = 1234…`，在服务器频道里输入）：

1. Discord 通过 Gateway 推 `INTERACTION_CREATE`，serenity 解成 `FullEvent::InteractionCreate`，
   poise 按指令名找到 `commands::bot_control::start`，把 option 解析成函数参数（命令层的形参叫
   `name`；进到路由层后同一个值叫 `raw_bot_name`）。
2. 命令层（`commands/bot_control.rs`）：
   * `ctx.defer()` 先把 3 秒红线挡掉（`stop_bot_sessions` 里的等待远超 3 秒）；
   * `ctx.author().id.get()` 取 `caller_id`；
   * 先 `ctx.say("已接受 /start duck 请求；涉及启动的操作将由全局启动队列执行，完成后会返回完整结果")`，
     它会替换"正在思考"那条占位消息，用户立刻看到进展；
   * 在 `tokio::task::spawn_blocking(...)` 里调
     `handle_bot_control(BotControlAction::Start, "duck", caller_id)`，因为业务层的 `screen`、文件与
     `sleep` 全是同步阻塞，必须挪出 async 线程。
3. `lib::handle_bot_control` 先 `permitted_bot_dir_entry` 判权限，再
   `bot_control::start::start_bot(&bot_dir_entry)`。
4. 权限判定用的条目带着归属用户 ID 与 Token 状态：`find_by_name` 在全服用户目录里按名字找，再按条目
   归属是不是调用者判一次（管理员一律放行）。
5. 条目里的 Token 状态确认已配好（`BotTokenStatus::from_dir` 在构造条目时取好），
   `utils::is_bot_screen_running` 确认未在运行。
6. `StartupQueue::start_bots()` 按 [CONTEXT 第 4 节](../../CONTEXTS/CONTEXT-zh_cn.md) 那四个显式步骤来做。
7. 结果由 `Report` 组装，经 `IntoReplyText::to_reply_text` 转成文本，再由 `ctx.say(报告)` 作为第二条
   消息发出（第一条已经把占位的额度用掉了）。

## 4. 必须遵守的不变量（与分层有关的部分）

完整清单（含回复口径、交互时序、配置读取规则）见 [CONTEXT 第 2 节](../../CONTEXTS/CONTEXT-zh_cn.md)。这里只列与谁负责
哪一层直接相关的几条，避免两处口径各自漂移。

* 一个账号一个 `screen` 会话：会话名是 `runtime.screen_prefix` 加规范化机器人名，唯一来源是
  `utils::session_name`；只有一份 `screen -list` 解析口径（`utils::screen`）。
* 目录即归属：机器人目录固定为 `base_dir/bots/Discord用户ID/机器人名`，权限边界由用户 ID 那一层决定
  （`utils::bot_dirs`）。
* 每条指令都声明一个权限要求，判定只有一处：要求是 `utils::RequiredPermission`（`Everyone`、
  `MustBeAdmin`、`OwnerOrAdmin(归属用户ID)`），校验一律走 `has_permission`：
  * 全员指令（`/help`、`/show status`、`/ping`、`/checkserver`、`/show *`）：由路由层的 `lib::handle_*`
    判一次；
  * 单机器人指令与账号管理指令：由路由层先 `BotDirEntry::find_by_name` 解析目标条目，再按
    `OwnerOrAdmin(条目归属)` 判一次（普通用户只能碰自己目录，管理员可跨目录）；
  * 全服批量指令：由路由层 `lib::handle_all_bots_control` 用
    `has_permission(caller_id, RequiredPermission::MustBeAdmin)` 判一次，`all_bots_control/*` 的函数
    因此不接 `caller_id`、也不再各自判断；
  * 命令层只数参数个数、看数字范围，不看权限；
  * 新增指令一律照这个来：命令层必须把结果交给 `lib::handle_*`（那一层声明要求并判一次），不许在命令
    层自己拼回复，连 `/whoami` 也一样。没有注册表或框架钩子兜底，只能依赖这条约定与评审。
* 框架只出现在命令层与进程入口：业务层（`bot_control` 等）与 `lib.rs` 不引用 serenity 与 poise，只吃
  普通参数、返回 `Result<String, String>`。这样业务逻辑能单测，也方便以后换通道。
* 命令层不阻塞 async 线程：所有业务调用都包 `tokio::task::spawn_blocking`（业务层是同步的）。
* 停止必须彻底：所有停止入口复用 `utils::stop_bot_sessions`（Ctrl+C、等待、强杀、复核），保证不留占着游戏
  账号的孤儿 JVM。
* 运行中不许动的动作先查会话：`/token`、`/bot del`、`/bot rename` 在真正动手前都先
  `is_bot_screen_running`；重启流程更严，它在停止之前就剔掉缺 Token 的目标。
* 退出信号只有一条路径：收到 SIGTERM 或 Ctrl+C 时走 `watchdog.exit_on_shutdown_signal()`，先记日志，再停掉全部
  机器人、就地拼一份与 `/stopall` 同口径的收尾报告（报告进程序日志，此外还发到各服务器的通报频道），最后以
  退出码 0 退出（systemd 不重拉）。掉线与
  `client.start()` 返回都不退出，而是停全部机器人、等一个判定窗口、重建连接（见
  [watchdog.md 第 4 节](../watchdog/watchdog-zh_cn.md)）；只有配置写错（Token 无效、机器人没被邀请进服务器）才以退出码
  1 退出，交给 systemd（单元里配 `RestartPreventExitStatus=1`，这类错误不重拉）。
* 两类日志与启动步骤都有专门说明：见 [utils-zh_cn.md 第 7 节](../utils/utils-zh_cn.md#7-两类日志) 与
  [process-log.md](../process-log/process-log-zh_cn.md)。
* 回复不夸大：能验证到哪一步就只说到哪一步。`/start` 与 `/restart` 只能确认命令已发出、会话已出现，
  所以回"已发送命令"；`/stop` 的流程当场复核会话已消失，才可以回"已成功停止"。新增指令按同一条原则
  选词。

## 5. 代码约定（要点）

完整清单（含每条理由）在 [docs/CONTEXTS/CONTEXT-zh_cn.md 第 2 节](../../CONTEXTS/CONTEXT-zh_cn.md)。

* 分层叫法固定：`src/commands/` 是命令层（取值、校验数字、`defer`、发结果）；`lib.rs` 的 `handle_*`
  是指令入口层（动作枚举到业务函数，权限只在这一层判一次）；下面是业务层（`bot_control`、
  `bot_manage`、`show`、`all_bots_control`）与 utils 原语层。
* 框架只出现在命令层与进程入口：业务层与 `lib.rs` 不引用 serenity 与 poise，只吃普通参数、返回
  `Result<String, String>`，这样业务逻辑能单测。
* 命名自解释：动作函数用动词短语（`start_bot`）；取返回值用名词（`all_running_bot_status_rows`，没有
  `get_` 前缀）；转换用 `to_` 与 `into_` 前缀（`to_clean_bot_name`、`into_report`，消费 `self` 的用
  `into_`、其余用 `to_`，clippy 也这么认）；需要磁盘查找的构造用 `find_*`；判断用 `is_` 与 `has_`；
  ID 一律叫 `*_id`（`caller_id`、`owner_id`）。
* 按返回形态添加后缀：列表用 `*_list`（`my_bots_list`）；状态表用 `*_status`
  （`all_running_bots_status`）；一次性探测或检查用 `*_report`（`ping_report`、
  `checkserver_report`）；当附件发出去的数据用 `*_view`（`fishingbot_config_view`、`bot_log_view`）。只有
  整段已经写好的文本用裸名词（`help`、`whoami`）。
* 全局单例的访问器采用相同命名：配置 `config::Config::get()`、文案 `text::get()`，都表示"取进程内那一份"。配置没装就
  `expect` panic（那是内部不变量）；文案没装时生产构建 panic（入口顺序错误），测试构建固定返回英语。
  `Language` 不实现 `Default`，`[i18n] language` 不受支持时启动校验直接报错退出。两者由进程入口
  各装一次（`config::Config::get_or_init()` 里顺带 `text::install()` 一次）。看门狗另走一条：`src/watchdog.rs`
  的 `WATCHDOG` 是进程级槽位，使用方调一次 `WATCHDOG.get_or_init()`（首次按配置创建）；事件回调只收函数指针、
  捕获不了闭包，因此也从同一入口取同一份，其余方法都吃 `&self`。
* 文件内的顺序固定：模块文档（`//!`）、`mod` 声明、`use`、对外转出（`pub use`）、常量与类型与函数；
  进程入口（`lib.rs`、`main.rs`）也照此，于是"这个文件里有哪些子模块"在文件最上面就能看到。
* 会变成可见文案的那一行写英文：命令的 `///` 第一段与选项的 `#[description]` 是 poise 宏烘进二进制的
  兜底（语言表缺项时才会露出来），所以写英文，且与 `en_us.rs` 表里那句逐字相同（英文表是其中一份实现，有
  测试逐字比对）；第一段之后的段落仍是中文，细节见 [commands-zh_cn.md 第 1.7.1 节](../commands/commands-zh_cn.md#171-宏里那一份算什么英文副本)。
* 同一份文本全链路只清洗一次：命令层只取值（参数名以 `raw_` 起），谁真正要用谁清洗（`clean_*`）。
* 注释里的记号一眼可辨：`?` 开头是待实跑确认的开放问题（确认后删记号、留结论）；`!` 开头是待填但有
  回落路径；`⚠️` 用在语言表里提醒硬约定。
* 关键词放哪儿：只有"各部署不同、必须自己填"的进 `[keywords]`（`online`、`filter_regex` 两项）；
  `/pause` 与 `/unpause` 的回显进 `[command_echo]`（`pause_echo`、`unpause_echo`；仅在所用 JAR 支持这两条指令时
  才会出现，换 JAR 时就地改）；JAR 打印的断线重连提示留在 i18n 的 `jar_*` 方法里，跟随 `[i18n] language`；
  进程是否结束不看日志，看 screen 会话残壳（`(Dead ...)`）。

## 6. 从源码构建与测试

```bash
rustup target add x86_64-unknown-linux-musl   # 默认目标，见 .cargo/config.toml
cargo check --all-targets
cargo build --release                         # target/x86_64-unknown-linux-musl/release/minecraft-fishingbot-manager-for-discord
cargo fmt
cargo test                                    # 库、命令层与对外契约的回归测试
```

* 默认编译目标是 musl 静态链接（产物不依赖发行版与 glibc 版本，可直接部署到服务器运行）。因为 rustls 的
  加密后端要用 C 编译器，还需要 `musl-gcc`（Arch 装 `musl`，Debian 与 Ubuntu 装 `musl-tools`），
  否则会报 `failed to find tool "x86_64-linux-musl-gcc"`。
* 发布构建在 `Cargo.toml` 的 `[profile.release]` 中启用了 LTO、`panic = "abort"` 与 `strip`。
* 改配置结构时要同步 `minecraft-fishingbot-manager-for-discord.conf.example.toml` 与 [config-zh_cn.md](../config/config-zh_cn.md)，
  否则 `cargo test` 会失败（有测试专门校验这两项）。
* 文档链接用 `cargo doc --no-deps` 复核（Rustdoc 注释）；Markdown 文档链接用 `cargo test --test docs_links` 复核。

## 7. 相关文档

* 全部文档见 [docs 文档索引](../../index-zh_cn.md)。
* 各层细节：[config-zh_cn.md](../config/config-zh_cn.md)（配置层）、[commands-zh_cn.md](../commands/commands-zh_cn.md)（命令层）、[lib-zh_cn.md](../lib/lib-zh_cn.md)（指令入口层）、[utils-zh_cn.md](../utils/utils-zh_cn.md)（原语层）。
* 硬约束与不变量：[CONTEXT-zh_cn.md](../../CONTEXTS/CONTEXT-zh_cn.md)。
