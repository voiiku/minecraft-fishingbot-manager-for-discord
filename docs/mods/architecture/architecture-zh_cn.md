# 架构与模块地图
[English](architecture-en_us.md) | **简体中文**

本文说明代码分几层、每层负责什么，以及一条 Discord 斜杠指令如何落到 `screen` 会话。按主题查找文档，见
[docs 文档索引](../../index-zh_cn.md)。

## 1. 分层

`discord_commands` 接收交互、传入调用者 ID 并发送回复。所有用户命令的权限只在 `operations` 检查；Discord 不预检、不扫描归属目录。

`operations` 处理**用户请求**，`runtime::bot_instances` 操作**本机 FishingBot 实例**。例如 `/stopall` 要检查管理员权限并回复调用者；管理器收到 SIGTERM 时没有调用者。两者复用 `runtime::bot_instances::stop` 停止并复核会话，只有进程收尾路径会退出管理器。

## 2. 目录结构

* `src/discord_commands/`: Discord 命令、注册、回执、附件和 Modal
* `src/operations/single_bot_control/`：单台指令，每条指令一个文件
* `src/operations/all_bots_control/`：批量机器人控制；独立的反 AFK 任务由 `src/operations/set_anti_afk.rs` 控制
* `src/operations/accounts/`：账号、凭据和登录
* `src/operations/queries/`：查询与展示，包括 `/log`、机器人候选、`/help` 和 `/ping`；只读不代表无需鉴权
* `src/text/report.rs`: Report 的构造和渲染
* `src/runtime/bot_instances/`：目录、会话、状态与机器人操作；`stop.rs` 只停止机器人，不退出管理器
* `src/runtime/`：`bot_startup` 负责机器人启动队列；`shutdown` 负责管理器退出；`log_files` 供程序与机器人日志共用；`discord_notices` 为掉线恢复和退出报告选择并投递到同一组通报频道。其余运行组件是 `config`、`process_log`、`bot_log`、`watchdog` 和 `anti_afk`
* `src/runtime/utils/`: 仅供 runtime 内部使用的基础工具；可依赖配置与外部 utils，不依赖文案或上层业务
* `src/utils/`: 名称规则、命令片段拼接、文件操作及时长工具；不依赖项目内其他模块或文案
* `src/text/`: 中英文文案与报告渲染；不依赖其他项目模块，调用方提供报告语言与长度上限

`bot_startup` 与 `bot_instances` 是运行时同层协作：启动队列使用实例的会话和状态接口，重启和停止流程也使用队列。

使用 Rust 库时，从功能所属模块导入业务接口，例如 `text::report::Report` 和
`utils::console_command::ToCleanBotCommand`。`utils` 存放底层实现，不转发上层业务接口。
源码模块改名不影响部署目录：机器人数据仍位于 `base_dir/bots`。

## 3. 一条指令的完整路径

以 `/start` 为例：

1. Discord 读取机器人名并 defer，把名字和调用者 ID 交给业务入口。
2. `operations::single_bot_control` 查找并鉴权一次，返回绑定该目录的待执行操作；拒绝时只有错误回复。
3. Discord 成功发送进度回执后才执行。`src/operations/single_bot_control/start_single_bot.rs` 检查凭据和会话，再提交启动队列。
4. 队列在宽限期后根据 `/status` 复核结果。Discord 入口最后把报告发送给用户。

## 4. 必须遵守的不变量（与分层有关的部分）

完整清单（含回复口径、交互时序、配置读取规则）见 [CONTEXT 第 2 节](../../CONTEXTS/CONTEXT-zh_cn.md)。这里只列与谁负责
哪一层直接相关的几条，避免两处口径各自漂移。

* 一个账号一个 `screen` 会话：会话名是 `runtime.screen_prefix` 加规范化机器人名，唯一来源是
  `runtime::bot_instances::screen::session_name`；`screen -list` 由内部 `runtime::utils::screen` 统一解析。
* 目录即归属：机器人目录固定为 `base_dir/bots/Discord用户ID/机器人名`，权限边界由用户 ID 那一层决定
  （`runtime::bot_instances::bot_dirs`）。
* 普通用户只能控制自己的机器人；批量控制必须是管理员。`operations` 负责这些规则，直接库调用与 Discord 调用共用同一业务入口逻辑。
* 框架只出现在命令层与进程入口：业务层（`bot_control` 等）与 `lib.rs` 不引用 serenity 与 poise，只吃
  普通参数、返回 `Result<String, String>`。这样业务逻辑能单测，也方便以后换通道。
* 命令层不阻塞 async 线程：所有业务调用都包 `tokio::task::spawn_blocking`（业务层是同步的）。
* 停止必须彻底：所有停止入口复用 `runtime::bot_instances::stop::stop_bot_sessions`（Ctrl+C、等待、强杀、复核），保证不留占着游戏
  账号的孤儿 JVM。
