# docs：文档索引
[English](index-en_us.md) | **简体中文**

关于文档须知：README,部署教程,错误排查的中文版是由作者手写的，其余文档由AI生成（未来逐步优化），英语版本文档全部由AI生成。

## 1. README 列表

* [English](./README/README-en_us.md)
* [简体中文](./README/README-zh_cn.md)

## 2. 入门指引

* [部署教程](tutorial/tutorial-zh_cn.md)
* [常见的故障排查提示与排查](troubleshooting/troubleshooting-zh_cn.md)
* [配置文件结构说明](mods/config/config-zh_cn.md)

## 3. 项目结构模块说明

项目总览（分层、目录结构、指令的完整路径）见 [architecture-zh_cn.md](mods/architecture/architecture-zh_cn.md)。

其他文档如下所示:

* [`lib`](mods/lib/lib-zh_cn.md)：指令入口层（路由层）。动作枚举、`handle_*`、权限校验、错误口径、退出与重连入口。
* [`commands` + `commands/`](mods/commands/commands-zh_cn.md)：命令层（poise 指令）。取值、校验参数个数、`defer`、把 `Err` 变成回复；7 个业务文件：`bot_control`、`bot_manage`、`token`、`log`、`all_bots`、`show`、`general`。
* `bot_control` + `bot_control/`：单机器人业务，一指令一文件（`start`、`stop`、`restart`、`pause`、`unpause`、`command`、`log`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 3 节。
* `bot_manage` + `bot_manage/`：账号管理业务（`add`、`del`、`rename`、`token`、`login`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 4 节。
* `all_bots_control` + `all_bots_control/`：全服批量业务，仅管理员（`startall`、`stopall`、`restartall`、`commandall`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 5 节。
* `show`：只读查询业务（`status`、`botconfig`、`mybots`、`allbots`、`admins`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 6 节。
* `general` + `general/`：通用业务（`help`、`ping`、`checkserver`、`whoami`）。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 7 节。
* [`config` + `config/`](mods/config/config-zh_cn.md)：外置配置。`schema`（配置结构体，与 toml 一一对应）、`parse`（反序列化 + 路径展开）、`validate`（启动校验）、`template`（首次启动生成配置模板）。
* [`text`](mods/i18n/i18n-zh_cn.md)：文案与多语言，一个进程一种语言（`[i18n] language` 启动时选定）。`en_us`（默认语言）、`zh_cn`（与 `en_us` 一一对应，只换字面量）、`bootstrap`（引导期英文）、`log_text`、`requirement`、`setup_failure`。
* [`watchdog`](mods/watchdog/watchdog-zh_cn.md)：网关看门狗。唯一判定"掉线"的地方：握手与探活两条判据，纯计时，请求重建连接由进程入口发出。
* [`process_log`](mods/process-log/process-log-zh_cn.md)：程序日志。终端（stderr）与 `<部署目录>/logs/时间戳.log` 双写。
* [`utils`](mods/utils/utils-zh_cn.md)：原语层，上层可复用的能力：`bot_command`、`bot_dirs`、`bot_name`、`bot_log_files`、`bot_status`、`fishingbot_config`、`fs`、`permission`、`reply`、`restart`、`running_bots`、`screen`、`shutdown`、`startup`、`uptime`。

## 4. 指令

* [commands-zh_cn.md](mods/commands/commands-zh_cn.md)：命令层与全部指令。第 1 节讲命令层的规矩，
  第 3 节到第 7 节按模块列出 27 条指令的说明（单机器人控制、账号管理、批量、查询、通用）。

## 5. 对照清单

* [refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md](refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md)：上游 jar 的全部指令与全部
  回复文案（中英对照），用于核对关键词与回显。查表类静态文档，不在改动流程里。
* [CONTEXTS/CONTEXT-zh_cn.md](CONTEXTS/CONTEXT-zh_cn.md)：改代码前先读这份。项目概况、全部硬约束与不变量、代码地图、待办，
  给下一位接手的人或 AI 会话。
