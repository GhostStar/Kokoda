# Development runtime validation — 2026-11-20

[简体中文](../zh-CN/validation.md) · [Build and limitations](development.md)

This records the initial architecture migration, not first-release acceptance. All windows and credentials used for live tests were temporary synthetic fixtures. No personal application content is included.

## Environment and passed checks

| Check | Result and scope |
|---|---|
| Shared Rust core on Apple Silicon macOS | 24 tests passed: controller, C ABI, admission, request validation, framing, pinned TLS certificate/token rejection, partial I/O and error 997 handling. |
| Windows 11 in Parallels, x64 MSVC target | 34 tests passed, including native pixel/alpha, scaling, capture pacing and encoder argument checks; optimized host and bridge compiled. This is a VM, not a physical NVIDIA/AMD/Intel GPU validation. |
| Native macOS UI | Input, groups, session recovery and multi-session suites passed. Covers local composition, Unicode commits, candidate positioning, focus recovery, popup decoration, Retina sizing, obsolete frames, bounded recovery, discarded input queues and live global quality changes. |
| English and Simplified Chinese | 112 canonical keys, generated resource parity and format arguments checked. Native localization/layout tests passed in English and zh-Hans. |
| Mac → Windows live video (earlier migration build) | WGC with software Media Foundation: 85 frames in approximately 5 seconds, 16.9 FPS; median encode 21.98 ms; median/max control ping 3.69/4.28 ms. Live codec/FPS changes, viewport changes, ordering and resume/close passed. These numbers describe that synthetic VM run only. |
| VideoToolbox decode | Independently decoded 92 captured H.264 frames successfully. |
| Mac → Windows window groups (earlier migration build) | Ownerless card, PNG capture, Unicode, idle-frame suppression, hide/re-show generations, stale-target rejection, destruction, modal hover and single-window scope passed. |
| FFmpeg C++ bridge on Windows | The `h264_mf` probe submitted/drained 60 frames; an independent FFmpeg decoder and ffprobe verified all 60 frames. This does not certify NVENC, AMF or QSV hardware operation. |
| Windows loopback TLS runtime | Two-app Unicode (with observed commits before switching), modifier isolation, stale releases and target checks, resume, independent close, multiline text copy/cut/paste, PNG pixel/alpha preservation and malformed payload rejection passed. |
| Host startup context | Session 0 is rejected before binding a listener; live tests use the signed-in user's interactive session. |

## Open regression: rapid cross-window input

The migrated prototype acknowledges input injection before the target application necessarily consumes it. Committing Unicode text and immediately activating another application can truncate the first application's text. This reproduces with standard Windows text boxes whose change handlers perform file I/O. A low-level delivery marker and a change to foreground activation did not resolve it; those experimental fixes were removed.

`tests/e2e/verify-windows-runtime.ps1 -StressRapidSwitch` preserves the failing reproduction. Normal mode waits for the fixture to observe each text commit before changing applications. A normal-mode pass must not be reported as a rapid-switch fix. A safe general solution must preserve event ownership without indefinitely attaching input queues, recording local keystrokes or replaying text after an uncertain acknowledgement.

## Mac network retest — passed on 2026-11-21

After the owner checked and allowed Kokoda / kokoda-bridge to reach the isolated Windows endpoint, the packaged bridge from commit `a747eb6` completed verified TLS authentication and real application tests. No transport code change, proxy, certificate bypass or network-filter modification was used for this retest. The earlier process-dependent connection failure did not recur; this result follows the owner's permission change and does not independently prove which filter caused the original failure.

- Native AppKit clipboard tests over the real Mac → Windows connection passed: multiline Unicode copy/cut/paste, native-copy polling, empty selections, stale targets and protection against overwriting newer local clipboard content.
- Image tests passed: PNG in both directions, Mac TIFF conversion, pixel/alpha preservation, bitmap-only native copy, malformed/oversized image rejection and recovery after a bad clipboard payload. Tests used named local pasteboards and synthetic Windows fixtures.
- Two application sessions streamed together; independent text input, modifier ownership, late release, wrong-target and duplicate-selection rejection, reconnection/resume and independent close all passed. This test observes each committed text before switching and does not certify the rapid-switch stress case above.
- Push video delivered 102 frames in approximately 5 seconds (20.34 FPS) using software Media Foundation. Median encode time was 20.96 ms; median/max control ping was 3.64/3.74 ms. Live FPS/bitrate/codec changes, viewport changes, ordered frames, resume and close passed. These are VM observations, not physical GPU performance claims.
- Window-group regression with H.264 push passed: independent cards, routed Unicode, idle suppression, hide/re-show generations, stale-target rejection, destruction, modal focus and single-window scope.

