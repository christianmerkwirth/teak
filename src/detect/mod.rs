//! File types: stat, exact names, magic bytes, and extensions.
//!
//! Labels follow the Magika names where one exists (`rust`, `markdown`,
//! `txt`), so the fast and full paths give the same words.

pub mod ext;
pub mod magic;
pub mod magika;
pub mod names;
pub mod sample;
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

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::chaff::{ChaffClass, ChaffTag};
use crate::tree::{Kind, Node, NodeId, Tree};
use crate::view::{RowKind, View};

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
    // The other Magika labels. New labels go at the end, so the ids in the
    // type cache stay valid.
    "3gp" Av "video/3gpp" false,
    "ace" Archive "application/x-ace-compressed" false,
    "ai" Text "application/pdf" false,
    "aidl" Unknown "text/plain" true,
    "apk" Exec "application/vnd.android.package-archive" false,
    "applebplist" Binary "application/x-bplist" false,
    "appleplist" Data "application/x-plist" true,
    "asp" Code "text/html" true,
    "autohotkey" Code "text/plain" true,
    "autoit" Code "text/plain" true,
    "bazel" Code "text/plain" true,
    "bib" Text "text/x-bibtex" true,
    "cab" Archive "application/vnd.ms-cab-compressed" false,
    "cat" Binary "application/octet-stream" false,
    "chm" Binary "application/chm" false,
    "cobol" Code "text/x-cobol" true,
    "coff" Exec "application/x-coff" false,
    "coffeescript" Code "text/coffeescript" true,
    "crt" Text "application/x-x509-ca-cert" false,
    "crx" Exec "application/x-chrome-extension" false,
    "csproj" Code "text/plain" true,
    "dex" Exec "application/x-android-dex" false,
    "dicom" Image "application/dicom" false,
    "diff" Text "text/plain" true,
    "dm" Code "text/plain" true,
    "dsstore" Unknown "application/octet-stream" false,
    "dwg" Image "image/x-dwg" false,
    "dxf" Image "image/vnd.dxf" true,
    "emf" Binary "application/octet-stream" false,
    "eml" Text "message/rfc822" true,
    "erb" Code "text/x-ruby" true,
    "flv" Av "video/x-flv" false,
    "fortran" Text "text/x-fortran" true,
    "gitattributes" Code "text/plain" true,
    "gitmodules" Code "text/plain" true,
    "h5" Archive "application/x-hdf5" false,
    "handlebars" Code "text/x-handlebars-template" true,
    "hcl" Code "text/x-hcl" true,
    "hlp" Binary "application/winhlp" false,
    "htaccess" Code "text/x-apache-conf" true,
    "icns" Image "image/x-icns" false,
    "ics" Data "text/calendar" true,
    "ignorefile" Code "text/plain" true,
    "internetshortcut" Data "application/x-mswinurl" true,
    "ipynb" Code "application/json" true,
    "jinja" Code "text/x-jinja2-template" true,
    "jp2" Image "image/jpeg2000" false,
    "lha" Archive "application/x-lha" false,
    "lnk" Binary "application/x-ms-shortcut" false,
    "m3u" Data "text/plain" true,
    "m4" Code "text/plain" true,
    "matlab" Code "text/x-matlab" true,
    "mht" Code "application/x-mimearchive" true,
    "midi" Av "audio/midi" false,
    "mscompress" Archive "application/x-ms-compress-szdd" false,
    "msi" Archive "application/x-msi" false,
    "mum" Data "text/xml" true,
    "npz" Archive "application/octet-stream" false,
    "nupkg" Unknown "application/octet-stream" false,
    "odp" Text "application/vnd.oasis.opendocument.presentation" false,
    "ods" Text "application/vnd.oasis.opendocument.spreadsheet" false,
    "odt" Text "application/vnd.oasis.opendocument.text" false,
    "one" Text "application/msonenote" false,
    "outlook" Binary "application/vnd.ms-outlook" false,
    "pascal" Code "text/x-pascal" true,
    "pcap" Binary "application/vnd.tcpdump.pcap" false,
    "pdb" Binary "application/octet-stream" false,
    "po" Data "text/gettext-translation" true,
    "postscript" Text "application/postscript" false,
    "ppt" Text "application/vnd.ms-powerpoint" false,
    "prolog" Code "text/x-prolog" true,
    "proteindb" Data "application/octet-stream" true,
    "pytorch" Binary "application/octet-stream" false,
    "randombytes" Unknown "application/octet-stream" false,
    "randomtxt" Text "text/plain" true,
    "rdf" Text "application/rdf+xml" true,
    "sgml" Text "application/sgml" true,
    "smali" Code "application/x-smali" true,
    "snap" Archive "application/octet-stream" false,
    "solidity" Code "text/plain" true,
    "squashfs" Archive "application/octet-stream" false,
    "srt" Data "text/srt" true,
    "stlbinary" Image "application/sla" false,
    "stltext" Image "application/sla" true,
    "sum" Data "text/plain" true,
    "swf" Exec "application/x-shockwave-flash" false,
    "tcl" Code "application/x-tcl" true,
    "textproto" Code "text/plain" true,
    "tga" Image "image/x-tga" false,
    "thumbsdb" Binary "image/vnd.ms-thumb" false,
    "torrent" Binary "application/x-bittorrent" false,
    "twig" Code "text/x-twig" true,
    "vba" Code "text/vbscript" true,
    "vcxproj" Code "application/xml" true,
    "verilog" Code "text/x-verilog" true,
    "vhdl" Code "text/x-vhdl" true,
    "vtt" Text "text/vtt" true,
    "winregistry" Data "text/x-ms-regedit" true,
    "wmf" Image "image/wmf" false,
    "xar" Archive "application/x-xar" false,
    "xls" Text "application/vnd.ms-excel" false,
    "xlsb" Text "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" false,
    "xpi" Archive "application/zip" false,
    "yara" Code "text/x-yara" true,
    "zlibstream" Binary "application/zlib" false,
}

