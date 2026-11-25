# commands: the command layer and every command
[简体中文](commands-zh_cn.md) | **English**

* `src/discord_commands/` receives interactions and sends replies. Only `src/operations/` authorizes callers and targets. For `/sync-commands`, `operations::begin_command_sync` checks the admin and acquires the sync lock; Discord registration calls stay in the adapter.

## 1. The rules of the command layer

### 1.1 Why every command defers first

Discord requires an interaction to be answered within 3 seconds, otherwise the user sees "interaction failed"; and what
we have to do (start a JVM, wait in the batch queue, run `screen -list`, wait per bot and read logs) takes far longer than
that. `ctx.defer()` answers first, after which there is a 15 minute window (the lifetime of the interaction token).

These cases do not defer first:

* `/help` and `/whoami` are purely local text, so they answer without deferring;
* `/token` collects its input in a Modal, and a modal has to be the first response of an interaction, so it cannot
  defer either.
* `/sync-commands` replies immediately if it cannot acquire a sync permit, without calling Discord registration APIs.

### 1.2 Why long commands send two messages

After a `defer`, Discord has a compatibility behaviour: the first reply replaces that "thinking" placeholder and only
the second one onwards is a new message. So the long-running commands (the six `*all` and the single-bot `start`,
`stop` and `restart`) send two:
1. start commands first acknowledge that the request was accepted into the global queue; other long-running commands
   acknowledge that execution has started. This replaces the "thinking" placeholder so the user sees progress at once;
2. then, once it is done, `ctx.say(<the result of Report::render()>)`, which is the real new message.

Query commands (`/show status`, `/show ...`, `/log`) send only the first one: the result replaces the placeholder directly.

#### 1.2.1 Shared replies

* `run_control_with_notice` defers and prepares the operation on the blocking pool. Denial or preparation failure sends only an error. On success it sends the progress notice before executing and sending the final report. Failure to deliver the notice drops the pending work. `/startall` estimates and submits the same candidate snapshot.
* `/bot add|del|rename` and text-only `/show` queries share `run_blocking_and_reply`: defer, run the operation supplied by the command on the blocking pool, then send a single result. `run_async_and_reply` handles async operations.
* `/log`, `/show botconfig`, `/bot login` and `/token` retain their attachment, private reply or modal flows.

### 1.3 Sensitive content: ephemeral and Modal