Capture lifecycle also passed five open/capture/close cycles, a 60-second disconnected idle interval, then five more cycles after authenticating again.

## Selected-window viewing — 2026-11-21

The new policy engine passed four cross-platform tests covering default denial, rule combinations, canonical Unicode titles, owner inheritance and revocation. Windows additionally passed configuration-loader denial/BOM and native process-identity checks. The workspace totals are now 28 tests on macOS and 40 on Windows.

Real Mac → Windows tests passed in both pull and push modes: only the matching root and verified owned dialog appeared; an independent same-process window and guessed capture IDs were denied. Detaching the dialog or changing the root title revoked viewing, and old resume tickets failed after observed denial. With no sharing option, the real host returned an empty catalog and denied guessed selections and all input/clipboard commands. Restricted controls are denied by the host, not merely hidden in the client.

Native `SharingTests`, the existing input suite, and English/Chinese layout tests passed. All 114 localization keys and generated resources were checked. Full-control mode retained two-app streaming/input/resume and the Mac text/PNG/TIFF clipboard regression passes. The x64 package script produced a ZIP containing only explicitly staged binaries, FFmpeg dependencies, license, bilingual instructions and an empty policy example; no working identity is packaged.

See [scope and configuration](restricted-sharing.md). Static policies require restart; live policy editing and restricted interactive control are not implemented. The rapid-switch input defect above remains open.

## Release gates still open

- Interactive restricted sharing, live policy editing, approval/revocation.
- Windows and Linux GUI clients, including Wayland and X11 behavior.
- Physical GPU and driver matrix, fresh installations, signed distribution and upgrade checks.
- Shared recovery/scheduling integration and a complete typed, versioned protocol contract.

Full control still requires explicit `--allow-all-windows`; selected-window viewing uses `--sharing-policy FILE`. This remains a development build, not a first product release.

## Windows host GUI — 2026-11-21

The workspace passes 30 Rust tests on macOS and 42 on Windows x64. `HostGui.Tests.ps1` passed against the release GUI in the signed-in Parallels Windows desktop: GUI PE subsystem, native MSAA button role/action, invalid port, occupied-port failure, real loopback TLS with exported pairing files, default-empty catalog, title-rule preview and enforcement, explicit full-control capability, settings persistence, stop/restart, close and forced-GUI-exit child cleanup. Temporary identities and the one-off launch task were removed. Chinese layout was inspected at the VM's 250% scale; 148 bilingual resource keys passed validation. The ARM64 runner's UI Automation proxy exposed the x64 button as a pane; its native MSAA role/action passed. This does not establish full screen-reader compatibility on every machine.

`KokodaHost.exe` is now the normal Windows entry point in the x64 ZIP, with GUI-first English/Chinese instructions. The Mac executable is unchanged by this UI-only feature; generated shared localization resources were updated in source. No new physical GPU or cross-machine media claim is made by this GUI regression.

## Password authentication — 2026-11-21

The password path replaces pairing-folder export in the normal Windows GUI and Mac client. The workspace passes 34 Rust tests on macOS and 47 on Windows x64, including real TLS/OPAQUE success and wrong-password rejection, fresh certificates on successive connections, exporter-context mismatch rejection, bounded auth/prelude input, attempt limiting and native DPAPI tamper rejection. Native Mac secure-field and English/Chinese localization tests pass; all 154 bilingual keys match generated resources.

The release Windows GUI test passes password setup and replacement, old/wrong password rejection, no plaintext password in settings, no generated pairing directories, authenticated scope enforcement, restart and close/crash child cleanup. The GUI was checked at the VM's 250% display scale.

`PasswordLiveTests` passed on the actual Mac → Parallels Windows connection: the native Mac app logged in using stdin-delivered credentials, rendered the isolated fixture, restored authentication and its viewer after bridge termination, and stopped retrying on an incorrect password. Process arguments and diagnostics did not contain the test secret. Named pasteboards and temporary credentials were used; test hosts, fixtures, tasks and the temporary Mac-only firewall rule were removed. This is not a new physical-GPU or full media regression claim.

