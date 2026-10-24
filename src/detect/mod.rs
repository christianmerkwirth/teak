//! File types: stat, exact names, magic bytes, and extensions.
//!
//! Labels follow the Magika names where one exists (`rust`, `markdown`,
//! `txt`), so the fast and full paths give the same words.

pub mod ext;
pub mod magic;
pub mod names;
pub mod secret;

/// The fast path: no model.
pub mod fast {
    use super::{Source, TypeInfo, ext, label_id, magic, names};

    /// The type from the name and the first bytes: exact name rules, then
    /// magic bytes, then the extension, then a shebang or a text check.
    ///
    /// Pre: `head` comes from `io::read_head(path, 4096)`; it is `None` with
    /// `--detect off`, for non-regular files, and on a pseudo FS.
    pub fn type_of(name: &[u8], head: Option<&[u8]>) -> TypeInfo {
        if let Some(l) = names::by_name(name) {
            return TypeInfo::new(l, Source::Name);
        }
        let by_ext = ext::by_name(name);
        if let Some(head) = head
            && let Some((l, text)) = magic::by_magic(head)
            && (!text || by_ext.is_none())
        {
            let mut t = TypeInfo::new(l, Source::Magic);
            t.mismatch = by_ext.filter(|&e| !same_family(e, l));
            return t;
        }
        if let Some(l) = by_ext {
            return TypeInfo::new(l, Source::Ext);
        }
        if let Some(head) = head {
            if let Some(l) = magic::by_shebang(head) {
                return TypeInfo::new(l, Source::Magic);
            }
            if magic::looks_like_text(head) {
                return TypeInfo::new(label_id("txt"), Source::Magic);
            }
        }
        TypeInfo::new(label_id("unknown"), Source::None)
    }

    /// An extension label and a magic label that do not conflict: a `.jar`
    /// is a zip, a `.o` is an ELF file.
    fn same_family(ext: super::LabelId, magic: super::LabelId) -> bool {
        const PAIRS: &[(&str, &str)] = &[
            ("jar", "zip"),
            ("epub", "zip"),
            ("docx", "zip"),
            ("xlsx", "zip"),
            ("pptx", "zip"),
            ("object", "elf"),
            ("gzip", "gzip"),
            ("svg", "xml"),
            ("html", "xml"),
        ];
        let (e, m) = (super::label(ext).name, super::label(magic).name);
        e == m || PAIRS.iter().any(|&(a, b)| a == e && b == m)
    }
}

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::tree::{Kind, Node};

/// An index into [`LABELS`].
pub type LabelId = u16;

/// The color group of a type (PRD §5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeGroup {
    Code,
    Text,
    Data,
    Image,
    Av,
    Archive,
    Exec,
    Binary,
    Unknown,
}

pub struct Label {
    pub name: &'static str,
    pub group: TypeGroup,
    pub mime: &'static str,
    pub text: bool,
}

macro_rules! labels {
    ($($name:literal $group:ident $mime:literal $text:literal),* $(,)?) => {
        pub static LABELS: &[Label] = &[
            $(Label { name: $name, group: TypeGroup::$group, mime: $mime, text: $text }),*
        ];
    };
}

