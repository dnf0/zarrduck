mod common;
use common::*;

/// Strip ANSI escape sequences so heatmap snapshots are readable text.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Consume everything up to and including the final byte of the CSI sequence.
            // CSI sequences end with a byte in 0x40–0x7E ('m' for SGR).
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                for inner in chars.by_ref() {
                    if inner.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn clean(s: &str) -> String {
    let stripped = strip_ansi(s);
    stripped
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_plot(plot_type: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let db = make_plot_db(&dir);
    let assert = eider(&dir)
        .arg("plot")
        .arg(&db)
        .args(["--plot-type", plot_type])
        .args(["--value", "air_temperature"])
        .assert()
        .success();
    clean(&String::from_utf8(assert.get_output().stdout.clone()).unwrap())
}

#[test]
fn plot_hist_snapshot() {
    insta::assert_snapshot!(run_plot("hist"));
}

#[test]
fn plot_line_snapshot() {
    insta::assert_snapshot!(run_plot("line"));
}

#[test]
fn plot_heatmap_snapshot() {
    insta::assert_snapshot!(run_plot("heatmap"));
}

#[test]
fn plot_bad_table_errors() {
    let dir = tempfile::tempdir().unwrap();
    let db = make_plot_db(&dir);
    eider(&dir)
        .arg("plot")
        .arg(&db)
        .args(["--plot-type", "hist", "--table", "nonexistent_table"])
        .assert()
        .failure();
}
