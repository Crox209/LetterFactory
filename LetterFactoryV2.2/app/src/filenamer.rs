//! Filename helpers. Port of the Java `FileNamer`.

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

fn illegal_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Windows illegal: \ / : * ? " < > |
    RE.get_or_init(|| Regex::new(r#"[\\/:*?"<>|]"#).unwrap())
}

fn multi_underscore_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"_+").unwrap())
}

/// Replace filesystem-illegal characters, collapse runs of `_`, and trim
/// leading/trailing underscores and whitespace.
pub fn sanitize(s: &str) -> String {
    let replaced = illegal_re().replace_all(s, "_");
    let collapsed = multi_underscore_re().replace_all(&replaced, "_");
    collapsed.trim_matches('_').trim().to_string()
}

/// Concatenate the sanitized values of the selected placeholder parts.
pub fn build_name_from_parts(parts: &[String], values: &std::collections::HashMap<String, String>) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let mut sb = String::new();
    for p in parts {
        let v = values.get(p).map(|s| s.as_str()).unwrap_or("");
        sb.push_str(&sanitize(v));
    }
    sb
}

/// Ensure the base name is unique within `used`, appending a zero-padded index
/// of width `index_width` on collision. Inserts the chosen name into `used`.
pub fn ensure_unique(base: &str, used: &mut HashSet<String>, index_width: usize) -> String {
    let b = if base.trim().is_empty() { "Document" } else { base };
    let mut name = b.to_string();
    let mut i = 1usize;
    while used.contains(&name) {
        name = format!("{b}_{i:0width$}", width = index_width);
        i += 1;
    }
    used.insert(name.clone());
    name
}
