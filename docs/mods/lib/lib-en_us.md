# lib: the command entry layer (routing)
[简体中文](lib-zh_cn.md) | **English**

* Code: `src/lib.rs` (command entry points); the autocomplete candidate lookup `bot_name_candidate_states(caller_id)` lives in the command layer at `src/commands/autocomplete.rs`
* Role: the layer that turns a chat command into an action function (the architecture doc calls it the command
  entry layer, or routing layer for short - see the terminology note in
  [architecture-en_us.md](../architecture/architecture-en_us.md) section 1), plus one place for error text and reply shapes.
* Above it sits the command layer (parse the text, count the arguments); below it sit `bot_control`, `bot_manage`,
  `show` and `all_bots_control` (the ones that do the work).
* Every permission check happens here, once (`has_permission`); the functions below never check. Each command
  declares a requirement (`utils::RequiredPermission`, see section 3); for a single-bot command that requirement is
  "the target bot belongs to the caller, or the caller is an admin", and ownership can only be read off the target
  entry, so this layer uses `BotDirEntry::find_by_name` to find the entry, checks it, and only then hands it to the
  business function.
* Reports are assembled by the caller: utils returns raw outcomes only (stopping gives a `StopOutcome`, for
  example, which carries `into_report()` and `to_failure_text()`), and `into_report()` has to be called explicitly -
  utils never hands you a finished `Report`. The batch stop is assembled explicitly in
  `all_bots_control::stopall::stop_all_bots()`, and the process shutdown assembles the same details on the spot inside
  `watchdog::Watchdog::exit_on_shutdown_signal()` (only the title differs).
* Visibility, one public face per module: the crate's public face is `lib.rs` alone (`pub fn handle_*`, the two
  view types re-exported with `pub use` - `BotLogView` and `FishingbotConfigView` - plus `pub mod config` and
  `pub mod text`). The five business modules below and `utils` have the same shape: submodules are private `mod`s,
  and the face is one hand-picked `pub(crate) use` list at the top of the parent module. So `lib.rs` only knows
  paths such as `crate::bot_control::start_bot`, and "may this function be called from outside" has exactly one
  answer: that list at the top of the parent module (there are six: `utils.rs`, `bot_control.rs`, `bot_manage.rs`,
  `general.rs`, `show.rs`, `all_bots_control.rs`).
* Nothing is cleaned up here either: the command layer only splits values and passes them through (the action enums
  carry `raw_*`), and whoever really needs a value cleans it once (bot names in
  `utils::ToCleanBotName::to_clean_bot_name`, command fragments in `utils::ToCleanBotCommand`).

## 1. Naming

* `handle_*`: the entry points. Take the command, dispatch the action, turn errors into text for the user (the
  permission checks for everyone-commands and batch commands happen here).
* `bot_*_reply`: the middle layer that folds an action enum and the raw arguments into the reply text; it lives in
  the command layer, not here.
* `*Action` enums: the action dimension of a command, produced by parsing (arguments are passed through untouched
  by the command layer) and interpreted only here.

The four action enums and their entry points:

* `BotControlAction`, handled by `handle_bot_control`: `/start`, `/stop`, `/restart`, `/pause`, `/unpause`,
  `/command`.
