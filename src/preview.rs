//! Per-type previews (FR-56, PRD §8.4).
//!
//! A preview comes from the head that detection read. Only an image whose
//! size is not in the head, and the zip directory at the end of a file,
//! read a few KiB more (through `io::read_preview`). Text previews escape
//! control characters as `\xHH`. Secrets never get a preview (FR-77).

use std::io::Read;

use crate::detect::{TypeGroup, TypeInfo, fast, label};
use crate::layout::escape;
use crate::tree::{Kind, NodeId, Tree, flag};

/// Bytes of a compressed file that a peek decompresses.
const PEEK: usize = 4096;

/// The most bytes that a preview of an image header reads.
const IMAGE_MAX: usize = 64 * 1024;

/// The end of a zip file that holds the end-of-directory record.
const ZIP_TAIL: u64 = 22 + 0xFFFF;

/// The longest preview text that teak keeps; layout cuts it to the width.
const KEEP: usize = 200;

/// The preview of a regular file from its `head`. `None` for secrets, for
/// files with nothing to show, and for non-regular files (symlink targets
/// are a flag, from stat).
pub fn of(tree: &Tree, id: NodeId, info: &TypeInfo, head: &[u8]) -> Option<String> {
    let n = tree.node(id);
    if n.has(flag::SECRET) || n.kind != Kind::File || n.size == 0 {
        return None;
    }
    if let Some(arch) = binary_arch(head) {
        return Some(arch);
    }
    let name = label(info.label).name;
    let text = match name {
        "markdown" => first_heading(head),
        "json" => json_keys(head),
        "yaml" => yaml_keys(head),
        "toml" => toml_keys(head),
        "csv" | "tsv" => csv(head, n.size),
        "gzip" | "zstd" | "xz" => return peek(tree, id, name, head),
        "zip" | "jar" => return zip_entries(tree, id),
        _ if info.group == TypeGroup::Image => return image_size(tree, id, head),
        _ if info.group == TypeGroup::Code => first_code_line(head),
        _ if info.is_text => first_line(head),
        _ => None,
    }?;
    Some(clean(&text))
}

/// Tabs become spaces; control characters and bytes that are not UTF-8
/// become `\xHH`; the text is at most [`KEEP`] characters.
fn clean(text: &[u8]) -> String {
    let tabs: Vec<u8> = text
        .iter()
        .map(|&b| if b == b'\t' { b' ' } else { b })
        .collect();
    escape(&tabs).chars().take(KEEP).collect()
}

/// The lines of the head, without `\r\n` ends.
fn lines(head: &[u8]) -> impl Iterator<Item = &[u8]> {
    head.split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
}

fn trim(line: &[u8]) -> &[u8] {
    line.trim_ascii()
}

fn first_line(head: &[u8]) -> Option<Vec<u8>> {
    lines(head)
        .map(trim)
        .find(|l| !l.is_empty())
        .map(<[u8]>::to_vec)
}

fn first_heading(head: &[u8]) -> Option<Vec<u8>> {
    lines(head)
        .map(trim)
        .find(|l| l.starts_with(b"#"))
        .map(<[u8]>::to_vec)
        .or_else(|| first_line(head))
}

/// A comment line or a block-comment start, in the common languages.
fn is_comment(line: &[u8]) -> bool {
    [
        &b"//"[..],
        b"#",
        b"--",
        b";",
        b"/*",
        b"*",
        b"<!--",
        b"\"\"\"",
        b"'''",
    ]
    .iter()
    .any(|p| line.starts_with(p))
        && !line.starts_with(b"#!")
        && !line.starts_with(b"#[")
        && !line.starts_with(b"#include")
        && !line.starts_with(b"#define")
}

fn legal(text: &[u8]) -> bool {
    let lower = text.to_ascii_lowercase();
    [&b"spdx-license"[..], b"copyright", b"license", b"(c)"]
        .iter()
        .any(|w| lower.windows(w.len()).any(|x| x == *w))
}

/// The first line that is not blank, not an SPDX line, and not part of a
/// license header (comment lines or a block comment that names a license or
/// a copyright).
fn first_code_line(head: &[u8]) -> Option<Vec<u8>> {
    let all: Vec<&[u8]> = lines(head).map(trim).collect();
    let mut i = 0;
    while i < all.len() {
        let line = all[i];
        if line.is_empty() {
            i += 1;
            continue;
        }
        if line.starts_with(b"/*") && !contains(line, b"*/") {
            let end = (i..all.len())
                .find(|&k| contains(all[k], b"*/"))
                .unwrap_or(all.len() - 1);
            if all[i..=end].iter().any(|l| legal(l)) {
                i = end + 1;
                continue;
            }
        }
        if is_comment(line) {
            // A run of comment lines that names a license is a header.
            let end = (i..all.len())
                .find(|&k| !is_comment(all[k]))
                .unwrap_or(all.len());
            if all[i..end].iter().any(|l| legal(l)) {
                i = end;
                continue;
            }
        }
        return Some(line.to_vec());
    }
    None
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// `{a, b, c}` from a list of top-level keys; `…` when the head ends first.
fn key_list(keys: Vec<Vec<u8>>, complete: bool) -> Option<Vec<u8>> {
    if keys.is_empty() {
        return None;
    }
    let mut out = b"{".to_vec();
    for (i, k) in keys.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(b", ");
        }
        out.extend_from_slice(k);
    }
    if !complete {
        out.extend_from_slice(", …".as_bytes());
    }
    out.push(b'}');
    Some(out)
}

