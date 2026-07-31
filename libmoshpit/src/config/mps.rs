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
