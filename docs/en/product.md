# Product and release plan

[简体中文](../zh-CN/product.md)

Pair a client with a host, browse its allowed windows and open multiple applications independently. Client title-bar close, minimize and maximize always stay local. Default border resizing scales the local view; a per-window option explicitly changes host window size. Buttons inside the remote application still operate the host. See [window roles and scaling](window-management.md).

Use the local input method and clipboard. Commit composed text once. Copy text and images only for the intended window. Apply the global quality preset to every open, new and restored session. Start with the clear 30 FPS preset; frame rates are targets.

Phase one ships macOS, Windows and Linux clients together against the same Windows host. Gate each client on installation, pairing and revocation, two concurrent apps, dialogs, IME and scaling, bidirectional clipboard, live quality changes and recovery without replaying input. Run Linux checks under Wayland and X11 and publish the tested configurations.

Phase two adds a macOS host. Verify capture and input permissions, including denial and revocation, then repeat the matrix for all three clients. Later work includes adaptive streaming and application audio, followed by microphone forwarding. File transfer and drag-and-drop are deferred.

[One-to-one connections](implementation.md#one-to-one-connections)
