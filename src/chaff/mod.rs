//! Chaff: minor and boilerplate entries, folded into one line per directory.

pub mod content;
pub mod gitattributes;
pub mod gitignore;
pub mod packs;
pub mod rules;
pub mod templates;

use crate::tree::{Kind, NONE, NodeId, ROOT, Tree, flag};

/// A chaff class (PRD §6.1), plus the pseudo-class `Ignored` for gitignored
/// entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChaffClass {
    Vendor,
    Build,
    Cache,
    Vcs,
    Editor,
    Lock,
    Generated,
    Minified,
    Boilerplate,
    Empty,
    Duplicate,
    Ignored,
}

impl ChaffClass {
    pub const ALL: [ChaffClass; 12] = [
        ChaffClass::Vendor,
        ChaffClass::Build,
        ChaffClass::Cache,
        ChaffClass::Vcs,
        ChaffClass::Editor,
        ChaffClass::Lock,
        ChaffClass::Generated,
        ChaffClass::Minified,
        ChaffClass::Boilerplate,
        ChaffClass::Empty,
        ChaffClass::Duplicate,
        ChaffClass::Ignored,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ChaffClass::Vendor => "vendor",
            ChaffClass::Build => "build",
            ChaffClass::Cache => "cache",
            ChaffClass::Vcs => "vcs",
            ChaffClass::Editor => "editor",
            ChaffClass::Lock => "lock",
            ChaffClass::Generated => "generated",
            ChaffClass::Minified => "minified",
            ChaffClass::Boilerplate => "boilerplate",
            ChaffClass::Empty => "empty",
            ChaffClass::Duplicate => "duplicate",
            ChaffClass::Ignored => "ignored",
        }
    }

    /// The default action from PRD §6.1.
    pub fn default_action(self) -> Action {
        match self {
            ChaffClass::Vcs | ChaffClass::Editor => Action::Hide,
            _ => Action::Fold,
        }
    }
}

/// What teak does with an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A normal row.
    Show,
    /// A normal row with a low score.
    Dim,
    /// Goes into the directory's chaff line.
    Fold,
    /// Counted only in the footer.
    Hide,
}

/// Where a tag came from (FR-31 order, then the fallbacks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// No rule matched.
    None,
    User,
    Gitattributes,
    Pack,
    Content,
    Global,
    Gitignore,
    /// A dotfile with no class.
    Hidden,
    /// Inside a folded or hidden directory: it goes with that directory.
    Inherited,
}

#[derive(Clone, Copy, Debug)]
pub struct ChaffTag {
    pub class: Option<ChaffClass>,
    pub action: Action,
    pub source: Source,
    /// A template id (index + 1), for a label like `LICENSE (MIT)`; 0 is none.
    pub label: u16,
}

impl ChaffTag {
    pub const NONE: ChaffTag = ChaffTag {
        class: None,
        action: Action::Show,
        source: Source::None,
        label: 0,
    };

    /// The group name in the footer: the class, or `dotfile`.
    pub fn group_name(&self) -> &'static str {
        match self.class {
            Some(c) => c.name(),
            None => "dotfile",
        }
    }
}

/// The chaff settings from the command line.
#[derive(Clone, Debug, Default)]
pub struct ChaffOpts {
    /// `-A`: hidden entries fold.
    pub almost_all: bool,
    /// `-a`: everything shows, no gitignore.
    pub all: bool,
    pub no_gitignore: bool,
    /// `--chaff LEVEL`.
    pub level: Option<Action>,
    /// `--show/--fold/--hide`, in command-line order.
    pub overrides: Vec<(ChaffClass, Action)>,
}

pub struct Classifier<'a> {
    opts: &'a ChaffOpts,
    templates: &'a templates::Templates,
}

/// The largest rule file that teak reads.
const RULE_FILE_MAX: usize = 1 << 20;

/// One open directory in the top-down pass.
struct Frame {
    end: NodeId,
    /// The length of this directory's relative path in the path buffer.
    path_len: usize,
    ignore: Option<gitignore::DirRules>,
    attrs: Option<gitattributes::Attrs>,
    /// Packs whose marker is a child of this directory.
    packs: Vec<&'static packs::Pack>,
    tag: ChaffTag,
}

