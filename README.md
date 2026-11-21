# Kokoda

[简体中文](README.zh-CN.md)

Use a window from another computer alongside the apps on your desktop. Kokoda is being designed for one client connecting to one host at a time on a trusted local network.

The first release targets macOS, Windows and Linux clients connecting to a Windows host. The second phase adds macOS hosting for all three clients. The repository contains the product design, shared core and the initial development runtime.

- [Product and release plan](docs/en/product.md)
- [Client and host interface](docs/en/interface.md) · [interactive mockup](docs/en/prototype.html) (open locally)
- [Host sharing permissions](docs/en/sharing.md)
- [Architecture, layout and test plan](docs/en/engineering.md)
- [Contributor and AI development guidelines](AGENTS.md)

License: [AGPL-3.0-only](LICENSE).

[Implementation design and delivery slices](docs/en/implementation.md)

[Product code structure](docs/en/code-structure.md)

The first Windows-host/macOS-client development runtime is available in source. See the [build guide and implementation boundaries](docs/en/development.md). Windows and Linux GUI development clients are also available; see [desktop client build and usage](docs/en/desktop-clients.md). Release authorization, native IME and hardware acceptance remain in progress.

[Selected-window viewing configuration](docs/en/restricted-sharing.md)

Connect using the Windows host address and a connection password; no pairing-folder transfer is needed. See [password setup and security boundaries](docs/en/password-authentication.md).
