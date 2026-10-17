# Minecraft FishingBot Manager for Discord

[English](../../README.md) | **简体中文**

* **本项目暂时不支持离线模式的服务器或单人游戏**。
* **本项目暂时只支持运行在 64 位 Linux 系统下**。
* 想使用 QQ？可以考虑 [QQ 版本（已停止功能更新）](https://github.com/voiiku/minecraft-fishingbot-manager-for-qq)。
* 由于 [FishingBot](https://github.com/MrKinau/FishingBot) 的官方构建版本尚未支持 `/pause` 和 `/unpause` 命令（[已计划于26.4加入](https://github.com/MrKinau/FishingBot/issues/291#issuecomment-6206998408)），暂时无法正确处理暂停钓鱼功能(逻辑已经实现)，其余功能不受影响。

## 1. 项目概要

本项目通过 Discord 群聊管理多个 Minecraft 自动钓鱼机器人。机器人本身由独立的 [FishingBot](https://github.com/MrKinau/FishingBot) 项目提供，运行 `FishingBot.jar` 需要 Java JRE。

简而言之，本项目只提供管理 FishingBot 钓鱼机器人的方法，暂时不涉及具体的钓鱼机器人行为逻辑。

* 适用人群：希望在 MC 服务器中挂机钓鱼，拥有多个游戏账号且希望集中管理的（一群）玩家。
* 不适用人群：单人游戏玩家。
* 本项目解决的问题：
  * 节省分散挂机钓鱼的开销（当一群好友在各自的电脑上挂机钓鱼时，开销比集中钓鱼大得多）。
  * 管理多个账号。
  * 满足一些常见需求（暂停钓鱼以作为假人使用、可向机器人发送命令、向服务器内发送聊天消息、查看日志等）。

## 2. 开始部署

见 [部署教程](../tutorial/tutorial-zh_cn.md)。

## 3. 命令预览

所有人可用：

* `/help`：帮助菜单。整段文本来自配置中的 `text.help`，你可以自行设置。
* `/whoami`：查看调用者的 Discord 用户 ID。
* `/ping [地址] [端口]`：探测游戏服务器；不带参数时用配置中 `[ping]` 设定的默认值。
* `/checkserver`：查看钓鱼服务器自身的性能情况（CPU、内存、磁盘、运行时长）。
* `/show status`：检查全部运行中机器人的状态。
* `/show botconfig`：查看全部钓鱼机器人共用的配置文件，以附件发出。
* `/show mybots`、`/show allbots`、`/show admins`：查看自己名下的机器人、全服机器人、管理员名单。
* `/bot add <机器人名>`：添加机器人（还需要设置 Token 才可开始使用）。

机器人的创建者或管理员可用：

* `/bot del <机器人名>`、`/bot rename <旧名> <新名>`：删除或更名机器人。
* `/bot login <机器人名>`：通过网站登录微软账号并填写代码，以获取机器人 Token 并在服务器上自动保存，成功时自动启动机器人。
* `/token <机器人名>`：手动写入机器人 Token，不建议使用。
* `/log <机器人名> [行数]`：查看某机器人最新的运行日志，以附件发送。
* `/start`、`/stop`、`/restart`：启动、停止、重启单个机器人。
* `/pause`、`/unpause`：暂停钓鱼但不下线、恢复钓鱼（取决于 `FishingBot.jar` 是否支持，不影响其他功能）。
* `/command <机器人名> <指令>`：向机器人所在的游戏服务器发送指令。

仅管理员可用：

* `/startall`、`/stopall`、`/restartall`、`/commandall <指令>`：对全服机器人批量操作。

## 4. 项目现状

* 单人维护，目前仍处于测试阶段，配置项与指令回复随更新而调整，更新会在 Release 中声明不兼容变更。
* 原版 `FishingBot.jar` 尚未加入游戏内 `/pause`、`/unpause` 指令，上游已计划支持。在所用 JAR 包含这两条指令之前，`/pause` 与 `/unpause` 只会回复“命令已发送”，暂停状态不会更新；其他功能不受影响。
* 语言目前仅支持 `ENGLISH` 与 `CHINESE_SIMPLIFIED` 两种（取值就是 FishingBot 项目对语言的命名）。全部的英语文本和英语文档均为 AI 翻译。
* 非目标：Web 面板、内置代理服务。
* 未来目标：适配 Windows。内建钓鱼机器人，允许用户选择使用 `FishingBot.jar` 或内置的机器人。

## 5. 许可证

本项目许可证采用 [MIT License](../../LICENSE)。

第三方许可证请查看 [THIRD-PARTY-NOTICES.html](../../THIRD-PARTY-NOTICES.html)。

[FishingBot](https://github.com/MrKinau/FishingBot) 是使用 [GPL-3.0 许可证](https://github.com/MrKinau/FishingBot/blob/main/LICENSE)的独立程序，本项目只管理它，不包含它，也不与它一起打包发布。

## 6. 参与贡献

欢迎参与贡献。发现问题或有功能建议，请[提交 Issue](https://github.com/voiiku/minecraft-fishingbot-manager-for-discord/issues)。也欢迎提交 Pull Request；对于较大的改动，建议先开 Issue 讨论方案。

### 翻译

面向用户的文本目前定义在 `src/text/` 模块中，并在启动时加载。修改或新增翻译时，请参考现有语言模块，例如 `src/text/zh_cn.rs`。语言加载前显示的消息会使用英语。

**计划未来重构多语言功能。**

## 7. 文档

全部文档见 [docs 文档索引](../index-zh_cn.md)，常用文档如下：

* [docs/tutorial/tutorial-zh_cn.md](../tutorial/tutorial-zh_cn.md)：从零开始的部署教程（面向第一次用 Discord 的人）。
* [docs/troubleshooting/troubleshooting-zh_cn.md](../troubleshooting/troubleshooting-zh_cn.md)：故障提示与排查。
* [docs/mods/config/config-zh_cn.md](../mods/config/config-zh_cn.md)：配置文件的说明、加载流程、每个字段的含义与启动校验。
