use if_addrs::IfAddr;
use std::net::IpAddr;
use local_ip_address::local_ip;

/// `get_local_address` - get the local ip address, return an `Option<String>`. when it fails, return `None`.
#[must_use]
pub fn get_local_addr() -> Result<IpAddr, local_ip_address::Error> {
    local_ip()
}

#[must_use]
pub fn get_interfaces() -> Vec<String> {
    let mut interfaces: Vec<String> = Vec::new();
    let ifaces = if_addrs::get_if_addrs().expect("could not get interfaces");
    ifaces
        .iter()
        .filter(|iface| matches!(iface.addr, IfAddr::V4(..)))
        .for_each(|iface| interfaces.push(iface.addr.ip().to_string()));
    interfaces
}
