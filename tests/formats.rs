//! The agent formats: Markdown (FR-63) and JSON (FR-64).

mod support;

use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde_json::Value;

use support::{Fixture, run, run_in};

/// A tree with every row kind: open and closed directories, an overflow,
/// a series, and a chaff line.
fn mixed_fixture() -> Fixture {
    let fx = Fixture::new();
    fx.file("Cargo.toml", "[package]\nname = \"x\"\n")
        .file("Cargo.lock", "# lock\n")
        .file("README.md", "# Mixed\n")
        .file("src/main.rs", "fn main() {}\n")
        .file("src/lib.rs", "pub fn f() {}\n")
        .file("node_modules/left-pad/index.js", "module.exports = 1;\n")
        .file(".git/HEAD", "ref: refs/heads/main\n");
    for i in 0..30 {
        fx.sized(&format!("data/{}.csv", support::word(i)), 2000 + i as u64);
    }
    // At the root: a uniform `img/` would fold the series into its summary.
    for i in 1..=12 {
        fx.sized(&format!("IMG_{i:04}.png"), 5000);
    }
    fx.mtime_all(3600);
    fx
}

/// The root label of a fixture run in its own directory.
fn root_name(fx: &Fixture) -> String {
    fx.path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// What a CommonMark parser sees in a Markdown document.
#[derive(Debug, Default)]
struct Md {
    /// Fenced code blocks: (info string, text).
    blocks: Vec<(String, String)>,
    /// The cells of each table row (the head first).
    tables: Vec<Vec<Vec<String>>>,
    /// Paragraph text, with code spans in backticks.
    paragraphs: Vec<String>,
    /// Headings, as text.
    headings: Vec<String>,
}

fn parse_md(text: &str) -> Md {
    let mut md = Md::default();
    let mut block: Option<(String, String)> = None;
    let mut cell: Option<String> = None;
    let mut para: Option<String> = None;
    let mut heading: Option<String> = None;
    for event in Parser::new_ext(text, Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let info = match kind {
                    CodeBlockKind::Fenced(info) => info.to_string(),
                    CodeBlockKind::Indented => "(indented)".to_string(),
                };
                block = Some((info, String::new()));
            }
            Event::End(TagEnd::CodeBlock) => md.blocks.push(block.take().unwrap()),
            Event::Start(Tag::Table(_)) => md.tables.push(Vec::new()),
            Event::Start(Tag::TableHead | Tag::TableRow) => {
                md.tables.last_mut().unwrap().push(Vec::new())
            }
            Event::Start(Tag::TableCell) => cell = Some(String::new()),
            Event::End(TagEnd::TableCell) => md
                .tables
                .last_mut()
                .unwrap()
                .last_mut()
                .unwrap()
                .push(cell.take().unwrap()),
            Event::Start(Tag::Paragraph) => para = Some(String::new()),
            Event::End(TagEnd::Paragraph) => md.paragraphs.push(para.take().unwrap()),
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => md.headings.push(heading.take().unwrap()),
            Event::Text(t) => {
                if let Some((_, s)) = &mut block {
                    s.push_str(&t);
                }
                for s in [&mut cell, &mut para, &mut heading].into_iter().flatten() {
                    s.push_str(&t);
                }
            }
            Event::Code(t) => {
                for s in [&mut cell, &mut para, &mut heading].into_iter().flatten() {
                    s.push('`');
                    s.push_str(&t);
                    s.push('`');
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                for s in [&mut para, &mut heading].into_iter().flatten() {
                    s.push('\n');
                }
            }
            _ => {}
        }
    }
    assert!(block.is_none(), "an unclosed code block");
    md
}

/// The fenced `text` block of a Markdown output.
fn tree_block(md: &Md) -> &str {
    let blocks: Vec<_> = md.blocks.iter().filter(|(i, _)| i == "text").collect();
    assert_eq!(blocks.len(), 1, "{md:?}");
    &blocks[0].1
}

// FR-63: Markdown

