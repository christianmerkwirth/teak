//! Chaff: minor and boilerplate entries, folded into one line per directory.

use crate::tree::{NodeId, Tree};

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
    /// A label id for the chaff line, for example `MIT` (task-3); 0 is none.
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
    #[allow(dead_code)]
    opts: &'a ChaffOpts,
}

impl<'a> Classifier<'a> {
    pub fn new(_tree: &Tree, opts: &'a ChaffOpts) -> Classifier<'a> {
        Classifier { opts }
    }

    /// One tag per node.
    pub fn classify_names(&self, tree: &Tree) -> Vec<ChaffTag> {
        vec![ChaffTag::NONE; tree.len()]
    }
}

/// True when the node is counted only in the footer.
pub fn is_hidden(tags: &[ChaffTag], id: NodeId) -> bool {
    tags[id as usize].action == Action::Hide
}