Run the native test with `scripts/run-macos-test.sh PasswordLiveTests BRIDGE ADDRESS SECRET_JSON` against an isolated host containing the synthetic `Kokoda Password Fixture`; the JSON contains a temporary `password` field and must be deleted afterward. See [password authentication](password-authentication.md) for the prerelease dependency and security-review boundary. The existing rapid-switch input issue and release gates above remain open.

## Popup video and native dragging — 2026-11-21

All 34 macOS and 48 Windows x64 Rust tests pass. Native Mac group/multi-session, alpha composition and asynchronous frame tests pass: opaque/partial/cached/scaled masks, malformed masks, independent Retina viewports, global quality routing, ordered background decode, bounded reader backpressure, disconnect release and stale-completion rejection.

The real password-authenticated Mac → Parallels Windows test used three simultaneous synthetic animations: an ordinary owned popup, a layered popup with 65% uniform opacity, and an independent same-application window. With software Media Foundation, legacy PNG measured 6.95 / 14.70 / 14.50 FPS (independent / layered / ordinary), versus 19.36 / 19.96 / 19.16 FPS with per-window H.264. Control ping median/max was 3.62 / 3.94 ms. This is one VM workload, not a physical-GPU benchmark or guaranteed frame rate.

Every popup switched live between video/image modes and FPS settings; each changed epoch, and a single-popup viewport change left other windows' dimensions independent. Recorded H.264 decoded through Mac VideoToolbox; the layered-window output measured alpha 165/255. A native four-viewer test completed 46 scheduled window moves during two seconds in event-tracking mode and received 48 popup frames across tracking plus 0.4 seconds of recovery, without reconnect or position snapback. The earlier synchronous UI decode path failed this tracking responsiveness test; the background path passed.

Tests use only synthetic windows and temporary credentials. Their hosts, fixtures, launch task and Mac-only firewall rule were removed. Existing user client sessions were left running. The Mac build now compiles/signs a separate bundle before replacing the old directory, so it never truncates a running client's executable; quit and reopen to use the new code. See [window video](window-video.md) for negotiated compatibility and remaining GPU/rendering limits.

## Remote title-bar coordinate feedback — 2026-11-21

The workspace passes 35 macOS and 49 Windows x64 Rust tests. Native Mac input, group, drag and multi-session regressions pass, including frozen screen coordinates across viewer movement/resize, cumulative move coalescing without losing gesture boundaries, outside-view release, focus-loss cancellation and legacy-host compatibility. All 154 bilingual resource keys match generated resources.

The real Mac bridge → Parallels Windows password connection reproduced the old moving-origin bug: normalized samples requesting a cumulative 40-pixel horizontal drag displaced the standard window by 100 pixels. With anchored gestures, both standard and custom borderless title bars completed forward/reverse movements, repeated samples and return-to-origin with a maximum measured position error of 0 pixels. Geometry reports matched the fixture. Native text selection beyond the window and explicit button release passed. The fixture's UI timer may pause at native caption-down until the first movement, so that initial mouse-button state is not used as a timing assertion.

This validates real Windows input/capture and separately tests Mac coordinate conversion/queue behavior; it is not a physical QQ, multi-monitor or packet-loss benchmark. TCP + TLS 1.3 remains unchanged. Both endpoint builds are required to negotiate the new input path. Temporary hosts, windows, credentials, launch task and the scoped firewall rule were removed after the test; the user's running client was left untouched.

## Input pipeline and playback feedback — 2026-11-21

All 42 macOS and 56 Windows x64 Rust tests pass. New tests cover partial/997 queued writes, real TLS receiving control while its video writer is blocked, bridge FIFO response correlation with interleaved frames, per-window credits, invalid/duplicate feedback and conservative rate recovery. Native Mac tests cover 12 ordered input batches before acknowledgement, clipboard barriers, fragmented background JSON/Base64 parsing and input bounds, independent decoding, control delivery during blocked decode, disconnect and stale-result rejection. All 154 bilingual resource keys remain consistent.

