# commands: the command layer and every command
[简体中文](commands-zh_cn.md) | **English**

* Code: `src/commands.rs`, `src/commands/autocomplete.rs` and `src/commands/{general,show,bot_manage,token,log,bot_control,all_bots}.rs`
* Responsibility: read options, check numbers, `defer`, and send out whatever `handle_*` returned. Permissions,
  argument cleaning and business state checks all happen one layer down (`src/lib.rs` and the business modules).
* Hard convention: every command must end up in `lib::handle_*` (that layer declares the permission requirement and
  checks it once), and this layer must not build its own reply - even a command such as `/whoami`, which only reads the
  author's information, goes through `handle_whoami`. Skipping that step means skipping the permission check (there is
  no registry or framework hook to fall back on; this convention plus review are the guarantee).
* Relation to the business layer: `commands/bot_control.rs` (the command layer) only declares which commands exist,
  what the arguments look like and how to reply; the real action is in `bot_control/start.rs` (the business layer). The
  file names are deliberately the same on both sides so they can be read side by side.

## 1. The rules of the command layer

### 1.1 Why every command defers first

Discord requires an interaction to be answered within 3 seconds, otherwise the user sees "interaction failed"; and what
we have to do (start a JVM, wait in the batch queue, run `screen -list`, wait per bot and read logs) takes far longer than
that. `ctx.defer()` answers first, after which there is a 15 minute window (the lifetime of the interaction token).

There are only two exceptions:

* `/help` and `/whoami` are purely local text, so they answer without deferring;
* `/token` collects its input in a Modal, and a modal has to be the first response of an interaction, so it cannot
  defer either.

### 1.2 Why long commands send two messages

After a `defer`, Discord has a compatibility behaviour: the first reply replaces that "thinking" placeholder and only
the second one onwards is a new message. So the long-running commands (the four `*all` and the single-bot `start`,
`stop` and `restart`) send two:
1. start commands first acknowledge that the request was accepted into the global queue; other long-running commands
   acknowledge that execution has started. This replaces the "thinking" placeholder so the user sees progress at once;
2. then, once it is done, `ctx.say(<the result of Report::render()>)`, which is the real new message.

Query commands (`/show status`, `/show ...`, `/log`) send only the first one: the result replaces the placeholder directly.

#### 1.2.1 The five reply funnels (named after the kind of command, not after the number of replies)

Each command file picks exactly one of the five funnels below, and which one it picks follows from which kind of
command it is. `defer`, the progress acknowledgement and `spawn_blocking` live in the two private functions in
`src/commands.rs`, `run_and_reply` (run it, then reply with the result) and `run_and_reply_with_progress` (reply with
the progress acknowledgement first, then with the full result), so one change takes effect everywhere. What has a name
is "which kind of command", not "how many replies it sends". Every reply is recorded in the program log (`say_and_log`
and `log_reply` go through `LogText::CommandReplied`: who called it, which command, and what was answered), so when
troubleshooting "what did it actually answer" the log is the reference, see
[process-log-en_us.md](../process-log/process-log-en_us.md).

* `single_bot_control`: covers `/start`, `/stop`, `/restart`, `/pause`, `/unpause` and `/command`. The user gets a
  progress acknowledgement first and the full result once it is done.
* `single_bot_manage`: covers `/bot add`, `/bot del` and `/bot rename`. The user only gets the final result.
* `all_bots_manage`: covers `/startall`, `/stopall`, `/restartall` and `/commandall`. The user gets a progress
  acknowledgement first and the full result once it is done.
* `read_only_query`: covers `/show status` and `/show *`. The user only gets the final result.
* `server_probe`: covers `/ping` and `/checkserver`. The user only gets the final result (an asynchronous probe, so it
  never enters the blocking thread pool).

Three cases are outside these five, each for its own reason: `/help` and `/whoami` are purely local text (no `defer`);
`/log` and `/show botconfig` have to send an attachment themselves; and `/token`'s modal has to be the first response
of the interaction.

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

`/bot login` is the only command whose **two replies both get their content from the business layer**: the first one (a
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

The business layer is entirely synchronous and blocking (files, `screen`, `sleep`), and running it directly inside
async would occupy tokio's worker threads. The command layer uses `tokio::task::spawn_blocking(...)` uniformly, and
the closure has to be `Send + 'static` (it must not borrow `ctx`), so every command first takes out the values it needs
(`caller_id`, `String` arguments) and then moves them into the closure.

### 1.5 Registration and contexts

* The command set comes from `commands::all_commands()`, and `setup` registers it once with
  `poise::builtins::register_globally`: everything is registered globally (up to an hour to propagate on Discord's
  side). After a successful registration the leftovers are cleared from every server too - the set registered into a
  server earlier never disappears on its own, and the client lists both (the same command twice). The cleanup has a
  hard precondition: it first makes sure this same global set was already in place for a full hour before this run
  started (same names, and the youngest command Id old enough), because otherwise there would be a window with
  neither set visible. The decision reads the creation time out of the command Ids, so no state file is involved.
* A command description comes from the first paragraph of the function's doc comment (poise's rule: the macro does not
  accept `description = "..."`, only `description_localized`). That first paragraph is written in English (the same
  sentence as in the default language table) and the details go into the paragraphs after it. Why, how the length is
  counted and what the fallback language is: section 1.7.
