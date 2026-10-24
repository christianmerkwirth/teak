//! teak: fit a large directory tree into a fixed line budget.
//!
//! [`run`] drives the pipeline for each root: walk → chaff tags → totals →
//! salience → selection → detection on the visible rows → layout → render.

pub mod aggregate;
pub mod cache;
pub mod chaff;
pub mod cli;
pub mod detect;
pub mod dupes;
pub mod explain;
pub mod fnv;
pub mod git;
pub mod io;
pub mod kind;
pub mod layout;
pub mod preview;
pub mod render;
pub mod salience;
pub mod select;
pub mod series;
pub mod style;
pub mod tree;
pub mod view;
pub mod walk;

use std::collections::HashMap;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use aggregate::Aggs;
use chaff::ChaffTag;
use cli::{Budget, ColorWhen, Opts};
use detect::{LabelId, TypeInfo};
use layout::{Glyphs, LayoutOpts};
use render::Format;
use style::Style;
use tree::{NodeId, ScanError, Tree, flag};
use view::View;

/// The `teak --version` line, without the newline.
pub fn version_line() -> String {
    let version = env!("CARGO_PKG_VERSION");
    #[cfg(feature = "magika")]
    return format!(
        "teak {version} (magika: on, model {})",
        detect::magika::Model::version()
    );
    #[cfg(not(feature = "magika"))]
    return format!("teak {version} (magika: off)");
}

/// The process facts that the options depend on.
#[derive(Clone, Debug)]
pub struct Env {
    pub stdout_tty: bool,
    /// Columns and rows of the terminal on stdout.
    pub term: Option<(usize, usize)>,
    /// Seconds since the epoch: `SOURCE_DATE_EPOCH`, or now.
    pub scan_time: i64,
    pub no_color: bool,
    pub clicolor_force: bool,
    pub ls_colors: Option<String>,
    /// Where the type cache lives (see [`cache::dir_from_env`]).
    pub cache_dir: Option<PathBuf>,
}

impl Env {
    pub fn from_process() -> Env {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let scan_time = var("SOURCE_DATE_EPOCH")
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as i64)
            });
        Env {
            stdout_tty: std::io::stdout().is_terminal(),
            term: terminal_size::terminal_size().map(|(w, h)| (usize::from(w.0), usize::from(h.0))),
            scan_time,
            no_color: var("NO_COLOR").is_some(),
            clicolor_force: var("CLICOLOR_FORCE").is_some_and(|v| v != "0"),
            ls_colors: var("LS_COLORS"),
            cache_dir: cache::dir_from_env(),
        }
    }

    /// A fixed environment for library callers: a pipe, no colors, no
    /// type cache.
    pub fn fixed(scan_time: i64) -> Env {
        Env {
            stdout_tty: false,
            term: None,
            scan_time,
            no_color: false,
            clicolor_force: false,
            ls_colors: None,
            cache_dir: None,
        }
    }
}

/// The settings that follow from the options and the environment.
pub struct Settings {
    pub format: Format,
    pub width: usize,
    pub budget: usize,
    pub color: bool,
    pub glyphs: &'static Glyphs,
}

pub fn settings(opts: &Opts, env: &Env) -> Settings {
    let format = opts.format.unwrap_or(if env.stdout_tty {
        Format::Tty
    } else {
        Format::Plain
    });
    let width = opts.width.unwrap_or(match (format, env.term) {
        (Format::Tty, Some((cols, _))) if cols > 0 => cols,
        _ => 120,
    });
    let budget = match opts.budget {
        Budget::Lines(n) => n,
        Budget::Auto => match env.term {
            Some((_, rows)) if env.stdout_tty => rows.saturating_sub(2).max(3),
            _ => 100,
        },
    };
    // `md` and `json` never have colors. Then --color wins, then
    // CLICOLOR_FORCE, then NO_COLOR, then the format.
    let color = matches!(format, Format::Tty | Format::Plain)
        && match opts.color {
            ColorWhen::Always => true,
            ColorWhen::Never => false,
            ColorWhen::Auto => {
                if env.clicolor_force {
                    true
                } else if env.no_color {
                    false
                } else {
                    format == Format::Tty
                }
            }
        };
    Settings {
        format,
        width,
        budget,
        color,
        glyphs: if opts.ascii {
            &layout::ASCII
        } else {
            &layout::UNICODE
        },
    }
}