* 运行中不许动的动作先查会话：`/token`、`/bot del`、`/bot rename` 在真正动手前都先
  `is_bot_screen_running`；重启流程更严，它在停止之前就剔掉缺 Token 的目标。
* 退出信号只有一条路径：收到 SIGTERM 或 Ctrl+C 时走 `runtime::shutdown::exit_on_shutdown_signal(notice_ctx)`，先记日志，再停掉全部
  机器人、就地拼一份与 `/stopall` 同口径的收尾报告（报告进程序日志，此外还发到各服务器的通报频道），最后以
  退出码 0 退出（systemd 不重拉）。掉线与
  `client.start()` 返回都不退出，而是停全部机器人、等一个判定窗口、重建连接（见
  [watchdog.md 第 4 节](../watchdog/watchdog-zh_cn.md)）；只有配置写错（Token 无效、机器人没被邀请进服务器）才以退出码
  1 退出，交给 systemd（单元里配 `RestartPreventExitStatus=1`，这类错误不重拉）。
* 两类日志与启动步骤都有专门说明：见 [runtime-zh_cn.md 第 7 节](../runtime/runtime-zh_cn.md#7-两类日志) 与
  [process-log.md](../process-log/process-log-zh_cn.md)。
* 回复不夸大：能验证到哪一步就只说到哪一步。`/start` 与 `/restart` 只能确认命令已发出、会话已出现，
  所以回"已发送命令"；`/stop` 的流程当场复核会话已消失，才可以回"已成功停止"。新增指令按同一条原则
  选词。

## 5. 代码约定（要点）

完整清单（含每条理由）在 [docs/CONTEXTS/CONTEXT-zh_cn.md 第 2 节](../../CONTEXTS/CONTEXT-zh_cn.md)。

* 框架只出现在命令层与进程入口：业务层与 `lib.rs` 不引用 serenity 与 poise，只吃普通参数、返回
  `Result<String, String>`，这样业务逻辑能单测。
* 命名自解释：动作函数用动词短语（`start_bot`）；取返回值用名词（`token_ready_bot_names_with_screen_sessions`，没有
  `get_` 前缀）；转换用 `to_` 与 `into_` 前缀（`to_clean_bot_name`、`into_report`，消费 `self` 的用
  `into_`、其余用 `to_`，clippy 也这么认）；需要磁盘查找的构造用 `find_*`；判断用 `is_` 与 `has_`；
  ID 一律叫 `*_id`（`caller_id`、`owner_id`）。
* 按返回形态添加后缀：列表用 `*_list`（`my_bots_list`）；状态报告用 `*_status_report`
  （`bot_and_anti_afk_status_report`）；作为附件发送的数据用 `*_view`（`fishingbot_config_view`、`bot_log_view`）。
  一次性探测、检查与整段文本使用裸名词（`ping`、`checkserver`、`help`、`whoami`）。
* 配置通过 `config::Config::get()` 获取，文案通过 `config::Config::get().text()` 从配置选择。
  配置未加载时访问会 panic；不支持的 `[i18n] language` 在启动解析时报错。`text` 不依赖其他项目模块，
  也没有单独的语言单例。看门狗使用 `src/lib.rs`
  声明的 `WATCHDOG` 保存实例，通过 `Watchdog::get_or_init()` 获取（首次按配置创建）；事件回调只收函数指针、
  捕获不了闭包，因此也从同一入口取同一份，其余方法都吃 `&self`。
* 文件内的顺序固定：模块文档（`//!`）、`mod` 声明、`use`、对外转出（`pub use`）、常量与类型与函数；
  进程入口（`lib.rs`、`main.rs`）也照此，于是"这个文件里有哪些子模块"在文件最上面就能看到。
* 指令及选项说明只保留在语言表，注册前校验并填入，细节见
  [commands-zh_cn.md 第 1.7.1 节](../commands/commands-zh_cn.md#171-文案表校验)。
* 同一份文本全链路只清洗一次：命令层只取值（参数名以 `raw_` 起），谁真正要用谁清洗（`clean_*`）。
* 注释里的记号一眼可辨：`?` 开头是待实跑确认的开放问题（确认后删记号、留结论）；`!` 开头是待填但有
  回落路径；`⚠️` 用在语言表里提醒硬约定。
* 日志过滤规则 `filter_regex` 位于 `[keywords]`；JAR 的状态回显留在 i18n 的 `jar_*` 方法里，跟随 `[i18n] language`；
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
* 各层细节：[config-zh_cn.md](../config/config-zh_cn.md)（配置层）、[commands-zh_cn.md](../commands/commands-zh_cn.md)（命令层）、[lib-zh_cn.md](../lib/lib-zh_cn.md)（指令入口层）、[runtime-zh_cn.md](../runtime/runtime-zh_cn.md)（原语层）。
* 硬约束与不变量：[CONTEXT-zh_cn.md](../../CONTEXTS/CONTEXT-zh_cn.md)。
