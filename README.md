<img src="assets/main_icon.png" alt="" width="96" align="right">

# win-make-ro

[![CI](https://github.com/PHPCraftdream/win-make-ro/actions/workflows/ci.yml/badge.svg)](https://github.com/PHPCraftdream/win-make-ro/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust 2024](https://img.shields.io/badge/rust-edition%202024-orange.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![MSRV 1.85](https://img.shields.io/badge/rustc-1.85%2B-orange.svg)](#tests)
[![Platform: Windows](https://img.shields.io/badge/platform-Windows%2010%2F11-0078d4.svg)](#build-install-remove)

Explorer context-menu items **Make read only** / **Remove read only** for files and
folders on NTFS.

Windows already has a read-only checkbox, but it sets a file *attribute*:
advisory, cleared by anyone who can reach the file, and ignored outright by
most programs that overwrite in place. This sets a deny ACE instead, which the
kernel enforces against every account — administrators included — and which
Windows propagates down a folder tree on its own. The second menu item appears
only where the lock is recognisably this tool's own, so nothing else in your
ACLs is disturbed.

## Status

Version 0.1.0, and no release has been published yet: the `scoop` and `npm`
instructions below describe how the packages are built and what they do, not
something you can install today. 99 tests run on every push in each profile,
alongside the packaging and release scripts, on Windows 10 — see
[Tests](#tests) for what they reach and what they do not. Windows 11 has not
been tried, and neither has the UAC prompt end to end.

The binaries are not code-signed, so SmartScreen will warn the first time one
runs. Building from source is three commands and is described under
[Build, install, remove](#build-install-remove).

## How the lock works

The lock is an explicit *deny* ACE for `Everyone` in the item's DACL with exactly
these rights: write data, append data, write EA, write attributes, delete,
delete child. That exact mask is the signature: an ACE with this mask and this
trustee is recognised as ours, nothing else is. No extra marker is stored.

- Reading stays allowed; writing, renaming, deleting and creating inside a
  locked folder are denied for every account, including administrators.
- A locked *file* additionally gets the READONLY attribute, which nobody can
  clear while the ACE denies `FILE_WRITE_ATTRIBUTES`. Windows still lets a
  caller who holds `FILE_DELETE_CHILD` on the parent folder rename the file, or
  delete it with POSIX delete semantics that ignore the attribute (Rust's
  `std::fs::remove_file` does this; Explorer, `del`, PowerShell do not). To
  rule that out, lock the folder.
- On a folder the ACE is inheritable (OI|CI), so Windows propagates it to the
  whole subtree in one call. Descendants whose inheritance is disabled get an
  explicit ACE during the recursive walk.
- **Remove read only** on a folder removes the folder's ACE (inherited copies
  disappear with it) and then walks the tree removing every explicit lock set
  earlier on nested items.
- An inherited lock is trusted only when nothing shadows it. Windows evaluates
  a DACL in order, so an explicit allow on a child would win over the deny
  inherited from the parent; such children get an explicit lock of their own.
- An item with a NULL DACL is refused. A NULL DACL switches the access check
  off rather than granting a set of rights, no ACL reproduces that, and no
  later unlock could tell a reconstructed one from a deliberate
  `Everyone: FullControl`. Such an item is reported and left untouched.
- An ACL stores its own size in a 16-bit field, so a DACL stops at 64 KB. An
  item whose DACL has no room left for one more entry is reported and left
  alone; the rest of the selection is still processed.
- Items that are locked through a parent show a disabled entry *Read only
  (inherited from parent folder)*; unlock the parent instead. Removing an
  item's own ACE while a parent's lock still applies is refused, since it
  could not make the item writable anyway — an item carrying both its own and
  an inherited lock therefore reports as parent-locked.
- Symlinks and junctions are never touched or descended.
- Changing a DACL needs `WRITE_DAC`. The owner always has it, so own files need
  no elevation. On `ERROR_ACCESS_DENIED` the helper re-launches itself through
  UAC and re-runs the same (idempotent) operation.
- Windows checks access when a handle is opened, not when it is used, so a
  program that already had the file open for writing keeps that right until it
  closes the handle. The lock applies to everything opened after it.
- A folder is not sealed all at once. NTFS keeps a DACL per file, and Windows
  rewrites them one at a time while `SetNamedSecurityInfoW` runs, top to
  bottom; until it reaches a given file, that file is still writable. Measured
  on an ordinary SSD: 3 000 files take about a fifth of a second, 30 000 about
  two. There is no way round it — writing a DACL on a folder walks the whole
  subtree whether or not the entry being written is inheritable, since every
  child's inherited entries have to be recomputed. So Explorer is told to
  re-read the folder once a second while the work runs and once more when it
  ends: the read-only marks spread through the listing as the lock does, and
  the last refresh is the one that says it is finished. Work that is over
  inside the first second gets only that last refresh — a small folder should
  not flicker. The refreshes cost about 150 ms on a 30 000-file folder and
  nothing at all on a small one.

## Layout

| crate          | what                                                              |
|----------------|-------------------------------------------------------------------|
| `ro-core`      | ACL logic: `lock_state`, `lock`, `unlock`, `lock_tree`, `unlock_tree` |
| `ro-register`  | per-user registry (`HKCU\Software\Classes`) install / uninstall  |
| `win-make-ro`  | helper exe: `lock`, `unlock`, `status`, `install`, `reinstall`, `uninstall`, `restart-explorer` |
| `ro-shellext`  | COM DLL: `IShellExtInit` + `IContextMenu`, launches the helper    |

Packaging lives in `bucket/` (the Scoop manifest, where `scoop bucket add`
expects it) and `npm/` (the published package; `npm/vendor/` is filled by the
release workflow).

## Install

Either package registers the Explorer context menu for the current user, with
no admin rights. The C runtime is linked into the binaries, so nothing has to
be installed alongside them.

```
scoop bucket add win-make-ro https://github.com/PHPCraftdream/win-make-ro
scoop install win-make-ro
```

```
npm install -g win-make-ro
```

The npm package ships the binaries and is marked `win32`/`x64`; a project-local
install deliberately skips registration, so run `npx win-make-ro install` for
that case. Set `WIN_MAKE_RO_SKIP_REGISTER=1` to suppress it entirely.

A global install runs `reinstall --here` (see below), so **upgrading restarts
Explorer**: npm has just rewritten the DLL the running one still has mapped,
and only a restart makes the new code the one in use. `--here` registers the
copy npm unpacked and touches no other directory. Nothing was registered
before? Then nothing asked Explorer to load anything, and the desktop is left
alone — that is a rule about the registry, not a look at what Explorer has
mapped. From
an elevated shell the restart is skipped — Explorer would keep the elevated
token — and the install says so; run `win-make-ro reinstall --here` yourself
from an ordinary window.

npm 11 upgrades by renaming the old package directory aside, much as
`reinstall` does, so a mapped DLL does not stop the new version from being
installed — it surfaces as a cleanup warning about the directory npm could not
delete afterwards. Older npm, or a different lock, may still refuse: restart
Explorer and repeat the upgrade if it does.

A release also carries a zip with both binaries and a `.sha256` beside it, if
you would rather unpack it yourself and run `win-make-ro.exe install`.

## Removal

Scoop unregisters the context menu for you. npm cannot: since version 7 it
runs no scripts on uninstall, so unregister first, while the executable is
still there. npm also hides the output of scripts that succeed, so the
reminder the package prints on install is only visible with
`npm install -g win-make-ro --foreground-scripts`; `win-make-ro` with no
arguments repeats it.

```
win-make-ro uninstall
npm uninstall -g win-make-ro
```

If the package is already gone, the leftovers are three keys under
`HKCU\Software\Classes`, and deleting them by hand is safe:

```
reg delete "HKCU\Software\Classes\CLSID\{7A3C1F0E-5B2D-4E8A-9C61-0D4F2B7E9A11}" /f
reg delete "HKCU\Software\Classes\*\shellex\ContextMenuHandlers\WinMakeRO" /f
reg delete "HKCU\Software\Classes\Directory\shellex\ContextMenuHandlers\WinMakeRO" /f
```

Until they are removed Explorer keeps trying to load a DLL that is no longer
there, which costs nothing but shows no menu items either.

`uninstall` removes the registration whichever installation wrote it — there is
only one set of keys, and they are not stamped with who put them there. If you
have the menu from two sources at once, removing either takes the menu away;
re-register the one you are keeping with `win-make-ro reinstall --here`.

## Build, install, remove

```
cargo build --release
mkdir dist && copy target\release\win-make-ro.exe dist\ && copy target\release\ro_shellext.dll dist\
dist\win-make-ro.exe install        # or: regsvr32 dist\ro_shellext.dll
dist\win-make-ro.exe uninstall      # or: regsvr32 /u dist\ro_shellext.dll
```

Registration is per user and needs no admin rights. Keep `win-make-ro.exe`
next to `ro_shellext.dll`; the DLL looks for the helper in its own directory.

Explorer maps the DLL the first time a menu opens and keeps it until it exits,
so a rebuilt DLL cannot simply be copied over the installed one. `reinstall`
does the whole exchange:

```
target\release\win-make-ro.exe reinstall --to dist
```

It writes both new binaries into `dist\` under temporary names, renames the
installed pair aside — a rename moves the directory entry and leaves the
running Explorer with the image it already has — puts the new pair in their
place, registers it, asks Explorer to close, starts it again and then deletes
the renamed copies. Anything still in use is swept by the next run.

```
dist\win-make-ro.exe reinstall            # same as --here
```

Without `--to`, `reinstall` copies nothing: it registers the pair beside the
executable where it already is and restarts Explorer. That is the difference
that matters, and why the destination is never guessed — an earlier version
took it from whatever was registered, so an `npm install -g` on a machine
carrying a Scoop or hand-made installation wrote its binaries into that other
directory. Name the directory or get your own.

Explorer is restarted only when something was registered before — a rule about
the registry rather than a measurement, since an earlier `uninstall` leaves the
DLL mapped and the keys gone — and never from an elevated process, which would
hand the new shell its token for the rest of the session; there it registers
and says the restart was skipped. A restart that was asked for and failed is
reported as itself; the installation is registered either way.

The exchange itself runs in a child process:

```
win-make-ro restart-explorer
```

Asking Explorer to close queues a request that cannot be withdrawn, so from
that moment somebody owes the machine a shell. That child has no deadline and
does not give the duty up. `reinstall` watches it for half a minute and then
stops watching — and says so, because the work is still going, not finished
and not failed. Run the command on its own whenever you want the shell
recycled without reinstalling anything.

Relative destinations are made absolute before anything uses them: the path
goes into the registry as given, and Explorer does not inherit the working
directory you typed the command in.

Scoop gives each version a directory of its own, so there is nothing to rename
there and its manifest uses `install`; restart Explorer afterwards to drop the
copy it still has mapped.

## CLI

```
win-make-ro lock   [--gui] [--no-elevate] -- <path>...
win-make-ro unlock [--gui] [--no-elevate] -- <path>...
win-make-ro status -- <path>...          # prints "<unlocked|locked|inherited>\t<path>"
win-make-ro install | uninstall
win-make-ro reinstall [--here | --to <dir>]
win-make-ro restart-explorer
```

Exit codes: 0 ok, 1 some item failed (details on stderr, or a message box with
`--gui`), 2 usage error.

## Tests

```
cargo test --workspace
cargo test --workspace --release   # the COM tests load a built DLL
cargo clippy --workspace --all-targets -- -D warnings
cargo +1.85 check --workspace --all-targets
node npm/test/lifecycle.test.js    # the npm packaging contract
pwsh -File .github/scripts/plan-release-assets.tests.ps1
pwsh -File .github/scripts/update-scoop-manifest.tests.ps1
```

Nothing is mocked where a real one would do: the ACL tests write real DACLs,
the COM tests load the real DLL, the CLI tests drive the real executable.

- `ro-core/tests`: real ACLs in a temp directory (files, folders, inheritance,
  nested locks, blocked inheritance, junctions, foreign deny ACEs).
- `ro-core/tests/regressions.rs`: the review findings, each pinned to the
  behaviour that used to be wrong.
- `win-make-ro/tests/cli.rs`: the built helper end-to-end, down to a DACL
  filled to the 64 KB limit and names holding unpaired surrogates.
- `ro-shellext/tests/com_e2e.rs`: loads the DLL, builds a shell data object,
  checks the menu items and invokes them; the DLL waits for the helper when
  `WIN_MAKE_RO_SYNC=1` is set.
- `ro-shellext/tests/runtime_deps.rs`: reads the import table of the built exe
  and dll and refuses any C runtime import, so the artifacts stay loadable on a
  Windows with no redistributable.
- `ro-register/tests`: registry writes against a scratch key under HKCU.
- `npm/test/lifecycle.test.js`: runs the real install scripts with spawning,
  the file system and the platform substituted, so the packaging behaviour is
  checked without touching Explorer.

What the suite does not reach, and does not pretend to: the UAC prompt,
`reinstall` actually closing and reopening Explorer, and the Win32 failures
behind them — a token that will not read, an `OpenProcess` that is refused.
Those paths are shaped so the mistake is not expressible rather than caught by
a test.

## License

Copyright © 2026 Marat K ([@PHPCraftdream](https://github.com/PHPCraftdream)).

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