#[test]
fn fr63_markdown_has_heading_tree_table_and_folded_line() {
    let fx = mixed_fixture();
    let out = run(&fx, &["--format", "md", "-n", "30"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    // The heading names the root, its size, and its file count, as the
    // plain header does.
    let plain = run(&fx, &["-n", "30"]);
    let head = plain.header();
    let size = support::cells(head)[1].split(" · ").next().unwrap();
    let files = head.split(" · ").nth(1).unwrap();
    let want = format!("# {}/ — {size}, {files}", root_name(&fx));
    assert_eq!(out.lines()[0], want, "{}", out.stdout);
    // Line 2: the kind and the scan time (UTC).
    let facts = out.lines()[1];
    assert!(facts.contains("rust crate"), "{facts}");
    assert!(facts.contains("scanned 2025-06-15 15:06"), "{facts}");

    let md = parse_md(&out.stdout);
    assert_eq!(md.headings.len(), 1, "{md:?}");
    // The tree: the plain rows in ASCII, without the header and footer.
    let ascii = run(&fx, &["-n", "30", "--ascii"]);
    let rows = &ascii.lines()[1..ascii.lines().len() - 1];
    let block = tree_block(&md);
    assert_eq!(block.lines().collect::<Vec<_>>(), rows, "{}", out.stdout);
    assert!(block.contains("|-- ") || block.contains("`-- "), "{block}");
    assert!(!block.contains('├') && !block.contains('…'), "{block}");
    // The type table.
    assert_eq!(md.tables.len(), 1, "{}", out.stdout);
    assert_eq!(md.tables[0][0], ["Group", "Files", "Size"]);
    let groups: Vec<&str> = md.tables[0][1..].iter().map(|r| r[0].as_str()).collect();
    for g in ["code", "data", "image"] {
        assert!(groups.contains(&g), "{g} in {groups:?}");
    }
    // The folded line, after the table.
    let folded = md.paragraphs.last().expect("a folded line");
    assert!(folded.starts_with("Folded: "), "{folded}");
    assert!(folded.contains("vendor 1"), "{folded}");
    assert!(folded.contains("lock 1"), "{folded}");
    assert!(folded.ends_with("Run `teak -a` to show all."), "{folded}");
}

#[test]
fn fr63_markdown_never_has_ansi_codes() {
    let fx = mixed_fixture();
    let out = run(&fx, &["--format", "md", "--color", "always"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    assert!(!out.stdout.contains('\x1b'), "{}", out.stdout);
}

#[test]
fn fr63_the_budget_counts_the_lines_in_the_block() {
    let fx = mixed_fixture();
    for n in [3, 8, 12, 30] {
        let n_text = n.to_string();
        let out = run(&fx, &["--format", "md", "-n", &n_text]);
        let md = parse_md(&out.stdout);
        let lines = tree_block(&md).lines().count();
        assert!(lines <= n, "{lines} lines for -n {n}:\n{}", out.stdout);
    }
}

#[test]
fn fr63_backticks_in_a_name_get_a_longer_fence() {
    let fx = Fixture::new();
    fx.file("a```b.txt", "x\n")
        .file("c````d.txt", "y\n")
        .file("main.rs", "fn main() {}\n");
    let out = run(&fx, &["--format", "md"]);
    assert_eq!(out.code, Some(0), "{out:?}");
    let fence = out
        .lines()
        .into_iter()
        .find(|l| l.starts_with("```"))
        .expect("a fence");
    assert_eq!(fence, "`````text", "{}", out.stdout);
    let md = parse_md(&out.stdout);
    let block = tree_block(&md);
    assert!(block.contains("a```b.txt"), "{block}");
    assert!(block.contains("c````d.txt"), "{block}");
    // The block is closed: the table and the folded line come after it.
    assert_eq!(md.tables.len(), 1, "{}", out.stdout);
    assert!(md.paragraphs.last().unwrap().starts_with("Folded: "));
}

#[test]
fn fr63_markdown_in_a_name_does_not_change_the_heading() {
    let fx = Fixture::new();
    fs::create_dir(fx.path().join("*a_b*")).unwrap();
    fx.file("*a_b*/x.txt", "x\n");
    let out = run_in(fx.path(), &["--format", "md", "*a_b*"]);
    let md = parse_md(&out.stdout);
    assert!(md.headings[0].starts_with("*a_b*/ — "), "{}", out.stdout);
}

#[test]
fn fr63_no_folded_classes_says_none() {
    let fx = Fixture::new();
    fx.file("main.rs", "fn main() {}\n");
    let out = run(&fx, &["--format", "md"]);
    let md = parse_md(&out.stdout);
    assert_eq!(
        md.paragraphs.last().unwrap(),
        "Folded: none. Run `teak -a` to show all."
    );
}

// FR-64: JSON

fn json(out: &support::Run) -> Value {
    assert_eq!(out.code, Some(0), "{out:?}");
    serde_json::from_str(&out.stdout).unwrap_or_else(|e| panic!("{e}:\n{}", out.stdout))
}

fn schema() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/teak.schema.json");
    let text = fs::read_to_string(&path).expect("schema/teak.schema.json");
    serde_json::from_str(&text).expect("the schema is JSON")
}

fn assert_valid(doc: &Value) {
    let validator = jsonschema::validator_for(&schema()).expect("a valid schema");
    let errors: Vec<String> = validator
        .iter_errors(doc)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

fn rows(doc: &Value) -> &Vec<Value> {
    doc["rows"].as_array().expect("rows")
}

/// The row whose path is `path`.
fn row<'a>(doc: &'a Value, path: &str) -> &'a Value {
    rows(doc)
        .iter()
        .find(|r| r["path"] == path)
        .unwrap_or_else(|| panic!("no row {path} in {doc:#}"))
}

#[test]
fn fr64_json_is_valid_and_versioned() {
    let fx = mixed_fixture();
    let doc = json(&run(&fx, &["--format", "json", "-n", "30"]));
    assert_eq!(doc["schema_version"], 1);
    assert_valid(&doc);
    assert_eq!(doc["root"], format!("{}/", root_name(&fx)));
    assert_eq!(doc["scan_time"], support::SCAN_TIME);
    assert!(
        doc.get("entries").is_none(),
        "no entries without --json-full"
    );
    let keys: Vec<&str> = doc
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "schema_version",
            "root",
            "scan_time",
            "header",
            "rows",
            "footer",
            "types"
        ]
    );
}

