# Password connections

[简体中文](../zh-CN/password-authentication.md)

Normal operation uses a host address and connection password. On Windows, open `KokodaHost.exe`, choose **Set connection password**, enter it twice, select a sharing scope and start sharing. On Mac, enter the address and password and connect. There is no pairing-folder step. Both ends must be updated together.

Passwords contain 8–128 Unicode scalar values, without control characters; whitespace is preserved. Use a long, unique password. Stop sharing before changing the password. Setting a new password replaces the local registration; the next host start accepts only the new password and has no previous resume tickets. A password authorizes access to the configured sharing scope; selected-window mode is still read-only. Anyone who knows the password can authenticate: this is not per-device approval or revocation.

## Authentication and transport

`crates/transport/src/password.rs` integrates the upstream `opaque-ke` implementation of OPAQUE rather than implementing a PAKE primitive. The fixed suite uses Ristretto255, TripleDH/SHA-512 and Argon2id (the pinned library defaults: 19 MiB, two passes, parallelism one). Registration happens locally when the Windows owner sets the password; there is no unauthenticated network registration or password-reset endpoint.

Each accepted connection generates a fresh self-signed certificate and establishes TLS 1.3 with no early data or session tickets. Before authentication, the client verifies the TLS handshake signature but does not treat the certificate as a trusted host identity. OPAQUE's authenticated context contains `kokoda-password-1` and a 32-byte TLS exporter using `EXPORTER-Kokoda-OPAQUE-v1`. The authenticated context prevents relaying the exchange between two different TLS connections. The client verifies the server's OPAQUE response; the host verifies the final client proof, then sends the success marker. Only then can either side expose the normal application protocol. There is no fallback to token authentication on this endpoint.

TLS traffic keys protect application data; OPAQUE authenticates the exact TLS channel. Authentication frames have a 4 KiB limit checked before allocating their body, and sockets use 10-second read/write timeouts. The host budgets five connection attempts per peer IP and twenty globally in a rolling minute, including successful connections. Excess attempts are dropped before TLS processing. This bounds guessing and memory but is not a claim of resistance to network denial of service. The current host processes one session at a time.

## Storage and local process boundaries

Windows stores the OPAQUE server setup and registration in `%LOCALAPPDATA%\Kokoda\password.dat`, protected by DPAPI for the current Windows user. It does not store the plaintext password in `settings.json`. Existing damaged/unreadable records are not silently replaced; the owner can set a new password while stopped. DPAPI does not protect against code already running as that same user. New connections need no certificate files, and their certificate private keys stay in process memory.

The Mac uses `NSSecureTextField`. The password is retained in memory for reconnects while the app is open; there is no persistent remember-password feature yet. A bounded JSON prelude passes it to `kokoda-bridge` over its inherited stdin pipe, never command-line arguments or environment variables. Rust temporary secret buffers use `zeroize`; this is not a guarantee that every OS/Swift UI copy can be erased. Authentication failures do not trigger automatic retries.

The explicit legacy `--serve-development ... IDENTITY` and pairing-directory bridge syntax remain for existing developer fixtures only. The GUI launches `--serve-password ... password.dat` and never chooses the legacy path. Old identity/pairing directories are not used by the new GUI and are not automatically deleted. The password record is versioned; a future incompatible suite change must bump the record and authentication versions and provide an explicit reset/migration path.

## Dependency status and validation boundary

`opaque-ke` is pinned to official `5.0.0-pre.0` (a prerelease), with `voprf 0.6.0-pre.2` and `crypto-bigint 0.7.11` locked. The newer line avoids the older crypto-bigint dependency affected by [RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387). The project's historical upstream audit does not establish that this release or Kokoda's integration has been independently audited. This remains a development build; a release requires dependency/security review and broader device coverage.

Sources: [OPAQUE implementation](https://github.com/facebook/opaque-ke), [RFC 9807](https://www.rfc-editor.org/rfc/rfc9807.html), [TLS 1.3 exporters](https://www.rfc-editor.org/rfc/rfc8446.html#section-7.5). See the [validation record](validation.md) for actually executed tests.
