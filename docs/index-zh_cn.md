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

* [`lib`](mods/lib/lib-zh_cn.md)：库公开接口；`handlers/routes` 包含动作枚举、`handle_*` 路由与权限校验。
* [`discord_commands`](mods/commands/commands-zh_cn.md)：Discord 命令层（poise 指令），负责读取参数、校验数值、`defer` 与把 `Err` 转为回复；指令分类为 `bot_control`、`bot_manage`、`token`、`log`、`all_bots`、`show`、`general`。
* `handlers`：指令业务入口，包含机器人控制、账号管理、批量操作、只读查询和通用指令。见 [commands-zh_cn.md](mods/commands/commands-zh_cn.md) 第 3–7 节。
* [`config` + `config/`](mods/config/config-zh_cn.md)：外置配置。`schema`（与 TOML 对应的配置结构体）、`parse`（反序列化与路径展开）、`validate`（启动校验）、`template`（首次启动生成的模板）。
* [`text`](mods/i18n/i18n-zh_cn.md)：文案与多语言。一个进程一种语言，启动时由 `[i18n] language` 选定；`en_us`（英语）、`zh_cn`（与 `en_us` 一一对应，仅字面量不同）、`bootstrap`（引导期英文）、`log_text`、`requirement`、`setup_failure`、`watchdog`（看门狗内部不变量文案，随语言切换）。
* [`watchdog`](mods/watchdog/watchdog-zh_cn.md)：网关看门狗。唯一判定“掉线”的位置：握手与探活两条判据，均为纯计时；重建连接的请求由进程入口发出。
* [`process_log`](mods/process-log/process-log-zh_cn.md)：`runtime` 中的程序日志模块，同时写入终端和日志文件。
* [`utils`](mods/utils/utils-zh_cn.md)：全局共享工具，包括机器人目录、日志、状态、启动队列及进程日志等能力。

## 4. 指令

* [commands-zh_cn.md](mods/commands/commands-zh_cn.md)：命令层与全部指令。第 1 节说明命令层规则，第 3–7 节按模块说明全部指令（单机器人控制、账号管理、批量操作、查询、通用）。

## 5. 参考资料

* [refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md](refs/BOT_COMMANDS_AND_MESSAGES-zh_cn.md)：FishingBot JAR 的指令与回复文案（中英对照），用于核对关键词与回显；属于静态查表文档，不参与代码改动流程。
* [CONTEXTS/CONTEXT-zh_cn.md](CONTEXTS/CONTEXT-zh_cn.md)：修改代码前应先阅读。内容包括项目概况、硬约束与不变量、代码地图和待办事项，供后续维护者或 AI 会话使用。