/// Everything teak knows about one root, with its view.
pub struct Scan {
    pub tree: Tree,
    pub tags: Vec<ChaffTag>,
    /// Totals without hidden entries.
    pub aggs: Aggs,
    /// The label of every node: from the name, from detection for typed
    /// files, and from the samples for the rest of a sampled bucket.
    pub labels: Vec<LabelId>,
    /// Per node: the label is an estimate (`≈`).
    pub estimated: Vec<bool>,
    /// Detected types of the visible files.
    pub types: HashMap<NodeId, TypeInfo>,
    pub view: View,
    /// Template labels by `ChaffTag::label` - 1, for `LICENSE (MIT)`.
    pub template_labels: Vec<String>,
    /// The project kinds of the root (FR-59).
    pub kinds: Vec<&'static str>,
    /// Git status, when the root is in a repo and `--no-git` is off.
    pub git: Option<git::GitStatus>,
    pub git_head: Option<git::GitHead>,
    /// The previews of the visible files (FR-56).
    pub previews: HashMap<NodeId, String>,
    /// The `--why` lines (FR-60).
    pub why: Option<Vec<String>>,
    /// `--dupes`: copy → original (FR-43).
    pub dupes: HashMap<NodeId, NodeId>,
}

impl Scan {
    pub fn type_of(&self, id: NodeId) -> TypeInfo {
        self.types
            .get(&id)
            .copied()
            .unwrap_or_else(|| TypeInfo::new(self.labels[id as usize], detect::Source::Ext))
    }

    /// The template label of a node, for example `MIT`.
    pub fn template_label(&self, id: NodeId) -> Option<&str> {
        let l = self.tags[id as usize].label as usize;
        l.checked_sub(1)
            .and_then(|i| self.template_labels.get(i))
            .map(String::as_str)
    }
}

#[cfg(feature = "test-hooks")]
fn timer(name: &'static str) -> io::hooks::StageTimer {
    io::hooks::stage(name)
}

/// A stage timer that does nothing in builds without `test-hooks`.
#[cfg(not(feature = "test-hooks"))]
struct NoTimer;

#[cfg(not(feature = "test-hooks"))]
fn timer(_name: &'static str) -> NoTimer {
    NoTimer
}

/// What all roots of one run share: the detector with its Magika session,
/// and the template table.
pub struct Context {
    pub detector: detect::Detector,
    pub templates: chaff::templates::Templates,
}

impl Context {
    /// Starts to load the type cache on its own thread, so it loads while
    /// the walk runs (Plan A3). No cache with `--no-cache`, with
    /// `--detect off`, or without a cache directory. Fails when a
    /// `--templates` directory cannot be read.
    pub fn new(opts: &Opts, env: &Env) -> Result<Context, cli::UsageError> {
        let templates =
            chaff::templates::Templates::for_opts(&opts.templates, opts.templates_only)?;
        let mut detector = detect::Detector::new(opts.detect);
        if let Some(dir) = env.cache_dir.clone()
            && !opts.no_cache
            && opts.detect != detect::DetectMode::Off
        {
            let loading = std::thread::spawn(move || {
                let _t = timer("cache_load");
                cache::Cache::load(&dir, detect::magika::Model::version())
            });
            detector.set_cache(
                detect::PendingCache::Loading(loading),
                templates.fingerprint(),
                env.scan_time,
            );
        }
        Ok(Context {
            detector,
            templates,
        })
    }

    /// Saves the type cache, unless its directory is inside a scanned root:
    /// teak never writes into the tree it shows.
    pub fn finish(&mut self, roots: &[PathBuf]) {
        let Some(cache) = self.detector.take_cache() else {
            return;
        };
        let dir = std::path::absolute(cache.dir()).unwrap_or_else(|_| cache.dir().to_path_buf());
        let inside = roots.iter().any(|r| {
            let abs = std::path::absolute(r).unwrap_or_else(|_| r.clone());
            let real = std::fs::canonicalize(r).unwrap_or_else(|_| abs.clone());
            dir.starts_with(&abs) || dir.starts_with(&real)
        });
        if !inside {
            let _ = cache.save();
        }
    }
}

/// True when the content checks run: not with `--detect off` (no content
/// reads), and not with `TEAK_TEST_NO_CONTENT_CHECKS=1`.
fn checks_on(opts: &Opts) -> bool {
    #[cfg(feature = "test-hooks")]
    if io::hooks::content_checks_off() {
        return false;
    }
    opts.detect != detect::DetectMode::Off
}

