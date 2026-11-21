# Client and host interface

[简体中文](../zh-CN/interface.md)

Clients authenticate with a host address and connection password; pairing files and per-connection host approval are not required by the current flow. See [password authentication](password-authentication.md) for authentication and certificate lifetime. The Mac manager now uses a connection summary, global quality and Available windows / Viewing lists. See [window manager design](window-manager.md) for states, search, the quick picker and host selection. Qt retains its existing selector; see [desktop clients](desktop-clients.md) for current platform capabilities.

Each app has a native window identified by application and host. Opening it again focuses the existing viewer. Keep dialogs with their session, retain local window positions across reconnects, and block input while disconnected. Menus and settings share one global video quality selection. Show actual frame rate per app. Client title-bar close/minimize/maximize stay local; each decorated viewer has a title-bar Scale view / Resize host selector, also available in the Window menu. Local border resizing and maximize keep the picture proportional and fill its canvas. Explicit host sizing permits independent width/height changes. The Window menu also offers 50–200% display scale and reopening locally closed viewers. In-stream application buttons still operate the host.

The host manages sharing scope and previews matching windows locally. Selected scope is currently read-only, and changing rules requires restarting sharing rather than applying immediately on save. Device approval/revocation and live rule updates remain unfinished. Diagnostics include connection state and encoder without input or clipboard content. Support keyboard navigation, screen reader labels, scaling and native shortcuts.

[Implementation design and delivery slices](implementation.md)

[One-to-one connections](implementation.md#one-to-one-connections)

[Implemented Windows host GUI and its current limits](development.md#windows-host-gui). The lifecycle and sharing interactions described above remain the product target.

[Window roles and remote operations](window-management.md).

Windows/Linux use a compact selector above the canvas rather than an AppKit title-bar accessory. Wayland maximized geometry is compositor-owned. Current platform differences and unfinished parity items: [desktop clients](desktop-clients.md).