* Ephemeral (visible to the caller only): `/log` uses it for its result, so log contents are not shown to the channel.
* Modal (a pop-up): `/token` uses it to collect the token. The reason is that every call of a slash command creates a
  message in the channel (Discord's `CHAT_INPUT_COMMAND` message type is exactly that) and the argument values live in
  that message; `ephemeral` governs the reply only and not the invocation itself. A modal's input goes through the
  `MODAL` callback and never enters the channel.

The full path of `/token`: send the modal (`TokenModal::execute`) -> wait for the submission -> poise acknowledges the
submission for us (closing the modal) -> parse it into a struct -> `spawn_blocking` writes the file -> reply with
`CreateReply::ephemeral(true)`. The modal took the place of the first response, so that last step is a followup
message.

`/bot login` uses two private replies: the first one (a
login link and a device code) cannot be known until the JAR prints it, so the command layer cannot send a fixed
progress acknowledgement first the way other commands do. Its shape is: `defer_ephemeral` -> run the first stage
(launch the session, wait for the device code) -> send the first reply -> run the second stage (wait for the credential
to land on disk, or time out) -> send the second reply. Details are in [section 4.6](#46-bot-login).

One detail that is easy to trip over: `defer_ephemeral()` only pins down that "thinking..." placeholder response. In
poise, replies after a defer are **followups**, and their ephemeral flag comes from the **command attributes**
(`#[poise::command(..., ephemeral)]`, `false` when omitted), not from the defer. So "visible to the caller only" has to
be declared explicitly once on the `CreateReply` with `.ephemeral(true)` (`/token` and `/bot login` both do that).

`/log` used to rely on `defer_ephemeral` alone (its replies did not explicitly carry ephemeral); it now declares
`.ephemeral(true)` on every reply, the same convention as `/token` and `/bot login`, so it no longer depends on the
framework's default.

### 1.4 Why every business call is wrapped in spawn_blocking

Synchronous business operations involving files, `screen` and `sleep` would occupy Tokio worker threads if run directly
in async code. At the blocking-pool boundary, the command layer moves owned parameters into
`tokio::task::spawn_blocking(...)`; borrowed game-command fragments are copied before dispatch and passed to the route
inside the pool. Login, logs and configuration attachments retain their separate response flows.

### 1.5 Registration and contexts

* The command set comes from `discord_commands::all_commands()`. At startup, only a missing `/sync-commands` entry is
  created in the configured scope, without replacing other commands there. Server bootstrap failures are logged and do
  not prevent other servers from being initialized.
* When `discord.guild_ids` is empty or omitted, an administrator runs `/sync-commands` to register the global set, then
  immediately clear guild commands in servers the bot belongs to. Failed global registration prevents cleanup. Global
  propagation can take up to an hour and commands may temporarily be unavailable.
* With a nonempty `discord.guild_ids`, synchronization updates every target server first. If any update fails, old
  global and other server commands are retained. Once all targets succeed, old global commands are removed, followed
  by commands in non-target servers the bot belongs to. Cleanup failures are reported and require a manual retry.
  Server commands are normally available immediately, though deletion of cached global commands may take longer.
* Synchronization replaces this application's commands in the target scope and deletes all its commands in old scopes,
  including manually added ones; other applications are unaffected. There is no timer or automatic retry. Restart and
  run the command after changing configuration; first configure an administrator ID.
* Command and option descriptions come from language texts; `/log`'s default line count is filled dynamically.
  `all_commands()` validates them before registration (section 1.7).
* Context aliases: `Context<'a>` (ordinary commands) and `ApplicationContext<'a>` (commands that need a Modal, since a
  Modal only exists in the context of slash commands).
* Only `slash_command` is used; there is no `!prefix` command, because that needs the `MESSAGE_CONTENT` privileged
  intent, which is one more step in the developer portal.
* Parent and subcommands: `/bot` and `/show` are `subcommands("...")` parents and do nothing themselves.

### 1.6 How errors are handled

* An `Err` text returned by the business layer is treated as something to show the user, and the command layer sends it
  out as it is (`operations` has already converted it).
* An `Err` a command function returns itself (a genuine surprise) goes to `main.rs`'s `on_error`: it logs with
  `log::error!`, then the command layer sends a generic ephemeral error message to the caller. Internal error details are
  not shown to users. Command panics use the same reply path.
* Framework-level errors (`FrameworkError::Setup`, a rejected token for example) are split by cause: for a wrong config
  (an invalid token, a bot that was never invited to a server, or an invite missing the `applications.commands`
  scope) the cause is written to the log and the process exits with code 1 (handed to systemd, and
  `RestartPreventExitStatus=1` in the unit keeps it from being restarted); everything else is treated as a disconnect,
  stopping every bot and rebuilding the connection.

### 1.7 Command descriptions, option descriptions and modal labels are multilingual too

The texts in the command layer come in two groups, with different origins and different timing:

* Command descriptions and option descriptions: from `config::Config::get().text().command_texts()`, validated and filled before
  registration. The `/log` line description uses a language method and `Config::get().constants.default_log_lines`.
* Progress acknowledgements, usage hints, attachment bodies and the reply log entry: the same `config::Config::get().text()`
  (`command_started_notice`, `command_ping_port_usage`, `command_log_attachment_body`, ...), read on every call.
* Modal titles, field labels and placeholders: read on the spot in `TokenModal`'s `create()` (a hand-written
  `impl poise::Modal`), on every modal.

Poise macro literals cannot follow the configured language, so command definitions omit descriptions and the language
table supplies them:

* Command and option descriptions: after `discord_commands::all_commands()` has built the command tree, the
  `command_texts()` table of [`minecraft_fishingbot_manager_for_discord::text`] fills `Command.description` and
  `CommandParameter.description` one by one (both fields are public in poise and can be written directly).
* The `/token` modal's title, field labels and placeholders: instead of `#[derive(Modal)]`, a hand-written
  `impl poise::Modal` (the texts come from `create()`, the values from `parse()` by `custom_id`; the shape is exactly
  what derive would generate).

The texts themselves live in the language files (`COMMAND_TEXTS` in `src/text/languages/zh_cn.rs` and `src/text/languages/en_us.rs`), where
`name_path` must match the command name here word for word (a subcommand is written as `"bot add"`). Adding or changing
a command means updating both languages.

#### 1.7.1 Command text validation

The command tree and selected language table must match exactly. Missing, duplicate or unknown commands or options,
whitespace-only descriptions and descriptions outside 1–100 UTF-16 code units produce an error while building the
framework, before registration. Tests also check completeness, the actual registration structure and lengths for both
languages. Command macros no longer hold English descriptions; `autocomplete`, `rename` and option choices remain in
the command definitions.

Discord's `description_localized` is not used: it lets Discord pick which copy to show by client language, which is a
different mechanism from this project's "one language per process", and mixing them would produce an English
description with a Chinese reply.

## 2. The command list and their options

* `/help`, `/whoami`: no options, no defer, one reply.
* `/show status`, `/checkserver`: no options, `defer`, one reply.
* `/ping`: options `host?` and `port?` (`i64`), `defer`, one reply.
* `/show botconfig|mybots|allbots|admins`: no options, `defer`, one reply.
* `/bot add`, `/bot del`: option `name`, `defer`, one reply.
* `/bot rename`: options `old_name` and `new_name`, `defer`, one reply.
* `/bot login`: option `name`, `defer_ephemeral`, two replies (the first is the login link and the device code, the
  second is the result).
* `/token`: the options are the modal fields `bot_name` and `token`; no defer (the modal comes first), one reply marked
  ephemeral.
* `/log`: options `name` and `lines?` (`i64`), `defer_ephemeral`, one reply plus an attachment.
* `/start`, `/stop`, `/restart`, `/pause`, `/unpause`: option `name`, `defer`, two replies.
* `/command`: options `name` and the free text `command`, `defer`, two replies.
* `/commandall`: option `command`, `defer`, two replies.
* `/startall`, `/stopall`, `/restartall`: no options, `defer`, two replies.
* `/anti-afk`: required `command` choice (`on` or `off`), `defer`, one reply; admins only.

Numbers are the command layer's only conversion: Discord's integer options are signed 64-bit only, so the `/log` line
count and the `/ping` port are both declared as `i64` and checked in this layer (greater than 0, or 1 to 65535) before
being handed to the business layer (`usize`, `u16`); a failed check replies with a usage hint on the spot and passes
nothing down.

The free text of `/command` and `/commandall` uses a single `String` option: spaces are written directly (for example
`/command duck /home fishing`), with no quoting needed, because Discord's text options accept spaces in their own input
box. The only exception is pasting or typing the whole line at once: the client then splits on spaces and the extra
words try to match other options and fail (client behaviour, unavoidable). We do no processing at all (no unquoting, no
space merging, no `trim`), so whatever the user typed is what gets injected; the assembly and the "must not be empty"
check happen once in the business layer (`utils::console_command::ToCleanBotCommand`).

### 2.1 Autocomplete for the command options

Options such as the bot name `name` offer suggestions while the user types (a native Discord ability). The command-side
`operations/queries/list_bot_candidates.rs` queries permitted bots and their states; `discord_commands/autocomplete.rs` filters candidates by command scope,
builds display labels, and adapts the Discord interaction. Suggestions follow the meaning of the command rather than
always being "the running ones":

* `/stop`: everything permitted. A bot waiting to start can be cancelled by it too (the reply then says it was cancelled
  from the start queue), so the queued ones have to be among the suggestions; the display name carries the state suffix.
* `/pause`, `/unpause`, `/command`, `/restart`: permitted and running. For a target that is not running these
  operations are meaningless (`/restart` refuses outright and a stopped bot only answers "no need to stop").
* `/start`, `/bot del`, `/bot rename`, `/bot login`: permitted and neither running nor waiting in the start queue. Their
  precondition is that it has stopped (`/start` refuses a running or queued target; so does `/bot login`, which also
  refuses while "the session of a previous login is still open").
* `/log`: everything permitted. Historical logs are useful for both, and the display name gets a "(running)", "(queued)"
  or "(stopped)" suffix.
* `/bot add`, `/token`: no suggestions. `/bot add` wants a name that does not exist yet (nothing to list) and `/token`
  is a Modal, which Discord does not support suggestions for.

Details: the permission rule matches the server side (admins get every bot, everyone else the ones under their own
`bots/<user ID>/`); the state comes from one `screen -list` snapshot plus one start-queue lookup (every keystroke produces
an autocomplete interaction, so it must not probe per bot); at most `runtime.max_autocomplete_choices` entries (default
25, Discord's hard limit, enforced by the candidate logic in `src/discord_commands/autocomplete.rs`), with the running ones first
and the queued ones next. The suggestions are only suggestions: the user can still type any name, and ownership plus
whether the session is running are decided by the `operations` functions as before.

## 3. Single-bot control: `/start`, `/stop`, `/restart`, `/pause`, `/unpause`, `/command`, `/log`

Only `operations` resolves and authorizes the bot. `/token` checks after modal submission, before writing credentials. Autocomplete suggests only visible names; it does not grant access.

### 3.1 The command list

* `/start <bot name>`: `single_bot_control::start_single_bot(name, caller_id)`. Launches the session and checks it twice (it counts as
  started only when it is still there on the second check, see section 3.2), then replies with the verification result:
  joined, not currently joined, or an issue.
* `/stop <bot name>`: `single_bot_control::stop_single_bot(name, caller_id)`. Stops the session (releasing the game account); the stop is
  re-checked on the spot and it replies "stopped successfully".
* `/restart <bot name>`: `single_bot_control::restart_single_bot(name, caller_id)`. Stops and relaunches an existing session, then reports
  confirmed server entry, not currently joined, or the failure reason.
* `/pause <bot name>` and `/unpause <bot name>`: after authorization, both call `runtime::bot_instances::fishing_pause::set_fishing_pause`.
  They request opposite pause states but share command delivery and `/status` confirmation.
* `/command <bot name> <command>`: `single_bot_control::send_single_bot_command(name, args, caller_id)`. Forwards an
  in-game command.
* `/log <bot name> <lines>`: `log::bot_log_view(&bot_dir_entry, line_count)`. The reply is ephemeral (visible to the
  caller only).

`name` is **one name** today: taking several bots at once (`/start duck duck2`) is not implemented yet (the assessment
and the way to do it are recorded in [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 8).

After sending `/pause` or `/unpause`, the manager sends `/status` and checks only log output appended after the recorded
offset. Manager commands and queries for the same bot are serialized. The query waits up to 2 seconds and scans the
last 100 new lines. A truncated or replaced log is an error; a missing response is a query timeout, and an unavailable
response is reported as an issue. The manager does not store state.

The result confirms the state at query time, not what caused it. "Not paused" does not guarantee a connection or active
fishing. The JAR must support `/status`, `/pause` and `/unpause`, and the log filter must not discard status responses.
Upstream responses have no request ID, so a late response to an earlier timed-out query or an external manual query
can be indistinguishable from the current response.

### 3.2 The fixed steps of the three action functions

`start_single_bot`, `stop_single_bot` and `restart_single_bot` share one structure:

1. Each business command resolves the target and checks owner or administrator access once with `permitted_bot_dir_entry`. The entry
   carries the owning Discord user ID, the bot name (cleaned), the path and the token status.
2. Pre-checks for the action:
   * `/start`: a `bot_token_status == BotTokenStatus::Missing` in the entry is refused outright (without a token the
     login must fail); a running one is refused too (launching again would take the old session's place).
   * `/stop`: when the session is not running it replies "no need to stop" and skips the stop flow.
   * `/restart`: a target that is not running is refused outright (replying "bot X is not running, no need to
     restart"), because the meaning of `/restart` is to restart a running one, and starting one for the user would hide
     the information "when did it disappear"; a target without a token is dropped by the restart flow before it stops
     anything and noted in the failure details, because a restart stops and then starts, a target without a token cannot
     start, and a pointless stop would take an account that is still fishing offline.
3. Call the corresponding operation:
   * `/start` goes through `StartupQueue::get_or_init().start_bots(std::slice::from_ref(&bot_dir_entry))`;
   * `/stop` calls `stop_single_bot_session(&bot_dir_entry)`, which returns success or the stop error;
   * `/restart` goes through `restart_bot_sessions(std::slice::from_ref(&bot_dir_entry))`.

   Adjacent launches are separated by `[startup_queue] bot_login_interval`. After all bots in the request are
   launched it checks once immediately and once after `timing.check_grace` (one wait per start request). The session
   must still exist and not be dead on the second check; see [runtime-en_us.md](../runtime/runtime-en_us.md) section 4.
4. Startup verification returns each bot's `BotStatus` or an issue. Single-bot commands reply directly; batch commands summarize the same results in order:
   * `/start` uses `/status` `joined` to report joined or not currently joined. Unavailable status, query errors and a
     vanished session are reported as issues; an abnormal status does not automatically stop the session.
   * `/stop` replies `Bot 'X' stopped successfully`. The stop flow re-checks on the spot that the session is gone (see
     `runtime::bot_instances::stop::stop_bot_sessions`), so this one can claim success.
   * `/restart` confirms the old session stopped and reports whether the new session's server entry was confirmed.
     Stop failures, start failures, cancellation and verification errors retain their respective reasons.

### 3.3 The details of `/command`

* The command fragment is passed down by the command layer as it is (no unquoting, no space merging, no `trim`, see
  [section 2](#2-the-command-list-and-their-options)); this function assembles it into one command with
  `to_clean_bot_command` (`ToCleanBotCommand`) (joined with single spaces, and `错误:发送的指令不能为空！` when it is
  empty), exactly once.
* The echo uses that assembled command.
* `send_single_bot_command` checks owner or administrator access, assembles the command and requires the session to be running
  (the token check is in `/start`).
* A session that is not running is refused, because a command cannot be pushed into a session that does not exist.
* The injection is done by `runtime::bot_instances::screen::send_command_to_single_bot` (a screen primitive that validates nothing; it appends a
  newline at the end, which is what makes `screen` submit the line).

### 3.4 The details of `/log`

* The line count must be greater than 0 and is capped by `runtime.max_reply_log_lines` (one command must not pull a whole
  large log back).
* Only the newest log is read (`runtime::bot_log::latest_log_path`, which only accepts timestamp-shaped names):
  `create_bot_log_file` creates the file before every session launch, and the session appends through `grep -v -E` and
  `tee -a`.
* FishingBot's own log is written to `raw/log0.log` beside it (a symlink to `/dev/null`, so its contents are
  discarded), which means there is only one log on disk and `/log` cannot read back unfiltered spam. Details in
  [runtime-en_us.md](../runtime/runtime-en_us.md) section 7.
* Reading uses the sliding window of `runtime::log_files::read_last_lines`, so however large the log is only the last N lines are
  kept.
* The raw log text goes into `Report::content` (a whole block), so that no line gets a list prefix; the title carries
  the log file name.
* The reply uses `defer_ephemeral` and every reply (the usage hint, the failure text and the body with the attachment)
  carries an explicit `.ephemeral(true)`: the whole reply, attachment included, is only visible to the caller. Why it
  has to be declared explicitly (and cannot rely on the defer alone) is in
  [section 1.3](#13-sensitive-content-ephemeral-and-modal).

## 4. Account management: `/bot add`, `/bot del`, `/bot rename`, `/bot login`, `/token`

The entry points are `add_bot`, `delete_bot`, `rename_bot` and `update_bot_token`. New accounts belong to the caller; existing accounts require ownership or administrator access. Login uses `begin_bot_login` and `finish_bot_login` so the device code can be sent privately first.

### 4.1 The command list

* `/bot add`: `accounts::add_bot(raw_bot_name, caller_id)`. Available to everyone, and it only creates inside the caller's
  own user directory.
* `/bot del`: `accounts::delete_bot(name, caller_id)`. The creator or an admin.
* `/bot rename`: `accounts::rename_bot(name, new_name, caller_id)`. The creator or an admin; not while running.
* `/bot login`: `login::begin_login(&bot_dir_entry)` and `login::await_login(prompt)` (two stages, called by
  `operations::accounts::begin_bot_login` and `operations::accounts::finish_bot_login`, see section 4.6). The creator or an admin; not while running, and
  both replies are visible to the caller only.
* `/token`: `token::update_bot_token_file(&bot_dir_entry, raw_token)`. The creator or an admin; the reply is visible to
  the caller only, and not while running.

`/bot add`, `/bot del`, `/bot rename` and `/bot login` are Discord subcommands (typing `/bot` in the client shows the
four subcommands); `/token` is a separate top-level command that collects its input in a modal (see section 5).

### 4.2 `/bot add`

* Both the name check (`utils::bot_name::ToCleanBotName::to_clean_bot_name`: an illegal character is an error instead of being
  filtered out) and the server-wide duplicate check live in `runtime::bot_instances::bot_dirs::create_bot_dir`, which returns the entry it created
  on success (the reply and the background log both use the `bot_name` from the entry).
* The directory always lands under the caller's own user directory: the path is derived from the caller's user ID, so
  the caller cannot write into someone else's directory.
* The reply clearly points out that "it still needs a token before it can start" and gives the next step: `/bot login`
  is recommended, with `/token` only as the manual fallback when that fails (the same wording is in the example
  config's `/help` text).

### 4.3 `/bot del`

1. The business command calls `permitted_bot_dir_entry` to locate the directory and check owner or administrator access (`find_by_name` first,
   then ownership); when it cannot be located or is not permitted it answers as not found and never reveals that "this
   bot belongs to someone else".
2. Deleting is refused while the session is running (`refusal_if_not_stopped`, with the wording from
   `runtime::bot_instances::bot_sessions`), otherwise it would delete the data directory the bot is writing to.
3. The directory is removed with `fs::remove_dir_all`, and both success and failure are written to the background log.

### 4.4 `/bot rename`

The order is deliberate; do not reshuffle it:

1. Resolve the old name and check owner or administrator access through `permitted_bot_dir_entry` (admins may cross directories).
2. Check that the new name is legal and differs from the old name.
3. A running session is refused outright (`refusal_if_not_stopped`, the same rule as `/bot del` and `/token`): a running
   process keeps writing to the old directory and renaming would lose the logs from then on. Stop it first with
   `/stop`.
4. The server-wide duplicate check (`is_bot_name_taken`): if the name is taken it is an error, and no other name is
   picked for you.
5. `fs::rename` renames inside the same user directory (the old and the new directory belong to the same account).

### 4.5 `/token`

* The input comes from a modal, not from command arguments: a slash command invocation appears in the channel (with the
  argument values in it) and a refreshToken is sensitive. A modal's input goes through the `MODAL` callback and never
  enters the channel; after the submission the business layer still receives a plain piece of text (`raw_token`) and
  does not rewrite it before writing. Details in [section 1.3](#13-sensitive-content-ephemeral-and-modal).
* It only writes the file and does not validate the token: what goes in is for FishingBot to judge.
* Changing the token of a running account is forbidden: a running JVM reads the token file once at startup, so changing
  it does not take effect and easily makes people believe it was replaced. Stop it first with `/stop` (the same rule as
  `/bot del` and `/bot rename`).
* The file name is `runtime.token_file` and it is written inside the bot directory; the path convention comes from
  `runtime::bot_instances::bot_dirs::token_file_path` and `fs::write` creates the file when it is missing and overwrites it when it exists.
* The token file is what decides whether a bot can start: `bot_token_status` in the entry (`BotTokenStatus::Ready` or
  `BotTokenStatus::Missing`), decided in exactly one place, `BotTokenStatus::from_dir`.
* The everyday way to get a credential is `/bot login` (section 4.6); `/token` is the fallback when that does not work.

### 4.6 `/bot login`

In one sentence: it uses the **Microsoft device code login** built into FishingBot.jar, hands the login link and the
device code the JAR printed to the caller, and lets the JAR write the credential into the bot directory itself.

Why this project does not implement the Microsoft login itself: the credential file is a serialised format written by
the JAR (MSAL) and it does not accept one we made. Its measured behaviour is: when the file `-refreshToken` points at
exists, it is used directly; only without a file does it run the device code flow.

#### 4.6.1 The flow (two stages, with the user's own time in between)

Stage one (`login::begin_login`, a few seconds):

1. A running session is refused: with a credential it is the usual "it is running, stop it first", and without one it
   is "a login is already in progress" (that session is most likely the previous login that has not finished, so the
   user should not `/stop` their own login).
2. An existing credential is moved to `<credential file name>.renewing` first: otherwise the JAR sees it, uses it
   directly and never prints a device code, which turns `/bot login` into "nothing happened". It also deals with
   leftovers from an interrupted attempt at the same time (both present -> delete the leftover; only `.renewing` ->
   put it back).
3. Launch the session: the same path as `/start` (`runtime::bot_startup::StartupQueue::launch_bot_to_login`: create the log -> prepare
   FishingBot's raw log directory -> `screen -dmS`), except that it does not refuse over the credential status - step
   zero of `/start` is exactly refusing a bot without a credential, which is why this flow needs an entry point of its
   own. On failure the old credential is put back and the error is returned.
4. Read the tail of the log waiting for the device code line (upstream message `auth-create-refresh-token`, which is
   the English text verbatim in both language files, see
   [BOT_COMMANDS_AND_MESSAGES-en_us.md](../../refs/BOT_COMMANDS_AND_MESSAGES-en_us.md) section 3.4), for at most 60
   seconds. If it does not arrive, stop the session, put the old credential back, and report either "the session started
   and exited" (reusing the conclusion of `/start`) or "the login prompt never arrived".

Without a valid credential the JAR just waits for you to log in and **does not connect to the game server**, so a login
neither occupies the game account nor leaves a "joined and dropped" trail in the game.

Stage two (`login::await_login`, at most 10 minutes): it polls for the credential file to appear - **the file is the
source of truth** and it does not parse the "authorised successfully" line (upstream changing that sentence does not
affect us). Three ways to end:

* The file appears -> success: delete the `.renewing` leftover, tighten the credential file's permissions to `0600`
  (it is equivalent to the account password) and reply "login successful, it is joining the server now". **The session
  stays where it is**: the login session is the bot's normal session, the JAR keeps joining the server and fishing on
  its own once authorised, and `/show status` and `/stop` all keep working during it.
* It reads `auth-could-not-get-refresh-token` -> Microsoft refused: stop the session (it would retry forever), put the
  old credential back, and reply with a failure that suggests trying again or using `/token`.
* The session disappears early -> report "failed to start" (reusing `start_failed`, the same sentence as a batch
  start). On a real machine the typical cause is Microsoft deciding the requests are too frequent and returning an
  empty token, after which the JAR ends the JVM itself.

Timeout (10 minutes): check the credential once (the user may have finished in the last few seconds), and if it is not
there, stop the session - the device code has expired and that JVM would only keep asking Microsoft for authorisation;
after stopping, check the credential once more (the JAR may have finished writing it exactly when it got the Ctrl+C),
and only when it is still missing reply with the timeout and put the old credential back.

If screen lookup fails during login, the error is returned and the session is stopped if possible. When stopping cannot
be confirmed, the old `.renewing` credential backup is retained to avoid overwriting a potentially running session;
stop failures are logged.

#### 4.6.2 A few deliberate choices

* Two replies: the first (a link and a code) has to reach the user **immediately**, while the credential only lands on
  disk after they authorise, so the command layer cannot send a fixed progress acknowledgement first the way other
  commands do. The shape at the command layer and the ephemeral convention are in
  [section 1.3](#13-sensitive-content-ephemeral-and-modal).
* The 10 minute wait: a Microsoft device code is valid for about 15 minutes and Discord gives one interaction a 15
  minute window, so 10 minutes leaves a margin.
* Stopping the session goes through the `/stop` path (Ctrl+C first, re-check, force kill when needed) rather than
  killing it outright: the JAR may only write the credential once it receives the signal.
* The device code line stays in the bot log (valid for about 15 minutes, single use): ephemeral only guarantees that
  others cannot see it in Discord, so the bot directory itself is best kept readable by the deployment account only.
* Concurrent logins have **no limit**: logging several bots in at once starts several JVMs at the same time and Microsoft
  may rate limit it (one possible symptom is "it started and exited one or two seconds later"). For now, users should
  stagger logins themselves; consider a global concurrency limit only if deployments frequently encounter rate limits.

## 5. Batch commands: `/startall`, `/stopall`, `/restartall`, `/pauseall`, `/unpauseall`, `/commandall`

Each batch command calls its corresponding admin-only operation, e.g. `/startall` → `operations::all_bots_control::start_all_bots`. Non-admin requests are rejected before progress notices.

### 5.1 The command list

* `/startall`: `all_bots_control::start_all_bots(caller_id)`, covering every bot with a configured token that is not running.
* `/stopall`: `all_bots_control::stop_all_bots(caller_id)`, covering every running bot (including leftover processes whose token is no
  longer valid).
* `/restartall`: `all_bots_control::restart_all_bots(caller_id)`, covering every bot with a configured token that is running.
* `/pauseall` and `/unpauseall`: `all_bots_control::pause_report::set_all_bots_fishing_pause` selects the same targets as `/commandall`,
  calls `set_fishing_pause` for each bot and collects the results.
* `/commandall <command>`: `all_bots_control::send_all_bots_command(args, caller_id)`, covering every running bot.

`/anti-afk <on|off>` uses the separate `operations::set_anti_afk` entry and is also admin-only. `on` starts the worker or reports that it is already running; `off` stops and joins it or reports that it is not running. It uses `[anti_afk]` input and intervals, matching every screen session whose name contains `runtime.screen_prefix` (including sessions outside this project). Starting confirms thread creation only, not a matching session or input delivery; `/show status` reports thread liveness. It does not start automatically after a process restart.

Each batch operation checks administrator access before providing a progress notice, querying screen or taking action.
They also pick their own titles (`/stopall` uses the command title; when the
process receives an exit signal, `runtime::shutdown::exit_on_shutdown_signal(notice_ctx)` assembles the same details on the spot
and only swaps in the "shutdown signal received, wrap-up" title: that report goes into the program log and is also
sent to every server's notice channel). For
comparison: a single-bot command's permission is decided inside each business
function by directory ownership, see
[section 3](#3-single-bot-control-start-stop-restart-pause-unpause-command-log).

### 5.2 Which directories are taken

Which batch is taken is decided by each command (`all_bot_dir_entries()` or `token_ready_bot_dir_entries()`), and then
grouped with `runtime::bot_instances::bot_sessions::split_bot_entries_by_is_running(&[BotDirEntry])` (one `screen -list` snapshot):

* `/startall`: `all_bot_dir_entries()`, split into two groups by token readiness first. An account without a token is
  not a failure but is counted as skipped, so everything is taken and then grouped.
* `/restartall`: `all_bot_dir_entries()`, taking only the running group. Restarting a target that is not running equals
  starting it (that is `/startall`'s job); targets without a token are dropped by the restart flow before it stops
  anything.
* `/stopall` and the process shutdown: `all_bot_dir_entries()`. After a token is deleted, a process launched earlier
  may still be running and holding the game account, and the shutdown has to be able to see it.
* `/commandall`, `/pauseall` and `/unpauseall`: `token_ready_bot_dir_entries()`, and the scope
  matches `/show status` (only bots with a configured token that are running).

`/restartall` deliberately does not filter by token: a restart stops and then starts, and a target without a token
cannot start (the login must fail), so a pointless stop would take an account that might still be fishing offline. The
filter therefore lives in the restart flow, before the stop, and those targets show up in the failure details as "token
file missing, left as it is" rather than disappearing silently.

### 5.3 The flow of each command

* `/startall` submits bots with a token and no session to the startup queue. The report lists verified starts and
  failures in ledger order. Already-running bots, missing tokens and cancellations are counted without per-bot details.
* `/stopall`: `runtime::bot_instances::stop::stop_all_bot_sessions()` returns the raw details (it is itself the combination of "ask the starter
  thread to launch nothing new", "clear the waiting list", `all_bot_dir_entries()`, grouping, stopping, and one more
  sweep for late sessions), and the report is assembled by this module's `all_bots_control::stop_all_bots(caller_id)`; it reports only
  successes and failures (its only count rows are "stopped" and "stop failed"; bots that were not running are not listed,
  while the cancelled entries from the waiting list go into `Report::skipped`, which is empty when nothing was queued;
  the batch that was already taken out and is being launched is reported by the `/startall` / `/restartall` reply that
  triggered it, as "starting was cancelled"; the "scanned bot directories" field is output only by `/show allbots`). When
  the process receives an exit signal, the wrap-up assembles the same
  details on the spot inside `runtime::shutdown::exit_on_shutdown_signal(notice_ctx)`, with the title swapped for "shutdown
  signal received, wrap-up"; the report is written to the program log and also sent to every server's notice channel,
  with a 30-second overall limit on that send (past it the send is dropped and the process exits; nothing is resent on
  the next startup).
* `/restartall`: only the running group of `all_bot_dir_entries()` is taken (a target that is not running is not a
  restart target); with none it replies `没有运行的机器人实例，无需重启`; otherwise it calls
  `runtime::bot_instances::restart::restart_bot_sessions` and then writes the target bots, sessions present after restart and restart issues with
  `Report::set_counts` (a target without a token appears in the restart failure details). Cancelled starts are listed
  under skipped. If a session already exists again before relaunch, the restart is counted as an issue.
* `/commandall`: the command fragment is passed through as it is by the command layer, and here it is assembled into
  one command with `to_clean_bot_command` (the same convention as `/command`) before being injected one by one.
  `runtime::bot_instances::bot_sessions::token_ready_bot_names_with_screen_sessions()` comes from one `screen -list` snapshot, including dead sessions, so the
  injection is done straight away without re-authenticating and re-probing each bot; the successes are counted one by
  one and the raw failure text goes into `Report::failed`.
* `/pauseall` and `/unpauseall`: use the same token and session filters, then call the single-bot pause or resume logic.
  Each target is queried with `/status`; unavailable, timed-out and failed queries are reported as issues.

### 5.4 Report conventions

* Batch commands output a `Report`: always a title, count rows and detail rows, where the count rows are generated
  by `Report::set_counts` uniformly in the form `label: N`.
* Replies have a length cap: `Report::render()` truncates line by line from the end according to
  `runtime.max_reply_chars` and notes how many lines were omitted. With many accounts the batch details reach the cap
  most easily (an overlong single Discord message fails to send outright).
* Start and restart checks pass when the session exists and the status query succeeds, not necessarily when joined. Timeouts and
  failures are counted as issues. Completed details distinguish joined and not currently joined; issue details retain
  their reasons. Stop results are based on session-exit verification.
* A command whose basic information is only known once the flow has run (`/startall`, `/restartall`) runs the flow first
  and then overwrites the count rows with `Report::set_counts(..)`. That is an explicit overwrite and does not stack on
  top of the rows the flow wrote.

### 5.5 Troubleshooting failures

If a process exits during startup, the result says "Bot X: failed to start"; check the logs for the cause. If memory is
insufficient, lower the memory allowance in `runtime.java_opts`; alternatively raise
`timing.check_grace` or `[startup_queue] bot_login_interval` (watching longer or signing in slower), or repeat the command later
(JVM memory usage improves over time).

## 6. Queries: `/show status` and `/show botconfig|mybots|allbots|admins`

Public queries call `show_status`, `show_my_bots`, `show_all_bots`, `show_admins` or `show_bot_config`. Only the personal list needs the caller ID.

### 6.1 The command list

* `/show status`: `show::show_status()` (using `show::show_status::status_row()`).
  The data comes from `screen -list`, each bot's newest filtered log, and one start-queue lookup (the queued bots get a
  `[Queued]` section below the report), and the reply is text (a table).
* `/show botconfig`: `botconfig::fishingbot_config_view()`. The data comes from `paths.fishingbot_config_file_name`
  (the copy every bot shares) and the reply is an attachment (a file name and the full text).
* `/show mybots`: `mybots::my_bots_list(caller_id)`. The data is the bots under the caller's own user directory, as
  text.
* `/show allbots`: `allbots::all_bots_list()`. The data is the bots under every user directory, as text.
* `/show admins`: `admins::admins_list()`. The data is the `[admins]` table from the config, as text.

### 6.2 `/show status`

* The report first shows whether the anti-AFK thread is running, even if no bots are running or queued; this does not confirm input delivery.
* The list is the intersection of "has a token" and "the session exists": read the bot directories once and split by token
  (the same scope as `/commandall`), then `show::show_status::status_row()` drops the ones that are not
  running using one `screen -list` snapshot. So everything in the table has a configured token and an existing session;
  a bot whose token file was deleted while the process is still running is not in the table (`/stopall` can still clean
  it up).
* Whether a bot is queued is asked by `/show status` itself, once, from the start queue (`StartupQueue::bots_in_queue`,
  the same wording as `/start` and `/stop`): a queued bot has no session yet, so it cannot be in the table above (no
  online / pause / uptime to report) and gets its own section below the report (`Report::queued`, with the section label
  `report_queued_label()`, `[Queued]`), one bot per line.
* A live session is queried for the five `BotStatus` fields: `joined`, `paused`, `no_rod_available`, `has_bobber`
  and `last_fish`. The first two are displayed as connection and pause status, the other booleans as `true`/`false`;
  `last_fish` is the Unix millisecond timestamp of the last fishing action, not necessarily a catch. Unavailable status
  reports an abnormal session, while timeouts and log errors are reported separately. A dead screen shell is reported
  as a stopped process.
* Uptime is formatted uniformly by `utils::uptime::format_uptime` as `1d-1h-5m`, so that no call site writes its own
  version.
* The output has one line per bot, which is easy to scan by eye. The table has no dedicated type: a row is built on
  the spot by `show::show_status::status_row()` and the header on the spot by `src/operations/queries/show_status.rs`, both as "a
  seven columns joined with ` | `", so the two must agree on the column order:
  * the column names (one set per language) live in `text`: `status_column_bot_name()`, `status_column_joined()`,
    `status_column_pause()`, `status_column_no_rod()`, `status_column_bobber()`,
    `status_column_last_fish()` and `status_column_uptime()`;
  * adding, removing or reordering a column means changing both places; nothing enforces the order at the type level,
    so the alignment rests on both sides reading the same column names and using the same separator (` | `, one space
    on each side);
  * a row is a string discarded right after rendering, so a snapshot cannot be rendered twice, and no getter or trait
    is needed for one table: `status_row` joins the cells it just computed;
  * why the rows live in `show_status` rather than in the general `text::report`: they need the session snapshot and the
    logs (both part of the running-state decision), so joining the cells there is simpler than introducing a snapshot
    type to consume.

### 6.3 `/show botconfig`

* What is sent is the text of the fishing config that `paths.fishingbot_config_file_name` points at (FishingBot's own
  `config.json`), not this project's `minecraft-fishingbot-manager-for-discord.conf.toml`: the point is to see what the fishing
  parameters are right now. That file is generated at startup by this program from `[fishingbot]` (the old file is
  deleted and a new one written, see [config-en_us.md](../config/config-en_us.md) section 3.2), so it is both "what the
  config says" and "what FishingBot is actually reading".
* Why an attachment: `config.json` easily exceeds Discord's 2000 character limit for a single message and the body would
  be truncated; an attachment gives the full text and naturally carries a file name (`config.json`).
* The content is not truncated: it is a structured file and cutting it in half invites misreading; when it exceeds the
  attachment limit (`Config::get().constants.max_attachment_bytes`, 8 MiB by default) it reports the error honestly and tells you to look at it on
  the server.
* When the file is missing or unreadable it replies with an error explaining why (the file is generated at startup by
  this program, so check `base_dir` and the write permission first) instead of silently returning nothing.

### 6.4 `/show mybots` and `/show allbots`

* Both share one rendering and one directory scope (both in `runtime::bot_instances::bot_dirs`, both sorted by bot name); the only
  difference is which directories are taken: `my_bots_list` goes through `bot_dir_entries_of_owner(user ID)` and
  `all_bots_list` through `all_bot_dir_entries()`.
* Each bot lists its name, its owning user ID (only meaningful for `allbots`) and whether a token exists. The token is
  only reported as present or not (`BotTokenStatus`); its contents are not shown and its validity is not checked
  online.
* The first row of `allbots` is the count row "scanned bot directories" (`count_scanned_dirs`, the number of entries
  `all_bot_dir_entries()` returned): that field is output only here, never in the stop reports or the process wrap-up.
* `allbots` is public and does not filter accounts by owner. Use `/show mybots` to list only your own accounts.

### 6.5 `/show admins`

* It prints `name : user ID` line by line, and the names come from the config only, with no online lookup at all.
* When the list is empty it gives a clear note (add a line `"user ID" = "name"` to `[admins]` in
  `minecraft-fishingbot-manager-for-discord.conf.toml`, with the user ID from `/whoami`) so that it does not look like an
  error.

## 7. General: `/help`, `/whoami`, `/ping`, `/checkserver`

* Code: `src/operations/queries/mod.rs` and `src/operations/queries/*.rs`
* No coupling to the other modules of this project: they do not read bot directories and do not touch `screen`
  sessions, so they can be used and tested on their own.
* These four queries are public and need no permission check. Other queries such as `/log` still check bot access.

### 7.1 The command list

* `/help`: `operations::queries::help()`, no network request.
* `/whoami`: `operations::queries::whoami(caller_id)`, no network request.
* `/ping [host] [port]`: `ping::ping(host, port)`, which does make a network request (a Minecraft server handshake).
* `/checkserver`: `checkserver::checkserver()`, no network request (it reads local resources).

`ping` and `checkserver` return the result of `Report::render()`; by the naming rule, one-off probes, checks and whole
pieces of text use a bare noun, see [architecture-en_us.md](../architecture/architecture-en_us.md) section 5.

### 7.2 `/help`

* It returns the `text.help` text from the config as it is (a triple-quoted multi-line string), so the wording can be
  changed without recompiling.
* It works in any channel (no private chat needed).
* The help menu and the commands actually supported can therefore drift apart: after changing a command remember to
  update the config, which the config file itself points out.

### 7.3 `/whoami`

* It returns the caller's own Discord user ID and gives the line that can be copied straight into `[admins]`.
* Its reason to exist is to untie the chicken-and-egg problem: configuring an admin means filling in your own user ID,
  and the usual ways to get it (developer mode, right-click the avatar, copy user ID) are hard to find for people who
  do not know Discord well.
* It has no permission requirement. `operations::queries::whoami` generates the text and the
  command layer does not build its own reply (see the hard convention in
  [section 1](#1-the-rules-of-the-command-layer)).

### 7.4 `/ping`

* Without arguments it uses the default target of the `[ping]` table (`default_ping_host` and `default_ping_port`); with
  arguments what was typed in the chat wins (an invalid port produces a usage error).
* Explicit hosts must resolve only to public addresses; the configured default host is trusted. Proxy Fake-IP DNS can
  cause a rejection; see [troubleshooting](../../troubleshooting/troubleshooting-en_us.md#ping-refuses-a-local-or-private-address).
* It does a Minecraft server list handshake (`craftping`), so what comes back is the game layer's answer: the player
  count and the maximum, and the latency of the "fishing host to game server" hop, rather than plain TCP
  reachability.
* The output is always three lines: `status: online`, `players: current/max` and
  `latency: N ms (from this fishing host to the game server)`; a failure to connect, a timeout and a protocol parse
  failure each get a readable message.
* DNS resolution, connection and handshake share `ping.timeout` (config validation guarantees it is not 0).
* An illegal address or port, a failure to connect and a timeout each get a readable message, and none of them panics.

### 7.5 `/checkserver`

* What it reports is the resources of the machine running this program (the machine the fishing process is on), not the
  game server.
* The data comes from `sysinfo`: CPU usage, used and total memory (MB or GB, chosen from the total), memory usage,
  disk and system uptime; this run's uptime is read from `/proc`, and the system time and time zone come from the
  host's `chrono::Local`.
* Failures to initialize system information or query disks or system uptime are returned with their reasons.
* CPU usage is computed from two samples taken a very short time apart: a single sample gives the average since boot,
  which stays high for a long time and looks like a machine at full load.
* The output is multi-line text with fixed fields, easy to screenshot, and it does not depend on a log file.

## 8. Related documents

* All documents: [docs index](../../index-en_us.md).
* Which layer the action enums and the permission check live in: [lib-en_us.md](../lib/lib-en_us.md).
* The start/stop waits and the two log lines: [runtime-en_us.md](../runtime/runtime-en_us.md) section 4 and section 7.
* The game-side config attachment: [config-en_us.md](../config/config-en_us.md) section 3.2.