/// The label for an id.
pub fn label(id: LabelId) -> &'static Label {
    &LABELS[id as usize]
}

/// The id of a label name. Panics on a name that is not in [`LABELS`].
pub fn label_id(name: &str) -> LabelId {
    find_label(name).unwrap_or_else(|| panic!("unknown label {name}"))
}

/// The id of a label name, if it is in [`LABELS`].
pub fn find_label(name: &str) -> Option<LabelId> {
    static INDEX: OnceLock<HashMap<&'static str, LabelId>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        LABELS
            .iter()
            .enumerate()
            .map(|(i, l)| (l.name, i as LabelId))
            .collect()
    });
    index.get(name).copied()
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

/// The lowest Magika score that can win over the extension or give `⚠`.
pub const SURE: f32 = 0.90;

/// The label the extension says, when it does not match the content label
/// (FR-53): the score is [`SURE`] or more, the extension maps to a known
/// label, and the two labels are not the same type.
pub fn mismatch(content: LabelId, score: f32, name: &[u8]) -> Option<LabelId> {
    if score < SURE || matches!(label(content).name, "unknown" | "empty") {
        return None;
    }
    let by_ext = ext::by_name(name)?;
    (!same_type(by_ext, content)).then_some(by_ext)
}

/// Labels that do not conflict: equal names, an extension and a content
/// type of one family (a `.jar` is a zip, JSON is YAML), or plain text and
/// any text type.
pub fn same_type(by_ext: LabelId, content: LabelId) -> bool {
    const FAMILIES: &[(&str, &str)] = &[
        ("jar", "zip"),
        ("epub", "zip"),
        ("docx", "zip"),
        ("xlsx", "zip"),
        ("pptx", "zip"),
        ("object", "elf"),
        ("object", "coff"),
        ("svg", "xml"),
        ("html", "xml"),
        ("javascript", "typescript"),
        ("typescript", "javascript"),
        ("c", "cpp"),
        ("cpp", "c"),
        ("json", "jsonl"),
        ("jsonl", "json"),
        ("yaml", "json"),
        ("ini", "toml"),
        ("toml", "ini"),
        ("pem", "crt"),
    ];
    let (e, c) = (label(by_ext), label(content));
    if e.name == c.name || FAMILIES.contains(&(e.name, c.name)) {
        return true;
    }
    (e.name == "txt" && c.text) || (c.name == "txt" && e.text)
}