The actual password-authenticated Mac bridge → Parallels Windows test withheld consumption feedback for one synthetic popup: it stopped at exactly two frames while an independent window delivered 63 frames in three seconds. Control ping median/max was 2.40 / 4.11 ms for this workload. Returning credits resumed playback. Deliberately injected 80 ms decode telemetry reduced the 30 FPS ceiling to 22 FPS, then normal telemetry recovered it to 24 FPS; a live 15 FPS preset capped all members. These injected values test controller behavior, not hardware decode speed.

With background IPC parsing and two concurrent decode workers, the native four-viewer test completed 26 scheduled window moves during two seconds of event-tracking mode, with 54 popup frames across tracking and 0.4 seconds of recovery, no reconnect and no snapback. Forced bridge termination then restored authentication, consumption-feedback negotiation and all four viewers. Real Windows standard/custom title-bar drag, reversal, repeated samples, geometry reporting, outside text selection and release passed again with maximum measured position error of 0 pixels.

These are synthetic tests on a Mac and a Parallels Windows VM, not physical NVIDIA/AMD/Intel, WAN-loss or guaranteed-FPS benchmarks. TCP/TLS/OPAQUE remain; two outstanding frames per window bound backlog but can limit throughput on high-RTT links. Rate adaptation stays below the global preset and does not change resolution. Existing rapid cross-application Unicode input limitations remain open. Both endpoint builds are required for consumption feedback. Temporary hosts, fixtures, credentials, task and scoped firewall rule were removed; the VM was suspended and the user's existing Mac client was left untouched.


## Window roles, lifecycle and remote resize — 2026-11-21

All 44 macOS and 58 Windows x64 Rust tests pass. Shared role fixtures and process-family tests cover independent/owned/modal/transient classification, exact executable identity, descendants, unrelated siblings, missing facts and PID reuse. Native Mac window-management, grouping, multi-session, input, pipeline, asynchronous-frame, sharing and English/Chinese localization suites pass; 156 bilingual resource keys match generated outputs.

Real Mac → Parallels Windows tests passed role classification including an owned APPWINDOW that remains independent; root/owned physical resize, native minimum-size constraints, fixed-size and stale-token rejection, unchanged host mouse position, minimize/maximize/restore, modal owner blocking, system-menu input/dismissal, descendant discovery, unrelated same-executable process exclusion, per-window close and surviving-root promotion. A separate restricted read-only host rejected all six window actions; child-process discovery did not expose denied catalog/group members.

The native five-viewer live test passed actual Mac popup border resize changing Windows bounds, stable surface identity and resumed playback after minimize/restore, zoom mapped to remote maximize/restore, modal owner input/focus blocking and dialog close, a separately presented descendant-process window, root close preserving an independent window, and End Viewing leaving the remote app running. Random-directory bridge copies timed out during this run while the stable bridge path connected; the test harness now links the caller's existing bridge executable and the full native test passes. This is consistent with program-specific local network filtering, not proof of a transport regression or its resolution.

The Windows classifier is heuristic for custom toolkits. Automatic process following covers verified owner chains and same-executable descendants; different-executable ownerless helpers and broker-launched existing processes still need separate selection. Windows/Linux native clients, a physical QQ/WeChat matrix, multi-monitor resize and WAN performance remain unvalidated. See [window management](window-management.md) for the cross-platform contract and close-versus-End-Viewing distinction. Both endpoint builds are needed for the new capability.

The signed Mac bundle and credential-free Windows x64 GUI archive were verified. Test processes, temporary credentials, launch task and scoped firewall rule were removed; the VM was suspended. Existing user client processes were left untouched.

## Application icon — 2026-11-21

The selected teal framed artwork is packaged as macOS ICNS, Windows ICO and a Linux-ready 512 px PNG. The Mac bundle declares its icon and passes signature verification, with ten 1×/2× representations. The Windows x64 GUI embeds one Explorer icon group; native Windows resource APIs successfully loaded all ten sizes from 16 to 256 px. The Windows workspace passed 58 tests and the x64 GUI ZIP was rebuilt. The icon-only Mac update compiled and passed bundle/resource checks; no new streaming benchmark is claimed. No test host or credentials were created for this check, and the VM was suspended afterward. See [icon generation](../../packaging/icons/README.md).

## Stable video quality and Retina bitrate — 2026-11-21

