# Host sharing permissions

[简体中文](../zh-CN/sharing.md)

Only the host owner configures sharing. All windows explicitly permits all shareable application windows. Selected windows is the initial mode with an empty allowlist. Saving an empty list shares nothing. Clients see only permitted metadata.

Match a resolved executable path, a case-insensitive normalized title substring, or both. Conditions in one rule are ANDed; any matching rule grants access. A title can be chosen by an app, so preview matches and optionally constrain the executable.

An owned dialog may inherit a permitted root after verifying its parent or owner chain and process instance. An independent popup requires its own match. Sharing one conversation by title does not expose sibling conversations with the same PID. Recheck after identity, title or owner changes.

Enforce policy on discovery, selection, group updates, capture and queued frames, input, clipboard and resume. Revalidate input focus and clipboard source; deny if the actual recipient or origin cannot be verified. On revocation stop capture, discard queued data and clear stale client views. Already delivered content cannot be recalled.

[One-to-one connections](implementation.md#one-to-one-connections)

[Current implementation: static selected-window viewing](restricted-sharing.md). Interactive restricted control and live policy editing remain planned.
