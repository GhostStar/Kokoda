//! LAN discovery supplies untrusted address hints. Password authentication remains mandatory.
use mdns_sd::{IfKind, Receiver, ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    net::Ipv4Addr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub const SERVICE_TYPE: &str = "_kokoda._tcp.local.";
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const MAX_HOSTS: usize = 128;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Host {
    pub id: String,
    pub name: String,
    pub addresses: Vec<String>,
}
fn usable(ip: Ipv4Addr) -> bool {
    !ip.is_loopback()
        && !ip.is_unspecified()
        && !ip.is_multicast()
        && !ip.is_broadcast()
        && ip.octets()[0] != 0
}
fn host(service: &ResolvedService) -> Option<Host> {
    if service.ty_domain != SERVICE_TYPE
        || service.port == 0
        || service.fullname.len() > 255
        || service.get_property_val_str("protocol") != Some("1")
        || service.get_property_val_str("auth") != Some("opaque")
    {
        return None;
    }
    let name = service.get_property_val_str("name")?;
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return None;
    }
    let mut addresses: Vec<_> = service
        .get_addresses_v4()
        .into_iter()
        .filter(|ip| usable(*ip))
        .map(|ip| format!("{ip}:{}", service.port))
        .collect();
    addresses.sort();
    addresses.dedup();
    addresses.truncate(8);
    if addresses.is_empty() {
        return None;
    }
    Some(Host {
        id: service.fullname.clone(),
        name: name.into(),
        addresses,
    })
}
fn daemon() -> Result<ServiceDaemon> {
    let daemon = ServiceDaemon::new()?;
    // Current GUI host binds IPv4. Do not advertise unusable IPv6 or loopback addresses.
    if let Err(error) = daemon.disable_interface(vec![IfKind::IPv6, IfKind::LoopbackV4]) {
        let _ = daemon.shutdown();
        return Err(error.into());
    }
    let _ = daemon.set_ip_check_interval(5);
    Ok(daemon)
}
pub struct Browser {
    daemon: ServiceDaemon,
    events: Receiver<ServiceEvent>,
    hosts: BTreeMap<String, Host>,
    checked: Instant,
}
impl Browser {
    pub fn new() -> Result<Self> {
        let daemon = daemon()?;
        let events = match daemon.browse(SERVICE_TYPE) {
            Ok(events) => events,
            Err(error) => {
                let _ = daemon.shutdown();
                return Err(error.into());
            }
        };
        Ok(Self {
            daemon,
            events,
            hosts: BTreeMap::new(),
            checked: Instant::now(),
        })
    }
    pub fn snapshot(&mut self) -> Vec<Host> {
        for _ in 0..1024 {
            let Ok(event) = self.events.try_recv() else {
                break;
            };
            match event {
                ServiceEvent::ServiceResolved(service) => {
                    // A changed/incompatible record must invalidate an earlier valid hint.
                    self.hosts.remove(&service.fullname);
                    if self.hosts.len() < MAX_HOSTS {
                        if let Some(host) = host(&service) {
                            self.hosts.insert(host.id.clone(), host);
                        }
                    }
                }
                ServiceEvent::ServiceRemoved(_, id) => {
                    self.hosts.remove(&id);
                }
                _ => {}
            }
        }
        if self.checked.elapsed() > Duration::from_secs(15) {
            self.checked = Instant::now();
            for id in self.hosts.keys() {
                let _ = self.daemon.verify(id.clone(), Duration::from_secs(5));
            }
        }
        self.hosts.values().cloned().collect()
    }
}
impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        let _ = self.daemon.shutdown();
    }
}
pub struct Advertiser {
    daemon: ServiceDaemon,
    id: String,
}
impl Advertiser {
    pub fn new(name: &str, port: u16) -> Result<Self> {
        if port == 0 || name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            return Err("invalid_discovery_metadata".into());
        }
        // Ephemeral DNS instance, not a persistent device identity or trust credential.
        let suffix = format!(
            "{:x}-{:x}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        );
        let hostname = format!("kokoda-{suffix}.local.");
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &format!("Kokoda-{suffix}"),
            &hostname,
            "",
            port,
            &[("protocol", "1"), ("auth", "opaque"), ("name", name)][..],
        )?
        .enable_addr_auto();
        let id = service.get_fullname().to_string();
        let daemon = daemon()?;
        if let Err(error) = daemon.register(service) {
            let _ = daemon.shutdown();
            return Err(error.into());
        }
        Ok(Self { daemon, id })
    }
}
impl Drop for Advertiser {
    fn drop(&mut self) {
        // Wait briefly for the goodbye before shutting down the responder thread.
        if let Ok(done) = self.daemon.unregister(&self.id) {
            let _ = done.recv_timeout(Duration::from_millis(300));
        }
        let _ = self.daemon.shutdown();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn record(ip: &str, protocol: &str, auth: &str, name: &str) -> ResolvedService {
        ServiceInfo::new(
            SERVICE_TYPE,
            "Fixture",
            "fixture.local.",
            ip,
            47831,
            &[("protocol", protocol), ("auth", auth), ("name", name)][..],
        )
        .unwrap()
        .as_resolved_service()
    }
    #[test]
    fn discovery_is_only_a_validated_untrusted_hint() {
        assert!(host(&record("127.0.0.1", "1", "opaque", "Fixture")).is_none());
        let mut service = record("192.0.2.10", "1", "opaque", "Fixture");
        assert_eq!(host(&service).unwrap().addresses, ["192.0.2.10:47831"]);
        service.port = 0;
        assert!(host(&service).is_none());
        for (protocol, auth, name) in [
            ("2", "opaque", "Fixture"),
            ("1", "none", "Fixture"),
            ("1", "opaque", "bad\nname"),
        ] {
            assert!(host(&record("192.0.2.10", protocol, auth, name)).is_none());
        }
        assert!(!usable(Ipv4Addr::LOCALHOST));
        assert!(!usable(Ipv4Addr::BROADCAST));
        assert!(!usable(Ipv4Addr::new(224, 0, 0, 251)));
    }
}
