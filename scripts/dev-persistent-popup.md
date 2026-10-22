# Persistent popup local acceptance

This fork is experimental. Do not replace an installed Herdr binary or hand off a
live session to it. The debug build and disposable session below are independent
of Home Manager and the installed stable server.

From the checkout, run in a fresh terminal:

```sh
nix develop --command scripts/dev-persistent-popup.sh
```

The launcher builds `target/debug/herdr`, validates a temporary config, clears
inherited Herdr socket/session/pane selectors, and uses isolated XDG config/state
directories under `/var/tmp`. Nested launch is enabled only in that test config.
It stops only its disposable session when its client exits. It prints the sandbox
path and retains configuration/logs there for inspection. The shells use `/bin/sh`
to avoid requiring Zsh or modifying your normal shell configuration.

## Acceptance checks

1. **Retained shell:** press F12. Run `TOKEN=retained; echo $$; pwd`, change to
   `/tmp`, and print a distinctive marker. Press F12 twice. The PID, `TOKEN`, CWD,
   and earlier output should be unchanged. Run `echo "$TOKEN"; echo $$; pwd`.
2. **Hidden process:** run `(sleep 2; echo HIDDEN_DONE) &`, hide with F12, do work
   in the tiled pane, then reopen. `HIDDEN_DONE` should appear without starting
   a new shell.
3. **Workspace scope:** create another workspace with Herdr's normal controls.
   F12 there should have a different PID and no `TOKEN`; returning to the first
   workspace and pressing F12 should recover its original PID and variable.
4. **Global scope:** F11 opens a top drawer. Set a different variable and note
   its PID. Hide, change workspace/tab, and press F11. It should be the same shell.
5. **Tab scope:** F10 opens a centered overlay. Repeat the isolation check between
   two tabs in the same workspace. Each tab should keep a distinct terminal.
6. **Presentation:** F12 is full-width and bottom-anchored; F11 is top-anchored;
   F10 is centered. Resize the outer terminal while visible and while hidden.
   Borders, cursor, mouse coordinates, and terminal dimensions should remain valid.
7. **Exclusion:** while a persistent terminal is shown, invoke another persistent
   binding. Only the new popup should appear. Reopening the first should retain it.
   Ordinary typing, Escape, paste, and terminal mouse input must reach the popup.
8. **Exit:** `exit` closes the shell. The next invocation should have a fresh PID
   and no old variables. Leaving the popup should immediately restore tiled input.
9. **Owner cleanup:** close a tab/workspace with a hidden scoped shell. Its process
   must exit; another owner's terminal must not be affected. A global shell must
   survive workspace closure while the disposable server remains alive.
10. **No-output navigation:** hide and reopen repeatedly while shells are idle.
    Toggle should not depend on PTY output. Switch tabs using the mouse and invoke
    the same global/workspace binding there; it should move the retained terminal
    to the invoking tab, not create a duplicate.

Do not run destructive commands or important jobs in the disposable session.
Persistent terminals are not restored after server restart/live binary handoff.
A hidden shell keeps the dimensions it last had until shown again.

## Automated checks

```sh
nix develop --command just ci
nix develop --command just docs-contract-test
nix develop --command just bench-render-scale
```

Some upstream tests hard-code `/bin/sleep` and `/usr/bin/true`. These paths are
absent on NixOS even inside `nix develop`. Run those checks in a disposable FHS
environment or an unprivileged user/mount namespace; do not add symlinks to the
host's `/bin` or `/usr/bin`. The manual launcher above needs only `/bin/sh`.

`just check` additionally requires an explicitly provisioned Windows SDK. Native
checks do not establish Windows or macOS runtime correctness. The optional popup
presentation metadata leaves frozen generation-1 binary codecs unchanged; older
clients are not offered persistent commands.

If Zig reports `TlsInitializationFailed` fetching Ghostty's pinned archives,
download the specific URLs using `curl`, then run `zig fetch /absolute/archive`
from `vendor/libghostty-vt`. Compare its printed hash with the dependency's
`build.zig.zon` hash. Do not change dependency pins or disable TLS verification.
