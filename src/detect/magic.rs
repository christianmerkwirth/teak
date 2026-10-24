//! Magic bytes (`infer`), shebang lines, and a text check on the head.

use super::{LabelId, label_id};

/// A binary format from its magic bytes. Text formats (`html`, `xml`, and
/// shell) come back with `text: true`; they only count when the extension
/// says nothing.
pub fn by_magic(head: &[u8]) -> Option<(LabelId, bool)> {
    let kind = infer::get(head)?;
    let text = kind.matcher_type() == infer::MatcherType::Text;
    let label = match kind.extension() {
        "png" => "png",
        "jpg" => "jpeg",
        "gif" => "gif",
        "webp" => "webp",
        "bmp" => "bmp",
        "tif" | "cr2" => "tiff",
        "ico" => "ico",
        "heif" => "heif",
        "avif" => "avif",
        "psd" => "psd",
        "pdf" => "pdf",
        "epub" => "epub",
        "doc" => "doc",
        "docx" => "docx",
        "xlsx" => "xlsx",
        "pptx" => "pptx",
        "rtf" => "rtf",
        "zip" => "zip",
        "tar" => "tar",
        "gz" => "gzip",
        "bz2" => "bzip2",
        "xz" => "xz",
        "zst" => "zstd",
        "7z" => "7zip",
        "rar" => "rar",
        "deb" => "deb",
        "rpm" => "rpm",
        "sqlite" => "sqlite",
        "elf" => "elf",
        "mach" => "macho",
        "exe" | "dll" => "pebin",
        "wasm" => "wasm",
        "class" => "javabytecode",
        "obj" => "object",
        "pem" | "der" => "pem",
        "woff" | "woff2" | "ttf" | "otf" => "font",
        "mp3" => "mp3",
        "m4a" => "m4a",
        "ogg" | "opus" => "ogg",
        "flac" => "flac",
        "wav" => "wav",
        "aac" => "aac",
        "mp4" | "m4v" => "mp4",
        "mkv" => "mkv",
        "webm" => "webm",
        "mov" => "mov",
        "avi" => "avi",
        "html" => "html",
        "xml" => "xml",
        "sh" => "shell",
        _ => return None,
    };
    Some((label_id(label), text))
}

/// The interpreter of a `#!` line.
pub fn by_shebang(head: &[u8]) -> Option<LabelId> {
    let line = head.strip_prefix(b"#!")?;
    let line = &line[..line.iter().position(|&b| b == b'\n').unwrap_or(line.len())];
    let text = String::from_utf8_lossy(line);
    let mut words = text.split_whitespace();
    let mut prog = words.next()?.rsplit('/').next()?;
    if prog == "env" {
        prog = words.find(|w| !w.starts_with('-'))?;
    }
    let prog = prog.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    let label = match prog {
        "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish" => "shell",
        "python" => "python",
        "node" | "deno" | "bun" => "javascript",
        "perl" => "perl",
        "ruby" => "ruby",
        "php" => "php",
        "lua" => "lua",
        "awk" | "gawk" => "awk",
        "make" => "makefile",
        _ => return None,
    };
    Some(label_id(label))
}

/// True when the head looks like text: valid UTF-8 (a cut last character is
/// fine) with no NUL bytes and few control characters.
pub fn looks_like_text(head: &[u8]) -> bool {
    if head.contains(&0) {
        return false;
    }
    let valid = match std::str::from_utf8(head) {
        Ok(s) => s,
        Err(e) if e.error_len().is_none() => {
            // The head ends inside a character.
            std::str::from_utf8(&head[..e.valid_up_to()]).unwrap_or("")
        }
        Err(_) => return false,
    };
    let control = valid
        .chars()
        .filter(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t' | '\x0c'))
        .count();
    control * 100 <= valid.len().max(1)
}