/// Why a root gives no view.
#[derive(Debug)]
pub enum BuildError {
    /// The root cannot be read (exit code 1).
    Scan(ScanError),
    /// A bad `--focus` or `--why` path (exit code 2).
    Usage(cli::UsageError),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Scan(e) => e.fmt(f),
            BuildError::Usage(e) => e.fmt(f),
        }
    }
}

/// Scans one root and builds its view.
pub fn build(
    root: &Path,
    opts: &Opts,
    cx: &mut Context,
    budget: usize,
    scan_time: i64,
) -> Result<Scan, BuildError> {
    let rel = |p: &Option<PathBuf>, flag: &str| {
        p.as_deref()
            .map(|p| explain::resolve(root, p, flag).map(|r| (p.to_path_buf(), r)))
            .transpose()
            .map_err(BuildError::Usage)
    };
    let focus_rel = rel(&opts.focus, "--focus")?;
    let why_rel = rel(&opts.why, "--why")?;
    let threads = opts
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));
    let mut tree = {
        let _t = timer("walk");
        walk::scan(
            root,
            &walk::WalkOpts {
                follow: opts.follow,
                one_fs: opts.one_fs,
                threads,
            },
            scan_time,
        )
        .map_err(BuildError::Scan)?
    };
    let node = |r: &Option<(PathBuf, Vec<u8>)>, flag: &str| {
        r.as_ref()
            .map(|(p, rel)| explain::find(&tree, rel, p, flag))
            .transpose()
            .map_err(BuildError::Usage)
    };
    let focus = node(&focus_rel, "--focus")?;
    let why_id = node(&why_rel, "--why")?;
    let repo = if opts.no_git { None } else { git::open(root) };
    let classifier = chaff::Classifier::new(&tree, &opts.chaff, &cx.templates);
    let (mut tags, ignored) = {
        let _t = timer("chaff");
        classifier.classify_names_and_ignored(&tree, repo.is_some())
    };
    let mut aggs = {
        let _t = timer("aggregate");
        aggregate::compute(&tree, |id| chaff::is_hidden(&tags, id))
    };
    let mut labels: Vec<LabelId> = tree
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| detect::name_label(n, tree.name(i as NodeId)))
        .collect();
    let (git, git_head) = match &repo {
        Some(repo) => {
            let _t = timer("git");
            let status = repo.status(&tree, |id| ignored[id as usize]);
            (Some(status), repo.head())
        }
        None => (None, None),
    };
    if opts.mismatches {
        let found = {
            let _t = timer("detect");
            detect::mismatched(
                &mut tree,
                |id| chaff::is_hidden(&tags, id),
                &mut cx.detector,
                detect::Cap::DEFAULT,
            )
        };
        let files: Vec<NodeId> = found.iter().map(|(id, _)| *id).collect();
        let view = select::mismatch_view(&tree, &tags, &files, budget);
        let kinds = project_kinds(&tree, &labels, opts, git_head.is_some());
        return Ok(Scan {
            tree,
            tags,
            aggs,
            estimated: vec![false; labels.len()],
            labels,
            types: found.into_iter().collect(),
            view,
            template_labels: cx.templates.labels(),
            kinds,
            git,
            git_head,
            previews: HashMap::new(),
            why: None,
            dupes: HashMap::new(),
        });
    }
    let weights = salience::Weights::default();
    let mut why_terms = None;
    let mut view = {
        let _t = timer("select");
        let inputs = salience::ScoreInputs {
            tree: &tree,
            aggs: &aggs,
            tags: &tags,
            git: git.as_ref(),
            focus,
        };
        let scores = salience::score(&inputs, &weights);
        if let Some(id) = why_id {
            why_terms = Some(salience::terms(&inputs, id));
        }
        select::plan(
            &select::SelectInputs {
                tree: &tree,
                aggs: &aggs,
                tags: &tags,
                scores: &scores.score,
                labels: &labels,
            },
            budget,
            &select::SelectOpts {
                depth: opts.depth,
                series_min: opts.series_min,
                sort: opts.sort,
                focus,
            },
        )
    };
    let detected = {
        let _t = timer("detect");
        detect::run(
            &view,
            &mut tree,
            &tags,
            &mut labels,
            &mut cx.detector,
            detect::Cap::DEFAULT,
            checks_on(opts),
        )
    };
    if checks_on(opts) {
        let _t = timer("checks");
        let mut moved = Vec::new();
        for &(id, cached) in &detected.checks {
            let tag = tags[id as usize];
            let named = tag.class == Some(chaff::ChaffClass::Boilerplate);
            if tag.class.is_some() && !named {
                continue;
            }
            let check = match cached {
                Some(c) => c,
                None => {
                    let Ok(head) = io::read_head(&tree.path(id), chaff::content::READ_MAX) else {
                        continue;
                    };
                    let size = tree.node(id).size;
                    let c = if named {
                        cache::Check {
                            class: Some(chaff::ChaffClass::Boilerplate),
                            template: classifier.template_label(&head, size).unwrap_or(0),
                        }
                    } else {
                        let found = chaff::content::check(&head, size, &cx.templates);
                        cache::Check {
                            class: found.map(|(class, _)| class),
                            template: found.map_or(0, |(_, t)| t),
                        }
                    };
                    let key = cache::Key::of(tree.node(id));
                    if let Some(cache) = cx.detector.cache() {
                        cache.set_check(key, c);
                    }
                    c
                }
            };
            if named {
                tags[id as usize].label = check.template;
                continue;
            }
            if let Some(class) = check.class {
                let tag = classifier.content_tag(&tree, id, class, check.template);
                tags[id as usize] = tag;
                if matches!(tag.action, chaff::Action::Fold | chaff::Action::Hide) {
                    moved.push(id);
                }
            }
        }
        if moved.iter().any(|&id| chaff::is_hidden(&tags, id)) {
            aggs = aggregate::compute(&tree, |id| chaff::is_hidden(&tags, id));
        }
        select::apply_content(&mut view, &tree, &aggs, &tags, &moved);
    }
    let mut dupes = HashMap::new();
    if opts.dupes {
        let _t = timer("dupes");
        let visible: Vec<NodeId> = view
            .rows
            .iter()
            .filter_map(|r| match r.kind {
                view::RowKind::Entry { node, .. } if !tree.node(node).is_dir() => Some(node),
                _ => None,
            })
            .collect();
        dupes = dupes::find(&tree, &visible, |id| {
            matches!(
                tags[id as usize].action,
                chaff::Action::Fold | chaff::Action::Hide
            )
        });
        let mut copies: Vec<NodeId> = dupes.keys().copied().collect();
        copies.sort_unstable();
        let mut moved = Vec::new();
        for id in copies {
            let tag = classifier.content_tag(&tree, id, chaff::ChaffClass::Duplicate, 0);
            tags[id as usize] = tag;
            if matches!(tag.action, chaff::Action::Fold | chaff::Action::Hide) {
                moved.push(id);
            }
        }
        if moved.iter().any(|&id| chaff::is_hidden(&tags, id)) {
            aggs = aggregate::compute(&tree, |id| chaff::is_hidden(&tags, id));
        }
        select::apply_content(&mut view, &tree, &aggs, &tags, &moved);
    }
    let kinds = project_kinds(&tree, &labels, opts, git_head.is_some());
    let previews = {
        let _t = timer("preview");
        previews(&view, &tree, &detected, opts)
    };
    let mut scan = Scan {
        tree,
        tags,
        aggs,
        labels,
        estimated: detected.estimated,
        types: detected.types,
        view,
        template_labels: cx.templates.labels(),
        kinds,
        git,
        git_head,
        previews,
        why: None,
        dupes,
    };
    if let (Some(id), Some(terms)) = (why_id, why_terms) {
        let rule = classifier.why(&scan.tree, id);
        scan.why = Some(explain::why(&scan, id, &rule, &terms, &weights));
    }
    Ok(scan)
}

