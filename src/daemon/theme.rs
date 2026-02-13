use std::fmt::Write as FmtWrite;

/// Escape HTML special characters for safe rendering.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Percent-encode a path segment for use in href attributes.
pub fn encode_path_segment(s: &str) -> String {
    s.bytes()
        .fold(String::with_capacity(s.len()), |mut out, b| {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(b as char)
                }
                _ => {
                    let _ = write!(out, "%{b:02X}");
                }
            }
            out
        })
}

// ── SVG Icons ───────────────────────────────────────────────

/// Folder icon in brand orange.
pub const FOLDER_ICON: &str = concat!(
    r##"<svg class="ei" viewBox="0 0 20 16" width="18" height="14">"##,
    r##"<path d="M2 2C2 1.4 2.4 1 3 1h4l2 2h8c.6 0 1 .4 1 1"##,
    r##"v9c0 .6-.4 1-1 1H3c-.6 0-1-.4-1-1V2z" fill="#E8853A"/>"##,
    r##"<path d="M2 5h16v8c0 .6-.4 1-1 1H3"##,
    r##"c-.6 0-1-.4-1-1V5z" fill="#F0A050"/>"##,
    "</svg>",
);

/// File icon in brand teal.
pub const FILE_ICON: &str = concat!(
    r##"<svg class="ei" viewBox="0 0 16 20" width="14" height="17">"##,
    r##"<path d="M2 1c0-.6.4-1 1-1h7l5 5v13"##,
    r##"c0 .6-.4 1-1 1H3c-.6 0-1-.4-1-1V1z" fill="#3BB8A2"/>"##,
    r##"<path d="M10 0v4c0 .6.4 1 1 1h4z" fill="#2D9B87"/>"##,
    "</svg>",
);

/// Small home icon for breadcrumb root.
pub const HOME_ICON: &str = concat!(
    r##"<svg class="bc-home" viewBox="0 0 16 16" width="14" height="14">"##,
    r##"<path d="M8 1.5L1.5 7H4v6h3v-4h2v4h3V7h2.5z" fill="currentColor"/>"##,
    "</svg>",
);

// ── Page Shell ──────────────────────────────────────────────

/// Render a complete themed HTML page.
///
/// Wraps the given body content in the Roxy branded page shell
/// with header, footer, and common styles. Pass page-specific
/// CSS and JS via `extra_css` and `extra_js`.
pub fn render_page(title: &str, body: &str, extra_css: &str, extra_js: &str) -> String {
    let mut html = String::with_capacity(8192 + body.len());
    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str(
        "<meta name=\"viewport\" \
         content=\"width=device-width, initial-scale=1\">\n",
    );
    html.push_str("<title>");
    html.push_str(&html_escape(title));
    html.push_str(" - Roxy</title>\n<style>\n");
    html.push_str(COMMON_CSS);
    if !extra_css.is_empty() {
        html.push('\n');
        html.push_str(extra_css);
    }
    html.push_str("\n</style>\n</head>\n<body>\n");
    html.push_str(HEADER_HTML);
    html.push_str("<main class=\"roxy-main\">\n");
    html.push_str(body);
    html.push_str("\n</main>\n");
    html.push_str(FOOTER_HTML);
    if !extra_js.is_empty() {
        html.push_str("<script>\n");
        html.push_str(extra_js);
        html.push_str("\n</script>\n");
    }
    html.push_str("</body>\n</html>");
    html
}

// ── Private Constants ───────────────────────────────────────

