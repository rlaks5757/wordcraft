//! Whether the page starts WordCraft, decided from build and page facts only (testable natively).
//!
//! The PPM build (`ppm/build-web.sh`, feature `ppm-host`) is always host mode: it runs only inside
//! the PPM page that frames it (a same-origin parent with `?host=parent`), which owns the files.
//! Opened on its own it shows [`NEED_HOST_MESSAGE`] and starts nothing: no document, no pickers.
//! The upstream build (no feature) is unchanged.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

/// Whether this build is the PPM one. A compile-time value: no URL flag or stored setting turns it off.
pub const HOST_BUILD: bool = cfg!(feature = "ppm-host");

/// What the page shows when the PPM build is opened without PPM.
pub const NEED_HOST_MESSAGE: &str = "PPM에서 열어 주세요";

#[derive(Debug, PartialEq, Eq)]
pub enum Launch {
    /// Start the editor; `host` says it is locked to its parent page.
    Start { host: bool },
    /// Show [`NEED_HOST_MESSAGE`] only.
    NeedHost,
}

/// `bridge`: the page is framed by a same-origin parent that asked for `?host=parent`.
pub fn decide(host_build: bool, bridge: bool) -> Launch {
    match (host_build, bridge) {
        (true, false) => Launch::NeedHost,
        (true, true) => Launch::Start { host: true },
        (false, _) => Launch::Start { host: false },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ppm_build_without_a_parent_opens_nothing() {
        assert_eq!(decide(true, false), Launch::NeedHost);
    }

    #[test]
    fn the_ppm_build_with_a_parent_starts_locked() {
        assert_eq!(decide(true, true), Launch::Start { host: true });
    }

    /// The upstream build behaves as before, framed or not: it is never locked.
    #[test]
    fn the_upstream_build_is_unchanged() {
        assert_eq!(decide(false, false), Launch::Start { host: false });
        assert_eq!(decide(false, true), Launch::Start { host: false });
    }
}
