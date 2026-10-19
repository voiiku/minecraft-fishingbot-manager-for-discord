# 文档索引
[English](index-en_us.md) | **简体中文**

简体中文 README、部署教程与故障排查由作者手写；其余中文文档目前由 AI 生成，后续会逐步优化。英文文档均为 AI 翻译，仍在审校。

## 1. README

* [English](./README/README-en_us.md)
* [简体中文](./README/README-zh_cn.md)

## 2. 入门

* [部署教程](tutorial/tutorial-zh_cn.md)
* [常见故障与排查](troubleshooting/troubleshooting-zh_cn.md)
* [配置文件说明](mods/config/config-zh_cn.md)

## 3. 模块文档

项目总览（分层、目录结构、一条指令的完整路径）见 [architecture-zh_cn.md](mods/architecture/architecture-zh_cn.md)。

其余模块文档：

* [`lib`](mods/lib/lib-zh_cn.md)：指令入口层（路由层），包括动作枚举、`handle_*`、权限校验、错误处理约定，以及退出与重连入口。
* [`commands` + `commands/`](mods/commands/commands-zh_cn.md)：命令层（poise 指令），负责读取参数、校验数值、`defer` 与把 `Err` 转为回复；包含 7 个业务文件：`bot_control`、`bot_manage`、`token`、`log`、`all_bots`、`show`、`general`。
* `bot_control` + `bot_control/`：单机器人操作，一指令一文件（`start`、`stop`、`restart`、`pause`、`unpause`、`command`、`log`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 3 节。
* `bot_manage` + `bot_manage/`：账号管理（`add`、`del`、`rename`、`token`、`login`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 4 节。
* `all_bots_control` + `all_bots_control/`：全机器人批量操作，仅管理员可用（`startall`、`stopall`、`restartall`、`commandall`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 5 节。
* `show`：只读查询（`status`、`botconfig`、`mybots`、`allbots`、`admins`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 6 节。
* `general` + `general/`：通用指令（`help`、`ping`、`checkserver`、`whoami`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 7 节。
* [`config` + `config/`](mods/config/config-zh_cn.md)：外置配置。`schema`（与 TOML 对应的配置结构体）、`parse`（反序列化与路径展开）、`validate`（启动校验）、`template`（首次启动生成的模板）。
* [`text`](mods/i18n/i18n-zh_cn.md)：文案与多语言。一个进程一种语言，启动时由 `[i18n] language` 选定；`en_us`（英语）、`zh_cn`（与 `en_us` 一一对应，仅字面量不同）、`bootstrap`（引导期英文）、`log_text`、`requirement`、`setup_failure`、`watchdog`（看门狗内部不变量文案，随语言切换）。
* [`watchdog`](mods/watchdog/watchdog-zh_cn.md)：网关看门狗。唯一判定“掉线”的位置：握手与探活两条判据，均为纯计时；重建连接的请求由进程入口发出。
* [`process_log`](mods/process-log/process-log-zh_cn.md)：程序日志，同时写入终端（stderr）与 `<部署目录>/logs/时间戳.log`。
* [`utils`](mods/utils/utils-zh_cn.md)：原语层，提供上层可复用的能力：`bot_command`、`bot_dirs`、`bot_name`、`bot_logs`、`bot_status`、`fishingbot_config`、`fs`、`permission`、`reply`、`restart`、`running_bots`、`screen`、`shutdown`、`startup`、`uptime`。

## 4. 指令

* [commands-zh_cn.md](mods/commands/commands-zh_cn.md)：命令层与全部指令。第 1 节说明命令层规则，第 3–7 节按模块说明全部指令（单机器人控制、账号管理、批量操作、查询、通用）。

## 5. 参考资料

* [refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md](refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md)：FishingBot JAR 的指令与回复文案（中英对照），用于核对关键词与回显；属于静态查表文档，不参与代码改动流程。
* [CONTEXTS/CONTEXT-zh_cn.md](CONTEXTS/CONTEXT-zh_cn.md)：修改代码前应先阅读。内容包括项目概况、硬约束与不变量、代码地图和待办事项，供后续维护者或 AI 会话使用。