/// The keys of the top-level JSON object.
fn json_keys(head: &[u8]) -> Option<Vec<u8>> {
    let start = head.iter().position(|b| !b.is_ascii_whitespace())?;
    if head[start] != b'{' {
        return None;
    }
    let mut keys: Vec<Vec<u8>> = Vec::new();
    let mut depth = 0usize;
    let mut want_key = false;
    let mut i = start;
    while i < head.len() {
        match head[i] {
            b'"' => {
                let s = i + 1;
                let mut k = s;
                while k < head.len() && head[k] != b'"' {
                    k += if head[k] == b'\\' { 2 } else { 1 };
                }
                if k >= head.len() {
                    break;
                }
                if depth == 1 && want_key {
                    keys.push(head[s..k].to_vec());
                    want_key = false;
                }
                i = k;
            }
            b'{' | b'[' => {
                depth += 1;
                want_key = depth == 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return key_list(keys, true);
                }
            }
            b',' if depth == 1 => want_key = true,
            _ => {}
        }
        i += 1;
    }
    key_list(keys, false)
}

fn push_key(keys: &mut Vec<Vec<u8>>, key: &[u8]) {
    let key = trim(key);
    let key = key
        .strip_prefix(b"\"")
        .and_then(|k| k.strip_suffix(b"\""))
        .unwrap_or(key);
    if !key.is_empty() && !keys.iter().any(|k| k == key) {
        keys.push(key.to_vec());
    }
}

/// Keys at column 0 (`name: x`).
fn yaml_keys(head: &[u8]) -> Option<Vec<u8>> {
    let mut keys = Vec::new();
    for line in lines(head) {
        let first = line.first().copied().unwrap_or(b' ');
        if first.is_ascii_whitespace() || matches!(first, b'#' | b'-' | b'.') {
            continue;
        }
        if let Some(c) = line.iter().position(|&b| b == b':') {
            push_key(&mut keys, &line[..c]);
        }
    }
    key_list(keys, true)
}

/// Keys before the first table, then the table names (`[package]` gives
/// `package`).
fn toml_keys(head: &[u8]) -> Option<Vec<u8>> {
    let mut keys = Vec::new();
    let mut in_table = false;
    for line in lines(head).map(trim) {
        if line.starts_with(b"[") {
            in_table = true;
            let open = line.iter().position(|&b| b != b'[').unwrap_or(line.len());
            let name = trim(&line[open..]);
            let end = name
                .iter()
                .position(|&b| matches!(b, b'.' | b']'))
                .unwrap_or(name.len());
            push_key(&mut keys, &name[..end]);
        } else if !in_table
            && !line.starts_with(b"#")
            && let Some(eq) = line.iter().position(|&b| b == b'=')
        {
            let key = &line[..eq];
            let end = key.iter().position(|&b| b == b'.').unwrap_or(key.len());
            push_key(&mut keys, &key[..end]);
        }
    }
    key_list(keys, true)
}

/// The header row and the row count: exact when the whole file is in the
/// head, else an estimate from the mean row length (`≈1.0k rows`).
fn csv(head: &[u8], size: u64) -> Option<Vec<u8>> {
    let nl = head.iter().position(|&b| b == b'\n')?;
    let header = head[..nl].strip_suffix(b"\r").unwrap_or(&head[..nl]);
    let body = &head[nl + 1..];
    let whole = head.len() as u64 >= size;
    let rows_text = if whole {
        let rows = lines(body).filter(|l| !l.is_empty()).count() as u64;
        format!("{} {}", crate::layout::thousands(rows), plural_rows(rows))
    } else {
        let last = body.iter().rposition(|&b| b == b'\n')?;
        let rows = body[..=last].iter().filter(|&&b| b == b'\n').count() as u64;
        if rows == 0 {
            return Some(header.to_vec());
        }
        let mean = (last as u64 + 1) as f64 / rows as f64;
        let est = ((size - nl as u64 - 1) as f64 / mean).round() as u64;
        format!("≈{} {}", crate::layout::compact(est), plural_rows(est))
    };
    let mut out = header.to_vec();
    out.push(b' ');
    out.extend_from_slice(rows_text.as_bytes());
    Some(out)
}

fn plural_rows(n: u64) -> &'static str {
    if n == 1 { "row" } else { "rows" }
}