* Context aliases: `Context<'a>` (ordinary commands) and `ApplicationContext<'a>` (commands that need a Modal, since a
  Modal only exists in the context of slash commands).
* Only `slash_command` is used; there is no `!prefix` command, because that needs the `MESSAGE_CONTENT` privileged
  intent, which is one more step in the developer portal.
* Parent and subcommands: `/bot` and `/show` are `subcommands("...")` parents and do nothing themselves.

### 1.6 How errors are handled

* An `Err` text returned by the business layer is treated as something to show the user, and the command layer sends it
  out as it is (`handle_*` has already converted it).
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

* Command descriptions and option descriptions: from the table `text::get().command_texts()` (which overwrites the one
  the macros generated, before registration), in effect at startup and registration time.
* Progress acknowledgements, usage hints, attachment bodies and the reply log entry: the same `text::get()`
  (`command_started_notice`, `command_ping_port_usage`, `command_log_attachment_body`, ...), read on every call.
* Modal titles, field labels and placeholders: read on the spot in `TokenModal`'s `create()` (a hand-written
  `impl poise::Modal`), on every modal.

Why the description texts cannot be written directly in `src/commands/*.rs`: poise's `#[poise::command]` and
`#[derive(Modal)]` macros only accept literals, so the descriptions are baked into the binary at compile time and
cannot be replaced by reading a config at run time. The way round it is to rewrite the texts once before registration
(and once only):

* Command and option descriptions: after `commands::all_commands()` has built the command tree, the
  `command_texts()` table of [`minecraft_fishingbot_manager_for_discord::text`] overwrites `Command.description` and
  `CommandParameter.description` one by one (both fields are public in poise and can be written directly).
* The `/token` modal's title, field labels and placeholders: instead of `#[derive(Modal)]`, a hand-written
  `impl poise::Modal` (the texts come from `create()`, the values from `parse()` by `custom_id`; the shape is exactly
  what derive would generate).

The texts themselves live in the language files (`COMMAND_TEXTS` in `src/text/zh_cn.rs` and `src/text/en_us.rs`), where
`name_path` must match the command name here word for word (a subcommand is written as `"bot add"`). Adding or changing
a command means updating both languages, and the overwrite test below lists whatever is missing.

#### 1.7.1 What the copy inside the macro is: the English fallback

The first paragraph of the `///` and an option's `#[description = "..."]` are the fallbacks that only show when the
table is missing an entry (the macro only accepts literals and they cannot be replaced). The rule is to always write
them in English, word for word identical to the sentence in the `en_us.rs` table, so the fallback can never be in the
other language and no second copy has to be maintained (that English sentence is the one in the default language table;
the default language is in [i18n-en_us.md](../i18n/i18n-en_us.md) section 4).

* Why English: the default language is English (`Language::default()`), and when the fallback matches it a missing
  entry cannot mix languages.
* The paragraphs after the first stay Chinese: they are notes for the maintainer and are not sent to users (poise only
  treats the first paragraph as the `description`; the rest become the local `Command::help_text`).
* Three tests pin this down (all in `src/commands.rs`):
  * `every_command_and_option_has_text_in_every_language`: the table is complete (anything missing is listed).
  * `fallback_texts_match_the_default_language_table`: the fallback in the macro equals the sentence in the default
    language table (word for word).
  * `every_description_fits_discords_100_character_limit`: a description is no longer than 100 characters (Discord's
    limit; option descriptions have no compile-time check).

poise's exact rules (read from `src/command/mod.rs` of `poise_macros` 0.7, not guessed):

* A doc comment is split into paragraphs on blank lines; the first paragraph becomes the `description` (line breaks
  inside it are joined into one space) and the rest become the `help_text`.
* A command description longer than 100 characters is a compile error (`slash command description too long`); option
  descriptions have no compile-time check, and the comment on that line in poise's source says it is left for a
  translation framework like fluent to fill in at run time, which is exactly how this project uses it, so the length
  has to be counted by hand (`/command` and `/commandall` once grew to 103 characters and were pushed back under 100 by
  that third test).
* Writing an aside such as `[...]` inside a description is fine (both languages use `[]` uniformly), as long as it does
  not get too long.

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

Numbers are the command layer's only conversion: Discord's integer options are signed 64-bit only, so the `/log` line
count and the `/ping` port are both declared as `i64` and checked in this layer (greater than 0, or 1 to 65535) before
being handed to the business layer (`usize`, `u16`); a failed check replies with a usage hint on the spot and passes
nothing down.

The free text of `/command` and `/commandall` uses a single `String` option: spaces are written directly (for example
`/command duck /home fishing`), with no quoting needed, because Discord's text options accept spaces in their own input
box. The only exception is pasting or typing the whole line at once: the client then splits on spaces and the extra
words try to match other options and fail (client behaviour, unavoidable). We do no processing at all (no unquoting, no
space merging, no `trim`), so whatever the user typed is what gets injected; the assembly and the "must not be empty"
check happen once in the business layer (`utils::ToCleanBotCommand`).

