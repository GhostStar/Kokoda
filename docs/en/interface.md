# Client and host interface

[简体中文](../zh-CN/interface.md)

The client opens on Devices. Add an address, enter the pairing code and wait for local approval. A changed certificate requires renewed confirmation. Connected devices show permitted windows, searchable by title. An empty result allows clearing search or refreshing the list.

Each app has a native window identified by application and host. Opening it again focuses the existing viewer. Keep dialogs with their session, retain local window positions across reconnects, and block input while disconnected. Menus and settings share one global video quality selection. Show actual frame rate per app. Client title-bar close/minimize/maximize stay local; each decorated viewer has a title-bar Scale view / Resize host selector, also available in the Window menu. Local border resizing and maximize keep the picture proportional and fill its canvas. Explicit host sizing permits independent width/height changes. The Window menu also offers 50–200% display scale and reopening locally closed viewers. In-stream application buttons still operate the host.

The host opens on Sharing Scope. Preview matching windows locally, save to apply immediately, or cancel the draft. Devices lists paired clients and active sessions for revocation. Diagnostics include connection state and encoder without input or clipboard content. Support keyboard navigation, screen reader labels, scaling and native shortcuts.

[Implementation design and delivery slices](implementation.md)

[One-to-one connections](implementation.md#one-to-one-connections)

[Implemented Windows host GUI and its current limits](development.md#windows-host-gui). The lifecycle and sharing interactions described above remain the product target.

[Window roles and remote operations](window-management.md).

Windows/Linux use a compact selector above the canvas rather than an AppKit title-bar accessory. Wayland maximized geometry is compositor-owned. Current platform differences and unfinished parity items: [desktop clients](desktop-clients.md).
