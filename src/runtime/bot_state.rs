//! 机器人暂停状态的进程内存储。

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::models::BotPauseStatus;

pub(crate) static PAUSE_STATUSES: LazyLock<Mutex<HashMap<String, BotPauseStatus>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