### 2.1 Autocomplete for the command options

Options such as the bot name `name` offer suggestions while the user types (a native Discord ability). The command-side
`commands/autocomplete.rs` queries permitted bot directories and running states, filters candidates by command scope,
builds display labels, and adapts the Discord interaction. Suggestions follow the meaning of the command rather than
always being "the running ones":

* `/stop`, `/pause`, `/unpause`, `/command`, `/restart`: permitted and running. For a target that is not running these
  operations are meaningless (`/restart` refuses outright and `/stop` only answers "no need to stop").
* `/start`, `/bot del`, `/bot rename`, `/bot login`: permitted and not running. Their precondition is that it has
  stopped (`/start` refuses a running target; so does `/bot login`, which also refuses while "the session of a previous
  login is still open").
* `/log`: everything permitted. Historical logs are useful for both, and the display name gets a "(running)" or
  "(stopped)" suffix.
* `/bot add`, `/token`: no suggestions. `/bot add` wants a name that does not exist yet (nothing to list) and `/token`
  is a Modal, which Discord does not support suggestions for.

Details: the permission rule matches the server side (admins get every bot, everyone else the ones under their own
`bots/<user ID>/`); the run state comes from one `screen -list` snapshot (every keystroke produces an autocomplete
interaction, so it must not probe per bot); at most `runtime.max_autocomplete_choices` entries (default 25, Discord's
hard limit, enforced by the candidate logic in `src/commands/autocomplete.rs`), with the running ones first. The
suggestions are only suggestions: the user can still
type any name, and ownership plus
whether the session is running are decided by the `handle_*` functions as before.

## 3. Single-bot control: `/start`, `/stop`, `/restart`, `/pause`, `/unpause`, `/command`, `/log`

* Code: `src/bot_control.rs` (the module docs) and `src/bot_control/*.rs` (one file per command)
* Entry point: `lib::handle_bot_control` (the action enum `BotControlAction`)
* Every session operation reuses the primitives in `utils`; this layer only validates, converts results and replies.
* Permissions are not in this module: the requirement of a single-bot command such as `/start` is
  `RequiredPermission::OwnerOrAdmin(owner of the target bot)`, checked once by the routing layer, where
  `handle_bot_control` and `handle_bot_log` resolve the target entry with `BotDirEntry::find_by_name` first. The batch
  `/startall` group requires `RequiredPermission::MustBeAdmin`, also checked in the routing layer.
* The autocomplete for the `name` option belongs to the command layer, not to this business module:
  `src/commands/autocomplete.rs` picks a batch by command meaning (`/start`, `/bot del` and `/bot rename` only look
  at the ones not running; `/stop`, `/pause`, `/unpause`, `/command` and `/restart` only at the running ones; `/log`
  gives all of them), see [section 2](#2-the-command-list-and-their-options).

### 3.1 The command list

* `/start <bot name>`: `start::start_bot(&bot_dir_entry)`. Launches the session and checks it twice (it counts as
  started only when it is still there on the second check, see section 3.2) and replies "start command sent".
* `/stop <bot name>`: `stop::stop_bot(&bot_dir_entry)`. Stops the session (releasing the game account); the stop is
  re-checked on the spot and it replies "stopped successfully".
* `/restart <bot name>`: `restart::restart_bot(&bot_dir_entry)`. Stops and then launches; a target that is not running
  is refused (use `/start` to start one) and it replies "restart command sent".
* `/pause <bot name>`: `pause::pause_fishing(&bot_dir_entry)`. An in-game `/pause` without logging out; after sending
  it waits `command_echo.wait_ms` (300 ms by default) to confirm the log echo and replies "pause confirmed" or
  "command sent".
* `/unpause <bot name>`: `unpause::unpause_fishing(&bot_dir_entry)`. An in-game `/unpause`; the confirmation and the
  wording are as above.
* `/command <bot name> <command>`: `command::send_command_to_bot(&bot_dir_entry, raw_bot_command_args)`. Forwards an
  in-game command.
* `/log <bot name> <lines>`: `log::bot_log_view(&bot_dir_entry, line_count)`. The reply is ephemeral (visible to the
  caller only).

`name` is **one name** today: taking several bots at once (`/start duck duck2`) is not implemented yet (the assessment
and the way to do it are recorded in [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md) section 8).

`/pause` and `/unpause` are not separate implementations: the injection goes down the same path as `/command`
(`utils::bot_status::confirm_pause_command` takes that injection step, with this layer passing
`command::send_command_to_bot` in, so the validation - directory ownership and the session running - is exactly the same
as for `/command`). The only difference is that these two also confirm the effect afterwards, which is exactly why the
"confirm and remember" logic lives in `utils/bot_status.rs` (and not in `command.rs`): what other commands send is
decided by the user, so a successful echo cannot be predicted and therefore cannot be confirmed.

The steps that confirm the effect:

1. Wait `command_echo.wait_ms` from the config (300 ms by default). The JAR prints the echo line synchronously when it
   receives the command, and that line has to reach the log first.
2. Look for the echo keyword in the last `command_echo.scan_lines` lines (50 by default) of the newest log. Only the
   tail is scanned rather than the whole file backwards, because scanning a long-running log from the start is
   expensive.
3. Only when the echo is seen does it say "pause confirmed" or "fishing resumed"; without it, it only says "command
   sent", because it says exactly as much as can be verified (see the "replies never overstate" rule in
   [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md)). When even the newest log cannot be found (the session is
   running but the log was moved away or has not been created yet) that is not treated as "no echo" but reported as "log
   file not found", because it means the confirmation step cannot be performed at all, not that the command had no
   effect.
4. When the echo is confirmed, `confirm_pause_command` records the state as well (`BotPauseStatus::set_paused` and
   `set_fishing`), and `/show status` then uses that memory instead of scanning a log back to the beginning of the file
   looking for a pause keyword. Sending `/pause` by hand with `/command`, or typing it inside the `screen` session,
   does not update it, so `/show status` may show a stale value in that case, and the next `/pause` or `/unpause` corrects
   it.

All of this assumes the JAR has those two commands. Upstream FishingBot has no `/pause` and `/unpause`; the build this
project uses is patched to add them (the patch also prints the two echoes looked for below). With an upstream jar those
two commands only answer "command sent" and the fishing status in `/show status` stays "fishing" forever.

The echo keywords to look for are `pause_echo` and `unpause_echo` in the config's `[command_echo]` (upstream messages
`command-pause-executed` and `command-unpause-executed`), and they are not passed by the caller:
`PauseCommand::echo_keyword` picks one of them by command. They only exist in a patched build (upstream has neither
command), so they are configured in place and changing the jar build means changing the config: the defaults match an
English jar, while a Chinese jar takes `已暂停钓鱼` / `已恢复钓鱼`. The two must differ and neither may contain the
other; startup validates this, otherwise that echo line cannot be attributed to one of the commands.

