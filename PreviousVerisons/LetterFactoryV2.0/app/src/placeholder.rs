//! Shared `[[placeholder]]` syntax used across template scan and replacement.
//! Port of the Java `PlaceholderPatterns`.

use regex::Regex;
use std::sync::OnceLock;

fn placeholder_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[([^\]]+?)\]\]").unwrap())
}

/// Build a token from an inner placeholder name, e.g. `Name` -> `[[Name]]`.
pub fn token(inner_name: &str) -> String {
    format!("[[{inner_name}]]")
}

/// Scan text for `[[...]]` placeholders, returning the unique inner names in
/// first-seen order (case-sensitive, trimmed).
pub fn scan_text(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if text.trim().is_empty() {
        return out;
    }
    for caps in placeholder_re().captures_iter(text) {
        if let Some(m) = caps.get(1) {
            let trimmed = m.as_str().trim();
            if !trimmed.is_empty() && !out.iter().any(|e| e == trimmed) {
                out.push(trimmed.to_string());
            }
        }
    }
    out
}

/// True if the text still contains an unresolved `[[...]]` token.
#[allow(dead_code)]
pub fn contains_unresolved(text: &str) -> bool {
    placeholder_re().is_match(text)
}
