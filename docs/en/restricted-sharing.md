# Selected-window viewing (development)

[简体中文](../zh-CN/restricted-sharing.md)

The host can now filter the remote catalog and captured windows using a local JSON policy. This first policy slice is **view only**: input, text/image clipboard actions and clipboard polling are rejected on the host. The macOS client labels its viewers accordingly and does not read the local clipboard for remote actions. The full-control development mode remains available with explicit `--allow-all-windows`.

This does not complete product authorization. Device approval/revocation, safe input/clipboard recipient verification remain pending.

Discovery and selection replies advertise `read_only`; denied interactive requests return `sharing_read_only`. All clients must use these host capabilities for their controls, while the host remains authoritative.

## Configure on Windows

Create a new policy file without overwriting an existing file:

```powershell
kokoda-windows-host.exe --init-policy sharing.json
```

The initial configuration is `{"mode":"selected","rules":[]}` and shares nothing. For example:

```json
{
  "mode": "selected",
  "rules": [
    {"executable": "C:\\Apps\\Example.exe", "title_contains": "Work"},
    {"title_contains": "Kokoda Public Root"}
  ]
}
```

An executable condition requires an existing absolute path, resolved on the host before comparison. Title substrings use Unicode NFC normalization and lowercase comparison; whitespace is preserved. Conditions in one rule are ANDed, while separate rules are ORed. Empty conditions/rules, unknown JSON fields and invalid paths fail startup. Files are bounded to 64 KiB and 128 rules. UTF-8 with or without BOM is accepted.

Preview matches locally, then start in the signed-in user's desktop:

```powershell
kokoda-windows-host.exe --preview-policy sharing.json
kokoda-windows-host.exe --serve-development 0.0.0.0:47831 identity --sharing-policy sharing.json
```

The identity and pairing procedure is in the [development guide](development.md). Without a sharing argument, `--serve-development ADDRESS IDENTITY` returns an empty, read-only catalog. To keep using input and clipboard across all application windows, explicitly use `--allow-all-windows`; the JSON loader rejects `mode: all` so a file cannot silently enable full access.

## Enforcement and limits

The shared rule engine consumes host-observed facts. A dialog inherits permission only through a verified owner chain in the same process instance (PID plus process creation time). An independent window requires its own rule match, even with the same PID. Unreadable process facts fail closed. Executable paths remain host-local; private owner/foreground window IDs are removed from outgoing metadata.

Selection, groups, capture workers and delivered frames recheck permission. Root title changes or detached ownership can revoke access; denied pending data is discarded. An observed denial invalidates the session's resume ticket, and sharing again requires a fresh selection. Already transmitted data cannot be recalled. Title-based matching cannot isolate content against an application deliberately choosing matching titles, and it cannot distinguish private content drawn inside an otherwise allowed window.

Policies are immutable for the host process lifetime. **Restart after editing the file**; restarting disconnects clients and invalidates all resume tickets. Live policy editing is not implemented. Hidden/missing roots fail closed in restricted mode. Capture workers check while active and during bounded queue waits; catalog and delivery checks do not provide an atomic transaction with another application's UI changes.

## Verification

`kokoda-host` tests cover default denial, invalid rules, combined conditions, normalized titles, owner chains, independent siblings, process reuse and revocation. `SharingTests` verifies native client labels and input/IME/clipboard suppression. The Windows fixture and `tests/e2e/verify-sharing.py` exercise the real TLS path; `--empty` checks the default catalog and `--push` exercises streaming. Run only against the isolated `sharing-windows.ps1` fixture, using explicit state/command file paths.

For graphical setup, use [the Windows host GUI](development.md#windows-host-gui).
