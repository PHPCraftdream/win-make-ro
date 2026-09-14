//! What the shipped artifacts expect to find on the machine that runs them.
//!
//! Neither the zip, the npm package nor the Scoop manifest can install a
//! Visual C++ redistributable — that needs administrator rights, which this
//! tool promises not to ask for. So the artifacts have to be loadable on a
//! Windows that has never seen a Visual Studio installer, and the only way to
//! see whether they are is to read their import table.

use std::path::Path;

mod common;

use common::{ensure_built, target_dir};

fn u16_at(d: &[u8], o: usize) -> usize {
    u16::from_le_bytes([d[o], d[o + 1]]) as usize
}

fn u32_at(d: &[u8], o: usize) -> usize {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) as usize
}

/// The DLLs a PE image names in its import directory.
fn imports(path: &Path) -> Vec<String> {
    let d = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let pe = u32_at(&d, 0x3C);
    assert_eq!(&d[pe..pe + 4], b"PE\0\0", "{} is not a PE image", path.display());
    let coff = pe + 4;
    let sections = u16_at(&d, coff + 2);
    let opt = coff + 20;
    // PE32+ carries an 8-byte ImageBase and no BaseOfData, which puts the data
    // directories 16 bytes further along than in PE32.
    let dirs = opt + if u16_at(&d, opt) == 0x20b { 112 } else { 96 };
    let import_rva = u32_at(&d, dirs + 8);
    if import_rva == 0 {
        return Vec::new();
    }

    let table = opt + u16_at(&d, coff + 16);
    let secs: Vec<(usize, usize, usize)> = (0..sections)
        .map(|i| {
            let s = table + i * 40;
            // VirtualSize can be 0 in an image built by some linkers, so the
            // larger of the two sizes decides what the section covers.
            (u32_at(&d, s + 12), u32_at(&d, s + 8).max(u32_at(&d, s + 16)), u32_at(&d, s + 20))
        })
        .collect();
    let at = |rva: usize| -> usize {
        secs.iter()
            .find_map(|&(va, size, raw)| (rva >= va && rva < va + size).then_some(raw + rva - va))
            .unwrap_or_else(|| panic!("{}: rva {rva:#x} is in no section", path.display()))
    };

    let mut names = Vec::new();
    let mut p = at(import_rva);
    // The directory ends at an all-zero descriptor; only its Name matters here.
    loop {
        let name_rva = u32_at(&d, p + 12);
        if name_rva == 0 {
            return names;
        }
        let s = at(name_rva);
        let end = s + d[s..].iter().position(|&b| b == 0).expect("unterminated import name");
        names.push(String::from_utf8_lossy(&d[s..end]).into_owned());
        p += 20;
    }
}

/// True for the C runtime DLLs, whichever flavour.
///
/// `vcruntime`/`msvcp`/`msvcr` come with the redistributable and are the ones
/// that actually keep the artifacts from loading. The `api-ms-win-crt-*` stubs
/// are part of Windows 10, so they would do no harm on their own — but they go
/// away with the same setting, and their reappearance is the plainest sign
/// that it has stopped applying.
fn is_c_runtime(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["vcruntime", "msvcp", "msvcr", "ucrtbase", "api-ms-win-crt-"].iter().any(|p| n.starts_with(p))
}

#[test]
fn the_artifacts_bring_their_own_c_runtime() {
    ensure_built();
    for name in ["win-make-ro.exe", "ro_shellext.dll"] {
        let path = target_dir().join(name);
        let all = imports(&path);
        assert!(!all.is_empty(), "{name}: no imports read, the parser is looking at nothing");
        let runtime: Vec<_> = all.iter().filter(|n| is_c_runtime(n)).collect();
        assert!(
            runtime.is_empty(),
            "{name} imports {runtime:?}; a Windows without the Visual C++ \
             redistributable could not load it. Imports: {all:?}"
        );
    }
}
