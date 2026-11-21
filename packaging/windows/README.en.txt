Kokoda Windows x64 Host · GUI development build

Extract the whole folder, then double-click KokodaHost.exe. No terminal or administrator
privileges are required. Keep kokoda-windows-host.exe and the ffmpeg folder beside it.

1. Click Set connection password and enter an 8–128 character password twice.
2. Choose a sharing scope:
   - Selected windows (default, view only): browse for a program and/or enter title text,
     then click Add rule. Conditions in one rule must both match; any matching rule allows
     a window. An empty list shares nothing. Preview lists matches locally. Verified owned
     dialogs can inherit permission.
   - All application windows (full control): use for applications requiring keyboard/mouse
     input and clipboard. This grants a paired client access to all application windows;
     it does not isolate control to selected windows.
3. Click Start sharing. Wait until the status says sharing is active.
4. In the updated Mac client, enter a LAN address/port shown on Windows and the password.
   No certificate or pairing-folder copy is required. If several addresses are listed,
   choose the network reachable from your Mac.
5. Stop sharing disconnects clients. Stop before editing the scope, then start to apply it.
   Closing the host window also stops sharing; minimizing keeps it running.
   Opening the app always starts in the stopped state.

Settings live in %LOCALAPPDATA%\Kokoda. password.dat is an authentication record protected
by current-user Windows DPAPI, not a plaintext password. Stop sharing to change it; after
restarting, only the new password works. The Mac keeps the password in memory until quit.
Update both endpoints together. Old pairing files are not used or automatically deleted.
Each connection automatically generates a new temporary certificate.
Rules cannot be edited while running. Preview, Start and normal window close save settings.
The UI follows the Windows display language (Chinese or English). Errors such as port
conflicts appear at the bottom of the window. If Windows Firewall blocks the connection,
allow the host only on trusted networks and for the intended client.

This is a development build. Device approval/revocation, live policy changes
and interactive restricted sharing remain pending. CLI tools remain for diagnostics;
normal use starts with KokodaHost.exe.
Immediate switching to another app after Unicode injection can still truncate pending text.
Windows/Linux clients and physical NVIDIA/AMD/Intel driver validation remain in progress.

The ffmpeg folder includes replaceable LGPL DLLs, licensing and provenance. No personal
pairing credentials are included. See LICENSE and THIRD_PARTY.md.

Anyone with the password can authenticate; there is no separate device approval.
Each peer IP may attempt five connections per minute, including successes; wait a minute
if this limit is reached.

With updated clients, independent windows appear separately; owned panels do not get separate taskbar entries. Native close/minimize/maximize/resize controls operate the corresponding remote window in full-control mode. Use Window > End Viewing This Application to stop viewing without closing remote applications. Automatic process following covers verified owners and same-executable descendants; other helpers still need separate selection.