The workspace passes 49 macOS and 63 Windows x64 Rust tests. New controller tests cover prolonged local queue pressure without bitrate loss, delivery congestion with a per-frame clarity floor, bounded recovery, isolated spikes, mixed causes, idle gaps, resized ceilings and stale-epoch feedback. Pixel-budget tests cover Retina scaling, quantization and the per-window cap. Native pipeline/asynchronous-frame tests verify receiver-owned timestamps, UI and feedback scheduling delay accounting, no local timestamp on the wire, stale-stat rejection and capability negotiation. Multi-session and English/Chinese full-statistics layout tests pass; 161 bilingual keys match generated resources.

The actual Mac bridge → Parallels Windows test paused one synthetic popup at two credits while its sibling delivered 60 frames in three seconds (control ping median/max 2.67/16.41 ms for this run). Injected slow decoding reduced 30 FPS to 24 and recovered to 27; sustained injected client queue pressure retained 2 Mbps instead of lowering image quality. Resizing one popup to 1280×720 with a 640-wide reference preset gave it an 8 Mbps budget while the other retained its independent 2.25 Mbps budget. Live preset replacement reset all members to the new ceiling. These injected delays validate controller behavior, not actual network or hardware decode performance.

The native four-viewer test passed live drag presentation and automatic reconnect with the new quality controller. The pinned FFmpeg build exposes NVENC P3 and ultra-low-latency options; the new buffer is bounded to at most three frames/100 ms, except one-frame minimum at low FPS. Windows VM software encoding was exercised; no physical RTX 5090D/NVENC quality or latency improvement is certified. Source resolution remains unchanged by congestion adaptation. See [current video policy](window-video.md) for the negotiated pixel-budget limits and delivery-delay measurement boundary.

The Mac bundle passed signature/resource checks and the final x64 GUI archive passed architecture/credential checks. Test hosts, fixtures, credentials and the scoped firewall rule were removed; the VM was suspended and existing user clients were left running.

## Local viewer controls and independent scaling (2026-11-21)

This supersedes the earlier native-title-bar remote-close/minimize/zoom behavior. Mac title-bar close/minimize/maximize now remain local, including in the explicit host-resize mode. Default border resizing scales the local view. The host reports capture-monitor DPI; 100% local size is normalized independently of Retina video pixels. Bilingual window menus add resize mode, display scale and reopening locally closed viewers.

49 Rust tests passed on macOS and 63 on Windows x64. Native WindowManagement, Group, MultiSession, Session and Input regressions passed, including disabled/read-only local close, hidden-frame suppression, reconnect preferences, legacy-popup close, 250% source-DPI conversion and 150% local zoom. The real Mac → Parallels Windows session passed local border scaling without a host bounds change, opted-in remote panel resizing, local minimize/restore, normal-window and panel maximize/restore without host changes, local modal close retaining host blocking, bounded hidden-frame feedback and resumed animated video, descendant windows and independent-sibling survival. The synthetic role fixture now offers an explicit animation command so playback tests do not depend on static windows producing frames.

Actual AppKit zoom emits live-resize notifications, so native wrappers suppress remote size requests during it. Panel zoom also needs a final owner-relative offset update; otherwise a later snapshot moves the enlarged panel off screen. Both cases were reproduced and passed after correction. Locally closed child windows detach from native parent ordering until restored, preventing reattachment from reopening them. Windows/Linux GUI clients, physical multi-monitor DPI changes and the user's RTX/QQ combination were not retested in this run.

## Unframed application icon adaptation (2026-11-21)

Replaced the earlier framed build input with the user's supplied unframed PNG, kept unchanged in `docs/branding/kokoda-logo-unframed.png`. The native preprocessing step finds an 880×670 visual bounding rectangle including edge margin, fits it to 896×682 on a transparent 1024×1024 canvas and retains the supplied colors and window composition. No enclosing frame or white background is added. ICNS, ICO and Linux PNG now share this normalized master.

The Mac bundle compiled, passed strict signature verification and contained the exact generated ICNS. Kokoda was normally quit and reopened; the system-returned application icon was visually checked and shows the enlarged unframed mark. macOS still supplies its rounded background treatment. Windows passed 63 workspace tests; native resource APIs loaded all ten icon sizes from the rebuilt x64 GUI EXE, and its extracted 256 px icon was visually checked. This is an icon-only change; no new streaming performance result is claimed.


## Direct resize controls and proportional local viewers (2026-11-21)