### 3.2 The fixed steps of the three action functions

`start_bot`, `stop_bot` and `restart_bot` share one structure, so the code can be read in these four steps:

1. Permission and target resolution are done in the routing layer: `lib::handle_bot_control` checks `OwnerOrAdmin` once
   with `permitted_bot_dir_entry` and passes the entry down, so this function only takes a `&BotDirEntry`. The entry
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
3. Call the `utils` flow functions (neither the waits nor the staggering take parameters; `utils` decides from how many
   bots are in this batch):
   * `/start` goes through `start_bot_sessions(std::slice::from_ref(bot_dir_entry))`;
   * `/stop` goes through `stop_single_bot_session(bot_dir_entry.clone())` (which returns the details; success or
     failure is read off them);
   * `/restart` goes through `restart_bot_sessions(std::slice::from_ref(bot_dir_entry))`.

   All three take a `&[BotDirEntry]`, and a single bot is a slice of one element - which is exactly why "support
   several bots at once later" will not need them rewritten (see [CONTEXT-en_us.md](../../CONTEXTS/CONTEXT-en_us.md)
   section 8).

   Between two neighbouring launches it waits `[startup_queue] login_interval_ms`, and after launching them all it waits
   (count x `timing.settle_per_bot_ms`) to check twice, with the same wait in between again, because the JVM may exit by itself one or two seconds after
   starting and a single check would report "started and gone" as a success; see
   [utils-en_us.md](../utils/utils-en_us.md) section 4.
4. Failure is always decided by the methods on the details utils returns (when `outcome.has_failed_lines()` is true,
   `outcome.to_failure_text()` is returned as the error; the stop details use a `has_failed_lines()` of the same
   shape). utils returns the details, and the assembly method has to be called explicitly here (see
   [utils-en_us.md](../utils/utils-en_us.md)). The wording follows the principle of saying only as much as can be
   verified:
   * `/start` replies `Start command sent for bot 'X'. Check /show status to confirm its state.` All that can be
     confirmed right away is that the command went out and the session appeared; logging in and joining the server
     still depend on log keywords.
   * `/stop` replies `Bot 'X' stopped successfully`. The stop flow re-checks on the spot that the session is gone (see
     `utils::stop_bot_sessions`), so this one can claim success.
   * `/restart` replies `Restart command sent for bot 'X'. The stop was confirmed; check /show status to confirm its
     state.` The stop stage can be confirmed; the launch stage can only say "sent", like `/start`.

### 3.3 The details of `/command`

