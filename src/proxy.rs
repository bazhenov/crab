use crate::prelude::*;
use rand::{rngs::ThreadRng, seq::SliceRandom, thread_rng};
use reqwest::Proxy;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    ops::{AddAssign, SubAssign},
    path::Path,
};

type AliveCounter = SaturatedI8<-2, 2>;
pub type ProxyId = usize;

/// List of proxies
///
/// Tracks which proxies are alive and which are dead. Each proxy get saturated counter in a range `-2..=2`.
/// Each time request has been processed proxy counter is incremented (in case of successfull response)
/// or decremented (in case of failure). Dead proxy is defined as a proxy with undersaturated counter (`-2`).
#[derive(Default)]
pub struct Proxies {
    proxies: Vec<(Proxy, ProxyStat)>,
    rng: ThreadRng,
}

#[derive(Default, Clone)]
pub struct ProxyStat {
    /// Number of requests attempted via proxy
    pub requests: u32,

    /// Number of requests executed successfully via proxy
    pub successfull_requests: u32,

    alive_counter: AliveCounter,
}

impl Proxies {
    pub(crate) fn from_file(proxy_list: impl AsRef<Path>) -> Result<Self> {
        let file = BufReader::new(File::open(proxy_list.as_ref())?);
        let mut proxies = vec![];
        for line in file.lines() {
            let line = line?.trim().to_owned();
            if !line.is_empty() {
                proxies.push((Proxy::all(line)?, ProxyStat::default()));
            }
        }
        let rng = thread_rng();
        Ok(Self { proxies, rng })
    }

    /// Called when proxy failed to process a request
    pub(crate) fn proxy_failed(&mut self, proxy_id: ProxyId) {
        let Some((proxy, stat)) = self.proxies.get_mut(proxy_id) else {
            return;
        };
        stat.requests += 1;
        stat.alive_counter -= 1;
        if stat.alive_counter.state() == CounterState::SaturatedDown {
            info!("Proxy found dead: {:?}", proxy);
        }
    }

    /// Called when proxy successfully process a request
    pub(crate) fn proxy_succeseed(&mut self, proxy_id: ProxyId) {
        let Some((_, stat)) = self.proxies.get_mut(proxy_id) else {
            return
        };
        stat.requests += 1;
        stat.successfull_requests += 1;
        stat.alive_counter += 1;
    }

    pub(crate) fn stat(&self) -> Vec<(Proxy, ProxyStat)> {
        self.proxies.clone()
    }
}

impl Iterator for Proxies {
    type Item = (Proxy, ProxyId);

    /// Returning next proxy to be used
    ///
    /// Tries to select a proxy from the list of the non dead proxies first. If all proxies are dead
    /// keeps trying a random dead proxy.
    fn next(&mut self) -> Option<Self::Item> {
        let not_dead_proxies = self
            .proxies
            .iter()
            .enumerate()
            .filter(|(_, (_, stat))| stat.alive_counter.state() != CounterState::SaturatedDown)
            .map(|(id, (proxy, _))| (id, proxy))
            .collect::<Vec<_>>();

        if let Some((id, proxy)) = not_dead_proxies.choose(&mut self.rng) {
            return Some(((*proxy).clone(), *id));
        }

        // No alive proxies left. Trying again all proxies in the list.
        let all_proxies = self
            .proxies
            .iter()
            .enumerate()
            .map(|(id, (proxy, _))| (id, proxy))
            .collect::<Vec<_>>();

        if let Some((id, proxy)) = all_proxies.choose(&mut self.rng) {
            return Some(((*proxy).clone(), *id));
        }

        None
    }
}

// Saturated i8 between MIN and MAX
#[derive(Default, Clone)]
struct SaturatedI8<const MIN: i8, const MAX: i8>(i8);

#[derive(Debug, PartialEq)]
enum CounterState {
    NotSaturated,
    SaturatedDown,
    SaturatedUp,
}

impl<const MIN: i8, const MAX: i8> AddAssign<i8> for SaturatedI8<MIN, MAX> {
    fn add_assign(&mut self, rhs: i8) {
        self.0 = MAX.min(self.0 + rhs);
    }
}

impl<const MIN: i8, const MAX: i8> SubAssign<i8> for SaturatedI8<MIN, MAX> {
    fn sub_assign(&mut self, rhs: i8) {
        self.0 = MIN.max(self.0 - rhs);
    }
}

impl<const MIN: i8, const MAX: i8> SaturatedI8<MIN, MAX> {
    pub fn state(&self) -> CounterState {
        if self.0 == MIN {
            CounterState::SaturatedDown
        } else if self.0 == MAX {
            CounterState::SaturatedUp
        } else {
            CounterState::NotSaturated
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn proxies() -> Result<()> {
        let dir = tempdir()?;
        let proxy_list = dir.as_ref().join("proxy.list");
        let mut file = File::create(&proxy_list)?;
        writeln!(&mut file, "socks5://127.1")?;
        writeln!(&mut file, "socks5://127.2")?;

        let proxies = Proxies::from_file(proxy_list)?;
        assert_eq!(proxies.proxies.len(), 2);

        Ok(())
    }

    #[test]
    fn check_saturated_counter() {
        type Counter = SaturatedI8<-1, 1>;
        let mut counter = Counter::default();

        counter += 1; // 1
        assert_eq!(counter.state(), CounterState::SaturatedUp);

        counter -= 1; // 0
        assert_eq!(counter.state(), CounterState::NotSaturated);

        counter -= 2; // -1
        assert_eq!(counter.state(), CounterState::SaturatedDown);

        counter += 1; // 0
        assert_eq!(counter.state(), CounterState::NotSaturated);

        counter += 1; // 1
        assert_eq!(counter.state(), CounterState::SaturatedUp);
    }
}