* `BotManageAction`, handled by `handle_bot_manage`: `/bot add`, `/bot del`, `/bot rename`, `/token`.
* `/bot login` does not use an action enum. It is **two-stage**, with two entry points: `handle_bot_login` (stage
  one: check permission, launch the session, wait for the device code) and `handle_bot_login_await` (stage two: wait
  for the credential to land on disk, or time out). Why it alone is split in two is in
  [commands-en_us.md section 4.6](../commands/commands-en_us.md#46-bot-login).
* `ShowAction`, handled by `handle_show`: `/show mybots|allbots|admins`. `/show botconfig` does not use an action
  enum (it returns an attachment instead of text); it goes through `handle_fishingbot_config`.
* `AllBotsControlAction`, handled by `handle_all_bots_control`: `/startall`, `/stopall`, `/restartall`,
  `/commandall`.

One more entry point does not run a command: `bot_name_candidate_states(caller_id)` returns bot names and run states the
caller is allowed to see; missing bot directories produce an empty list and explicit directory I/O failures return
`Err`. The autocomplete feature itself belongs to the command layer and lives in `src/commands/autocomplete.rs`: it owns
`BotNameScope`, prefix filtering, state labels and the candidate cap, then adapts the result to Discord. These are only
suggestions and do not perform bot operations. Which `scope` each command uses is in
[commands-en_us.md](../commands/commands-en_us.md), section 2.

## 2. The two kinds of error

* A missing permission has two sources, and the text always comes from the layer that produces it:
  * Batch commands: this layer (`handle_all_bots_control`) returns
    `Discord user ID X is not an admin, not allowed to do that!` directly (no business function runs, no `Report`);
  * Single-bot and account commands: `permitted_bot_dir_entry` returns `Err` with the text from
    `BotDirEntry::find_by_name`, `Error: no directory found for bot 'X', or you have no permission to access it`
    (not found and not allowed are deliberately the same answer, see
    [commands-en_us.md section 4](../commands/commands-en_us.md)).
* Argument or business errors: a business function returns `Result<String, String>` where the `Err` is text meant
  for the user; this layer only passes it back as the reply and rewrites nothing (so one thing never gets two
  different wordings).

## 3. Permission rules

Every command declares one requirement, and every check goes through `has_permission(caller_id, requirement)`:

* `Everyone`: `/help`, `/whoami`, `/show status`, `/ping`, `/checkserver`, `/show *` (including `/show botconfig`),
  checked once by each `handle_*` here.
* `MustBeAdmin`: `/startall`, `/stopall`, `/restartall`, `/commandall`, checked once by `handle_all_bots_control`
  here (which is why nothing under `all_bots_control/*` takes a `caller_id`).
* `OwnerOrAdmin(owner user ID)`: single-bot commands (`/start`, `/stop`, `/log`, ...) and account commands
  (`/bot del`, `/bot rename`, `/token`), checked once by `permitted_bot_dir_entry` here (it resolves the entry with
  `find_by_name` first, then checks ownership).

Three more rules:

* An ordinary user can only touch the bots in their own user directory; admins (the `[admins]` table) always pass.
* Not found and not allowed get the same answer (deliberately, see
  [commands-en_us.md section 4](../commands/commands-en_us.md)).
* `/bot add` creates inside the caller's own user directory, so ownership follows from the path and the requirement
  is simply `Everyone`.

## 4. Process-level lifecycle

The process-level lifecycle lives in `src/watchdog/lifecycle.rs`, as methods on the `Watchdog` object (the routing
layer only re-exports them for `main.rs`):

* `WATCHDOG.get_or_init()`: fetch the process-wide watchdog (the first call builds it from the config `[timing]`,
  later calls return that same one; the entry point and the event callbacks both go through it). Call it after
  `config::Config::get_or_init()`: the first call reads the config. `Watchdog::new(...)` takes explicit
  timings and is for tests only.
* `watchdog.shutdown_all_bots()`: stop every running bot session (without exiting the process).
* `watchdog.exit_on_shutdown_signal()`: the cleanup after SIGTERM / Ctrl+C (the entry point calls it when a signal
  arrives): log it, stop every bot, send the wrap-up report to every server's notice channel (when this connection
  never handshaked only the log is written), exit with code 0 (systemd will not restart it).
* `watchdog.exit_on_permanent_failure(reason)`: stop every bot and exit the process with code 1 (`reason` goes into
  the program log). Only for a wrong config (an invalid token, the bot was not invited to the server): exit code 1 is
  this project's deliberate-failure code, and `RestartPreventExitStatus=1` in the systemd unit stops it from being
  restarted.
* `watchdog.request_reconnect(reason)`: whoever judged the disconnect (the watchdog's decision side, or the entry
  point's own setup-error exit) hands the reason over and wakes the process entry with it; the entry point then does
  the cleanup and rebuilds the connection, without exiting the process (see
  [watchdog-en_us.md section 4](../watchdog/watchdog-en_us.md)).
* `watchdog.wait_for_attempt(client)`: the entry point uses it to wait for this attempt to end and to clean up
  accordingly: `client.start()` returning calls the private `stop_all_bots_on_disconnect()` (stop every bot + record
  the reason and the moment), the watchdog judging a disconnect hands its reason over and cleans up the same way, and
  a shutdown signal exits the process.
* `watchdog.wait_before_reconnect()`: wait one judging window before rebuilding the connection; a shutdown signal
  during that wait exits the process.
* `watchdog.remember_notice_context(ctx)`: the entry point records the current context on every successful handshake
  (`Ready` / `Resume`); the exit-signal wrap-up uses it to send the channel message (a disconnect wrap-up sends no
  channel message, so it does not need it).
* `watchdog.notify_reconnect_success(ctx)`: after an outage, when `Ready` / `Resume` arrives again, send one notice to
  every server this bot is in; does nothing when there was no outage.

How setup failures are classified (which cause gets which log record, and whether it counts as permanent) lives in
the text layer: `text::setup_failure_log_text` and `text::failure_is_permanent` (see
[i18n-en_us.md](../i18n/i18n-en_us.md)). The setup-error exit in `main.rs` uses them to decide which record to write
and whether to call `exit_on_permanent_failure`.

## 5. Related documents

* How commands are registered, parsed and dispatched: [commands-en_us.md](../commands/commands-en_us.md) (the
  command layer) and [architecture-en_us.md](../architecture/architecture-en_us.md) section 3 (the full path of one
  command).
* Every command: [commands-en_us.md](../commands/commands-en_us.md), sections 3 to 7.
