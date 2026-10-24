#!/usr/bin/env bash
# Hyperfine runs for FR-71 to FR-75.
#
# usage: scripts/bench.sh <case>
#   10k        FR-71  teak on 10k files, warm teak cache
#   100k       FR-72  teak --detect full --no-cache on 100k files
#   1m         FR-73  teak --no-cache on 1M files, plus peak RSS
#   magika100  FR-74  detection stage on about 100 visible files (test-hooks build)
#   warmcache  FR-75  teak --detect full on 100k files, warm teak cache (test-hooks build)
#   all        every case above
#
# Each case runs hyperfine with 3 warmup runs and 10 runs, and writes JSON to
# target/bench/<case>.json. The fixture trees come from examples/gen_tree.rs
# with seed 42 and live in target/bench-data/ (shared with benches/walk.rs).
#
# BENCH_IGNORE_FAILURE=1 lets hyperfine time a teak that exits non-zero (the
# phase-0 stub exits 2 for every scan).
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
data="$root/target/bench-data"
out="$root/target/bench"
seed=42
hf=(hyperfine --warmup 3 --runs 10 --style basic)
if [[ "${BENCH_IGNORE_FAILURE:-0}" == 1 ]]; then
    hf+=(--ignore-failure)
fi

die() { echo "bench.sh: $*" >&2; exit 2; }
command -v hyperfine >/dev/null || die "hyperfine is not installed (cargo install hyperfine)"

# Keep the teak type cache out of the user's real cache directory.
export XDG_CACHE_HOME="$out/cache"
mkdir -p "$out" "$data" "$XDG_CACHE_HOME"

teak_bin() {
    cargo build --release --quiet --manifest-path "$root/Cargo.toml"
    echo "$root/target/release/teak"
}

# A separate target dir, so the test-hooks binary does not replace the release one.
teak_hooks_bin() {
    cargo build --release --quiet --manifest-path "$root/Cargo.toml" --features test-hooks --target-dir "$root/target/hooks"
    echo "$root/target/hooks/release/teak"
}

# tree <dir-name> <files>: prints the path of the generated tree.
tree() {
    local dir="$data/$1"
    if [[ ! -e "$dir.ok" ]]; then
        rm -rf "$dir"
        cargo run --release --quiet --manifest-path "$root/Cargo.toml" --example gen_tree -- \
            --files "$2" --seed "$seed" --out "$dir" >&2
        touch "$dir.ok"
    fi
    echo "$dir"
}

# report_field <report.json> detect|reads: the detect stage ms, or the sum of content reads.
report_field() {
    python3 - "$1" "$2" <<'EOF'
import json, sys
try:
    r = json.load(open(sys.argv[1]))
except (OSError, ValueError):
    print("n/a (no report)")
    sys.exit()
print(r["stages_ms"].get("detect", "n/a") if sys.argv[2] == "detect" else sum(r["reads"].values()))
EOF
}

peak_rss_mb() {
    local log="$out/rss.log"
    if [[ "$(uname)" == Darwin ]]; then
        /usr/bin/time -l "$@" >/dev/null 2>"$log" || true
        awk '/maximum resident set size/ { printf "%.1f\n", $1 / 1048576 }' "$log"
    else
        /usr/bin/time -v "$@" >/dev/null 2>"$log" || true
        awk -F: '/Maximum resident set size/ { printf "%.1f\n", $2 / 1024 }' "$log"
    fi
}

run_case() {
    local case=$1 bin dir rep
    case "$case" in
    10k)
        bin=$(teak_bin); dir=$(tree files-10000-seed-$seed 10000)
        "${hf[@]}" --export-json "$out/10k.json" "$bin $dir"
        ;;
    100k)
        bin=$(teak_bin); dir=$(tree files-100000-seed-$seed 100000)
        "${hf[@]}" --export-json "$out/100k.json" "$bin --detect full --no-cache $dir"
        ;;
    1m)
        bin=$(teak_bin); dir=$(tree files-1000000-seed-$seed 1000000)
        "${hf[@]}" --export-json "$out/1m.json" "$bin --no-cache $dir"
        echo "peak RSS: $(peak_rss_mb "$bin" --no-cache "$dir") MB" | tee "$out/1m-rss.txt"
        ;;
    magika100)
        bin=$(teak_hooks_bin); dir=$(tree magika-100-seed-$seed 100)
        "${hf[@]}" --export-json "$out/magika100.json" "$bin --detect full --no-cache $dir"
        rep="$out/magika100-report.json"; rm -f "$rep"
        TEAK_TEST_REPORT="$rep" "$bin" --detect full --no-cache "$dir" >/dev/null || true
        echo "detect stage: $(report_field "$rep" detect) ms" | tee "$out/magika100-stage.txt"
        ;;
    warmcache)
        bin=$(teak_hooks_bin); dir=$(tree files-100000-seed-$seed 100000)
        "${hf[@]}" --export-json "$out/warmcache.json" "$bin --detect full $dir"
        rep="$out/warmcache-report.json"; rm -f "$rep"
        TEAK_TEST_REPORT="$rep" "$bin" --detect full "$dir" >/dev/null || true
        { echo "detect stage: $(report_field "$rep" detect) ms"
          echo "content reads: $(report_field "$rep" reads) bytes"; } | tee "$out/warmcache-stage.txt"
        ;;
    *) die "unknown case '$case' (10k, 100k, 1m, magika100, warmcache, all)" ;;
    esac
}

[[ $# -eq 1 ]] || die "usage: scripts/bench.sh <10k|100k|1m|magika100|warmcache|all>"
if [[ "$1" == all ]]; then
    for c in 10k 100k 1m magika100 warmcache; do run_case "$c"; done
else
    run_case "$1"
fi
