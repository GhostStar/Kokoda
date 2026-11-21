# Product code structure

[简体中文](../zh-CN/code-structure.md)

Share interaction decisions across clients, with platform-specific capture, playback and OS integration. The first release retains one-to-one connections, connection passphrases and host window filtering. [Development runtime and remaining release boundaries](development.md)

## Directories and ownership

| Directory | Responsibility | Current implementation |
|---|---|---|
| apps/desktop/src, locales | Shared screens and translations | 112 bilingual keys; shared visual screens pending |
| apps/macos, windows, linux | Launchers, menus and native UI integration | macOS development runner; Windows/Linux reserved |
| apps/host-settings | Local management UI | Reserved |
| crates/app-model | Window facts, identity, input, quality and format | Shared Rust model; application grouping is not an authorization policy |
| crates/client | Viewer lifecycle, global quality and UI effects | Tested controller, C ABI, network bridge; native scheduling/recovery migration continues |
| crates/host | Admission, sessions, authorization orchestration | Admission plus migrated Windows group/session runtime; product authorization pending |
| crates/protocol | Kokoda messages, limits and validation | Development routing/input validation and quality profiles; full typed wire schema/version negotiation pending |
| crates/transport | Encryption, framing, timeouts and backpressure | Pinned TLS/token development pairing, bounded framing, partial I/O handling |
| crates/media | Frames, capture/player contracts and format intersection | Shared interfaces, limits and backend error boundary; full adapter adoption pending |
| platforms/windows | Window metadata, capture, encoding, input and clipboard | Native Cargo package, no transport dependency |
| platforms/macos | Playback, viewer surfaces, IME and clipboard images | Native Swift sources compiled by the macOS runner |
| platforms/linux | Native playback and integration | Reserved |
| crates/windows-host, macos-host | Host executable composition | Windows CLI; macOS reserved |
| tests/contracts, fixtures, platform, e2e | Shared vectors, synthetic windows, native and network regressions | Implemented suites; Rust tests also live with their crates |
| packaging, scripts | Distribution and build tooling | Development build/signing/SDK scripts; release installers pending |
| docs/en, docs/zh-CN | Aligned product and engineering documentation | Maintained together |

Keep unimplemented directories documented or tracked with `.gitkeep`. Add build targets when implementations exist. Native UI toolkit selection for Windows/Linux remains subject to IME, multi-window and video-surface verification.

## Dependencies and module boundaries

`app-model` has no endpoint or platform dependency. Client and protocol code depend on shared models. The client composes protocol and transport and exports a C ABI to native runners; it does not depend on a native codec. Windows platform code depends on models and media, without transport or endpoint orchestration. Host Windows modules compose native adapters, protocol and transport; the executable wires the runtime. Endpoint cores do not depend on each other.

The macOS runner feeds confirmed root identities into the shared controller and executes its global-quality effects. The native runner still owns the pending-open queue, local surfaces, request scheduling and recovery integration. Migrate these workflows into shared modules incrementally, keeping native IME, focus and media resources at the platform boundary. The C ABI is confined to the UI thread; returned strings are released through the same Rust library.

Product authorization belongs to the host, using platform facts. UI catalog filtering is insufficient. The development runtime defaults to an empty read-only catalog. A host-local rule engine filters discovery and capture; only explicit startup opt-in permits all windows with input/clipboard. Its current window IDs, application-group membership and bearer-token pairing are migration contracts, not the final privacy boundary. Complete interactive restricted sharing, live policy editing, discovery generations and request correlation before release.

RustDesk's framing module retains upstream attribution. FFmpeg supplies vendor codec integration through public APIs. See [third-party provenance](../../THIRD_PARTY.md). No RustDesk endpoint interoperability is required.

## Lifetimes and threads

One host loop owns Admission. Groups and input/capture resources are destroyed before releasing its connection slot. A late release cannot free another generation's slot. The development listener is serial; the explicit busy response and approved-device admission workflow remain pending. Serving from Windows Session 0 is rejected before binding a socket.

Capture/encoding workers own native resources. Output queues are bounded; predictive encoded frames are kept in order. Native callbacks carry a connection generation; viewers reject old window tokens/stream epochs and clear queued input on loss. Current resume tickets recover windows without replaying old input or clipboard operations. Product policy revocation must additionally clear and reauthorize those resources.

## Verification

Run `cargo test --workspace --locked` and `python3 tests/check-locales.py`. CI tests on Windows, macOS and Linux; only Windows compiles and runs its native backend. Mac native suites and real network drivers are described in [the build guide](development.md). [Validation record](validation.md)

The native Windows host settings runner now lives in `apps/host-settings/windows` and builds `KokodaHost.exe`. It owns local configuration and the child-host lifecycle; authorization stays in `kokoda-host`.