/// The head size for fast detection.
pub const HEAD_MAX: usize = 4096;

/// Series up to this size get every member typed; larger ones a sample.
pub const SERIES_ALL: usize = 64;

/// One typed file.
#[derive(Clone, Copy, Debug)]
pub struct Typed {
    pub info: TypeInfo,
    /// Detection read the content of this file.
    pub read: bool,
    /// Magika typed this file.
    pub model: bool,
    /// A secret by name, or a private-key header in the content.
    pub secret: bool,
}

/// The global cap of content detection (FR-51), with the Magika sub-cap of
/// Plan A1 (phase-0 report): Magika types the first `model_files` files of
/// a run, the fast path the rest.
#[derive(Clone, Copy, Debug)]
pub struct Cap {
    pub files: usize,
    pub time: Duration,
    pub model_files: usize,
}

impl Cap {
    pub const DEFAULT: Cap = Cap {
        files: 5_000,
        time: Duration::from_secs(2),
        model_files: 300,
    };
}

/// Types files: stat → names → (Magika | magic bytes) → extension. Content
/// comes only through `io`, never with `--detect off`, for non-regular
/// files, or on a pseudo file system.
pub struct Detector {
    pub mode: DetectMode,
    model: magika::Model,
    cap: Cap,
    start: Instant,
    /// Files whose content this run read.
    typed: usize,
    /// Files that Magika typed in this run.
    model_used: usize,
}

impl Detector {
    pub fn new(mode: DetectMode) -> Detector {
        Detector {
            mode,
            model: magika::Model::new(),
            cap: Cap::DEFAULT,
            start: Instant::now(),
            typed: 0,
            model_used: 0,
        }
    }

    /// Starts the cap for one root.
    pub fn begin(&mut self, cap: Cap) {
        self.cap = cap;
        self.start = Instant::now();
        self.typed = 0;
        self.model_used = 0;
    }

    /// True when the cap is reached: no more content reads.
    pub fn capped(&self) -> bool {
        self.typed >= self.cap.files || self.start.elapsed() >= self.cap.time
    }

    /// Types one file within the cap. After the cap, a file whose type
    /// needs its content gets the extension label, marked as estimated.
    pub fn type_capped(&mut self, tree: &Tree, id: NodeId) -> Typed {
        if self.mode != DetectMode::Off && self.capped() && sample::needs_content(tree, id) {
            let node = tree.node(id);
            let name = tree.name(id);
            let mut info = TypeInfo::new(name_label(node, name), Source::Ext);
            info.estimated = true;
            return Typed {
                info,
                read: false,
                model: false,
                secret: secret::is_secret(name, None),
            };
        }
        let use_model = self.model_used < self.cap.model_files;
        let t = self.type_file(tree, id, use_model);
        self.typed += usize::from(t.read);
        self.model_used += usize::from(t.model);
        t
    }

