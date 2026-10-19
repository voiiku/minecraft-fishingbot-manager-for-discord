# Deployment Tutorial from Scratch
**English** | [简体中文](tutorial-zh_cn.md)

Tip: if any step is unclear, send this document's link to an AI assistant and ask for help.

## Contents

- [Deployment Tutorial from Scratch](#deployment-tutorial-from-scratch)
  - [Contents](#contents)
  - [1. What you need before you start](#1-what-you-need-before-you-start)
  - [2. Prepare the Linux server](#2-prepare-the-linux-server)
    - [Install screen](#install-screen)
    - [Install a JRE (Java 25 recommended)](#install-a-jre-java-25-recommended)
    - [Create the working directory (using `/fishingbots` as the example)](#create-the-working-directory-using-fishingbots-as-the-example)
  - [3. Create a Discord server](#3-create-a-discord-server)
  - [4. Create a Discord application](#4-create-a-discord-application)
  - [5. Add the bot to your server](#5-add-the-bot-to-your-server)
  - [6. Put the project files in place and start once to generate the config template](#6-put-the-project-files-in-place-and-start-once-to-generate-the-config-template)
  - [7. Edit the config for your setup](#7-edit-the-config-for-your-setup)
  - [8. Start again and confirm the Discord connection](#8-start-again-and-confirm-the-discord-connection)
  - [9. Make yourself an admin (skip if you already did this in step 7)](#9-make-yourself-an-admin-skip-if-you-already-did-this-in-step-7)
  - [10. Look at the help in Discord and add your first bot](#10-look-at-the-help-in-discord-and-add-your-first-bot)
    - [10.1 Create the first bot](#101-create-the-first-bot)
    - [10.2 Log in to the first bot](#102-log-in-to-the-first-bot)
  - [11. Check the bot status](#11-check-the-bot-status)
  - [12. Configure a systemd service (optional, recommended)](#12-configure-a-systemd-service-optional-recommended)

## 1. What you need before you start

You need the following before you start:

* A Discord account ([register here](https://discord.com/register) if you do not have one yet).
* A Linux machine: 64-bit Linux, at least 1 GB of memory recommended.
* A working network: if your machine cannot reach `discord.com` directly, install a proxy (TUN mode recommended) and configure a node. Search online or ask an AI assistant for instructions.

## 2. Prepare the Linux server

```bash
# Connect to your Linux server first (ask an AI assistant how to connect to your cloud server)
ssh user@address
```

### Install screen

```bash
sudo apt install screen   # Debian / Ubuntu
sudo yum install screen   # RHEL / CentOS
```

### Install a JRE (Java 25 recommended)

```bash
sudo apt install openjdk-25-jre-headless    # Debian / Ubuntu
sudo dnf install java-25-openjdk-headless   # RHEL / CentOS
```

```bash
java -version     # Confirm it worked (printing the version you installed is enough)
```

### Create the working directory (using `/fishingbots` as the example)

```bash
# Create the working directory
sudo mkdir -p /fishingbots && sudo chown "$USER" /fishingbots
cd /fishingbots

# Download FishingBot.jar
curl -LO https://github.com/MrKinau/FishingBot/releases/download/v2.13.15/FishingBot-2.13.15.jar
```

## 3. Create a Discord server

A Discord server is the group chat here: commands are sent in its channels and answered there. To avoid confusion, this guide calls a Discord server a "server" throughout.

1. Open <https://discord.com/> or the client and log in.
2. Click the `+` at the bottom of the server list on the left, choose to create your own, select "For me and my friends", and create the server.
3. Turn on developer mode: the gear icon at the bottom left (User Settings), then Advanced, then Developer Mode.

## 4. Create a Discord application

The bot in your server is a Discord application underneath, so create one first.

1. Open <https://discord.com/developers/applications>, click `New Application` in the top right, give it any name you like, and click `Create`.
2. Open `Bot` on the left, click `Reset Token`, and copy the string that appears. Keep it — you will need it later.
   * It is shown only once. Treat it as your bot's password and never send it to anyone.
   * If you lose it or it leaks, come back to this page and click `Reset Token` again (the old token stops working immediately).
   * Leave every Privileged Gateway Intent off. This project only uses slash commands, so no privileged intent is needed.
3. Under `OAuth2` on the left, open `URL Generator`:
   * Under Scopes, tick `bot` and `applications.commands`;
   * Under Bot Permissions, tick `View Channels`, `Send Messages`, `Embed Links`, `Attach Files`, `Read Message History`;
   * The Generated URL at the bottom of the page is the invite link. Keep it — you will need it later.
4. Under `General Information` on the left, keep the Application ID — you will need it later. It is the bot's user ID.

## 5. Add the bot to your server

1. Open the invite link from step 4 in a browser.
2. In the Add to Server dropdown, select the server you just created, continue, and complete the authorisation.
3. Back in Discord the bot appears in the member list, offline (the program is not running yet).

## 6. Put the project files in place and start once to generate the config template

1. Place a 64-bit Linux executable in your working directory. If building from source, use the release target
   `x86_64-unknown-linux-musl`.
   ```bash
   cd /fishingbots
    ./minecraft-fishingbot-manager-for-discord
   ```
The program looks for its config beside the executable. On the first run it writes
`minecraft-fishingbot-manager-for-discord.conf.toml` there, asks you to edit it, and exits.

## 7. Edit the config for your setup

**Config file name: minecraft-fishingbot-manager-for-discord.conf.toml. Its comments explain every field; if something is unclear, ask an AI assistant to interpret it for you.**
```bash
chmod 600 minecraft-fishingbot-manager-for-discord.conf.toml    # The file holds a token; keep it readable by the owner only
nano minecraft-fishingbot-manager-for-discord.conf.toml         # Or vim
```
Edit it to match your setup.

Two things to remember:

* To change how the fishing bots behave, edit the fields under `[fishingbot]` in this config. Do not edit the `conf.json` file itself — your changes would be overwritten at startup.
* After every config change you must restart this program for it to take effect.

## 8. Start again and confirm the Discord connection

```bash
./minecraft-fishingbot-manager-for-discord
```

The terminal should print lines like these:

```
program log: /fishingbots/logs/2026-10-03-13-10-05.log
config loaded: /fishingbots/minecraft-fishingbot-manager-for-discord.conf.toml
process PID: 12345
connected to Discord (application ID 123456789012345678)
invite link: https://discord.com/oauth2/authorize?client_id=...&scope=bot+applications.commands&permissions=60416
gateway watchdog ready: judging window 30 s, probe every 10 s, first handshake timeout 60 s
```

If the bot's avatar in Discord turns from grey to online, or the terminal says "connected to Discord", the bot is online.

## 9. Make yourself an admin (skip if you already did this in step 7)

1. Press `Ctrl+C` to stop the process, then edit `minecraft-fishingbot-manager-for-discord.conf.toml`.
2. In a Discord channel, send `/whoami`. It replies with your user ID (for example `123456789012345678`).
3. Add it to the config:
   ```toml
   [admins]
   "123456789012345678" = "Your nickname"
   ```
   * The nickname is only used by `/show admins`.
   * You can add several admins.
4. Start the program again
   ```bash
   ./minecraft-fishingbot-manager-for-discord
   ```

## 10. Look at the help in Discord and add your first bot

In your Discord server, send `/help` to see the help text defined in the config (you can change it).
If the bot does not answer, look at the newest file under `logs`, and check [Troubleshooting](../troubleshooting/troubleshooting-en_us.md).

### 10.1 Create the first bot

In Discord, send `/bot add` with the bot name `duck` (just an example name).
The reply should look like this:
```
Bot 'duck' added!
Note: configure its token before it can start.
Use /bot login to log in online
Only if /bot login fails should you set the token by hand with /token
```

This creates a directory on the Linux server: `/fishingbots/bots/<your Discord ID>/duck/`.

### 10.2 Log in to the first bot

**Recommended:** in your Discord server, send `/bot login <your bot name>`. It replies with a message only you can see, containing a Microsoft login link and a code. Open that link in a browser, enter the code, and sign in with the Microsoft account you want to fish with. When the authorisation is done it reports "login successful", the credential (`refreshToken`) is saved on the server, and the bot is started for you.

**Manual method** (only if `/bot login` fails):

```bash
# Run this on your own computer (it needs a Java runtime)
java -jar /your/path/FishingBot-2.13.15.jar
# Replace `FishingBot-2.13.15.jar` and the path with the real names, or just try double-clicking the jar.
```
* When the window appears, click run. Since this is the first run, it asks you to log in through a web page; after a successful login a `refreshToken` file appears next to it.
* Open `refreshToken` and copy everything.
* Then use `/token` in your Discord server and paste that string into the dialog.
You can also drop the `refreshToken` file into the bot's directory yourself, renamed to the filename the config specifies (by default `refreshToken`).

## 11. Check the bot status

If you logged in successfully with `/bot login`, the bot should already be running. Otherwise, after `/token`, send `/start <your bot name>` in your Discord server; the bot sends two replies:

1. An acknowledgement: `Request accepted: /start duck. Any start operation will use the global queue. The complete result will follow in a separate message.`
2. A result reply (one of three):
   * the log confirms it joined: `Bot duck: start command sent, the session is up and the log confirms it joined`
   * the session survives the grace period but the log is not confirmed: `Bot duck: start command sent, the session is up but the state is not confirmed yet`
   * the session disappears before the grace period: `Bot duck: failed to start (usually because Microsoft throttles frequent logins, or because it ran out of memory)`

* `/start` waits for `timing.check_grace_ms` and then reports a conclusion from the session state and the log, instead of always replying "sent". The process may still change afterwards, so use `/show status` for the later state.

To check the status, send `/show status`. It returns a table of the running bots:

```
bot | server status | pause status | uptime
Your bot name | server online | fishing | 3 minutes
```
**Common problems**
* The status stays at "ready" or "initializing": the log keywords may not match your game server. Check `keywords.online` first; if only "reconnecting" is not recognised, check that `[i18n] language` is correct.

* The session appeared and then exited on its own: use `/log` to read the newest bot log and find the cause (an invalid refreshToken, `runtime.java_opts` not matching the jar, or the process being killed for lack of memory). `/start` checks each bot twice and only reports success if it is still there on the second check, so this case is reported honestly.

  For anything else, see [Troubleshooting](../troubleshooting/troubleshooting-en_us.md).

## 12. Configure a systemd service (optional, recommended)

With a systemd service you can:
* start this program automatically at boot;
* control this program with `systemctl` commands;
* make sure that when the program is stopped or crashes, every running fishing bot is stopped too and no orphaned process keeps a game account.

Create the service:
```bash
sudo nano /etc/systemd/system/minecraft-fishingbot-manager-for-discord.service
```
Paste this (adjust it for your setup; ask an AI assistant if you like):
```ini
[Unit]
Description=minecraft-fishingbot-manager-for-discord (Discord + FishingBot)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/fishingbots
ExecStart=/fishingbots/minecraft-fishingbot-manager-for-discord
Restart=on-failure
RestartSec=10
# When the program exits on its own because of an unrecoverable error (an invalid Discord bot token,
# for example) it exits with code 1. It is then not restarted automatically: the service goes to
# failed and waits for an administrator.
RestartPreventExitStatus=1
# When the program exits it cleans up the whole cgroup, so the game accounts are released.
KillMode=control-group
TimeoutStopSec=60

[Install]
WantedBy=multi-user.target
```
Enable the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now minecraft-fishingbot-manager-for-discord
```

Common commands:

* Check the status
```bash
systemctl status minecraft-fishingbot-manager-for-discord
```
* Follow the program log
```bash
journalctl -u minecraft-fishingbot-manager-for-discord -f
```
* Restart this program (usually after a config change, since the config is read only at startup)
```bash
sudo systemctl restart minecraft-fishingbot-manager-for-discord
```
* Stop this program and clean up every bot
```bash
sudo systemctl stop minecraft-fishingbot-manager-for-discord
```

The bots are now deployed and running. If something goes wrong later, start with [Troubleshooting](../troubleshooting/troubleshooting-en_us.md).