const COMMON_CSS: &str = "\
:root {\
    --fox-orange: #E8853A;\
    --deep-amber: #D4722A;\
    --teal: #3BB8A2;\
    --teal-dark: #2D9B87;\
    --purple: #8B6BB5;\
    --purple-light: #A08AC5;\
    --blue-bg: #9CB4D4;\
    --warm-bg: #FFF8F3;\
    --card-bg: #FFFFFF;\
    --text: #3D3D3D;\
    --text-light: #8D8682;\
    --border: #F0E6DD;\
    --border-hover: #E8D5C8;\
}\
*,*::before,*::after{margin:0;padding:0;box-sizing:border-box}\
body{\
    font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,'Helvetica Neue',Arial,sans-serif;\
    background:var(--warm-bg);color:var(--text);\
    line-height:1.6;min-height:100vh;\
    display:flex;flex-direction:column;\
}\
a{color:var(--teal-dark);text-decoration:none;transition:all .2s ease}\
a:hover{color:var(--fox-orange);transform:translateY(-1px)}\
code{font-family:'SF Mono',Monaco,'Cascadia Code',Menlo,Consolas,monospace;\
    font-size:.88em;background:rgba(232,133,58,.08);\
    padding:3px 7px;border-radius:5px;border:1px solid rgba(232,133,58,.12)}\
.roxy-header{\
    background:linear-gradient(135deg,var(--fox-orange) 0%,var(--deep-amber) 100%);\
    padding:16px 24px;color:#fff;\
    box-shadow:0 3px 16px rgba(191,100,30,.15);\
}\
.roxy-header-inner{\
    max-width:1000px;margin:0 auto;\
    display:flex;align-items:center;gap:12px;\
}\
.roxy-brand{\
    font-size:1.3em;font-weight:700;letter-spacing:.03em;\
    display:flex;align-items:center;gap:8px;\
}\
.roxy-logo{font-size:1.4em;filter:drop-shadow(0 2px 4px rgba(0,0,0,.1))}\
.roxy-main{\
    max-width:1000px;width:100%;margin:0 auto;\
    padding:32px 24px;flex:1;\
}\
.roxy-footer{\
    text-align:center;padding:20px 24px;\
    color:var(--text-light);font-size:.82em;\
    border-top:1px solid var(--border);background:#FEFCFB;\
}\
.roxy-footer a{color:var(--text-light);transition:color .2s}\
.roxy-footer a:hover{color:var(--fox-orange);text-decoration:none}\
";

const HEADER_HTML: &str = concat!(
    r##"<header class="roxy-header"><div class="roxy-header-inner">"##,
    r##"<span class="roxy-brand"><span class="roxy-logo">🦊</span>Roxy</span>"##,
    "</div></header>\n",
);

const FOOTER_HTML: &str = concat!(
    r##"<footer class="roxy-footer">"##,
    r##"<span class="roxy-fox">🦊</span> "##,
    r##"<a href="https://github.com/rbas/roxy">Roxy</a>"##,
    r##" &middot; made with ☕ by "##,
    r##"<a href="https://github.com/rbas">@rbas</a>"##,
    "</footer>\n",
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("hello"), "hello");
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a&b"), "a&amp;b");
        assert_eq!(html_escape(r#"say "hi""#), "say &quot;hi&quot;");
        assert_eq!(html_escape("it's"), "it&#x27;s");
    }

    #[test]
    fn test_encode_path_segment() {
        assert_eq!(encode_path_segment("hello"), "hello");
        assert_eq!(encode_path_segment("hello world"), "hello%20world");
        assert_eq!(encode_path_segment("file#1"), "file%231");
        assert_eq!(encode_path_segment("a+b"), "a%2Bb");
    }

    #[test]
    fn test_render_page_contains_structure() {
        let html = render_page("Test", "<p>body</p>", "", "");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>Test - Roxy</title>"));
        assert!(html.contains("roxy-header"));
        assert!(html.contains("<p>body</p>"));
        assert!(html.contains("roxy-footer"));
        assert!(html.contains("made with ☕ by"));
    }

    #[test]
    fn test_render_page_includes_extra_css_and_js() {
        let html = render_page("T", "", ".custom{color:red}", "alert(1)");
        assert!(html.contains(".custom{color:red}"));
        assert!(html.contains("alert(1)"));
    }

    #[test]
    fn test_render_page_escapes_title() {
        let html = render_page("<script>", "", "", "");
        assert!(html.contains("&lt;script&gt; - Roxy"));
    }

    #[test]
    fn test_render_page_omits_empty_script() {
        let html = render_page("T", "", "", "");
        assert!(!html.contains("<script>"));
    }
}
