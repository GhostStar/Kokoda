> Current authentication: [address + password](password-authentication.md). Older pairing-code/device-approval sections below are reserved future design, not the current connection flow.

# First release implementation design

[简体中文](../zh-CN/implementation.md) · [Interactive prototype](prototype.html)

## Screens and interaction

| Screen | Layout, actions and feedback |
|---|---|
| Client devices | Device sidebar and selected host details. First launch emphasizes Add Device. A paired device must still authenticate with the connection passphrase. Enter the address and the code shown on the host for a new device. Allow cancellation while awaiting approval, another code after expiration and identity verification after a certificate change. |
| App picker | Focus search on connection. Rows show app, title and Open or Return to Window. An empty host catalog directs the user to the owner; an empty search offers Clear Search. Refresh preserves selection and open sessions. |
| Viewer | Native window titled with app and host. Put statistics and global quality in the Display menu. Open multiple apps side by side; opening the same target focuses its viewer. A modal dialog blocks input to its parent. Dismissing the host dialog restores parent input; closing only its local viewer does not. Client title-bar close/minimize/maximize stay local. Border resizing defaults to local scaling, with an explicit per-viewer host-size mode. End Viewing releases the session without closing the application. See [window roles](window-management.md). |
| Host sharing | Tray shows current sharing status. Home shows scope, current connection status and Pause Sharing. Default to selected windows. Each rule row accepts an executable, a title substring or both. Show matching windows and reasons beside the editor. Before saving, show the additions and removals. Confirm expansion to all windows locally. Cancel discards edits. |
| Host devices and diagnostics | Show pending clients and identity verification details. Rejection or expiry ends the request. Show active session counts and explain that revoking a device disconnects it immediately. Copy diagnostics with connection stage, error category, encoder and frame rate. Exclude window titles, input and clipboard contents. |

Use the same action sequence and outcomes on each client. Use Command on macOS and Ctrl on Windows/Linux. Respect native window decoration, scaling, accessible labels and keyboard navigation; the Wayland compositor controls popup placement. Preserve user text and remote titles. The prototype uses local simulated devices and windows.

## Components and interfaces

| Owner | Responsibilities |
|---|---|
| Shared controller core (crates/client) | Accept pair, open, close and quality actions. Own connection state, per-app sessions, request correlation and recovery. Publish device, catalog and session changes to native views. Store global quality per client and apply it on connection, open and resume. |
| Native controllers (apps) | Own native windows, coordinate mapping, IME, decoding and clipboard. Route input only through the active session. Compose locally and send committed text. |
| Windows host (crates/windows-host) | Run in the signed-in user's interactive session. Authenticate devices and filter discovery. Each permitted session owns a window group, input target and bounded capture queue. Fall back on encoding failure and report the actual encoder. |
| Local management UI (apps/host-settings) | Read status, preview and save policy, approve and revoke devices through an interface restricted to the local user. Prefer a Windows named pipe with a user ACL. Closing settings leaves the host enforcing its saved policy. |
| Shared model and transport | Carry capability and version negotiation, correlated replies, window events and media over TLS. Distinguish device identity, session, window generation, media sequence and policy revision. Localize stable protocol error codes in the UI. |

### Interaction sequences

1. **Pair and recover.** The host issues a short-lived single-use code. The client requests pairing; the owner compares the identity shown at both ends and approves. The client pins the host identity and the host stores revocable client credentials. Cancellation, rejection or expiry ends the request. Use system credential storage. On temporary loss, drop queued input, pastes and stale frames. Verify the device and current connection passphrase, negotiate and verify the window identity before restoring. Revocation or identity mismatch stops automatic retries.
2. **Open and change quality.** The client selects a generation-bound target from the filtered catalog. The host authorizes it again, allocates a session, returns its window group and starts streaming. Create the viewer before displaying frames. Deduplicate retries by request ID. Send the latest global quality revision to each session, ignore obsolete acknowledgements and show actual state for sessions that fail while others continue.
3. **Apply policy.** Preview a draft locally. Save against the original revision; on conflict ask the owner to reload. Atomically replace the policy, reassess active sessions, stop capture and input to revoked targets and discard queued data. Notify viewers to clear surfaces and release focus. Narrowing scope, pausing sharing and revoking a device take effect on existing connections. Opening a newly reauthorized window requires a fresh selection.

## Delivery slices

| Order | Demonstration | Acceptance |
|---|---|---|
| 1 | Simulated host drives both UIs and the shared state model | First run, cancelled or denied pairing, empty catalogs, cancelled drafts, duplicate opens and revocation. Complete the flows by keyboard in both languages. |
| 2 | Windows host and macOS client complete the first live session | Start with an empty allowlist, pair and share a fixture window. Reject guessed IDs, stale generations and invalid resume tickets. A focus change must not redirect input to an unauthorized window. |
| 3 | Windows and Linux controllers implement the same contract | Exercise two apps, dialogs, composed text, bidirectional image/text clipboard, global quality and recovery on each platform. Cover Wayland and X11. |
| 4 | Fresh installs and upgrades | Record actual encoder and video performance. Revoke old credentials and stop streams on narrowed authorization. Release after all three clients pass. |

Evaluate Windows and Linux UI toolkits against the same input and window fixtures before selecting dependencies. In restricted mode, fail closed if the input recipient or clipboard owner cannot be verified. See [host sharing](sharing.md).

## One-to-one connections

Each client connects to one host at a time. Each host admits one active client. The connection can carry several independent app windows. To change computers, disconnect first.

Every connection and reconnect verifies the host connection password using OPAQUE bound to a fresh TLS 1.3 channel. There is no manual certificate copy or first-device approval in the implemented flow. Anyone holding the current password may authenticate, subject to the one-client limit and host sharing policy. Windows protects its registration with current-user DPAPI; clients do not persist passwords yet. See [password connections](password-authentication.md).

Reserve the connection slot atomically at admission. If occupied, return Host busy without revealing the connected device or its windows. Disconnect, revocation, passphrase rotation or heartbeat timeout ends the connection, discards queued input and invalidates the old connection before freeing the slot. Reconnect must authenticate and claim a free slot again. Apply the host's window rules to every app session.

Acceptance: open two allowed apps on one connection; attempt a second connection and verify the first remains usable. Disconnect or time out the first, reconnect with valid credentials and confirm old input is never replayed.
