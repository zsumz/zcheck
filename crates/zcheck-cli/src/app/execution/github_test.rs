//! Focused encoding and bounded-context tests for GitHub reporting.

use std::fs;

use super::{
    MAX_ANNOTATION_BYTES, MAX_ANNOTATION_TITLE_BYTES, bounded, command, escape_data,
    escape_property, read_tail, safe_lines,
};

#[test]
fn workflow_command_fields_follow_toolkit_escaping() {
    assert_eq!(escape_data("100%\r\nnext"), "100%25%0D%0Anext");
    assert_eq!(escape_property("task:a,b%"), "task%3Aa%2Cb%25");
}

#[test]
fn annotations_are_utf8_safe_and_strictly_bounded() {
    let source = "🦀".repeat(MAX_ANNOTATION_BYTES);
    let result = bounded(&source, MAX_ANNOTATION_BYTES);
    assert!(result.len() <= MAX_ANNOTATION_BYTES);
    assert!(result.ends_with("[truncated by zcheck]"));
    let encoded = command(
        "error",
        Some(&"title".repeat(1024)),
        &"%\r\n🦀".repeat(4096),
    );
    assert!(!encoded.contains('\r') && !encoded.contains('\n'));
    assert!(encoded.len() <= MAX_ANNOTATION_BYTES + MAX_ANNOTATION_TITLE_BYTES + 32);
}

#[test]
fn grouped_task_data_cannot_start_a_workflow_command() {
    let mut lines = Vec::new();
    safe_lines("path", "one\n::error::injected\rnext\0", &mut lines);
    assert_eq!(lines, ["| path: one", "| ::error::injected�next�"]);
    assert!(lines.iter().all(|line| line.starts_with("| ")));
}

#[test]
fn failure_context_reads_only_a_bounded_complete_line_tail() {
    let directory = std::env::temp_dir().join(format!("zcheck-github-tail-{}", std::process::id()));
    assert!(fs::create_dir_all(&directory).is_ok());
    let path = directory.join("task.log");
    let source = format!("partial{}\nlast-one\nlast-two\n", "x".repeat(20 * 1024));
    assert!(fs::write(&path, source).is_ok());
    let tail = read_tail(&path);
    assert!(
        tail.as_deref()
            .is_ok_and(|value| value == "last-one\nlast-two")
    );
    assert!(fs::remove_dir_all(directory).is_ok());
}