Native ViewerSizing, Input, WindowManagement, Group, MultiSession and Session tests pass. ViewerSizing and Localization pass in English and Simplified Chinese; all 175 shared bilingual keys match generated resources. Coverage includes title-bar selectors routed to their owning panel despite another key window, IME responder restoration, horizontal/vertical proportional resizing, free-axis host sizing, switching back without padding, source bounds arriving before decoded video, portrait/wide source changes, native window/panel zoom, read-only and blocked targets. Proportional rendering covers sub-point frame-rounding seams without stretching the image.

The actual Mac → Parallels Windows five-viewer test passed unchanged host bounds under local scaling, title-bar-selected popup resizing changing the host dimensions, no padding after local resize/zoom, local minimize/restore and normal/panel zoom in host mode, modal close/restore with bounded hidden-frame feedback, separate child-process windows and surviving independent siblings. Temporary credentials, host/fixture processes and the scoped firewall rule were removed, and the VM was suspended. These are synthetic Windows VM tests, not a physical QQ/WeChat or multi-monitor hardware certification.

Testing exposed an AppKit zoom crash after assigning a zero content aspect ratio. The implementation cancels the ratio using unit resize increments instead, consistent with Apple's [aspect-ratio reset guidance](https://developer.apple.com/documentation/appkit/nswindow/aspectratio). Native test launches also ignore saved window state so a previous failing assertion cannot leave subsequent tests waiting at a restore-windows prompt. The rebuilt Mac bundle passed strict signature verification; no host protocol or binary change is needed for this update. The user's running client is not terminated; quit and reopen it to load the new executable.

## Windows / Linux GUI clients (2026-11-21)

The Qt client passed **16 cases** on Windows 11 x64 (MSVC/Qt 6.8.3 under Parallels)
and **16 cases** on Ubuntu 26.04 ARM64 (Qt 6.10.2, FFmpeg 8.0.1, X11/Xvfb), including
real password connections to the isolated Windows host. The live case observed
five authorized application windows, video/image quality changes without replacing
viewers, local proportional resize with unchanged host bounds, opted-in host resize,
local close/reopen, committed Chinese/emoji text, text paste and authenticated
reconnection with preserved geometry. Linux additionally verified remote text and
PNG clipboard transfer between separate operating systems. Linux used a temporary
Mac loopback relay because the machine's global proxy prevented direct VM routing;
this validates the real protocol, not direct-LAN latency/performance.

Wayland/Weston headless passed **15 cases**, with the optional live case skipped.
It covers native window behavior, composition events, focus release, zoom and
read-only handling, core ABI/loading, bilingual UI and repeated non-aligned H.264
frames. Both English and Chinese UI screenshots were inspected. CTest offscreen
also passed. The Mac Rust client/bridge tests passed (6 cases); localization checks
passed for 184 shared keys and generated native resources.

The new regressions found and fixed two implementation defects: tightly allocated
Qt RGBA memory did not satisfy FFmpeg conversion padding, and clipboard polling
could run after authentication but before application resume, invalidating the
session. FFmpeg-owned aligned buffers and resumed-session gating now cover these.
Fixture commands now include observed acknowledgments so VM file-sync delays do
not masquerade as input failures.

Physical GPU decoding/performance, real Windows IME candidate windows, Linux
IBus/Fcitx integration, Wayland clipboard over a live host, HDR/color fidelity and
multiple display/DPI transitions still require hardware/user validation. VM tests
do not certify RTX/AMD/Intel acceleration. The Qt video presenter currently copies
GPU output back to CPU; it is not a zero-copy renderer. File transfer/audio remain
unimplemented. See [desktop clients](desktop-clients.md) for build and limits.


## Mac manager and window selection — 2026-11-22

The new AppKit manager and shared state/action/search contract passed local regression on Apple Silicon macOS. The manager suite uses synthetic protocol responses and real native controls, covering duplicate/empty titles, row reordering, observed handle reuse, stable selection, pending-open deduplication, owned-window expansion, targeted restore without restoring independent siblings, modal routing, cancel/late-response rejection, viewer-generation checks and revocation/disconnect metadata cleanup. English and Simplified Chinese layouts were captured using only synthetic windows and inspected; collapsed and expanded settings fit the content bounds.