labels! {
    "unknown" Unknown "application/octet-stream" false,
    "empty" Unknown "inode/x-empty" true,
    "dir" Unknown "inode/directory" false,
    "symlink" Unknown "inode/symlink" false,
    "fifo" Unknown "inode/fifo" false,
    "socket" Unknown "inode/socket" false,
    "device" Unknown "inode/device" false,
    // text and documents
    "txt" Text "text/plain" true,
    "markdown" Text "text/markdown" true,
    "rst" Text "text/x-rst" true,
    "tex" Text "text/x-tex" true,
    "rtf" Text "text/rtf" true,
    "pdf" Text "application/pdf" false,
    "doc" Text "application/msword" false,
    "docx" Text "application/vnd.openxmlformats-officedocument.wordprocessingml.document" false,
    "xlsx" Text "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" false,
    "pptx" Text "application/vnd.openxmlformats-officedocument.presentationml.presentation" false,
    "epub" Text "application/epub+zip" false,
    // code
    "rust" Code "text/x-rust" true,
    "python" Code "text/x-python" true,
    "javascript" Code "text/javascript" true,
    "typescript" Code "text/x-typescript" true,
    "c" Code "text/x-c" true,
    "cpp" Code "text/x-c++" true,
    "cs" Code "text/x-csharp" true,
    "go" Code "text/x-go" true,
    "java" Code "text/x-java" true,
    "kotlin" Code "text/x-kotlin" true,
    "scala" Code "text/x-scala" true,
    "swift" Code "text/x-swift" true,
    "objectivec" Code "text/x-objectivec" true,
    "ruby" Code "text/x-ruby" true,
    "php" Code "text/x-php" true,
    "perl" Code "text/x-perl" true,
    "lua" Code "text/x-lua" true,
    "r" Code "text/x-r" true,
    "julia" Code "text/x-julia" true,
    "haskell" Code "text/x-haskell" true,
    "ocaml" Code "text/x-ocaml" true,
    "elixir" Code "text/x-elixir" true,
    "erlang" Code "text/x-erlang" true,
    "clojure" Code "text/x-clojure" true,
    "dart" Code "text/x-dart" true,
    "zig" Code "text/x-zig" true,
    "asm" Code "text/x-asm" true,
    "shell" Code "text/x-shellscript" true,
    "powershell" Code "text/x-powershell" true,
    "batch" Code "text/x-msdos-batch" true,
    "makefile" Code "text/x-makefile" true,
    "dockerfile" Code "text/x-dockerfile" true,
    "cmake" Code "text/x-cmake" true,
    "html" Code "text/html" true,
    "css" Code "text/css" true,
    "scss" Code "text/x-scss" true,
    "vue" Code "text/x-vue" true,
    "sql" Code "application/sql" true,
    "proto" Code "text/x-protobuf" true,
    "groovy" Code "text/x-groovy" true,
    "vim" Code "text/x-vim" true,
    "lisp" Code "text/x-lisp" true,
    "nix" Code "text/x-nix" true,
    "terraform" Code "text/x-terraform" true,
    "kconfig" Code "text/x-kconfig" true,
    "devicetree" Code "text/x-devicetree" true,
    "awk" Code "text/x-awk" true,
    // data
    "json" Data "application/json" true,
    "jsonl" Data "application/jsonl" true,
    "toml" Data "application/toml" true,
    "yaml" Data "application/yaml" true,
    "xml" Data "text/xml" true,
    "csv" Data "text/csv" true,
    "tsv" Data "text/tab-separated-values" true,
    "ini" Data "text/x-ini" true,
    "graphql" Data "application/graphql" true,
    "parquet" Data "application/vnd.apache.parquet" false,
    "avro" Data "application/avro" false,
    "sqlite" Data "application/vnd.sqlite3" false,
    "npy" Data "application/x-npy" false,
    // images
    "png" Image "image/png" false,
    "jpeg" Image "image/jpeg" false,
    "gif" Image "image/gif" false,
    "webp" Image "image/webp" false,
    "svg" Image "image/svg+xml" true,
    "bmp" Image "image/bmp" false,
    "tiff" Image "image/tiff" false,
    "ico" Image "image/vnd.microsoft.icon" false,
    "heif" Image "image/heif" false,
    "avif" Image "image/avif" false,
    "psd" Image "image/vnd.adobe.photoshop" false,
    // audio and video
    "mp3" Av "audio/mpeg" false,
    "wav" Av "audio/wav" false,
    "flac" Av "audio/flac" false,
    "ogg" Av "audio/ogg" false,
    "m4a" Av "audio/mp4" false,
    "aac" Av "audio/aac" false,
    "mp4" Av "video/mp4" false,
    "mkv" Av "video/x-matroska" false,
    "webm" Av "video/webm" false,
    "mov" Av "video/quicktime" false,
    "avi" Av "video/x-msvideo" false,
    // archives
    "zip" Archive "application/zip" false,
    "gzip" Archive "application/gzip" false,
    "tar" Archive "application/x-tar" false,
    "xz" Archive "application/x-xz" false,
    "zstd" Archive "application/zstd" false,
    "bzip2" Archive "application/x-bzip2" false,
    "7zip" Archive "application/x-7z-compressed" false,
    "rar" Archive "application/vnd.rar" false,
    "jar" Archive "application/java-archive" false,
    "deb" Archive "application/vnd.debian.binary-package" false,
    "rpm" Archive "application/x-rpm" false,
    "dmg" Archive "application/x-apple-diskimage" false,
    "iso" Archive "application/x-iso9660-image" false,
    // executables
    "elf" Exec "application/x-elf" false,
    "macho" Exec "application/x-mach-binary" false,
    "pebin" Exec "application/vnd.microsoft.portable-executable" false,
    "wasm" Exec "application/wasm" false,
    // other binaries
    "javabytecode" Binary "application/x-java-applet" false,
    "pyc" Binary "application/x-python-bytecode" false,
    "object" Binary "application/x-object" false,
    "font" Binary "font/ttf" false,
    "pickle" Binary "application/x-pickle" false,
    "onnx" Binary "application/x-onnx" false,
    "pem" Text "application/x-pem-file" true,
}

