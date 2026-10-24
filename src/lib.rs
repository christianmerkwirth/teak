//! teak: fit a large directory tree into a fixed line budget.
//!
//! [`run`] drives the pipeline for each root: walk → chaff tags → totals →
//! salience → selection → detection on the visible rows → layout → render.

pub mod aggregate;
pub mod cache;
pub mod chaff;
pub mod cli;
pub mod detect;
pub mod io;
pub mod layout;
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
use std::path::Path;

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
        }
    }

    /// A fixed environment for library callers: a pipe, no colors.
    pub fn fixed(scan_time: i64) -> Env {
        Env {
            stdout_tty: false,
            term: None,
            scan_time,
            no_color: false,
            clicolor_force: false,
            ls_colors: None,
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
    // --color wins, then CLICOLOR_FORCE, then NO_COLOR, then the format.
    let color = match opts.color {
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
    /// Fails when a `--templates` directory cannot be read.
    pub fn new(opts: &Opts) -> Result<Context, cli::UsageError> {
        Ok(Context {
            detector: detect::Detector::new(opts.detect),
            templates: chaff::templates::Templates::for_opts(&opts.templates, opts.templates_only)?,
        })
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

/// Scans one root and builds its view.
pub fn build(
    root: &Path,
    opts: &Opts,
    cx: &mut Context,
    budget: usize,
    scan_time: i64,
) -> Result<Scan, ScanError> {
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
        )?
    };
    let classifier = chaff::Classifier::new(&tree, &opts.chaff, &cx.templates);
    let mut tags = {
        let _t = timer("chaff");
        classifier.classify_names(&tree)
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
        return Ok(Scan {
            tree,
            tags,
            aggs,
            estimated: vec![false; labels.len()],
            labels,
            types: found.into_iter().collect(),
            view,
            template_labels: cx.templates.labels(),
        });
    }
    let mut view = {
        let _t = timer("select");
        let scores = salience::score(
            &salience::ScoreInputs {
                tree: &tree,
                aggs: &aggs,
                tags: &tags,
            },
            &salience::Weights::default(),
        );
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
        )
    };
    if checks_on(opts) {
        let _t = timer("checks");
        let mut moved = Vec::new();
        for &id in &detected.read_text {
            let tag = tags[id as usize];
            let named = tag.class == Some(chaff::ChaffClass::Boilerplate);
            if tag.class.is_some() && !named {
                continue;
            }
            let Ok(head) = io::read_head(&tree.path(id), chaff::content::READ_MAX) else {
                continue;
            };
            let size = tree.node(id).size;
            if named {
                if let Some(label) = classifier.template_label(&head, size) {
                    tags[id as usize].label = label;
                }
                continue;
            }
            if let Some(tag) = classifier.classify_content(&tree, id, &head, size) {
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
    Ok(Scan {
        tree,
        tags,
        aggs,
        labels,
        estimated: detected.estimated,
        types: detected.types,
        view,
        template_labels: cx.templates.labels(),
    })
}

/// The layout options for a run.
pub fn layout_opts(opts: &Opts, set: &Settings) -> LayoutOpts {
    LayoutOpts {
        width: set.width,
        glyphs: set.glyphs,
        units: opts.units,
        time: opts.time,
        preview: opts.preview,
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
    let mut cx = match Context::new(opts) {
        Ok(cx) => cx,
        Err(e) => {
            let _ = writeln!(err, "teak: {e}");
            return 2;
        }
    };
    let mut code = 0;
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
                let lines = {
                    let _t = timer("layout");
                    layout::fit(&scan, &lopts)
                };
                if render::write(&lines, &scan.tree, &style, out).is_err() {
                    // A closed pipe (for example `teak | head`): stop quietly.
                    return code;
                }
            }
            Err(e) => {
                let _ = writeln!(err, "teak: {e}");
                code = 1;
            }
        }
    }
    let _ = out.flush();
    code
}
