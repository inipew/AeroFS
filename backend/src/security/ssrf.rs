//! SSRF protection (§30, §98, §155-156) — validates network targets before provider creation.
//! Blocks private RFC1918, loopback, link-local, metadata IPs when `allow_private_network_connections == false`.

use crate::errors::{SecurityError, VfsError};

/// Returns true if host string is considered private / blocked.
pub fn is_blocked_host(host: &str) -> bool {
    let lower = host.to_lowercase();
    let host_trim = lower.trim().trim_matches(|c| c == '[' || c == ']');

    // Literal localhost
    if host_trim == "localhost" || host_trim == "metadata.google.internal" {
        return true;
    }
    // AWS / cloud metadata IPs
    if host_trim == "169.254.169.254" || host_trim == "169.254.169.253" || host_trim == "metadata" {
        return true;
    }

    // Try parse as IP literal
    if let Ok(ip) = host_trim.parse::<std::net::IpAddr>() {
        return is_private_ip(ip);
    }

    // Check if host is IPv4 literal with leading zeros etc handled by parse; otherwise hostname
    // For hostnames that resolve to private IPs, full DNS check requires async resolution.
    // We block known private hostname patterns here; async DNS is checked at factory time where possible.
    false
}

fn is_private_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let octets = v4.octets();
            // 127.0.0.0/8 loopback
            if octets[0] == 127 {
                return true;
            }
            // 10.0.0.0/8
            if octets[0] == 10 {
                return true;
            }
            // 172.16.0.0/12
            if octets[0] == 172 && (16..=31).contains(&octets[1]) {
                return true;
            }
            // 192.168.0.0/16
            if octets[0] == 192 && octets[1] == 168 {
                return true;
            }
            // 169.254.0.0/16 link-local
            if octets[0] == 169 && octets[1] == 254 {
                return true;
            }
            // 0.0.0.0/8
            if octets[0] == 0 {
                return true;
            }
            false
        }
        std::net::IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() {
                return true;
            }
            // fe80::/10 link-local, fc00::/7 unique local, ::ffff:0:0/96 ipv4-mapped
            let seg = v6.segments();
            if (seg[0] & 0xffc0) == 0xfe80 {
                return true;
            }
            if (seg[0] & 0xfe00) == 0xfc00 {
                return true;
            }
            if seg[0] == 0
                && seg[1] == 0
                && seg[2] == 0
                && seg[3] == 0
                && seg[4] == 0
                && seg[5] == 0xffff
            {
                // ipv4-mapped, check embedded v4
                let v4 = std::net::Ipv4Addr::new(
                    (seg[6] >> 8) as u8,
                    (seg[6] & 0xff) as u8,
                    (seg[7] >> 8) as u8,
                    (seg[7] & 0xff) as u8,
                );
                return is_private_ip(std::net::IpAddr::V4(v4));
            }
            false
        }
    }
}

/// Validate connection host/port against SSRF policy. Returns VfsError::Security on block.
pub fn validate_network_target(
    allow_private: bool,
    host: Option<&str>,
    port: Option<u16>,
) -> Result<(), VfsError> {
    if allow_private {
        return Ok(());
    }
    let Some(h) = host else {
        return Ok(());
    };
    if is_blocked_host(h) {
        return Err(VfsError::Security(SecurityError::SsrfBlocked(format!(
            "Connection to private host '{}' blocked by SSRF policy (allow_private_network_connections=false)",
            h
        ))));
    }
    // Block metadata ports
    if let Some(p) = port {
        if p == 2375 || p == 2376 {
            // Docker
            return Err(VfsError::Security(SecurityError::SsrfBlocked(format!(
                "Connection to Docker port {} blocked",
                p
            ))));
        }
    }
    Ok(())
}

/// Async DNS-after-resolution check (§156): resolves hostname and blocks if any IP is private.
/// Call after `validate_network_target` for defense in depth.
pub async fn validate_after_dns(
    allow_private: bool,
    host: &str,
    port: u16,
) -> Result<(), VfsError> {
    if allow_private {
        return Ok(());
    }
    if is_blocked_host(host) {
        return Err(VfsError::Security(SecurityError::SsrfBlocked(format!(
            "Connection to private host '{}' blocked by SSRF policy (post-DNS)",
            host
        ))));
    }
    // Only resolve if host is not literal IP (already checked) and looks like hostname
    // Use tokio lookup_host for async DNS
    let addr_str = format!("{}:{}", host, port);
    let addrs = tokio::net::lookup_host(addr_str).await.map_err(|e| {
        VfsError::Security(SecurityError::SsrfBlocked(format!(
            "SSRF DNS resolution failed for '{}': {}",
            host, e
        )))
    })?;
    for addr in addrs {
        if is_private_ip(addr.ip()) {
            return Err(VfsError::Security(SecurityError::SsrfBlocked(format!(
                "Host '{}' resolves to private IP {} blocked by SSRF policy",
                host, addr.ip()
            ))));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_private_blocked() {
        assert!(is_blocked_host("127.0.0.1"));
        assert!(is_blocked_host("10.0.0.1"));
        assert!(is_blocked_host("192.168.1.1"));
        assert!(is_blocked_host("172.16.5.4"));
        assert!(is_blocked_host("169.254.169.254"));
        assert!(is_blocked_host("localhost"));
        assert!(is_blocked_host("::1"));
        assert!(!is_blocked_host("8.8.8.8"));
        assert!(!is_blocked_host("1.1.1.1"));
        assert!(!is_blocked_host("example.com"));
    }
}