/// `1920×1080`, from the head or from the first 64 KiB.
fn image_size(tree: &Tree, id: NodeId, head: &[u8]) -> Option<String> {
    let dims = imagesize::blob_size(head).ok().or_else(|| {
        let more = crate::io::read_preview(&tree.path(id), 0, IMAGE_MAX).ok()?;
        imagesize::blob_size(&more).ok()
    })?;
    Some(format!("{}×{}", dims.width, dims.height))
}

/// `gzip → sql`: the type of the first decompressed bytes. The inner name
/// is the name without its last extension (`dump.sql.gz` → `dump.sql`).
fn peek(tree: &Tree, id: NodeId, outer: &str, head: &[u8]) -> Option<String> {
    let mut inner = Vec::new();
    let reader: Box<dyn Read + '_> = match outer {
        "gzip" => Box::new(flate2::read::MultiGzDecoder::new(head)),
        "zstd" => Box::new(zstd::stream::read::Decoder::with_buffer(head).ok()?),
        _ => Box::new(xz2::read::XzDecoder::new(head)),
    };
    let mut reader = reader.take(PEEK as u64);
    let mut buf = [0u8; 1024];
    // A head that cuts the stream ends in an error; keep what came before.
    while let Ok(n) = reader.read(&mut buf) {
        if n == 0 {
            break;
        }
        inner.extend_from_slice(&buf[..n]);
    }
    if inner.is_empty() {
        return Some(outer.to_string());
    }
    let name = tree.name(id);
    let stem = name
        .iter()
        .rposition(|&b| b == b'.')
        .map_or(&name[..0], |i| &name[..i]);
    let t = fast::type_of(stem, Some(&inner));
    let what = match t.name() {
        "unknown" | "empty" => return Some(outer.to_string()),
        n => n,
    };
    Some(format!("{outer} → {what}"))
}

/// `12 entries`, from the end-of-directory record (and the zip64 record
/// when the count does not fit 16 bits).
fn zip_entries(tree: &Tree, id: NodeId) -> Option<String> {
    let path = tree.path(id);
    let size = tree.node(id).size;
    let start = size.saturating_sub(ZIP_TAIL);
    let tail = crate::io::read_preview(&path, start, (size - start) as usize).ok()?;
    let at = tail.windows(4).rposition(|w| w == b"PK\x05\x06")?;
    let eocd = tail.get(at..at + 22)?;
    let mut count = u64::from(u16::from_le_bytes([eocd[10], eocd[11]]));
    if count == 0xFFFF && at >= 20 && &tail[at - 20..at - 16] == b"PK\x06\x07" {
        let loc = &tail[at - 20..at];
        let off = u64::from_le_bytes(loc[8..16].try_into().ok()?);
        let rec = crate::io::read_preview(&path, off, 56).ok()?;
        if rec.len() == 56 && rec.starts_with(b"PK\x06\x06") {
            count = u64::from_le_bytes(rec[32..40].try_into().ok()?);
        }
    }
    let word = if count == 1 { "entry" } else { "entries" };
    Some(format!("{} {word}", crate::layout::thousands(count)))
}

/// `ELF x86-64`, `Mach-O arm64`, `PE x86-64`, from the header bytes.
fn binary_arch(head: &[u8]) -> Option<String> {
    let u16le = |o: usize| head.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u16be = |o: usize| head.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let u32le = |o: usize| {
        head.get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    let u32be = |o: usize| {
        head.get(o..o + 4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
    };
    if head.starts_with(b"\x7fELF") {
        let machine = if head.get(5) == Some(&2) {
            u16be(18)?
        } else {
            u16le(18)?
        };
        let arch = match machine {
            0x03 => "x86",
            0x08 => "mips",
            0x14 => "ppc",
            0x15 => "ppc64",
            0x28 => "arm",
            0x3E => "x86-64",
            0xB7 => "aarch64",
            0xF3 => "riscv",
            _ => "",
        };
        return Some(if arch.is_empty() {
            "ELF".to_string()
        } else {
            format!("ELF {arch}")
        });
    }
    let macho = |cpu: u32| match cpu {
        7 => "x86",
        0x0100_0007 => "x86-64",
        12 => "arm",
        0x0100_000C => "arm64",
        _ => "",
    };
    let magic = u32le(0)?;
    if matches!(magic, 0xFEED_FACE | 0xFEED_FACF) {
        let arch = macho(u32le(4)?);
        return Some(format!("Mach-O {arch}").trim_end().to_string());
    }
    // Universal binaries (big endian). Java class files share the magic,
    // but have a version of 45 or more where this has the arch count.
    if u32be(0)? == 0xCAFE_BABE && u32be(4).is_some_and(|n| (1..20).contains(&n)) {
        return Some("Mach-O universal".to_string());
    }
    if head.starts_with(b"MZ") {
        let pe = u32le(0x3C)? as usize;
        if head.get(pe..pe + 4) == Some(b"PE\0\0") {
            let arch = match u16le(pe + 4)? {
                0x014C => "x86",
                0x8664 => "x86-64",
                0xAA64 => "arm64",
                0x01C4 => "arm",
                _ => "",
            };
            return Some(format!("PE {arch}").trim_end().to_string());
        }
    }
    None
}