    /// Types one file. `use_model` lets Magika run (with `--detect full`).
    pub fn type_file(&mut self, tree: &Tree, id: NodeId, use_model: bool) -> Typed {
        let node = *tree.node(id);
        let done = |info: TypeInfo| Typed {
            info,
            read: false,
            model: false,
            secret: false,
        };
        if let Some(l) = stat_label(&node) {
            return done(TypeInfo::new(l, Source::Stat));
        }
        let name = tree.name(id);
        if node.size == 0 {
            return Typed {
                secret: secret::is_secret(name, None),
                ..done(TypeInfo::new(label_id("empty"), Source::Stat))
            };
        }
        let readable = self.mode != DetectMode::Off
            && node.kind == Kind::File
            && !node.has(crate::tree::flag::PSEUDO_FS);
        if !readable {
            return Typed {
                secret: secret::is_secret(name, None),
                ..done(fast_only(name, None))
            };
        }
        let path = tree.path(id);
        // Exact names need no model; the head read still feeds the secret check.
        if use_model
            && self.mode == DetectMode::Full
            && names::by_name(name).is_none()
            && let Some(m) = self.model.identify(&path)
        {
            let by_ext = ext::by_name(name);
            // Magic bytes that agree with the extension on a binary format
            // win over the model: two signals against one.
            let agreed = magic::by_magic(&m.head)
                .filter(|&(l, text)| !text && by_ext.is_some_and(|e| same_type(e, l)));
            let info = if let Some((l, _)) = agreed {
                TypeInfo::new(l, Source::Magic)
            } else if m.generic || (!label(m.label).text && magic::looks_like_text(&m.head)) {
                // A binary label for text is a model error (a run of `x`
                // comes back as `iso`).
                fast_only(name, Some(&m.head))
            } else {
                from_model(name, m.label, m.score)
            };
            return Typed {
                info,
                read: true,
                model: true,
                secret: secret::is_secret(name, Some(&m.head)),
            };
        }
        let head = crate::io::read_head(&path, HEAD_MAX).ok();
        Typed {
            info: fast_only(name, head.as_deref()),
            read: head.is_some(),
            model: false,
            secret: secret::is_secret(name, head.as_deref()),
        }
    }
}

/// The fast path, with no `⚠`: only a sure Magika label gives one (FR-53).
fn fast_only(name: &[u8], head: Option<&[u8]>) -> TypeInfo {
    TypeInfo {
        mismatch: None,
        ..fast::type_of(name, head)
    }
}

/// A Magika label. An unsure label loses to the extension.
fn from_model(name: &[u8], content: LabelId, score: f32) -> TypeInfo {
    let mut t = match ext::by_name(name) {
        Some(by_ext) if score < SURE => TypeInfo::new(by_ext, Source::Ext),
        _ => TypeInfo::new(content, Source::Magika),
    };
    t.score = score;
    if t.source == Source::Magika {
        t.mismatch = mismatch(content, score, name);
    }
    t
}

/// The result of [`run`].
pub struct Detected {
    /// The types of the files in the visible rows.
    pub types: HashMap<NodeId, TypeInfo>,
    /// Text files with their own row (or named in a chaff line) whose
    /// content detection read: the files for the content checks (FR-42).
    pub read_text: Vec<NodeId>,
    /// Per node: the label is an estimate (from a sample, or the extension
    /// after the cap).
    pub estimated: Vec<bool>,
}

/// The subtrees whose type mix shows in a row: closed directories and the
/// entries of overflow rows.
fn folded_nodes(view: &View) -> Vec<NodeId> {
    let mut out = Vec::new();
    for row in &view.rows {
        match &row.kind {
            RowKind::Summary { node, joined, .. } => {
                out.push(joined.last().copied().unwrap_or(*node))
            }
            RowKind::Overflow { nodes, .. } => out.extend_from_slice(nodes),
            _ => {}
        }
    }
    out
}

