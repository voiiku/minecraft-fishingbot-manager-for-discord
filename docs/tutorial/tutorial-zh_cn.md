# 从零开始的部署教程
[English](tutorial-en_us.md) | **简体中文**

提示：如果有哪一步看不懂，你可以把本文档的链接发送给 AI，向 AI 求助。

## 目录

- [从零开始的部署教程](#从零开始的部署教程)
  - [目录](#目录)
  - [1. 开始部署前的准备](#1-开始部署前的准备)
  - [2. 为 Linux 服务器安装环境](#2-为-linux-服务器安装环境)
    - [安装 screen](#安装-screen)
    - [安装 JRE（推荐 JAVA 25）](#安装-jre推荐-java-25)
    - [建立本项目的工作目录(以 `/fishingbots` 为例)：](#建立本项目的工作目录以-fishingbots-为例)
  - [3. 创建一个 Discord 群聊](#3-创建一个-discord-群聊)
  - [4. 创建一个 Discord 应用](#4-创建一个-discord-应用)
  - [5. 把 Discord 机器人添加到你的群聊](#5-把-discord-机器人添加到你的群聊)
  - [6. 放置本项目文件，第一次启动以生成配置模板](#6-放置本项目文件第一次启动以生成配置模板)
  - [7. 根据实际情况修改配置](#7-根据实际情况修改配置)
  - [8. 再次启动，确认连接 Discord](#8-再次启动确认连接-discord)
  - [9. 设置管理员（如果已经在第 7 步完成，就忽略）](#9-设置管理员如果已经在第-7-步完成就忽略)
  - [10. 在 Discord 群聊中查看帮助并添加第一个机器人](#10-在-discord-群聊中查看帮助并添加第一个机器人)
    - [10.1 创建第一个机器人](#101-创建第一个机器人)
    - [10.2 登录第一个机器人](#102-登录第一个机器人)
  - [11. 确认机器人状态](#11-确认机器人状态)
  - [12. 配置 systemd 服务（可选，推荐）](#12-配置-systemd-服务可选推荐)

## 1. 开始部署前的准备

你需要准备以下内容以开始部署：
* 一个 Discord 账号（[如果你还没有账号，请先注册](https://discord.com/register)）。
* 一台 Linux 机器：操作系统为 64 位 Linux，内存建议不小于 1G。
* 流畅的网络环境：如果你的 Linux 机器不能直连 `discord.com`，那么你还需要为其安装代理软件（推荐 TUN 模式）并配置代理节点，请自行询问AI或网络搜索相关教程。

## 2. 为 Linux 服务器安装环境

```bash
# 首先连接到你的 Linux 服务器（你可以询问 AI 如何连接到你的云服务器）
ssh user@address
```

### 安装 screen

```bash
sudo apt install screen   # Debian / Ubuntu 系
sudo yum install screen   # RHEL / CentOS 系
```

### 安装 JRE（推荐 JAVA 25）

```bash
sudo apt install openjdk-25-jre-headless    # Debian / Ubuntu 系
sudo dnf install java-25-openjdk-headless   # RHEL / CentOS 系
```
```bash
java -version     # 确认是否成功（能打印出你安装的版本即可）
```

### 建立本项目的工作目录(以 `/fishingbots` 为例)：

```bash
# 建立工作目录
sudo mkdir -p /fishingbots && sudo chown "$USER" /fishingbots
cd /fishingbots

# 下载 FishingBot.jar
curl -LO https://github.com/MrKinau/FishingBot/releases/download/v2.13.15/FishingBot-2.13.15.jar
```

## 3. 创建一个 Discord 群聊

Discord 中的服务器（Server）相当于一个群聊，指令都在它的频道里发送并被响应，为了避免混淆，之后统一将 Discord 服务器称为群聊。

1. 打开 <https://discord.com/> 或客户端，登录。
2. 点击左侧服务器列表最下方的 `+`按钮，选择亲自创建，选择"仅供我和我的朋友使用"，然后创建群聊。
3. 开启开发者模式：左下角齿轮图标（用户设置），进入高级设置（Advanced），开启开发者模式（Developer Mode）。

## 4. 创建一个 Discord 应用

群聊中的机器人本质上是一个 Discord 应用，需要先自助申请。

1. 打开 <https://discord.com/developers/applications>，右上角点 `New Application`，起个你喜欢的名字，点击 `Create` 创建。
2. 左侧 `Bot`，找到 `Reset Token` 并点击，复制弹出来的字符串，保存起来，之后还需要用。
   * 它只显示一次，可以理解为你机器人的密码，不要发给任何人！
   * 如果你弄丢了它或是它泄露了，回到这个页面再 `Reset Token` 一次（旧 Token 立刻失效）。
   * 不要开启 Privileged Gateway Intents ，因为本项目只用斜杠指令，不需要任何特权意图。
3. 左侧 `OAuth2` 中的 `URL Generator`：
   * Scopes 勾选 `bot` 与 `applications.commands`；
   * Bot Permissions 勾选 `View Channels`、`Send Messages`、`Embed Links`、`Attach Files`、`Read Message History`；
   * 页面最下方 Generated URL 就是邀请链接，保存起来，之后还需要用。

4. 左侧 `General Information` 中的 Application ID 保存起来，之后还需要用，它等于机器人的用户 ID。

## 5. 把 Discord 机器人添加到你的群聊

1. 用浏览器打开第 4 步获取的邀请链接。
2. 在添加到服务器（Add to Server）下拉里选中你刚刚创建的那个群聊，点继续并完成授权。
3. 回到 Discord，服务器成员列表里会出现你的机器人，状态是离线（因为此时程序还没启动）。

## 6. 放置本项目文件，第一次启动以生成配置模板

1. 从本项目的发布页（Releases）下载编译好的二进制文件并运行
   ```bash
    ./minecraft-fishingbot-manager-for-discord
   ```
此时应该提示你本目录下没有配置文件，会自动生成模板配置文件（`./minecraft-fishingbot-manager-for-discord.conf.toml`）。

## 7. 根据实际情况修改配置

**配置文件名：minecraft-fishingbot-manager-for-discord.conf.toml，其内部拥有注释提示，如果你看不懂，可以让 AI 帮你解读**。
```bash
chmod 600 minecraft-fishingbot-manager-for-discord.conf.toml    # 文件内含 token，限制为仅所有者可读
nano minecraft-fishingbot-manager-for-discord.conf.toml         # 或者 vim
```
根据你的实际情况修改即可。

提醒：

* 如果你想更改钓鱼机器人的行为配置文件 `conf.json`，应该前往本程序配置文件中的 `[fishingbot]` 修改相应字段，不要直接改动`conf.json`文件本身，否则你的改动会在启动时被覆盖。
* 每次修改完配置文件后必须重启本程序才能生效。

## 8. 再次启动，确认连接 Discord

```bash
./minecraft-fishingbot-manager-for-discord
```

终端中应当看到类似于这几行的输出：

```
程序日志: /fishingbots/logs/2026-10-03-13-10-05.log
已加载配置: /fishingbots/minecraft-fishingbot-manager-for-discord.conf.toml
进程 PID: 12345
已连接 Discord [应用 ID 123456789012345678]
邀请链接: https://discord.com/oauth2/authorize?client_id=...&scope=bot+applications.commands&permissions=60416
网关看门狗已就绪：判定窗口 30 秒、探活间隔 10 秒、首次握手超时 60 秒
```

如果 Discord 机器人头像由灰色变为在线或终端中出现"已连接 Discord"，说明机器人成功上线。

## 9. 设置管理员（如果已经在第 7 步完成，就忽略）

1. `Ctrl+C` 停止进程，修改 minecraft-fishingbot-manager-for-discord.conf.toml
2. 在 Discord 频道里发 `/whoami`，它会回复你的用户 ID（形如 `123456789012345678`）。
3. 写入配置：
   ```toml
   [admins]
   "123456789012345678" = "你的昵称"
   ```
   * 昵称只用于 `/show admins` 展示，
   * 你可以添加多个管理员。
4. 重新启动本程序
   ```bash
   ./minecraft-fishingbot-manager-for-discord
   ```

## 10. 在 Discord 群聊中查看帮助并添加第一个机器人

在 Discord 群聊中输入 `/help` 即可查看配置文件中定义的帮助文本（你可以修改它）。
如果机器人没有正常回复，请查看 `logs` 下最新的日志文件，并查看[故障提示与排查](../troubleshooting/troubleshooting-zh_cn.md)。

### 10.1 创建第一个机器人

在 Discord 里发 `/bot add`，机器人名 `duck`（这只是个示例名字）：
返回应该类似于
```
添加机器人 'duck' 成功！
注意：还需为其配置 Token 后才可启动。
建议使用 /bot login 在线登录
只有 /bot login 登录失败时，才应当使用 /token 命令手动设置 token
```

此时在 Linux 服务器上会创建目录：`/fishingbots/bots/<你的 Discord ID>/duck/`。

### 10.2 登录第一个机器人

**推荐做法**：在 Discord 群聊内发送 `/bot login <你的机器人名字>`，它会回复一条消息，包含一个微软的登录链接和一个代码，在浏览器里打开那个链接，输入代码并用微软账号登录。授权完成后它会报告"登录成功"，登录凭据（`refreshToken`）将保存至服务器，并自动启动机器人。

**手动操作**（仅当 `/bot login` 失败时使用）：

```bash
# 在你自己的电脑上（需要 JAVA 环境）执行
java -jar /your/path/FishingBot-2.13.15.jar
# 把 `FishingBot-2.13.15.jar` 与路径替换为实际的名字，或者你也可以尝试直接双击打开。
```
* 窗口出现后点击运行，由于你是第一次运行，它会请求你网页登录，登录成功后同级目录会生成 `refreshToken` 文件。
* 打开 `refreshToken` 全选复制它。
* 然后在 Discord 群聊中使用 `/token` 命令，把 `refreshToken` 文件内的字符串复制进弹窗内。
或者你也可以手动将 `refreshToken` 文件放到对应机器人的目录下，并按照配置文件中指定的文件名改名（默认就是 `refreshToken`）。

## 11. 确认机器人状态

如果你成功通过 `/bot login` 登录，那么它应该已经启动了，否则在执行`/token`后，在 Discord 群聊里发送 `/start <你的机器人名>`，机器人会产生两条回复信息：

1. 响应回复：`已接受 /start duck 请求；涉及启动的操作将由全局启动队列执行，完成后会返回完整结果`
2. 结果回复（三种之一）：
   * 日志已确认进服：`机器人 duck: 已确认进服`
   * 会话活过宽限期但日志尚未确认：`机器人 duck: 会话已存在但尚未确认状态（可稍后用/show status 确认）`
   * 会话在宽限期前消失：`机器人 duck: 启动失败 (常见原因是微软对频繁登录限流，或内存不足)`

* `/start` 会等满 `timing.check_grace` 再按会话状态与日志下结论，不再固定回"已发送"。复核结束后进程仍可能发生变化，因此后续状态以 `/show status` 为准。

确认状态：发送 `/show status`，会得到一个运行中的机器人的状态表：

```
反 AFK: 未运行
机器人 | 服务器状态 | 暂停状态 | 运行时长
你的机器人名 | 已连接 | 钓鱼中 | 3 分钟
```
**常见错误**
* 状态一直显示"就绪"或"初始中"：可能是日志关键词不匹配你的游戏服务器，先查看 `keywords.online`；若只有"重连"识别不出，再检查`[i18n] language` 有效

* 会话出现后自行退出：用`/log`指令查看最新机器人日志，并定位原因（refreshToken 失效、`runtime.java_opts` 与 jar 不匹配、或内存不足被杀）。`/start` 会对每台机器人复核两遍，第二遍仍然存在才算启动成功，因此这种情况会被如实报告。

  其他情况请见 [故障提示与排查](../troubleshooting/troubleshooting-zh_cn.md)。

## 12. 配置 systemd 服务（可选，推荐）

配置 systemd 服务后，你可以：
* 设置本程序为开机自启动。
* 使用 `systemctl` 命令控制本程序。
* 确保当程序意外被停止/崩溃时，能自动停止全部运行中的钓鱼机器人，不会留下孤儿进程。

创建服务：
```bash
sudo nano /etc/systemd/system/minecraft-fishingbot-manager-for-discord.service
```
写入以下内容（根据实际情况修改，你可以询问 AI）:
```ini
[Unit]
Description=minecraft-fishingbot-manager-for-discord
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/fishingbots
ExecStart=/fishingbots/minecraft-fishingbot-manager-for-discord
Restart=on-failure
RestartSec=10
# 当程序因不可恢复的错误而主动退出时（如 Discord 机器人 token 无效），将会以退出码 1 退出。
# 此时不再自动重启，变为 failed 并等待管理员手动处理
RestartPreventExitStatus=1
# 当程序退出时，会清理整个 cgroup，确保停机时释放游戏账号
KillMode=control-group
TimeoutStopSec=60

[Install]
WantedBy=multi-user.target
```
启用本服务：

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now minecraft-fishingbot-manager-for-discord
```

常用命令：

* 看状态
```bash
systemctl status minecraft-fishingbot-manager-for-discord
```
* 看实时日志
```bash
journalctl -u minecraft-fishingbot-manager-for-discord -f
```
* 重启本程序（一般是改完配置后使用，因为配置只在启动时读一次）
```bash
sudo systemctl restart minecraft-fishingbot-manager-for-discord
```
* 停止本程序并清理全部机器人
```bash
sudo systemctl stop minecraft-fishingbot-manager-for-discord
```

此时机器人已经被正确部署并正常运行，之后遇到错误请先看 [故障提示与排查](../troubleshooting/troubleshooting-zh_cn.md)。