/// The previews of the file rows. A file typed from the cache has no head,
/// so the preview reads one (counted as a preview read). No previews with
/// `--preview off` or `--detect off` (no content reads).
fn previews(
    view: &View,
    tree: &Tree,
    detected: &detect::Detected,
    opts: &Opts,
) -> HashMap<NodeId, String> {
    let mut out = HashMap::new();
    if opts.preview == layout::PreviewMode::Off || opts.detect == detect::DetectMode::Off {
        return out;
    }
    for row in &view.rows {
        let view::RowKind::Entry { node, .. } = row.kind else {
            continue;
        };
        let Some(info) = detected.types.get(&node) else {
            continue;
        };
        let read;
        let head = match detected.heads.get(&node) {
            Some(h) => h.as_slice(),
            None if detected.cache_hits.contains(&node) => {
                let Ok(h) = io::read_preview(&tree.path(node), 0, detect::HEAD_MAX) else {
                    continue;
                };
                read = h;
                read.as_slice()
            }
            None => continue,
        };
        if let Some(p) = preview::of(tree, node, info, head) {
            out.insert(node, p);
        }
    }
    out
}

/// The project kinds of the root. The EXIF check reads image heads, so it
/// needs content reads (not with `--detect off`). `git repo` shows only
/// when teak could not read the git state: with it, the header shows the
/// branch; with `--no-git`, the header has no git data (FR-57).
fn project_kinds(
    tree: &Tree,
    labels: &[LabelId],
    opts: &Opts,
    git_shown: bool,
) -> Vec<&'static str> {
    let reads = opts.detect != detect::DetectMode::Off;
    let mut kinds = kind::detect(tree, tree::ROOT, labels, |id| {
        let n = tree.node(id);
        reads
            && n.kind == tree::Kind::File
            && !n.has(flag::PSEUDO_FS)
            && io::read_head(&tree.path(id), 4096).is_ok_and(|h| kind::head_has_exif(&h))
    });
    if git_shown || opts.no_git {
        kinds.retain(|k| *k != "git repo");
    }
    kinds
}