/// The label for an id.
pub fn label(id: LabelId) -> &'static Label {
    &LABELS[id as usize]
}

/// The id of a label name. Panics on a name that is not in [`LABELS`].
pub fn label_id(name: &str) -> LabelId {
    static INDEX: OnceLock<HashMap<&'static str, LabelId>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        LABELS
            .iter()
            .enumerate()
            .map(|(i, l)| (l.name, i as LabelId))
            .collect()
    });
    *index
        .get(name)
        .unwrap_or_else(|| panic!("unknown label {name}"))
}

/// Where a type came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Stat,
    Name,
    Magika,
    Magic,
    Ext,
    /// Nothing matched.
    None,
}

#[derive(Clone, Copy, Debug)]
pub struct TypeInfo {
    pub label: LabelId,
    pub group: TypeGroup,
    pub mime: &'static str,
    pub is_text: bool,
    pub score: f32,
    pub source: Source,
    /// From a sample, not from this file.
    pub estimated: bool,
    /// The label the extension says, when the content says something else.
    pub mismatch: Option<LabelId>,
}

impl TypeInfo {
    pub fn new(label: LabelId, source: Source) -> TypeInfo {
        let l = self::label(label);
        TypeInfo {
            label,
            group: l.group,
            mime: l.mime,
            is_text: l.text,
            score: 1.0,
            source,
            estimated: false,
            mismatch: None,
        }
    }

    pub fn name(&self) -> &'static str {
        label(self.label).name
    }
}

/// `--detect`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectMode {
    Off,
    Fast,
    Full,
}

/// The type from stat alone: directories, symlinks, and special files.
pub fn stat_label(node: &Node) -> Option<LabelId> {
    let name = match node.kind {
        Kind::Dir => "dir",
        Kind::Symlink => "symlink",
        Kind::Fifo => "fifo",
        Kind::Socket => "socket",
        Kind::Device => "device",
        Kind::File => return None,
    };
    Some(label_id(name))
}

/// The type from stat and the name only: no content. Used for type mixes
/// and before detection runs.
pub fn name_label(node: &Node, name: &[u8]) -> LabelId {
    if let Some(l) = stat_label(node) {
        return l;
    }
    if node.size == 0 {
        return label_id("empty");
    }
    names::by_name(name)
        .or_else(|| ext::by_name(name))
        .unwrap_or_else(|| label_id("unknown"))
}

/// The head size for fast detection.
pub const HEAD_MAX: usize = 4096;

/// Series up to this size get every member typed; larger ones a sample.
pub const SERIES_ALL: usize = 64;

/// Types the files of the visible rows, and marks secrets. Content comes
/// only from `io::read_head`, never with `--detect off`, for non-regular
/// files, or on a pseudo file system.
pub fn run(
    view: &crate::view::View,
    tree: &mut crate::tree::Tree,
    _labels: &[LabelId],
    mode: DetectMode,
) -> HashMap<crate::tree::NodeId, TypeInfo> {
    use crate::tree::flag;
    use crate::view::RowKind;
    let mut ids: Vec<crate::tree::NodeId> = Vec::new();
    for row in &view.rows {
        match &row.kind {
            RowKind::Entry { node, .. } if !tree.node(*node).is_dir() => ids.push(*node),
            // Small series: every member; large ones: first, middle, last.
            RowKind::Series { series, .. } => {
                let m = &series.members;
                if m.len() <= SERIES_ALL {
                    ids.extend_from_slice(m);
                } else {
                    ids.extend([m[0], m[m.len() / 2], m[m.len() - 1]]);
                }
            }
            _ => {}
        }
    }
    let mut out = HashMap::new();
    for id in ids {
        let node = *tree.node(id);
        let name = tree.name(id).to_vec();
        if let Some(l) = stat_label(&node) {
            out.insert(id, TypeInfo::new(l, Source::Stat));
            continue;
        }
        let readable = mode != DetectMode::Off
            && node.kind == Kind::File
            && node.size > 0
            && !node.has(flag::PSEUDO_FS);
        let head = if readable {
            crate::io::read_head(&tree.path(id), HEAD_MAX).ok()
        } else {
            None
        };
        let info = if node.size == 0 {
            TypeInfo::new(label_id("empty"), Source::Stat)
        } else {
            fast::type_of(&name, head.as_deref())
        };
        if secret::is_secret(&name, head.as_deref()) {
            tree.nodes[id as usize].flags |= flag::SECRET;
        }
        out.insert(id, info);
    }
    out
}