/// Types the files of the visible rows, then samples of the folded
/// directories (FR-50), within the cap (FR-51). Marks secrets. Files that
/// are boilerplate by name and named in a chaff line get typed too, so the
/// template check can label them.
///
/// `labels` holds the name labels on entry; on return, every typed file has
/// its detected label and every file of a sampled bucket the label of its
/// samples.
pub fn run(
    view: &View,
    tree: &mut Tree,
    tags: &[ChaffTag],
    labels: &mut [LabelId],
    det: &mut Detector,
    cap: Cap,
) -> Detected {
    det.begin(cap);
    let mut ids: Vec<NodeId> = Vec::new();
    let mut entries = HashSet::new();
    for row in &view.rows {
        match &row.kind {
            RowKind::Entry { node, .. } if !tree.node(*node).is_dir() => {
                ids.push(*node);
                entries.insert(*node);
            }
            RowKind::Chaff { items, .. } => {
                for item in items {
                    let boilerplate =
                        tags[item.node as usize].class == Some(ChaffClass::Boilerplate);
                    if boilerplate && !tree.node(item.node).is_dir() {
                        ids.push(item.node);
                        entries.insert(item.node);
                    }
                }
            }
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
    let mut types = HashMap::new();
    let mut read_text = Vec::new();
    let mut estimated = vec![false; tree.len()];
    let mut mark = |tree: &mut Tree, id: NodeId, t: &Typed| {
        if t.secret {
            tree.nodes[id as usize].flags |= crate::tree::flag::SECRET;
        }
        labels[id as usize] = t.info.label;
        estimated[id as usize] = t.info.estimated;
    };
    for id in ids {
        let t = det.type_capped(tree, id);
        mark(tree, id, &t);
        if t.read && t.info.is_text && entries.contains(&id) {
            read_text.push(id);
        }
        types.insert(id, t.info);
    }
    if det.mode != DetectMode::Off {
        let folded = folded_nodes(view);
        let hidden = |id: NodeId| crate::chaff::is_hidden(tags, id);
        let mut sampled: HashMap<NodeId, LabelId> = HashMap::new();
        let mut votes: HashMap<(NodeId, u64), Vec<LabelId>> = HashMap::new();
        for id in sample::plan(tree, &folded, sample::K, hidden) {
            if det.capped() {
                break;
            }
            let t = det.type_capped(tree, id);
            mark(tree, id, &t);
            sampled.insert(id, t.info.label);
            if let Some(key) = sample::bucket(tree, id) {
                votes.entry(key).or_default().push(t.info.label);
            }
        }
        sample::for_each_member(tree, &folded, &hidden, |id| {
            if sampled.contains_key(&id) {
                return;
            }
            estimated[id as usize] = true;
            if let Some(v) = sample::bucket(tree, id).and_then(|key| votes.get(&key)) {
                labels[id as usize] = majority(v);
            }
        });
    }
    Detected {
        types,
        read_text,
        estimated,
    }
}

/// The most common label; a tie goes to the label sampled first.
fn majority(labels: &[LabelId]) -> LabelId {
    let mut best = (0, labels[0]);
    for &l in labels {
        let n = labels.iter().filter(|&&m| m == l).count();
        if n > best.0 {
            best = (n, l);
        }
    }
    best.1
}

/// `--mismatches`: types every regular file that is not hidden, within the
/// cap (Magika for all of them, not only the first 300), and gives the
/// mismatched ones in path order.
pub fn mismatched(
    tree: &mut Tree,
    hidden: impl Fn(NodeId) -> bool,
    det: &mut Detector,
    cap: Cap,
) -> Vec<(NodeId, TypeInfo)> {
    det.begin(Cap {
        model_files: cap.files,
        ..cap
    });
    let mut out = Vec::new();
    let mut id: NodeId = 0;
    while (id as usize) < tree.len() && !det.capped() {
        if hidden(id) {
            id = tree.node(id).end;
            continue;
        }
        if tree.node(id).kind == Kind::File {
            let t = det.type_capped(tree, id);
            if t.secret {
                tree.nodes[id as usize].flags |= crate::tree::flag::SECRET;
            }
            if t.info.mismatch.is_some() {
                out.push((id, t.info));
            }
        }
        id += 1;
    }
    // Pre-order ids sort by name bytes per directory; a full path sort puts
    // `a/b` before `a.c`, as `sort` does.
    out.sort_by_cached_key(|(id, _)| tree.rel_path(*id));
    out
}