/// The layout options for a run.
pub fn layout_opts(opts: &Opts, set: &Settings) -> LayoutOpts {
    LayoutOpts {
        width: set.width,
        glyphs: set.glyphs,
        units: opts.units,
        time: opts.time,
        preview: opts.preview,
        long: opts.long,
    }
}

/// Runs the whole pipeline for each root in order and returns the exit
/// code: 0, or 1 when a root is missing or an entry had no access.
pub fn run(opts: &Opts, env: &Env, out: &mut impl Write, err: &mut impl Write) -> u8 {
    let set = settings(opts, env);
    let style = Style::new(set.color, env.ls_colors.as_deref());
    let lopts = layout_opts(opts, &set);
    if let Some(dir) = &opts.dump_templates {
        return match chaff::templates::Templates::dump(dir) {
            Ok(()) => 0,
            Err(e) => {
                let _ = writeln!(err, "teak: {e}");
                2
            }
        };
    }
    let mut cx = match Context::new(opts, env) {
        Ok(cx) => cx,
        Err(e) => {
            let _ = writeln!(err, "teak: {e}");
            return 2;
        }
    };
    let mut code = 0;
    // Several roots in `json` form one array of documents.
    let array = set.format == Format::Json && opts.paths.len() > 1 && opts.why.is_none();
    let mut docs = 0;
    for path in &opts.paths {
        match build(path, opts, &mut cx, set.budget, env.scan_time) {
            Ok(scan) => {
                for e in &scan.tree.errors {
                    let _ = writeln!(err, "teak: {e}");
                }
                if !scan.tree.errors.is_empty()
                    || scan.tree.nodes.iter().any(|n| n.has(flag::NO_ACCESS))
                {
                    code = 1;
                }
                let _t = timer("layout");
                let written = if scan.why.is_none() && set.format == Format::Md {
                    if opts.stats {
                        render::md::write_stats(&scan, &lopts, out)
                    } else {
                        render::md::write(&scan, &lopts, out)
                    }
                } else if scan.why.is_none() && set.format == Format::Json {
                    let doc = render::json::document(&scan, &lopts, opts.json_full, opts.stats);
                    let sep = match (array, docs) {
                        (false, _) => "",
                        (true, 0) => "[\n",
                        (true, _) => ",\n",
                    };
                    docs += 1;
                    let end = if array { "" } else { "\n" };
                    out.write_all(format!("{sep}{doc}{end}").as_bytes())
                } else {
                    let lines = if let Some(why) = &scan.why {
                        why.iter()
                            .map(|l| layout::Line {
                                spans: vec![layout::Span {
                                    text: l.clone(),
                                    paint: layout::Paint::Plain,
                                }],
                                dim: false,
                            })
                            .collect()
                    } else if opts.stats {
                        layout::stats(&scan, &lopts)
                    } else {
                        layout::fit(&scan, &lopts)
                    };
                    render::write(&lines, &scan.tree, &style, out)
                };
                drop(_t);
                if written.is_err() {
                    // A closed pipe (for example `teak | head`): stop quietly.
                    cx.finish(&opts.paths);
                    return code;
                }
            }
            Err(BuildError::Usage(e)) => {
                let _ = writeln!(err, "teak: {e}");
                cx.finish(&opts.paths);
                return 2;
            }
            Err(BuildError::Scan(e)) => {
                let _ = writeln!(err, "teak: {e}");
                code = 1;
            }
        }
    }
    if array {
        let _ = out.write_all(if docs == 0 { b"[]\n" } else { b"\n]\n" });
    }
    let _ = out.flush();
    cx.finish(&opts.paths);
    code
}
