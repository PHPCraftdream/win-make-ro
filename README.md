# win-make-ro

[![CI](https://github.com/PHPCraftdream/win-make-ro/actions/workflows/ci.yml/badge.svg)](https://github.com/PHPCraftdream/win-make-ro/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust 2024](https://img.shields.io/badge/rust-edition%202024-orange.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![Platform: Windows](https://img.shields.io/badge/platform-Windows%2010%2F11-0078d4.svg)](#build-install-remove)

Explorer context-menu items **Make read only** / **Remove read only** for files and
folders on NTFS.

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
- Items that are locked through a parent show a disabled entry *Read only
  (inherited from parent folder)*; unlock the parent instead. Removing an
  item's own ACE while a parent's lock still applies is refused, since it
  could not make the item writable anyway — an item carrying both its own and
  an inherited lock therefore reports as parent-locked.
- Symlinks and junctions are never touched or descended.
- Changing a DACL needs `WRITE_DAC`. The owner always has it, so own files need
  no elevation. On `ERROR_ACCESS_DENIED` the helper re-launches itself through
  UAC and re-runs the same (idempotent) operation.

## Layout

| crate          | what                                                              |
|----------------|-------------------------------------------------------------------|
| `ro-core`      | ACL logic: `lock_state`, `lock`, `unlock`, `lock_tree`, `unlock_tree` |
| `ro-register`  | per-user registry (`HKCU\Software\Classes`) install / uninstall  |
| `win-make-ro`  | helper exe: `lock`, `unlock`, `status`, `install`, `uninstall`    |
| `ro-shellext`  | COM DLL: `IShellExtInit` + `IContextMenu`, launches the helper    |

## Build, install, remove

```
cargo build --release
mkdir dist && copy target\release\win-make-ro.exe dist\ && copy target\release\ro_shellext.dll dist\
dist\install.cmd                    # = dist\win-make-ro.exe install   (or regsvr32 dist\ro_shellext.dll)
dist\uninstall.cmd                  # = dist\win-make-ro.exe uninstall (or regsvr32 /u dist\ro_shellext.dll)
```

Registration is per user and needs no admin rights. Keep `win-make-ro.exe`
next to `ro_shellext.dll`; the DLL looks for the helper in its own directory.
Explorer keeps the DLL loaded after the first use, so replacing `dist\` files
requires restarting `explorer.exe` first.

## CLI

```
win-make-ro lock   [--gui] [--no-elevate] -- <path>...
win-make-ro unlock [--gui] [--no-elevate] -- <path>...
win-make-ro status -- <path>...          # prints "<unlocked|locked|inherited>\t<path>"
```

Exit codes: 0 ok, 1 some item failed (details on stderr, or a message box with
`--gui`), 2 usage error.

## Tests

```
cargo test --workspace
cargo test --workspace --release   # the COM tests load a built DLL
cargo +1.85 check --workspace --all-targets
```

- `ro-core/tests`: real ACLs in a temp directory (files, folders, inheritance,
  nested locks, blocked inheritance, junctions, foreign deny ACEs).
- `win-make-ro/tests/cli.rs`: the built helper end-to-end.
- `ro-shellext/tests/com_e2e.rs`: loads the DLL, builds a shell data object,
  checks the menu items and invokes them; the DLL waits for the helper when
  `WIN_MAKE_RO_SYNC=1` is set.
- `ro-register/tests`: registry writes against a scratch key under HKCU.
- `ro-core/tests/regressions.rs`: the review findings, each pinned to the
  behaviour that used to be wrong.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