impl<'a> Classifier<'a> {
    pub fn new(
        _tree: &Tree,
        opts: &'a ChaffOpts,
        templates: &'a templates::Templates,
    ) -> Classifier<'a> {
        Classifier { opts, templates }
    }

    /// The tag from a content check (generated, minified, or a template),
    /// for a text file with no class from its name.
    ///
    /// Pre: `head` came from `io::read_head` with [`content::READ_MAX`].
    pub fn classify_content(
        &self,
        tree: &Tree,
        id: NodeId,
        head: &[u8],
        size: u64,
    ) -> Option<ChaffTag> {
        let (class, label) = content::check(head, size, self.templates)?;
        Some(self.content_tag(tree, id, class, label))
    }

    /// The label for a file that is boilerplate by its name, when its text
    /// is a known template (`.gitignore (gitignore-Rust)`). The class stays.
    pub fn template_label(&self, head: &[u8], size: u64) -> Option<u16> {
        content::template(head, size, self.templates).map(|(_, id)| id)
    }

    /// The tag for a content class, as a check or the type cache gives it.
    pub fn content_tag(&self, tree: &Tree, id: NodeId, class: ChaffClass, label: u16) -> ChaffTag {
        ChaffTag {
            class: Some(class),
            action: self.action(Some(class), tree.is_dotfile(id)),
            source: Source::Content,
            label,
        }
    }

    fn gitignore_on(&self) -> bool {
        !self.opts.all && !self.opts.no_gitignore
    }

    /// The action for a class (or for an unclassed dotfile), after `-A`,
    /// `-a`, `--chaff`, and `--show/--fold/--hide` in command-line order.
    pub fn action(&self, class: Option<ChaffClass>, dotfile: bool) -> Action {
        let mut action = match class {
            Some(c) => c.default_action(),
            None if dotfile => Action::Hide,
            None => return Action::Show,
        };
        if self.opts.all {
            action = Action::Show;
        } else if self.opts.almost_all && action == Action::Hide {
            action = Action::Fold;
        }
        let Some(class) = class else {
            return action;
        };
        if let Some(level) = self.opts.level {
            action = level;
        }
        for &(c, a) in &self.opts.overrides {
            if c == class {
                action = a;
            }
        }
        action
    }

    /// One tag per node. The first matching source wins (FR-31): the user,
    /// `.gitattributes`, rule packs, global names, then gitignore. Entries
    /// inside a folded or hidden directory go with it.
    ///
    /// Post: every node has exactly one tag.
    pub fn classify_names(&self, tree: &Tree) -> Vec<ChaffTag> {
        let mut tags = vec![ChaffTag::NONE; tree.len()];
        if tree.is_empty() || !tree.node(ROOT).is_dir() {
            return tags;
        }
        let mut path: Vec<u8> = Vec::new();
        let mut stack: Vec<Frame> = vec![self.frame(tree, ROOT, &path, ChaffTag::NONE)];
        for id in 1..tree.len() as NodeId {
            while stack.last().is_some_and(|f| f.end <= id) {
                stack.pop();
            }
            let Some(parent) = stack.last() else { break };
            path.truncate(parent.path_len);
            if !path.is_empty() {
                path.push(b'/');
            }
            path.extend_from_slice(tree.name(id));
            let node = tree.node(id);
            let tag = if matches!(parent.tag.action, Action::Fold | Action::Hide) {
                ChaffTag {
                    source: Source::Inherited,
                    ..parent.tag
                }
            } else {
                self.classify(tree, id, &path, &stack)
            };
            tags[id as usize] = tag;
            if node.is_dir() {
                let frame = self.frame(tree, id, &path, tag);
                stack.push(frame);
            }
        }
        tags
    }

    fn classify(&self, tree: &Tree, id: NodeId, rel: &[u8], stack: &[Frame]) -> ChaffTag {
        let node = tree.node(id);
        let name = tree.name(id);
        let is_dir = node.is_dir();
        let tag = |class: Option<ChaffClass>, source: Source| ChaffTag {
            class,
            action: self.action(class, tree.is_dotfile(id)),
            source,
            label: 0,
        };
        // .gitattributes: the nearest file with an opinion decides.
        let mut blocked: Vec<ChaffClass> = Vec::new();
        for (class, pick) in [
            (
                ChaffClass::Generated,
                gitattributes::Attrs::generated
                    as fn(&gitattributes::Attrs, &[u8], bool) -> Option<bool>,
            ),
            (ChaffClass::Vendor, gitattributes::Attrs::vendored),
        ] {
            let found = stack
                .iter()
                .rev()
                .filter_map(|f| f.attrs.as_ref())
                .find_map(|a| pick(a, rel, is_dir));
            match found {
                Some(true) => return tag(Some(class), Source::Gitattributes),
                Some(false) => blocked.push(class),
                None => {}
            }
        }
        let ok = |c: &ChaffClass| !blocked.contains(c);
        // Rule packs of the parent directory.
        if let Some(parent) = stack.last()
            && let Some(class) = parent
                .packs
                .iter()
                .find_map(|p| p.class_of(name, is_dir))
                .filter(ok)
        {
            return tag(Some(class), Source::Pack);
        }
        if let Some(class) = rules::by_name(name, is_dir).filter(ok) {
            return tag(Some(class), Source::Global);
        }
        let unreadable = node.has(flag::NO_ACCESS | flag::MOUNT);
        let empty = match node.kind {
            Kind::File => node.size == 0 && !node.has(flag::LINK_DUP),
            Kind::Dir => node.first_child == NONE && !unreadable,
            _ => false,
        };
        if empty {
            return tag(Some(ChaffClass::Empty), Source::Global);
        }
        if self.gitignore_on() {
            let ignored = stack
                .iter()
                .rev()
                .filter_map(|f| f.ignore.as_ref())
                .find_map(|g| g.matched(rel, is_dir));
            if ignored == Some(true) {
                return tag(Some(ChaffClass::Ignored), Source::Gitignore);
            }
        }
        if tree.is_dotfile(id) {
            return tag(None, Source::Hidden);
        }
        ChaffTag::NONE
    }

    /// Reads the rule files and markers among the children of `dir`.
    fn frame(&self, tree: &Tree, dir: NodeId, rel: &[u8], tag: ChaffTag) -> Frame {
        let mut frame = Frame {
            end: tree.node(dir).end,
            path_len: rel.len(),
            ignore: None,
            attrs: None,
            packs: Vec::new(),
            tag,
        };
        if matches!(tag.action, Action::Fold | Action::Hide) {
            return frame;
        }
        let readable = |id: NodeId| {
            let n = tree.node(id);
            n.kind == Kind::File && !n.has(flag::PSEUDO_FS)
        };
        let mut ignore_texts: Vec<(std::path::PathBuf, Vec<u8>)> = Vec::new();
        for c in tree.children(dir) {
            let name = tree.name(c);
            for pack in packs::PACKS {
                // `*.xcodeproj` markers are directories; the others are files.
                let kind_ok = !tree.node(c).is_dir() || pack.marker.starts_with('*');
                if kind_ok
                    && pack.is_marker(name)
                    && !frame.packs.iter().any(|p| std::ptr::eq(*p, pack))
                {
                    frame.packs.push(pack);
                }
            }
            match name {
                b".gitattributes" if readable(c) => {
                    let path = tree.path(c);
                    if let Ok(text) = crate::io::read_rule_file(&path, RULE_FILE_MAX) {
                        frame.attrs = gitattributes::Attrs::parse(rel, &text);
                    }
                }
                b".gitignore" if readable(c) && self.gitignore_on() => {
                    let path = tree.path(c);
                    if let Ok(text) = crate::io::read_rule_file(&path, RULE_FILE_MAX) {
                        ignore_texts.push((path, text));
                    }
                }
                b".git" if self.gitignore_on() && tree.node(c).is_dir() => {
                    let exclude = tree
                        .children(c)
                        .find(|&i| tree.name(i) == b"info")
                        .and_then(|info| tree.children(info).find(|&e| tree.name(e) == b"exclude"));
                    if let Some(e) = exclude.filter(|&e| readable(e)) {
                        let path = tree.path(e);
                        if let Ok(text) = crate::io::read_rule_file(&path, RULE_FILE_MAX) {
                            // `info/exclude` comes first: `.gitignore` lines win.
                            ignore_texts.insert(0, (path, text));
                        }
                    }
                }
                _ => {}
            }
        }
        if !ignore_texts.is_empty() {
            let texts: Vec<(&std::path::Path, &[u8])> = ignore_texts
                .iter()
                .map(|(p, t)| (p.as_path(), t.as_slice()))
                .collect();
            frame.ignore = gitignore::DirRules::new(rel, &texts);
        }
        frame
    }
}

/// True when the node is counted only in the footer.
pub fn is_hidden(tags: &[ChaffTag], id: NodeId) -> bool {
    tags[id as usize].action == Action::Hide
}
