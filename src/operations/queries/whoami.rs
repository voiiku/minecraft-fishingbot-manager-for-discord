/// 返回调用者的 Discord 用户 ID。
pub fn whoami(caller_id: u64) -> String {
    crate::config::Config::get()
        .text()
        .command_whoami(caller_id)
}
