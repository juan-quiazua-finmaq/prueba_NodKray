//! Search helpers: FTS5 query sanitising, ranking and previews (spec §55).

/// Maximum preview length (characters).
pub const PREVIEW_LEN: usize = 200;

/// Turn a free-text query into a safe FTS5 MATCH expression.
///
/// Each whitespace-separated token is quoted (doubling embedded quotes), so
/// operator characters such as `-`, `:`, `*` or `"` cannot produce a syntax
/// error. Returns `None` when the query has no usable token.
pub fn build_match_query(query: &str) -> Option<String> {
    let tokens: Vec<String> = query
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

/// Convert an FTS5 `bm25()` value into a positive "higher is better" score.
///
/// SQLite's `bm25()` returns a negative number where more negative means more
/// relevant, so negating it yields an intuitive descending score.
pub fn score_from_bm25(bm25: f64) -> f64 {
    if bm25.is_finite() {
        -bm25
    } else {
        0.0
    }
}

/// Build a short preview from a content fragment.
pub fn make_preview(content: &str, max: usize) -> String {
    let trimmed = content.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let truncated: String = trimmed.chars().take(max).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_each_token_to_prevent_syntax_errors() {
        let expr = build_match_query("foo -bar:baz \"qux\"").expect("expression");
        assert_eq!(expr, "\"foo\" \"-bar:baz\" \"\"\"qux\"\"\"");
    }

    #[test]
    fn empty_query_yields_none() {
        assert!(build_match_query("   ").is_none());
    }

    #[test]
    fn bm25_is_negated() {
        assert_eq!(score_from_bm25(-2.5), 2.5);
        assert_eq!(score_from_bm25(f64::NAN), 0.0);
    }

    #[test]
    fn preview_truncates_on_char_boundaries() {
        assert_eq!(make_preview("short", 200), "short");
        let long = "á".repeat(10);
        assert!(make_preview(&long, 5).ends_with('…'));
    }
}
