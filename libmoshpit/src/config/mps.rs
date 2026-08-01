// Copyright (c) 2025 moshpit developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

/// Used in bartoc configuration to define the bartos instance to connect to
#[derive(Clone, CopyGetters, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize)]
pub struct Mps {
    /// The mps IP address to listen for connections on
    #[getset(get = "pub")]
    ip: String,
    /// The mps port
    #[getset(get_copy = "pub")]
    port: u16,
    /// Lowest UDP port the server will bind a data session on.
    ///
    /// One port is used per concurrent session, but the whole range has to be
    /// reachable through the firewall because the server picks which one. The
    /// default range is 10,000 ports wide; narrow it to what you actually need
    /// so there is less to open.
    #[serde(default = "default_udp_port_min")]
    #[getset(get_copy = "pub")]
    udp_port_min: u16,
    /// Highest UDP port the server will bind a data session on, inclusive.
    #[serde(default = "default_udp_port_max")]
    #[getset(get_copy = "pub")]
    udp_port_max: u16,
    /// Address to advertise to clients for the data session, when it differs
    /// from the interface they connected to.
    ///
    /// The server normally tells a client to send its data packets to the local
    /// address of the accepted TCP connection. That is right until the key
    /// exchange arrives through a tunnel: a client reaching `mps` over an SSH
    /// port forward connects to `127.0.0.1` and is then told to send UDP
    /// there — to its own loopback. Set this to the host's public IP and the
    /// key exchange can stay on loopback while the data session still lands on
    /// an address the client can reach.
    ///
    /// Empty (the default) keeps the existing behaviour.
    #[serde(default)]
    #[getset(get = "pub")]
    advertise_ip: String,
}

const fn default_udp_port_min() -> u16 {
    50000
}

const fn default_udp_port_max() -> u16 {
    59999
}

impl Mps {
    /// The UDP data-session port range, clamped to something sane.
    ///
    /// A reversed or zero range would otherwise produce an empty pool, and the
    /// only symptom would be every session failing to bind with "pool
    /// exhausted" — so fall back to the default range instead.
    #[must_use]
    pub fn udp_port_range(&self) -> std::ops::RangeInclusive<u16> {
        let (min, max) = (self.udp_port_min, self.udp_port_max);
        if min == 0 || max < min {
            default_udp_port_min()..=default_udp_port_max()
        } else {
            min..=max
        }
    }

    /// `local` with its IP replaced by `advertise_ip`, when that is set and parses.
    ///
    /// An unparseable value falls back to `local` rather than failing the
    /// connection: a typo in this field should not take the server down, and
    /// the old behaviour is still a working one on a directly-reachable host.
    #[must_use]
    pub fn advertise_addr(&self, local: std::net::SocketAddr) -> std::net::SocketAddr {
        match self.advertise_ip.parse() {
            Ok(ip) => std::net::SocketAddr::new(ip, local.port()),
            Err(_) => local,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mps;

    fn mps(min: u16, max: u16) -> Mps {
        let toml =
            format!("ip = \"0.0.0.0\"\nport = 40404\nudp_port_min = {min}\nudp_port_max = {max}\n");
        toml::from_str(&toml).expect("valid mps config")
    }

    #[test]
    fn a_narrow_range_is_honoured() {
        assert_eq!(mps(50000, 50009).udp_port_range().count(), 10);
        assert_eq!(mps(50000, 50009).udp_port_range(), 50000..=50009);
        // A single port is a legitimate configuration for one session at a time.
        assert_eq!(mps(51000, 51000).udp_port_range().count(), 1);
    }

    #[test]
    fn the_default_range_is_unchanged_when_unset() {
        let m: Mps = toml::from_str("ip = \"0.0.0.0\"\nport = 40404\n").expect("defaults");
        assert_eq!(m.udp_port_range(), 50000..=59999);
    }

    #[test]
    fn advertise_ip_replaces_only_the_address() {
        let local = "127.0.0.1:50001".parse().expect("hardcoded test address");
        let m: Mps = toml::from_str("ip = \"127.0.0.1\"\nport = 40404\nadvertise_ip = \"203.0.113.7\"\n")
            .expect("valid mps config");
        assert_eq!(
            m.advertise_addr(local),
            "203.0.113.7:50001".parse().expect("hardcoded test address")
        );
    }

    /// Unset or nonsense must not break a host that is directly reachable.
    #[test]
    fn advertise_ip_falls_back_to_the_local_address() {
        let local: std::net::SocketAddr =
            "10.0.0.4:50001".parse().expect("hardcoded test address");
        for value in ["", "not-an-ip", "203.0.113.7:9"] {
            let m: Mps = toml::from_str(&format!(
                "ip = \"0.0.0.0\"\nport = 40404\nadvertise_ip = \"{value}\"\n"
            ))
            .expect("valid mps config");
            assert_eq!(m.advertise_addr(local), local, "advertise_ip = {value:?}");
        }
        let unset: Mps = toml::from_str("ip = \"0.0.0.0\"\nport = 40404\n").expect("defaults");
        assert_eq!(unset.advertise_addr(local), local);
    }

    /// A reversed or zero range would make an empty pool, and the only symptom
    /// would be every session failing to bind with "pool exhausted".
    #[test]
    fn a_nonsense_range_falls_back_instead_of_emptying_the_pool() {
        for (min, max) in [(59999, 50000), (0, 0), (0, 60000), (50010, 50009)] {
            let range = mps(min, max).udp_port_range();
            assert!(!range.is_empty(), "{min}..={max} produced an empty pool");
            assert_eq!(range, 50000..=59999, "{min}..={max} should fall back");
        }
    }
}