#[test]
fn fr64_json_holds_the_visible_rows_only() {
    let fx = mixed_fixture();
    for n in ["8", "12", "30"] {
        let plain = run(&fx, &["-n", n]);
        let doc = json(&run(&fx, &["--format", "json", "-n", n]));
        // One row object per plain row, without the header and the footer.
        assert_eq!(rows(&doc).len(), plain.lines().len() - 2, "-n {n}: {doc:#}");
        assert_eq!(doc["footer"]["budget"], n.parse::<u64>().unwrap());
    }
}

#[test]
fn fr64_json_rows_carry_kind_class_and_type() {
    let fx = mixed_fixture();
    let doc = json(&run(&fx, &["--format", "json", "-n", "30"]));
    let main = row(&doc, "src/main.rs");
    assert_eq!(main["kind"], "entry");
    assert_eq!(main["depth"], 2);
    assert_eq!(main["type"], "rust");
    assert_eq!(main["files"], 1);
    assert_eq!(main["preview"], "fn main() {}");
    let src = row(&doc, "src/");
    assert!(src["flags"].as_array().unwrap().contains(&"open".into()));
    assert_eq!(src["type_mix"][0]["type"], "rust");
    assert_eq!(src["type_mix"][0]["files"], 2);
    let series = rows(&doc)
        .iter()
        .find(|r| r["kind"] == "series")
        .expect("a series row");
    assert_eq!(series["files"], 12);
    assert_eq!(series["size"], 12 * 5000);
    assert_eq!(series["type"], "png");
    let chaff = rows(&doc)
        .iter()
        .find(|r| r["kind"] == "chaff" && r["depth"] == 1)
        .expect("a root chaff row");
    let items = chaff["items"].as_array().unwrap();
    let classes: Vec<(&str, &str)> = items
        .iter()
        .map(|i| (i["path"].as_str().unwrap(), i["class"].as_str().unwrap()))
        .collect();
    assert!(
        classes.contains(&("node_modules/", "vendor")),
        "{classes:?}"
    );
    assert!(classes.contains(&("Cargo.lock", "lock")), "{classes:?}");
    let hidden = doc["footer"]["hidden"].as_array().unwrap();
    assert!(
        hidden
            .iter()
            .any(|h| h["class"] == "vcs" && h["files"] == 1),
        "{hidden:?}"
    );
}

