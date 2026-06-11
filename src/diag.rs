//! Diagnostics: every user-facing error in the language flows through here.
//!
//! Contract (docs/04-diagnostics.md): every Diagnostic has a stable code,
//! a `what` (one line, plain language), a `why` (the rule behind it), and
//! a `fix` (a concrete next step, copy-pasteable when possible).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset into the source, inclusive.
    pub start: usize,
    /// Byte offset into the source, exclusive.
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Lint,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub what: String,
    pub why: String,
    pub fix: String,
    pub span: Option<Span>,
}

impl Diagnostic {
    pub fn error(
        code: &'static str,
        what: String,
        why: String,
        fix: String,
        span: Option<Span>,
    ) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code,
            what,
            why,
            fix,
            span,
        }
    }

    pub fn lint(
        code: &'static str,
        what: String,
        why: String,
        fix: String,
        span: Option<Span>,
    ) -> Self {
        Diagnostic {
            severity: Severity::Lint,
            code,
            what,
            why,
            fix,
            span,
        }
    }

    /// Render in the exact format specified by docs/04-diagnostics.md.
    /// The ui snapshot tests pin this format; change it deliberately.
    pub fn render(&self, file: &str, src: &str) -> String {
        let mut out = String::new();
        let label = match self.severity {
            Severity::Error => "error",
            Severity::Lint => "warning",
        };
        out.push_str(&format!("{}[{}]: {}\n", label, self.code, self.what));
        if let Some(span) = self.span {
            let (line, col) = line_col(src, span.start);
            out.push_str(&format!("  --> {}:{}:{}\n", file, line, col));
            let line_text = src.lines().nth(line - 1).unwrap_or("");
            out.push_str("    |\n");
            out.push_str(&format!("{:>3} | {}\n", line, line_text));
            let snippet = src.get(span.start..span.end.min(src.len())).unwrap_or("");
            let mut caret_len = snippet.chars().take_while(|&c| c != '\n').count().max(1);
            let avail = line_text.chars().count().saturating_sub(col - 1);
            if avail > 0 {
                caret_len = caret_len.min(avail);
            }
            out.push_str("    | ");
            for _ in 1..col {
                out.push(' ');
            }
            for _ in 0..caret_len {
                out.push('^');
            }
            out.push('\n');
        }
        out.push_str(&format!(" why: {}\n", self.why));
        out.push_str(&format!(" fix: {}\n", self.fix));
        out
    }
}

/// 1-based (line, column). Columns count characters, not bytes.
/// TODO(M1): grapheme-aware columns so emoji and combining marks align.
fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in src.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Render a batch of diagnostics, blank line between each.
pub fn render_all(file: &str, src: &str, diags: &[Diagnostic]) -> String {
    let rendered: Vec<String> = diags.iter().map(|d| d.render(file, src)).collect();
    rendered.join("\n")
}
