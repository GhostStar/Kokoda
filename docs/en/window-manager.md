# Window manager and selection design

[简体中文](../zh-CN/window-manager.md)

Status: the first macOS manager slice is implemented on 2026-11-22. Qt still uses its existing selector. The sections below define the complete target design; the delivery notes distinguish shipped behavior from planned work.

## Reference and decisions

Reviewed WinBridge source at `bd19a74477bcc0bb2d2cfc4633d11b62eb409374`. Its GUI was not run; no code or assets were copied:

- [ManagerWindowController](https://github.com/WormW/winbridge/blob/bd19a74477bcc0bb2d2cfc4633d11b62eb409374/mac-client/Sources/WinBridgeCore/UI/ManagerWindowController.swift): connection profiles, running-window/application-launch tabs, icons and process subtitles, per-row project/detach actions, connection-dependent controls. Its manager search field is only shown on the application tab, not shared across both tabs.
- [QuickLauncherMatcher](https://github.com/WormW/winbridge/blob/bd19a74477bcc0bb2d2cfc4633d11b62eb409374/mac-client/Sources/WinBridgeCore/UI/QuickLauncherMatcher.swift) and [QuickLauncherWindowController](https://github.com/WormW/winbridge/blob/bd19a74477bcc0bb2d2cfc4633d11b62eb409374/mac-client/Sources/WinBridgeCore/UI/QuickLauncherWindowController.swift): combined window/application search, relevance and subscription ranking, arrow navigation, Return to act and Escape to dismiss.
- [AppState](https://github.com/WormW/winbridge/blob/bd19a74477bcc0bb2d2cfc4633d11b62eb409374/mac-client/Sources/WinBridgeCore/App/AppState.swift): focus existing viewers, unsubscribe, reconnect restoration and layout memory. Layout keys primarily use process names; restoration can fall back to a process-name match. Kokoda must not use those as identity or authorization evidence.

Adopt direct list actions, explicit states, quick search and layout restoration principles. Do not adopt remote application closure through the local red button, a normal setting to disable TLS, a full-desktop shortcut, or arbitrary same-process restoration. Quality stays global; sizing mode stays per window. File transfer remains deferred. Application launch, audio, discovery and wake-on-LAN are not implicitly included in this implementation slice.

## Client manager

Start in the window manager. When disconnected, show the connection form; after authentication, collapse it into a host summary rather than keeping passwords and technical parameters in the selection workflow. A client still connects to only one host at a time.

| Area | Content and actions |
|---|---|
| Connection summary | Host alias/address and disconnected/connecting/connected/reconnecting state; offer Connect, Cancel, Disconnect or Stop reconnecting as appropriate. Request a password only for authentication; do not persist it. |
| Global toolbar | Global quality and connection settings. Put clipboard, adaptive resolution and automatic reconnect in settings. Keep actual FPS, bitrate, dimensions and encoder in diagnostics rather than making codec names primary controls. |
| Navigation | Available windows and Viewing, both searchable. Counts represent independently listed/viewed windows, not a mixture of applications and transient surfaces. |
| Window row | Authorized application icon or generic fallback, title, application name, status, primary action and overflow menu. Put resolution/DPI in details. Use the application name or Untitled window for empty titles instead of hiding them. |
| Status | Actionable reasons distinguishing no shared windows, no search matches, a disappeared target, read-only access and disconnection. Do not expose unauthorized window counts or titles. |

Do not initially add a Launch applications tab. Authorized running windows and launchable applications are separate permission domains. Only expose a launch catalog when the host explicitly grants the relevant catalog and capability; do not expose the entire Start Menu by default.

Viewing has independent windows as primary rows, with expandable owned floating/modal windows. Transient menus and tooltips are not permanent rows. Identify rows by connection epoch, session and window generation, never by title, row index or PID alone. Application names are display/search data, not automatic group membership. Use generic presentation when the protocol lacks safe names, icons or roles; do not transmit full executable paths to fill UI fields.

Search ranks exact title, prefix and substring before fuzzy matches; break equivalent matches using viewed/recent items. Chinese matches original text; pinyin search requires a separate implementation and is not implied by Latin subsequence matching. Refresh preserves selection and scroll position. If the selected identity disappears, clear selection rather than applying an action to a replacement at the same row index.

## Viewer states and actions

| State | Double-click / Return / primary action | Other behavior |
|---|---|---|
| Available, unopened | Open and enter Opening | Read-only windows remain viewable; host rechecks identity and authorization. |
| Opening | Show progress without duplicate requests | Allow retry after failure; late success after End viewing must not reopen a viewer. |
| Visible | Switch to this local viewer | Do not create a duplicate; native activation is subject to platform restrictions. |
| Locally minimized/closed | Restore / Show again | Preserve sizing mode; do not alter host minimize/close state. |
| Blocked by a modal window | Switch to the authorized modal viewer | Do not bypass host blocking; explain if the modal is unavailable. |
| Reconnecting | Suspend opens and remote input | Clear old frames and retain local layout intent; Stop reconnecting remains available. |
| Target gone/access revoked | Reject reopening the old identity | Clear media and private metadata; any remaining status must be title-free. |

Close local window hides that viewer and its owned floating descendants, preserving independent siblings. Existing consumption suspension does not mean host capture resources were released. End viewing this application releases the entire application session and its media resources without closing the host application. Keep these actions separately named rather than using an ambiguous Close or Detach. Termination is currently session-wide; this design does not assume an existing per-member unsubscribe protocol.

Follow related windows is a viewing preference, always intersected with host authorization, not permission to share an entire process. Distinguish modal/necessary owned dialogs from optional same-application independent windows. Independent windows get normal local frames, floating/modal windows use their actual owner, and menus stay undecorated. Live membership changes need an explicit protocol and identity checks before being enabled; the current selection-time scope option must not be presented as a live toggle.

Viewers retain Scale view / Resize host. Local close, minimize and maximize always stay local. Row menus offer switching, showing again, sizing mode and ending viewing; do not add remote Close by default. In-stream application buttons continue to operate the host through authorized input.

## Quick selection and layout

Add Find window… to the menu, with Command+K on Mac and Ctrl+K on Windows/Linux bound by default only in the manager, avoiding interception of remote application shortcuts. Future global shortcuts are user-configured, with a menu fallback where Wayland does not support them.

The picker reuses the same authorized catalog, states and actions. Arrows select, Return acts, Escape dismisses; Return goes to the IME first during composition. Search only currently authorized windows. Clear the catalog on disconnect and show a connection action instead of retaining private titles. After opening, return input focus to the target viewer. Catalog refresh must not steal focus from the search field.

Initially preserve existing in-connection layout restoration only. Future persistence across launches must distinguish hosts, applications and window instances/explicit layout slots; a process name cannot identify multiple windows of one application. Without a reliable persistent host identity, do not restore automatically by IP address. Do not save chat titles, screenshots, passwords or input by default. Restore only after authentication and a fresh authorized catalog; ambiguous matches require selection rather than choosing the first same-name process. Move stranded windows into a visible work area after monitor removal, respecting platform and proportional-sizing constraints.

## Host selection and authorization

Client selection answers what to view; host sharing answers what the other computer may see. Client favorites, opening, related-window discovery and layout restoration never modify host policy.

Offer Selected windows / All application windows on the host, defaulting to an empty selected scope. The owner may choose a locally running window as a rule example, then explicitly select executable-path matching, title-contains matching or both, with a final match preview. Selecting a sample is not permission for only that transient HWND: explain before saving that the rule can match future windows. Do not offer a misleading single-window checkbox before transient-instance authorization exists.

Preview independently matched windows separately from eligible owned descendants. Other chats with the same PID are not automatically granted; independent windows must match their own rules. Paths remain host-local. Allow preview refresh and show its check time; runtime catalog/capture still revalidate, and preview is not a permanent authorization record.

Expose current limits directly: selected scope is read-only and rule changes require stopping and restarting sharing. Label the action Save and restart sharing, explaining disconnection when a client is active. Keep the draft on failure, leave effective policy unchanged on cancellation, and never fall back silently to all-window sharing. Only change capabilities and wording after restricted control and live policy updates are implemented.

## Delivery and acceptance

1. Align manager lists and actions across all clients: Available/Viewing, title search, row Open/Switch/Show again, End viewing and explicit empty states. Keep current transport and authentication.
2. Move catalog/viewer projections, search and action eligibility into `crates/client`/`crates/app-model`, rendered by AppKit and Qt. Deliver shared contracts alongside the first UI slice rather than allowing platforms to define Open independently. Add necessary safe host metadata capabilities before relying on them; do not infer missing facts on clients.
3. Build the quick picker on that model and add host rule-example selection/preview. Deliver cross-launch layouts, application launch and discovery separately.

Acceptance covers two independent same-process windows, duplicate titles, modal blocking, local owned-window close/reopen, row reordering, disconnect/cancel/revoke during opening, read-only scope, empty titles, connection epoch changes and stale results. Check picker IME composition, keyboard navigation, focus return and accessibility; check empty host rules, example-to-rule scope explanations and restart failures without expanded access. Mac, Windows and Linux X11/Wayland use the same behavioral contract; do not promise forced activation or arbitrary placement on Wayland. Update both language resources and documentation. See the delivery notes below for the implemented and tested subset.


## First macOS delivery — 2026-11-22

The AppKit manager now has a collapsible connection form, global quality, options, Available windows / Viewing tabs, title search and state-dependent row actions. Double-click, Return and the row button share one selection path. Find window… and manager-local Command+K focus that same search field; there is no separate quick-picker panel yet.

Viewing can expand owned floating/modal windows and excludes transient menus. Reopening one viewer restores only its owned subtree, preserving independent siblings that the user closed locally. End viewing cancels pending opens and rejects late selection/group replies. Refresh keeps selection by identity, and disconnect/revocation removes catalog metadata. Before selection, the current wire provides HWND/process/thread but no window generation: the client tracks observed incarnations and rejects observed disappearance/reuse; the host remains responsible for authoritative selection validation.

`crates/client::manager` supplies shared search ranks and state/action eligibility through the existing C ABI, with `tests/contracts/window-manager.json`. Native session/catalog projection remains in the Mac adapter; Qt adoption is next. Search currently supports case-insensitive exact, prefix and substring matching, including original Chinese text. Fuzzy/pinyin/recent-history ranking, application names/icons, row detail/overflow menus, a standalone quick picker, persistent layouts and the host rule picker remain planned. Current rows use generic icons and an Application Window fallback for empty titles. Unknown catalog roles are retained as selectable rows until a selected window group supplies authoritative roles.

Validation: shared contract tests, synthetic AppKit manager tests in both languages, localization, multi-session, recovery, group, window-management, sizing, input and drag regressions. These checks do not constitute a new live Windows/GPU run; see the [validation record](validation.md).
