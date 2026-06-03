use std::net::IpAddr;

/// List non-loopback IPv4 addresses for this machine (for LAN signaling hints).
pub fn list_lan_ipv4() -> Vec<String> {
    let mut addrs = Vec::new();
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            if iface.is_loopback() {
                continue;
            }
            let IpAddr::V4(ipv4) = iface.addr.ip() else {
                continue;
            };
            let ip = ipv4.to_string();
            if !addrs.contains(&ip) {
                addrs.push(ip);
            }
        }
    }
    addrs
}
