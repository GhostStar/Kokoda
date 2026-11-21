Kokoda development client — Windows / Linux

Windows: extract the entire ZIP and double-click Kokoda.exe. Keep its DLLs,
plugins and kokoda-bridge alongside it. The bridge starts in the background;
no terminal, pairing folder or separately launched bridge is needed.

Linux: install the matching architecture/distribution .deb and open Kokoda
from the application menu. The portable tar uses system Qt 6 and FFmpeg;
run bin/Kokoda only after installing the dependencies listed in BUILD.txt.

Start sharing in KokodaHost on Windows. Enter its address (usually port
47832) and connection password. Passwords stay in memory for reconnects and
are not saved. Only windows authorized by the host appear in the list.

Double-click an application to view it. Open another from the same catalog
without disconnecting. Floating dialogs remain associated with their app;
independent windows get separate native frames. Quality applies globally
and changes without reopening. Use the local IME and Ctrl+C/X/V for text
and images. File transfer and audio are not implemented.

Local close/minimize/maximize never operate the remote application's own
window buttons. Reopen locally closed viewers from Window. Each viewer's
Scale view mode preserves aspect ratio; Resize host explicitly changes the
remote window dimensions. On Wayland the compositor owns maximized size,
so unused space can remain around the proportional image while maximized.

This development build is not a signed/notarized production release. See
docs/en/desktop-clients.md and docs/en/validation.md in the source repository
for tested environments and known limits. See LICENSE, THIRD_PARTY.md and
licenses for dependency terms. Hardware acceleration depends on drivers;
KOKODA_SOFTWARE_DECODE=1 forces software decode for diagnosis.