* The command fragment is passed down by the command layer as it is (no unquoting, no space merging, no `trim`, see
  [section 2](#2-the-command-list-and-their-options)); this function assembles it into one command with
  `to_clean_bot_command` (`ToCleanBotCommand`) (joined with single spaces, and `错误:发送的指令不能为空！` when it is
  empty), exactly once.
* The echo uses that assembled command.
* The permission has been checked by the routing layer (`OwnerOrAdmin`) and ownership is not looked at again here; this
  function only assembles the command and requires the session to be running (the token check is in `/start`).
* A session that is not running is refused, because a command cannot be pushed into a session that does not exist.
* The injection is done by `utils::send_command_to_single_bot` (a screen primitive that validates nothing; it appends a
  newline at the end, which is what makes `screen` submit the line).

### 3.4 The details of `/log`

* The line count must be greater than 0 and is capped by `runtime.max_log_lines` (one command must not pull a whole
  large log back).
* Only the newest log is read (`utils::bot_log_files::latest_log_path`, which only accepts timestamp-shaped names):
  `create_bot_log_file` creates the file before every session launch, and the session appends through `grep -v -E` and
  `tee -a`.
* FishingBot's own log is written to `raw/log0.log` beside it (a symlink to `/dev/null`, so its contents are
  discarded), which means there is only one log on disk and `/log` cannot read back unfiltered spam. Details in
  [utils-en_us.md](../utils/utils-en_us.md) section 7.
* Reading uses the sliding window of `utils::read_last_lines`, so however large the log is only the last N lines are
  kept.
* The raw log text goes into `Report::content` (a whole block), so that no line gets a list prefix; the title carries
  the log file name.
* The reply uses `defer_ephemeral` and every reply (the usage hint, the failure text and the body with the attachment)
  carries an explicit `.ephemeral(true)`: the whole reply, attachment included, is only visible to the caller. Why it
  has to be declared explicitly (and cannot rely on the defer alone) is in
  [section 1.3](#13-sensitive-content-ephemeral-and-modal).

## 4. Account management: `/bot add`, `/bot del`, `/bot rename`, `/bot login`, `/token`

* Code: `src/bot_manage.rs` and `src/bot_manage/*.rs`
* Entry points: `lib::handle_bot_manage` (the action enum `BotManageAction`); `/bot login` has two more entry points,
  `lib::handle_bot_login` (stage one) and `lib::handle_bot_login_await` (stage two), see section 4.6.
* Permissions are neither in the command layer nor in this module: `/bot add` lands under the caller's own user
  directory (requirement `Everyone`); `del`, `rename`, `token` and `login` require
  `RequiredPermission::OwnerOrAdmin(owner of the target bot)`, checked once by the routing layer (`handle_bot_manage`,
  or `handle_bot_login` for `login`) after resolving the target entry with `BotDirEntry::find_by_name`. The command
  layer only reads values and counts arguments, never permissions.
* In common: directory names and background log entries always use the normalised name
  (`utils::ToCleanBotName::to_clean_bot_name`, which returns an `Err` when the name is illegal so the user can rename
  it), so the reply, the log and the directory name on disk always agree.

### 4.1 The command list

* `/bot add`: `add::add_bot(raw_bot_name, caller_id)`. Available to everyone, and it only creates inside the caller's
  own user directory.
* `/bot del`: `del::delete_bot(&bot_dir_entry)`. The creator or an admin.
* `/bot rename`: `rename::rename_bot(&bot_dir_entry, raw_new_bot_name)`. The creator or an admin; not while running.
* `/bot login`: `login::begin_login(&bot_dir_entry)` and `login::await_login(prompt)` (two stages, called by
  `handle_bot_login` and `handle_bot_login_await`, see section 4.6). The creator or an admin; not while running, and
  both replies are visible to the caller only.
* `/token`: `token::update_bot_token_file(&bot_dir_entry, raw_token)`. The creator or an admin; the reply is visible to
  the caller only, and not while running.

`/bot add`, `/bot del`, `/bot rename` and `/bot login` are Discord subcommands (typing `/bot` in the client shows the
four subcommands); `/token` is a separate top-level command that collects its input in a modal (see section 5).

### 4.2 `/bot add`

* Both the name check (`utils::ToCleanBotName::to_clean_bot_name`: an illegal character is an error instead of being
  filtered out) and the server-wide duplicate check live in `utils::create_bot_dir`, which returns the entry it created
  on success (the reply and the background log both use the `bot_name` from the entry).
* The directory always lands under the caller's own user directory: the path is derived from the caller's user ID, so
  the caller cannot write into someone else's directory.
* The reply clearly points out that "it still needs a token before it can start" and gives the next step: `/bot login`
  is recommended, with `/token` only as the manual fallback when that fails (the same wording is in the example
  config's `/help` text).

### 4.3 `/bot del`

1. The routing layer's `permitted_bot_dir_entry` locates the directory and checks `OwnerOrAdmin` (`find_by_name` first,
   then ownership); when it cannot be located or is not permitted it answers as not found and never reveals that "this
   bot belongs to someone else".
2. Deleting is refused while the session is running (`refusal_if_running`, with the wording from
   `utils::running_bots`), otherwise it would delete the data directory the bot is writing to.
3. The directory is removed with `fs::remove_dir_all`, and both success and failure are written to the background log.

### 4.4 `/bot rename`

The order is deliberate; do not reshuffle it:

1. Check that both the old and the new name are legal and that they differ.
2. The routing layer resolves the entry by the old name (`BotDirEntry::find_by_name`) and checks `OwnerOrAdmin` (the
   rule is directory ownership, and admins may cross directories).
3. A running session is refused outright (`refusal_if_running`, the same rule as `/bot del` and `/token`): a running
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
  `utils::token_file_path` and `fs::write` creates the file when it is missing and overwrites it when it exists.
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
3. Launch the session: the same path as `/start` (`utils::prepare_and_launch_bot`: create the log -> prepare
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
* The session disappears early -> report "exited after starting" (reusing `start_exited_soon`). On a real machine the
  typical cause is Microsoft deciding the requests are too frequent and returning an empty token, after which the JAR
  ends the JVM itself.

Timeout (10 minutes): check the credential once (the user may have finished in the last few seconds), and if it is not
there, stop the session - the device code has expired and that JVM would only keep asking Microsoft for authorisation;
after stopping, check the credential once more (the JAR may have finished writing it exactly when it got the Ctrl+C),
and only when it is still missing reply with the timeout and put the old credential back.

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

## 5. Batch commands: `/startall`, `/stopall`, `/restartall`, `/commandall`

* Code: `src/all_bots_control.rs` and `src/all_bots_control/*.rs`
* Entry point: `lib::handle_all_bots_control` (the action enum `AllBotsControlAction`), which is also the routing layer
  for this group.
* The permission is checked once, in the routing layer (`has_permission(.., RequiredPermission::MustBeAdmin)` inside
  `handle_all_bots_control`): functions in this module never take a `caller_id` and never check anything themselves, so
  there are not four copies of the same text with inconsistent wording. This module only calls the batch flows and
  assembles the reports.

### 5.1 The command list

* `/startall`: `startall::start_all_bots()`, covering every bot with a configured token that is not running.
* `/stopall`: `stopall::stop_all_bots()`, covering every running bot (including leftover processes whose token is no
  longer valid).
* `/restartall`: `restartall::restart_all_bots()`, covering every bot with a configured token that is running.
* `/commandall <command>`: `commandall::send_command_to_all_bots(raw_bot_command_args)`, covering every running bot.

None of these functions takes a `caller_id`: the routing layer `handle_all_bots_control` already stopped everyone else,
so whoever reaches them is an admin. They also pick their own titles (`/stopall` uses the command title; the process
shutdown path goes through `stopall::stop_all_bots_for_shutdown()` instead, which swaps in the log title because that
report goes into the program log). For comparison: a single-bot command's permission is decided inside each business
function by directory ownership, see
[section 3](#3-single-bot-control-start-stop-restart-pause-unpause-command-log).

### 5.2 Which directories are taken

Which batch is taken is decided by each command (`all_bot_dir_entries()` or `token_ready_bot_dir_entries()`), and then
grouped with `utils::split_bot_entries_by_is_running(&[BotDirEntry])` (one `screen -list` snapshot):

* `/startall`: `all_bot_dir_entries()`, split into two groups by token readiness first. An account without a token is
  not a failure but has to be visible (it goes into the skipped details), so everything is taken and then grouped.
* `/restartall`: `all_bot_dir_entries()`, taking only the running group. Restarting a target that is not running equals
  starting it (that is `/startall`'s job); targets without a token are dropped by the restart flow before it stops
  anything.
* `/stopall` and the process shutdown: `all_bot_dir_entries()`. After a token is deleted, a process launched earlier
  may still be running and holding the game account, and the shutdown has to be able to see it.
* `/commandall`: `token_ready_bot_dir_entries()`, names only. Sending a command needs no directory, and the scope
  matches `/show status` (only bots with a configured token that are running).

`/restartall` deliberately does not filter by token: a restart stops and then starts, and a target without a token
cannot start (the login must fail), so a pointless stop would take an account that might still be fishing offline. The
filter therefore lives in the restart flow, before the stop, and those targets show up in the failure details as "token
file missing, left as it is" rather than disappearing silently.

### 5.3 The flow of each command

* `/startall`: first split into two groups by token readiness (`split_bot_entries_by_token_status`), then split the
  valid accounts by run state (`split_bot_entries_by_is_running`). With no valid account at all it replies
  `未配置任何有效账号，未启动。` (with an extra note when there are also accounts missing a token); when all of them
  are running it replies `全部有效机器人均已运行`; otherwise it calls
  `utils::start_bot_sessions(&not_running_bot_entries)` and then overwrites the count rows with `Report::set_counts`
  (valid accounts in total, start commands sent, skipped (already running), skipped (token missing), start failures).
  Every account that was not started goes into the `skipped` details (one entry each for "already running" and "token
  missing") and does not count as a failure.
* `/stopall`: `utils::stop_all_bot_sessions()` returns the raw details (it is itself the combination of
  `all_bot_dir_entries()`, grouping and stopping), and the report is assembled by this module's
  `stopall::stop_all_bots()`; the process shutdown uses the same assembly through
  `stopall::stop_all_bots_for_shutdown()`, with only the title different (the log title).
* `/restartall`: only the running group of `all_bot_dir_entries()` is taken (a target that is not running is not a
  restart target); with none it replies `没有运行的机器人实例，无需重启`; otherwise it calls
  `utils::restart_bot_sessions` and then writes the running accounts, restart commands sent and restart failures with
  `Report::set_counts` (a target without a token explains itself in the restart failure details).
* `/commandall`: the command fragment is passed through as it is by the command layer, and here it is assembled into
  one command with `to_clean_bot_command` (the same convention as `/command`) before being injected one by one.
  `utils::all_running_bot_names()` comes from one `screen -list` snapshot, and a hit means it is running, so the
  injection is done straight away without re-authenticating and re-probing each bot; the successes are counted one by
  one and the raw failure text goes into `Report::failed`.

### 5.4 Report conventions

* All four commands output a `Report`: always a title, count rows and detail rows, where the count rows are generated
  by `Report::set_counts` uniformly in the form `label: N`.
* Replies have a length cap: `Report::render()` truncates line by line from the end according to
  `runtime.max_reply_chars` and notes how many lines were omitted. With many accounts the batch details reach the cap
  most easily (an overlong single Discord message fails to send outright).
* Count labels must not overstate: the start labels are "start commands sent" and "restart commands sent", not "started
  successfully", because a batch start can only confirm that the command went out and the session appeared (the same
  convention as `/start`, see [section 3.2](#32-the-fixed-steps-of-the-three-action-functions)); only the stop labels
  may say "stopped successfully" (the stop flow re-checks on the spot).
* A command whose basic information is only known once the flow has run (`/startall`, `/restartall`) runs the flow first
  and then overwrites the count rows with `Report::set_counts(..)`. That is an explicit overwrite and does not stack on
  top of the rows the flow wrote.

### 5.5 Troubleshooting failures

The transient memory peak of a batch start is often higher than a cloud server's available memory, and a process killed
by the OOM killer never shows up in `screen -list`, so the result says "the start command was sent but the process does
not exist (usually due to insufficient memory)". What to do: lower the memory allowance in `runtime.java_opts`, raise
`timing.settle_per_bot_ms` or `[startup_queue] login_interval_ms` (watching longer or signing in slower), or repeat the command later
(JVM memory usage improves over time).

## 6. Queries: `/show status` and `/show botconfig|mybots|allbots|admins`

* Code: `src/show.rs` and `src/show/*.rs`
* Entry points: `lib::handle_show` (the action enum `ShowAction::{MyBots, AllBots, Admins}`) and
  `lib::handle_fishingbot_config` (`/show botconfig` is separate because it returns an attachment instead of text).
* All of them are read-only: they touch no session and change nothing on disk, so no running check is needed.

### 6.1 The command list

* `/show status`: `status::all_running_bots_status()` (through `utils::running_bots::all_running_bot_infos()` internally).
  The data comes from `screen -list` and each bot's newest filtered log, and the reply is text (a table).
* `/show botconfig`: `botconfig::fishingbot_config_view()`. The data comes from `paths.fishingbot_config_file_name`
  (the copy every bot shares) and the reply is an attachment (a file name and the full text).
* `/show mybots`: `mybots::my_bots_list(caller_id)`. The data is the bots under the caller's own user directory, as
  text.
* `/show allbots`: `allbots::all_bots_list()`. The data is the bots under every user directory, as text.
* `/show admins`: `admins::admins_list()`. The data is the `[admins]` table from the config, as text.

### 6.2 `/show status`

* The list is the intersection of "has a token" and "the session exists": `utils::token_ready_bot_dir_entries()` first
  (the same scope as `/commandall`), then `utils::running_bots::all_running_bot_infos()` drops the ones that are not
  running using one `screen -list` snapshot. So everything in the table has a configured token and an existing session;
  a bot whose token file was deleted while the process is still running is not in the table (`/stopall` can still clean
  it up).
* Each bot's status is decided by `utils::running_bots`. The connection status has two sources: when the session is a
  `(Dead ...)` shell the log is not read at all and "process finished" is shown directly (the JVM has been killed and
  the last log line may still say "online"); when the session is normal it is read from the newest log
  (`BotDirEntry::connection_status`: read fresh on every call, scanning back from the end and stopping at the first
  conclusion). The pause status comes from the in-process memory `BotPauseStatus.pause_status()` (when a `/pause` and
  an `/unpause` for the same bot arrive almost at the same time, the last one in the echo window wins; it is updated
  when the echo is confirmed and reset when a session starts and ends, see
  [utils-en_us.md](../utils/utils-en_us.md) section 6). How the file is picked is in
  [utils-en_us.md](../utils/utils-en_us.md) section 7. The wording comes from `utils::bot_status`: the server status is
  "ready / initializing", "server online", "reconnecting / account online" or "process finished"; the three unreadable
  cases (no file picked, unreadable, empty) do not happen in normal operation, so the error reason is shown in that
  cell directly; the fishing status is "fishing" or "paused".
* Uptime is formatted uniformly by `utils::format_uptime` as "days-hours-minutes", so that no call site writes its own
  version.
* The output has one line per bot, which is easy to scan by eye. Both the header and the rows are generated by
  `utils::running_bots` (sitting next to each other in the same file):
  * the header is `status_table_header()` and the rows are `RunningBotInfo::into_row()`, both four-element arrays (read
    the columns side by side) sharing one separator constant (` | `, one space on each side);
  * adding a column, removing one or reordering means changing those two arrays plus the matching column names in
    `text` (one per language);
  * the snapshot fields are private and `into_row(self)` consumes itself, so every snapshot can only be rendered once
    and a mismatch such as "four header columns and three data columns" cannot happen;
  * why it lives in `running_bots` rather than in the general `reply`: this table only serves `/show status` (which only
    runs for running bots) and the snapshot fields are private, so keeping it beside them allows reading the fields
    directly without introducing a trait or getters for one table. The test `status_table_header_matches_row_columns`
    asserts cell by cell against both sides in the same module.

### 6.3 `/show botconfig`

* What is sent is the text of the fishing config that `paths.fishingbot_config_file_name` points at (FishingBot's own
  `conf.json`), not this project's `minecraft-fishingbot-manager-for-discord.conf.toml`: the point is to see what the fishing
  parameters are right now. That file is generated at startup by this program from `[fishingbot]` (the old file is
  deleted and a new one written, see [config-en_us.md](../config/config-en_us.md) section 3.2), so it is both "what the
  config says" and "what FishingBot is actually reading".
* Why an attachment: `conf.json` easily exceeds Discord's 2000 character limit for a single message and the body would
  be truncated; an attachment gives the full text and naturally carries a file name (`conf.json`).
* The content is not truncated: it is a structured file and cutting it in half invites misreading; when it exceeds the
  attachment limit (`utils::MAX_ATTACHMENT_BYTES`, 8 MiB) it reports the error honestly and tells you to look at it on
  the server.
* When the file is missing or unreadable it replies with an error explaining why (the file is generated at startup by
  this program, so check `base_dir` and the write permission first) instead of silently returning nothing.

### 6.4 `/show mybots` and `/show allbots`

* Both share one rendering and one directory scope (both in `utils::bot_dirs`, both sorted by bot name); the only
  difference is which directories are taken: `my_bots_list` goes through `bot_dir_entries_of_owner(user ID)` and
  `all_bots_list` through `all_bot_dir_entries()`.
* Each bot lists its name, its owning user ID (only meaningful for `allbots`) and whether a token exists. The token is
  only reported as present or not (`BotTokenStatus`); its contents are not shown and its validity is not checked
  online.
* `allbots` shows every account without regard for permissions, so it is admin-level information; its own permission
  requirement is `RequiredPermission::Everyone` (anyone may use it), it is just that an ordinary user usually only
  looks at their own directory and uses `/show mybots`.

### 6.5 `/show admins`

* It prints `name : user ID` line by line, and the names come from the config only, with no online lookup at all.
* When the list is empty it gives a clear note (add a line `"user ID" = "name"` to `[admins]` in
  `minecraft-fishingbot-manager-for-discord.conf.toml`, with the user ID from `/whoami`) so that it does not look like an
  error.

## 7. General: `/help`, `/whoami`, `/ping`, `/checkserver`

* Code: `src/general.rs` and `src/general/*.rs`
* No coupling to the other modules of this project: they do not read bot directories and do not touch `screen`
  sessions, so they can be used and tested on their own.
* All four commands require `RequiredPermission::Everyone` (anyone may use them), checked once each in `lib.rs`'s
  `handle_*`.

### 7.1 The command list

* `/help`: `help::help()`, no network request.
* `/whoami`: `whoami::whoami(caller_id)`, no network request.
* `/ping [host] [port]`: `ping::ping_report(host, port)`, which does make a network request (a Minecraft server
  handshake).
* `/checkserver`: `checkserver::checkserver_report()`, no network request (it reads local resources).

The `*_report` suffix follows the return shape: apart from `help` and `whoami`, which return a whole piece of text,
these return the result of `Report::render()`, following the same naming rule as `my_bots_list` and
`all_running_bots_status`, see [architecture-en_us.md](../architecture/architecture-en_us.md) section 5.

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
* It requires `Everyone`, checked once in `lib::handle_whoami`; the text is generated by `general/whoami.rs` and the
  command layer does not build its own reply (see the hard convention in
  [section 1](#1-the-rules-of-the-command-layer)).

### 7.4 `/ping`

* Without arguments it uses the default target of the `[ping]` table (`default_ping_host` and `default_ping_port`); with
  arguments what was typed in the chat wins (a port that fails to parse falls back to the default logic).
* It does a Minecraft server list handshake (`craftping`), so what comes back is the game layer's answer: the player
  count and the maximum, and the latency of the "fishing host to game server" hop, rather than plain TCP
  reachability.
* The output is always three lines: `status: online`, `players: current/max` and
  `latency: N ms (from this fishing host to the game server)`; a failure to connect, a timeout and a protocol parse
  failure each get a readable message.
* The connection timeout comes from `ping.timeout_ms` (config validation guarantees it is not 0, otherwise a response
  could never arrive).
* An illegal address or port, a failure to connect and a timeout each get a readable message, and none of them panics.

### 7.5 `/checkserver`

* What it reports is the resources of the machine running this program (the machine the fishing process is on), not the
  game server.
* The data comes from `sysinfo`: CPU usage, memory, disk and system uptime.
* CPU usage is computed from two samples taken a very short time apart: a single sample gives the average since boot,
  which stays high for a long time and looks like a machine at full load.
* The output is multi-line text with fixed fields, easy to screenshot, and it does not depend on a log file.

## 8. Related documents

* All documents: [docs index](../../index-en_us.md).
* Which layer the action enums and the permission check live in: [lib-en_us.md](../lib/lib-en_us.md).
* The start/stop waits and the two log lines: [utils-en_us.md](../utils/utils-en_us.md) section 4 and section 7.
* The game-side config attachment: [config-en_us.md](../config/config-en_us.md) section 3.2.
