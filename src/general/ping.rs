//! `/ping`：探测游戏服务器连通性与在线人数。

use std::time::{Duration, Instant};

use craftping::{PROTOCOL_VERSION_NOT_SET, tokio::ping as craftping_ping};
use tokio::net::TcpStream;

use crate::config;
use crate::text;
use crate::utils::Report;

/// 探测游戏服务器；未指定目标时使用配置默认值。
pub(crate) async fn ping_report(host: Option<&str>, port: Option<u16>) -> Result<String, String> {
    let ping_config = &config::get().ping;
    let target_host = host.unwrap_or(ping_config.default_ping_host.as_str());
    let target_port = port.unwrap_or(ping_config.default_ping_port);
    let timeout = Duration::from_millis(ping_config.timeout_ms);

    let start = Instant::now();

    let connect_result =
        tokio::time::timeout(timeout, TcpStream::connect((target_host, target_port))).await;

    let mut stream = match connect_result {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => {
            return Err(text::get().ping_connect_failed(target_host, target_port, &e.to_string()));
        }
        Err(_) => {
            return Err(text::get().ping_timeout(target_host, target_port, ping_config.timeout_ms));
        }
    };

    // 握手使用建连后剩余的总超时时间，不指定协议版本号。
    let remaining = timeout.saturating_sub(start.elapsed());
    match tokio::time::timeout(
        remaining,
        craftping_ping(
            &mut stream,
            target_host,
            target_port,
            PROTOCOL_VERSION_NOT_SET,
        ),
    )
    .await
    {
        Ok(Ok(response)) => {
            let latency = start.elapsed().as_millis();

            let mut report = Report::new();
            report.set_title(text::get().ping_title(target_host, target_port));
            report
                .basics
                .push(text::get().ping_status_online().to_string());
            report
                .basics
                .push(text::get().ping_players(response.online_players, response.max_players));
            report.basics.push(text::get().ping_latency(latency));

            Ok(report.render())
        }
        Ok(Err(e)) => {
            Err(text::get().ping_protocol_failed(target_host, target_port, &e.to_string()))
        }
        Err(_) => Err(text::get().ping_timeout(target_host, target_port, ping_config.timeout_ms)),
    }
}