`cargo test --workspace --locked` passed (50 tests on macOS), along with `ManagerTests` in both languages, `LocalizationTests` in both languages, `MultiSessionTests`, `SessionTests`, `GroupTests`, `WindowManagementTests`, `ViewerSizingTests`, `InputTests`, `DragTests` and `PasswordTests`. Locale validation checked 211 bilingual keys. The GitHub macOS job now runs these native regressions. Test applications exited after completion; no Windows fixture or host was started for this round.

This is local UI/model regression, not renewed Windows networking, Qt UI, physical GPU, performance or live IME-candidate acceptance. Live fixture drivers were updated to select through the new manager model but were not rerun. Previous documented limitations remain.

The first CI run exposed a screen-dependent assumption in the legacy group fixture: a requested native move was treated as its actual position. The fixture now checks the actual content position against the layout origin, preserves the no-snap-back assertion, and resizes the parent relative to its current size so smaller CI screens exercise the same behavior.


## Host picker and LAN discovery — 2026-11-22

The Windows x64 GUI and host built successfully in the Windows 11 Parallels VM; workspace tests passed on Windows and on Apple Silicon macOS (52 tests on macOS). English/Chinese discovery, manager and localization AppKit suites passed, and 236 locale keys passed parity checks. The signed Mac application was rebuilt.

The interactive Windows GUI regression passed against isolated native forms: searching the local catalog, a title change appearing through automatic refresh without losing selected identity, direct versus owned preview (excluding an independent same-process sibling), adding the combined executable/title rule, default discovery opt-in, password setup/rotation/rejection, OPAQUE/TLS loopback, empty-default sharing, preview, full scope, restart, normal exit and forced-exit child cleanup. Chinese host/picker screenshots were inspected at the VM's display scale; they contain only synthetic test content.

The Mac browser discovered the actual Windows GUI fixture's IPv4 endpoint using the new mDNS component. Local advertisement/goodbye lifecycle also passed. An initial 12-second local discovery attempt timed out; the system Bonjour browser observed the synthetic announcement, and the subsequent lifecycle and two-machine runs passed. This is not a guaranteed discovery-latency benchmark. No firewall rules were changed. Temporary fixture windows, credentials and scheduled task were cleaned up; the test VM was returned to its prior suspended state. This round does not certify physical GPU performance, routed/VPN multicast, or Windows/Linux discovery UI.

## Catalog identity and filtering — 2026-11-22

Windows x64 workspace tests and the isolated interactive host GUI suite passed in the Windows 11 ARM Parallels VM. Actual catalog checks covered the executable basename without paths, embedded 32×32 icon with transparent pixels, size/DPI, preservation of normal titleless forms, auxiliary classification of tool forms, restored dimensions for minimized forms, and unchanged empty-default/selected-scope enforcement. The Mac manager successfully decoded an icon extracted from the synthetic Windows host GUI.

Apple Silicon workspace tests, bilingual ManagerTests (including minimum-size/expanded-options bounds, app search and all three filters), DiscoveryTests, LocalizationTests, MultiSessionTests and GroupTests passed. Native screenshots were inspected. Locale validation passed 245 paired keys. The Mac app signature and Windows x64 package were checked. This verifies catalog/UI behavior, not physical GPU performance or WinBridge pixel-for-pixel parity; Qt retains its existing picker. Test fixtures, temporary credentials and scheduled task were cleaned up, no firewall changes were made, and the VM was restored to suspended state.

## Popup pointer and wheel regression — 2026-11-22

Mac and Windows x64 workspace tests passed. Mac InputTests, DragTests and GroupTests passed; the signed Mac app and Windows GUI host package were rebuilt. In the interactive Windows 11 ARM VM at 250% target DPI, a dedicated native fixture received a real click after moving during activation, received a positioned wheel from a deliberately DPI-unaware caller, confirmed restoration of caller awareness, and rejected a genuinely covering topmost window. The DPI-unaware case failed before the scoped input DPI fix and passed afterward. An earlier probe also observed an intermittent missing click before that fix; these are synthetic checks, not a WeChat-specific reproduction.

The reported WeChat Moments failure still requires the user's physical-host acceptance. Diagnostic failures now distinguish broad hit-test categories without exposing the blocker identity. Initial sandboxed workspace networking tests were denied local sockets; the authorized unsandboxed rerun passed. Only fixture-owned windows were driven; temporary test processes/tasks were removed and the VM returned to suspended state. Qt normalized-coordinate and positioned-wheel regressions are included in the desktop CI suite.
