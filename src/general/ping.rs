//! `/ping`：探测游戏服务器连通性与在线人数。

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};

use craftping::{PROTOCOL_VERSION_NOT_SET, tokio::ping as craftping_ping};
use tokio::net::TcpStream;

use crate::config;
use crate::text;
use crate::utils::Report;

/// 探测 Minecraft 服务器并返回格式化报告。
///
/// 未指定目标时使用 `[ping]` 中的默认值。显式指定的目标在解析后必须全部为公网地址；
/// 配置中的默认目标由运维人员提供，视为可信值。
pub(crate) async fn ping(host: Option<&str>, port: Option<u16>) -> Result<String, String> {
    let ping_config = &config::Config::get().ping;
    let target_host = host.unwrap_or(ping_config.default_ping_host.as_str());
    let target_port = port.unwrap_or(ping_config.default_ping_port);
    let timeout = Duration::from_millis(ping_config.timeout_ms);
    let require_public_target = host.is_some();

    let start = Instant::now();

    // 只解析一次，后续连接使用解析结果，避免 DNS 重绑定。
    let target_addresses =
        match tokio::time::timeout(timeout, tokio::net::lookup_host((target_host, target_port)))
            .await
        {
            Ok(Ok(addresses)) => {
                let addresses: Vec<_> = addresses.collect();
                if addresses.is_empty() {
                    return Err(text::get().ping_connect_failed(
                        target_host,
                        target_port,
                        "no address returned",
                    ));
                }
                addresses
            }
            Ok(Err(e)) => {
                return Err(text::get().ping_connect_failed(
                    target_host,
                    target_port,
                    &e.to_string(),
                ));
            }
            Err(_) => {
                return Err(text::get().ping_timeout(
                    target_host,
                    target_port,
                    ping_config.timeout_ms,
                ));
            }
        };

    // 用户显式指定的目标必须全部为公网地址；默认目标来自配置，视为可信值。
    if require_public_target
        && target_addresses
            .iter()
            .any(|address| is_local_target_ip(address.ip()))
    {
        return Err(text::get().ping_local_target(target_host, target_port));
    }

    let mut stream = None;
    let mut last_connect_error = None;
    for target_address in target_addresses {
        let remaining = timeout.saturating_sub(start.elapsed());
        match tokio::time::timeout(remaining, TcpStream::connect(target_address)).await {
            Ok(Ok(connected)) => {
                stream = Some(connected);
                break;
            }
            Ok(Err(error)) => last_connect_error = Some(error),
            Err(_) => {
                return Err(text::get().ping_timeout(
                    target_host,
                    target_port,
                    ping_config.timeout_ms,
                ));
            }
        }
    }

    let Some(mut stream) = stream else {
        let reason = last_connect_error.map_or_else(
            || "connection failed".to_string(),
            |error| error.to_string(),
        );
        return Err(text::get().ping_connect_failed(target_host, target_port, &reason));
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

/// 判断地址是否属于环回、私网、链路本地、共享地址或其他保留网段。
fn is_local_target_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_local_ipv4(ip),
        IpAddr::V6(ip) => is_local_ipv6(ip) || ip.to_ipv4_mapped().is_some_and(is_local_ipv4),
    }
}

fn is_local_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();

    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || octets[0] == 0
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
        || octets[0] >= 240
}

fn is_local_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.is_multicast()
        || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 本机、私网、链路本地、多播与保留地址都应被拒绝。
    #[test]
    fn local_and_private_addresses_are_rejected() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.1.1",
            "100.64.0.1",
            "198.18.0.1",
            "240.0.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(
                is_local_target_ip(address.parse().expect("test address")),
                "{address} should be rejected"
            );
        }
    }

    /// 公网单播地址不应被本地地址规则误拦。
    #[test]
    fn public_addresses_are_allowed() {
        for address in ["8.8.8.8", "1.1.1.1", "2001:4860:4860::8888"] {
            assert!(
                !is_local_target_ip(address.parse().expect("test address")),
                "{address} should be allowed"
            );
        }
    }
}