#[test]
fn fr64_header_and_types_match_the_tree() {
    let fx = mixed_fixture();
    let doc = json(&run(&fx, &["--format", "json"]));
    let header = &doc["header"];
    // 7 small files, 30 csv, 12 png.
    assert_eq!(header["files"], 49);
    // The fake `.git/` cannot be read, so it shows as a kind (FR-57).
    assert_eq!(
        header["kinds"],
        serde_json::json!(["rust crate", "git repo"])
    );
    let types = doc["types"].as_array().unwrap();
    let image = types.iter().find(|t| t["group"] == "image").unwrap();
    assert_eq!(image["files"], 12);
    assert_eq!(image["size"], 60_000);
}

#[test]
fn fr64_non_utf8_names_use_the_text_escape() {
    let fx = Fixture::new();
    let name = std::ffi::OsStr::from_bytes(b"b\xFFc.txt");
    fs::write(fx.path().join(name), "x\n").unwrap();
    fx.file("a.txt", "y\n");
    let out = support::Run::from_output(
        support::command(fx.path())
            .args(support::DEFAULT_FLAGS)
            .args(["--format", "json"])
            .output()
            .unwrap(),
    );
    let doc = json(&out);
    row(&doc, "b\\xFFc.txt");
    assert_valid(&doc);
}

/// Non-directory entries below `root`, without following symlinks.
fn count_files(root: &Path) -> u64 {
    let mut n = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if fs::symlink_metadata(&p).unwrap().is_dir() {
                stack.push(p);
            } else {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn fr64_json_full_has_every_entry() {
    let fx = mixed_fixture();
    fx.symlink("link", "README.md").mode("src/main.rs", 0o644);
    let out = run(&fx, &["--json-full", "-n", "12"]);
    let doc = json(&out);
    assert_valid(&doc);
    let entries = doc["entries"].as_array().expect("entries");
    let files = entries.iter().filter(|e| e["kind"] != "dir").count() as u64;
    assert_eq!(files, count_files(fx.path()));
    // Every entry once, in tree order; the root comes first.
    assert_eq!(entries[0]["path"], "./");
    let mut paths: Vec<&str> = entries
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    let n = paths.len();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(paths.len(), n);
    // `visible_in` points at the row that stands for the entry; hidden
    // entries have none.
    let rows = rows(&doc);
    let head = entries.iter().find(|e| e["path"] == ".git/HEAD").unwrap();
    assert!(head["visible_in"].is_null(), "{head}");
    let main = entries.iter().find(|e| e["path"] == "src/main.rs").unwrap();
    let i = main["visible_in"].as_u64().expect("main.rs is in a row") as usize;
    assert!(
        rows[i]["path"].as_str().unwrap().starts_with("src"),
        "{}",
        rows[i]
    );
    let link = entries.iter().find(|e| e["path"] == "link").unwrap();
    assert_eq!(link["kind"], "symlink");
    assert_eq!(main["mode"], "-rw-r--r--");
}

#[test]
fn fr64_several_roots_give_an_array() {
    let fx = Fixture::new();
    fx.file("a/x.txt", "x\n").file("b/y.txt", "y\n");
    let out = run(&fx, &["--format", "json", "a", "b"]);
    let doc = json(&out);
    let roots = doc.as_array().expect("an array");
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0]["root"], "a/");
    assert_eq!(roots[1]["root"], "b/");
    for r in roots {
        assert_valid(r);
    }
}

#[test]
fn fr64_stats_in_json_has_no_rows() {
    let fx = mixed_fixture();
    let doc = json(&run(&fx, &["--format", "json", "--stats"]));
    assert!(rows(&doc).is_empty(), "{doc:#}");
    assert!(!doc["types"].as_array().unwrap().is_empty());
    assert_valid(&doc);
}

#[cfg(feature = "magika")]
#[test]
fn fr64_mismatches_in_json() {
    let fx = Fixture::new();
    fx.file("data/report.pdf", support::corpus("report.pdf"))
        .file("src/main.rs", "fn main() {}\n");
    let doc = json(&run(&fx, &["--format", "json", "--mismatches"]));
    assert_valid(&doc);
    assert_eq!(rows(&doc).len(), 1, "{doc:#}");
    let r = row(&doc, "data/report.pdf");
    assert_eq!(r["kind"], "mismatch");
    assert_eq!(r["type"], "html");
    assert_eq!(r["mismatch"], "pdf");
}
