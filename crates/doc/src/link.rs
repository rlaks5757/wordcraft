//! Which hyperlink targets may leave the document: followed by the OS, or written into an
//! exported HTML/Markdown file where a browser would run them.

/// Unicode format characters (category Cf) that reorder or hide text: soft hyphen, Arabic letter
/// mark, zero-width and bidi marks/overrides/isolates, invisible operators, BOM, tag characters
/// (the full category).
fn is_format_char(c: char) -> bool {
    matches!(c, '\u{00AD}' | '\u{0600}'..='\u{0605}' | '\u{061C}' | '\u{06DD}' | '\u{070F}' | '\u{0890}'..='\u{0891}' | '\u{08E2}' | '\u{180E}'
            | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206F}' | '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}' | '\u{110CD}' | '\u{13430}'..='\u{1343F}' | '\u{1BCA0}'..='\u{1BCA3}' | '\u{1D173}'..='\u{1D17A}' | '\u{E0001}' | '\u{E0020}'..='\u{E007F}')
}

/// Whether Ctrl/⌘+click may hand a document's hyperlink to the system: web and email links only,
/// so a document can't launch `file:` paths, programs or custom-scheme handlers. Exact: a leading
/// space or a control character makes it not a link.
pub fn is_web_or_mail_link(url: &str) -> bool {
    if url.chars().any(is_format_char) {
        return false;
    }
    let Some((scheme, _)) = url.split_once(':') else { return false };
    ["http", "https", "mailto"].iter().any(|s| scheme.eq_ignore_ascii_case(s))
}

/// Whether a hyperlink may be written as an `href` (or Markdown destination, RTF `HYPERLINK`
/// field, ODT `xlink:href`, LaTeX `\href`) in an exported file: a web or email link, a bookmark
/// (`#name`) or a relative path. Scripts (`javascript:`, `vbscript:`), `data:`, `file:` and every
/// other scheme are refused, read the way a browser reads a URL: ASCII control characters and
/// spaces are dropped first, so `java\nscript:` is a script. A protocol-relative reference
/// (`//host/…`) or a UNC path (`\\host\…`, or `//host` once backslashes are read as separators)
/// names another machine — following it on Windows leaks the user's NTLM credentials — so it is
/// refused too.
pub fn is_safe_href(url: &str) -> bool {
    // Bidi and zero-width characters disguise where a link goes (`…/\u{202E}gpj.exe`).
    if url.chars().any(is_format_char) {
        return false;
    }
    let cleaned: String = url.chars().filter(|c| !c.is_ascii_control() && *c != ' ').collect();
    // `//host` (protocol-relative) and `\\host` (UNC), with `\` read as `/` the way Windows does.
    if cleaned.replace('\\', "/").starts_with("//") {
        return false;
    }
    match scheme(url) {
        Some(s) => matches!(s.as_str(), "http" | "https" | "mailto"),
        None => !cleaned.is_empty(),
    }
}

/// The scheme a browser would see in `url` (RFC 3986 §3.1: a letter, then letters, digits, `+`,
/// `-` or `.`, up to the first `:`), lower-cased; `None` for a relative reference.
fn scheme(url: &str) -> Option<String> {
    let cleaned: String = url.chars().filter(|c| !c.is_ascii_control() && *c != ' ').collect();
    let (s, _) = cleaned.split_once(':')?;
    let mut chars = s.chars();
    let first = chars.next()?;
    (first.is_ascii_alphabetic() && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))).then(|| s.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_and_mail_links_are_followable() {
        for ok in ["https://example.com/a?b#c", "http://example.com", "HTTPS://EXAMPLE.COM", "mailto:someone@example.com"] {
            assert!(is_web_or_mail_link(ok), "{ok}");
        }
        for bad in [
            "file:///etc/passwd",
            "C:\\Windows\\System32\\calc.exe",
            "/usr/bin/xterm",
            "javascript:alert(1)",
            "ms-msdt:/id",
            " https://example.com",
            "https\u{0}://example.com",
            "",
            "example.com",
            "#top",
        ] {
            assert!(!is_web_or_mail_link(bad), "{bad:?}");
        }
    }

    #[test]
    fn format_characters_make_a_link_unsafe() {
        for bad in [
            "https://example.com/\u{202E}gpj.exe",
            "https://exa\u{200B}mple.com",
            "mailto:a@b.c\u{FEFF}",
            "doc\u{2066}.html",
            "https://a.example/\u{061C}",
            "https://a.example/\u{00AD}",
            "https://a.example/\u{E0041}",
            "https://a.example/\u{0600}",
            "https://a.example/\u{206F}",
        ] {
            assert!(!is_web_or_mail_link(bad), "{bad:?}");
            assert!(!is_safe_href(bad), "{bad:?}");
        }
        assert!(is_web_or_mail_link("https://example.com/gpj.exe") && is_safe_href("https://example.com/gpj.exe"));
    }

    /// An exported `href` may be a web/mail link, a bookmark or a relative path: never a script
    /// or another scheme, however a browser would normalise it.
    #[test]
    fn exported_hrefs_allow_bookmarks_and_relative_paths_but_no_scripts() {
        for ok in ["https://example.com/x", "mailto:a@b.c", "#top", "other.html", "docs/guide.html#s2", "../up.html", "?q=1"] {
            assert!(is_safe_href(ok), "{ok}");
        }
        for bad in [
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "java\nscript:alert(1)",
            "\tjavascript:alert(1)",
            " javascript:alert(1)",
            "java\u{0}script:alert(1)",
            "vbscript:MsgBox(1)",
            "data:text/html,<script>alert(1)</script>",
            "file:///etc/passwd",
            "ms-msdt:/id",
            // Protocol-relative and UNC references name another machine (NTLM leak on Windows).
            "//attacker.example/share/x",
            "\\\\attacker.example\\share\\x",
            "/\\attacker.example\\x",
            " //attacker.example/x",
            "",
        ] {
            assert!(!is_safe_href(bad), "{bad:?}");
        }
    }
}
