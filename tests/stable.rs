//! Stable output (FR-65): the same tree and scan time give byte-identical
//! `plain`, `md`, and `json`, for any thread count.

mod support;

use support::{Fixture, gen_tree, run_in};

#[test]
fn fr65_four_runs_give_the_same_bytes_in_each_format() {
    let fx = Fixture::new();
    gen_tree::generate(&gen_tree::Opts {
        files: 3000,
        seed: 11,
        out: fx.path().join("t"),
        ..gen_tree::Opts::default()
    })
    .unwrap();
    for format in [
        &["--format", "plain"][..],
        &["--format", "md"],
        &["--format", "json"],
        &["--json-full"],
    ] {
        let outs: Vec<String> = ["1", "1", "8", "8"]
            .into_iter()
            .map(|j| {
                // Fast detection: Magika in a debug build on a slow CI
                // machine can reach the 2 s detection cap (FR-51), a
                // wall-clock limit, and then the output depends on timing.
                let mut args = vec!["t", "-n", "60", "-j", j, "--detect", "fast"];
                args.extend(format);
                let out = run_in(fx.path(), &args);
                assert_eq!(out.code, Some(0), "{out:?}");
                out.stdout
            })
            .collect();
        assert!(outs[0].lines().count() > 10, "{format:?}: {}", outs[0]);
        for (i, o) in outs.iter().enumerate().skip(1) {
            assert!(o == &outs[0], "{format:?}: run {i} differs");
        }
    }
}
