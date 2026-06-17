use neo_rainst::char_source::{BuiltinChars, CharSource, ClaudeCharSource};
use neo_rainst::transcript::{extract_claude_jsonl_entry, flatten_cwd};

#[test]
fn test_katakana_charset() {
    let src = BuiltinChars::from_charset_name("katakana");
    assert!(!src.chars().is_empty());
    assert!(src.chars().contains(&'\u{FF64}'));
    assert!(src.chars().contains(&'\u{FF9F}'));
}

#[test]
fn test_hex_charset() {
    let src = BuiltinChars::from_charset_name("hex");
    assert_eq!(src.chars().len(), 16);
    assert!(src.chars().contains(&'0'));
    assert!(src.chars().contains(&'F'));
}

#[test]
fn test_binary_charset() {
    let src = BuiltinChars::from_charset_name("binary");
    assert_eq!(src.chars(), &['0', '1']);
}

#[test]
fn test_transcript_dir_from_cwd_normalises_underscore() {
    let dir = flatten_cwd("/home/zsl/projects/kinds_exer/vibe-neo-matrix");
    assert_eq!(dir, "-home-zsl-projects-kinds-exer-vibe-neo-matrix");
}

#[test]
fn test_transcript_dir_from_cwd_simple_path() {
    let dir = flatten_cwd("/home/user/my_project");
    assert_eq!(dir, "-home-user-my-project");
}

#[test]
fn test_transcript_dir_from_cwd_root() {
    let dir = flatten_cwd("/");
    assert_eq!(dir, "-");
}

#[test]
fn test_extract_text_from_user_entry() {
    let json = serde_json::json!({
        "type": "user",
        "message": { "content": "Hello, how do I fix this bug?" }
    });
    assert_eq!(extract_claude_jsonl_entry(&json), "Hello, how do I fix this bug?");
}

#[test]
fn test_extract_text_from_assistant_entry() {
    let json = serde_json::json!({
        "type": "assistant",
        "message": { "content": [
            {"type": "text", "text": "Here is the fix:"},
            {"type": "text", "text": "  use std::io;"}
        ]}
    });
    assert_eq!(extract_claude_jsonl_entry(&json), "Here is the fix:  use std::io;");
}

#[test]
fn test_extract_text_from_user_entry_top_level_content() {
    let json = serde_json::json!({
        "type": "user",
        "content": "top level prompt text"
    });
    assert_eq!(extract_claude_jsonl_entry(&json), "top level prompt text");
}

#[test]
fn test_extract_text_skips_system_types() {
    for sys_type in &["attachment", "file-history-snapshot", "mode", "system"] {
        let json = serde_json::json!({
            "type": sys_type,
            "message": { "content": "should be ignored" }
        });
        assert!(extract_claude_jsonl_entry(&json).is_empty(),
            "type={} should be skipped", sys_type);
    }
}

#[test]
fn test_extract_text_empty_entry() {
    assert!(extract_claude_jsonl_entry(&serde_json::json!({})).is_empty());
}

#[test]
fn test_claude_char_source_reload() {
    let tmp = std::env::temp_dir().join("neo-rainst-test-claude-source");
    let _ = std::fs::create_dir_all(&tmp);

    let jsonl_path = tmp.join("test-session.jsonl");
    let content = r#"{"type":"user","message":{"content":"help me with rust"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"use std::io::Result;"}]}}
{"type":"system","message":{"content":"should be ignored"}}
{"type":"user","message":{"content":"thanks!"}}
"#;
    std::fs::write(&jsonl_path, content).unwrap();

    let source = ClaudeCharSource::new(&tmp, 10000).unwrap();
    let chars_str: String = source.chars().iter().collect();
    assert!(chars_str.contains("help me with rust"));
    assert!(chars_str.contains("use std::io::Result;"));
    assert!(chars_str.contains("thanks!"));
    assert!(!chars_str.contains("should be ignored"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_claude_char_source_max_chars_truncation() {
    let tmp = std::env::temp_dir().join("neo-rainst-test-claude-trunc");
    let _ = std::fs::create_dir_all(&tmp);

    let jsonl_path = tmp.join("test-session.jsonl");
    let long_text = "A".repeat(200);
    let content = format!(r#"{{"type":"user","message":{{"content":"{}"}}}}"#, long_text);
    std::fs::write(&jsonl_path, content).unwrap();

    let source = ClaudeCharSource::new(&tmp, 100).unwrap();
    assert_eq!(source.chars().len(), 100);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_claude_char_source_empty_dir() {
    let tmp = std::env::temp_dir().join("neo-rainst-test-empty");
    let _ = std::fs::create_dir_all(&tmp);

    let source = ClaudeCharSource::new(&tmp, 100).unwrap();
    assert!(source.chars().is_empty());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_claude_char_source_nonexistent_dir() {
    let tmp = std::env::temp_dir().join("neo-rainst-test-nonexistent-12345");
    let result = ClaudeCharSource::new(&tmp, 100);
    if let Ok(source) = result {
        assert!(source.chars().is_empty());
    }
}

#[test]
fn test_claude_char_source_no_duplicate_reload() {
    let tmp = std::env::temp_dir().join("neo-rainst-test-nodup");
    let _ = std::fs::create_dir_all(&tmp);

    let jsonl_path = tmp.join("test-session.jsonl");
    std::fs::write(&jsonl_path, r#"{"type":"user","message":{"content":"first"}}"#).unwrap();

    let mut source = ClaudeCharSource::new(&tmp, 100).unwrap();
    let first_chars = source.chars().to_vec();
    source.reload().unwrap();
    assert_eq!(source.chars(), first_chars.as_slice());

    let _ = std::fs::remove_dir_all(&tmp);
}
