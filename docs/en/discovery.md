# LAN discovery and local window selection

[简体中文](../zh-CN/discovery.md)

## Windows host

While sharing is stopped, select **Choose running window…**. The local list refreshes every three seconds and can search window titles or executable paths. Pick a window as a rule example, then choose application path AND title contains (default), application path only, or title contains only. The preview separates direct matches from eligible owned descendants. **Add sharing rule** updates the stopped host configuration; start sharing to apply it. An example is not a permanent HWND grant: the rule can match future windows too. An empty title never silently falls back to application-wide sharing. Revalidate the window/process instance, title and owner when accepting the example.

Independent windows with the same PID do not inherit a title rule. The existing host policy remains authoritative for catalog, grouping and capture. Selected scope is still read-only; changing rules still requires stopping and restarting sharing. A local executable path stays on the host and is not added to the remote catalog.

Enable **Discoverable on LAN while sharing** if clients should find the host without entering an address. This setting is off by default and broadcasts the computer name, IPv4 addresses, port, protocol version and password-authentication type. It does not broadcast window titles, application paths, sharing rules, passwords or certificates. Advertisement starts only after the host listener reports ready, and stops on Stop sharing, GUI exit or host failure. The CLI host alone does not advertise.

## Mac client

The disconnected manager automatically browses nearby Kokoda hosts. Select an entry to fill its address, enter the connection password and connect. Choosing another address clears the password field; discovery never starts authentication automatically. Same-name machines remain distinct by service identity and endpoint. Refresh discovery without altering a manually entered address. After connecting, browsing stops.

Discovery uses IPv4 multicast DNS-SD (`_kokoda._tcp.local.`, UDP 5353) through the pinned [mdns-sd library](https://docs.rs/mdns-sd/0.22.0/mdns_sd/). The media/control transport remains TCP/TLS with OPAQUE authentication. A discovery record is untrusted input, not proof of device identity. Invalid protocol/auth metadata, unusable addresses and oversized/control-character names are rejected; result counts are bounded. Resolved services are periodically verified so abrupt exits are eventually removed; graceful stops send a goodbye. There is no automatic fallback to a different endpoint using a retained password.

The computers need multicast reachability, usually on the same LAN. Guest Wi-Fi, routed/VPN networks, macOS local-network permission and Windows firewall settings can prevent discovery. The app does not change firewall rules; manual address entry remains supported. Windows/Linux client discovery UI is not in this slice; the Rust discovery component is reusable by their adapters. Both the updated Mac client and Windows GUI host are needed for discovery; older versions still connect by address.

## Verification

`cargo test --workspace --locked` includes metadata rejection and rule-example/owner contracts. `DiscoveryTests` checks native Mac selection without automatic login, password isolation and manual-address fallback. `HostGui.Tests.ps1` operates synthetic Windows forms, previews owned versus independent windows and validates the saved combined rule plus normal password/sharing lifecycle. `cargo run -p kokoda-discovery --example probe -- lifecycle` checks real advertisement and goodbye; `probe announce NAME PORT SECONDS` / `probe find NAME SECONDS` are isolated two-machine fixtures. Only use synthetic names and stop fixtures afterward.

Starting a connection pauses discovery and preserves its last results. A failed connection does not restart scanning; use **Search LAN** explicitly. Connection attempts clear old error details. Remote errors can be selected/copied in the status area, or read in full by hovering over it.
