//! Syntax highlighting from syntect and bat's syntax collection.
//!
//! The lighter of the two highlighters: about two hundred languages for a few
//! megabytes, where the full tree-sitter set costs tens. It is regex-based and
//! line-oriented, so there is no syntax tree behind it — folding is by braces
//! and editing decisions use the default `Code` context. An application that
//! needs a structural editor enables `tree-sitter` instead.
//!
//! Syntect names what it matched with TextMate scopes (`keyword.control.rust`,
//! `string.quoted.double`). Those are mapped onto the highlight names the
//! theme already defines (`keyword`, `string`), so colors keep following the
//! application's theme rather than a syntect theme.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::Range,
    rc::Rc,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use gpui::{HighlightStyle, SharedString, Task};
use gpui_base::input::{
    EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    InputHighlighterFactory,
};
use ropey::{LineType, Rope};
use syntect::{
    parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

/// Deserializing the syntax set takes tens of milliseconds, so it happens once,
/// on the first highlight, and is shared by every highlighter after that.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

/// Documents up to this size are parsed on the keystroke, from the edited line
/// on; larger ones keep their shifted highlights until a background parse
/// catches up.
const SYNC_PARSE_MAX_BYTES: usize = 64 * 1024;
const MAX_HIGHLIGHT_LINE_BYTES: usize = 4096;
const MAX_FOLD_BYTES: usize = 512 * 1024;
const PARSE_DEBOUNCE: Duration = Duration::from_millis(150);

/// Highlight names, ordered and non-overlapping.
type Highlights = Vec<(Range<usize>, &'static str)>;

pub(crate) fn input_highlighter_factory() -> InputHighlighterFactory {
    Rc::new(|language| {
        find_syntax(language)
            .map(|syntax| Box::new(SyntectInputHighlighter::new(language, syntax)) as _)
    })
}

/// Styles for one code block of a text view, or nothing for a language
/// syntect does not know.
pub(crate) fn highlight_code(
    language: &str,
    code: &str,
    resolver: &dyn HighlightStyleResolver,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let Some(syntax) = find_syntax(language) else {
        return Vec::new();
    };
    resolve_styles(&parse(syntax, code), &(0..code.len()), resolver)
}

/// Highlight names for `text`, for presentations that resolve styles later.
pub(crate) fn capture_names(language: &str, text: &str) -> Vec<(Range<usize>, SharedString)> {
    let Some(syntax) = find_syntax(language) else {
        return Vec::new();
    };
    parse(syntax, text)
        .into_iter()
        .map(|(range, name)| (range, SharedString::new_static(name)))
        .collect()
}

/// The syntax for a code fence or editor language name.
///
/// Syntect matches a token against extensions and syntax names. Most of the
/// names the rest of the library uses are already one of those; the few that
/// are not are spelled here the way syntect knows them.
fn find_syntax(language: &str) -> Option<&'static SyntaxReference> {
    let syntaxes = &*SYNTAXES;
    let canonical = super::language_name(language);
    let alias = match canonical.as_ref() {
        "text" => return None,
        "csharp" => "cs",
        "make" => "Makefile",
        "markdown_inline" => "md",
        "embedded_template" | "ejs" => "erb",
        other => other,
    };
    [language, canonical.as_ref(), alias]
        .into_iter()
        .find_map(|token| syntaxes.find_syntax_by_token(token))
        .filter(|syntax| syntax.name != "Plain Text")
}

/// Syntect's state where a line starts.
type LineState = (ParseState, ScopeStack);
type ScopeNames = HashMap<Scope, Option<&'static str>>;

/// Runs syntect over `text` and keeps the spans that map to a highlight name.
fn parse(syntax: &SyntaxReference, text: &str) -> Highlights {
    let mut state = (ParseState::new(syntax), ScopeStack::new());
    let mut names = ScopeNames::new();
    let mut highlights = Highlights::new();
    let mut offset = 0;
    for line in LinesWithEndings::from(text) {
        parse_line(line, offset, &mut state, &mut names, &mut highlights);
        offset += line.len();
    }
    highlights
}

/// Parses one line (with its newline) that starts at byte `offset`, and leaves
/// `state` as the next line starts. Highlights never run past their line, so
/// one line's highlights can be replaced on their own.
fn parse_line(
    line: &str,
    offset: usize,
    (parser, scopes): &mut LineState,
    names: &mut ScopeNames,
    highlights: &mut Highlights,
) {
    let line_start = highlights.len();
    let mut push = |range: Range<usize>, scopes: &ScopeStack, highlights: &mut Highlights| {
        if range.is_empty() {
            return;
        }
        // The innermost scope that names something wins, so the quotes of a
        // string — `punctuation.definition.string` inside `string` — color as
        // the string rather than as punctuation.
        let name = scopes
            .scopes
            .iter()
            .rev()
            .find_map(|scope| *names.entry(*scope).or_insert_with(|| semantic_name(*scope)));
        if let Some(name) = name {
            let same_line = highlights.len() > line_start;
            match highlights.last_mut() {
                Some((last, last_name))
                    if same_line && last.end == range.start && *last_name == name =>
                {
                    last.end = range.end;
                }
                _ => highlights.push((range, name)),
            }
        }
    };

    // Skip ultra-long minified lines to avoid regex backtracking stalls.
    if line.len() > MAX_HIGHLIGHT_LINE_BYTES {
        return;
    }

    // A line syntect cannot parse keeps the scopes it started with; the
    // next line starts over from them rather than losing the document.
    if let Ok(operations) = parser.parse_line(line, &SYNTAXES) {
        let mut cursor = 0;
        for (index, operation) in operations {
            push(offset + cursor..offset + index, scopes, highlights);
            let _ = scopes.apply(&operation);
            cursor = index;
        }
        push(offset + cursor..offset + line.len(), scopes, highlights);
    }
}

/// The editor's parse, kept by line. An edit is parsed from its own line and
/// stops at the first later line that starts in the same state as before, so
/// a keystroke costs a line or two instead of the whole document.
#[derive(Default)]
struct LineParse {
    /// The state each line of the last parsed text starts in. Lines in the
    /// same block start alike and share one copy.
    starts: Vec<Arc<LineState>>,
    /// The length of the last parsed text.
    len: usize,
    names: ScopeNames,
}

impl LineParse {
    /// Brings `highlights` up to date with `text` after `edit`. With no earlier
    /// parse, or an edit that does not fit it, the whole text is parsed.
    /// Returns how many lines were parsed.
    fn update(
        &mut self,
        syntax: &SyntaxReference,
        edit: Option<&InputEdit>,
        text: &Rope,
        highlights: &mut Highlights,
    ) -> usize {
        let lf = LineType::LF;
        let count = text.len_lines(lf);
        let old_count = self.starts.len() as isize;
        let line_of = |byte: usize| text.byte_to_line_idx(byte.min(text.len()), lf);
        // The edited lines are `first..=last` now and were `first..=old_last`.
        // An edit is only trusted when it turns the last parsed length into
        // this one; anything else (a change the highlighter never saw) parses
        // the whole text rather than keeping stale lines.
        let consistent = |edit: &&InputEdit| {
            edit.start_byte <= edit.old_end_byte
                && edit.start_byte <= edit.new_end_byte
                && edit.old_end_byte <= self.len
                && self.len - edit.old_end_byte + edit.new_end_byte == text.len()
        };
        let fits = edit.filter(|_| old_count > 0).filter(consistent).and_then(|edit| {
            let (first, last) = (line_of(edit.start_byte), line_of(edit.new_end_byte));
            let old_last = last as isize - (count as isize - old_count);
            (first as isize <= old_last && old_last < old_count).then_some((
                first,
                last,
                old_last as usize,
            ))
        });
        let (first, last, after) = match fits {
            Some((first, last, old_last)) => {
                let after = self.starts.split_off(old_last + 1);
                self.starts.truncate(first + 1);
                (first, last, after)
            }
            None => {
                self.starts = vec![Arc::new((ParseState::new(syntax), ScopeStack::new()))];
                highlights.clear();
                (0, count, Vec::new())
            }
        };

        let start = text.line_to_byte_idx(first, lf);
        let mut offset = start;
        let mut state = (*self.starts[first]).clone();
        let mut fresh = Highlights::new();
        let mut line = first;
        while line < count {
            let content = text.line(line, lf).to_string();
            parse_line(&content, offset, &mut state, &mut self.names, &mut fresh);
            offset += content.len();
            line += 1;
            if line == count {
                break;
            }
            // Past the edit, a line that starts as it did before ends the work:
            // the rest of the text parses as it did last time.
            if line > last {
                let ix = line - last - 1;
                if after.get(ix).is_some_and(|old| **old == state) {
                    self.starts.extend_from_slice(&after[ix..]);
                    break;
                }
            }
            let prev = self.starts.last().expect("the first line's state");
            let next = if **prev == state {
                prev.clone()
            } else {
                Arc::new(state.clone())
            };
            self.starts.push(next);
        }

        // `start..offset` is parsed again. The highlights after it move by the
        // change in length.
        let delta = text.len() as isize - self.len as isize;
        let old_end = (offset as isize - delta).max(start as isize) as usize;
        let from = highlights.partition_point(|(range, _)| range.end <= start);
        let to = highlights
            .partition_point(|(range, _)| range.start < old_end)
            .max(from);
        for (range, _) in &mut highlights[to..] {
            range.start = range.start.saturating_add_signed(delta);
            range.end = range.end.saturating_add_signed(delta);
        }
        highlights.splice(from..to, fresh);
        self.len = text.len();
        line - first
    }
}

/// The theme's highlight name for a TextMate scope, or `None` to let an
/// enclosing scope decide.
fn semantic_name(scope: Scope) -> Option<&'static str> {
    let scope = scope.build_string();
    let is = |prefix: &str| {
        scope == prefix
            || scope
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('.'))
    };

    let name = if is("punctuation.definition.string")
        || is("punctuation.definition.comment")
        || is("invalid")
        || is("meta")
        || is("source")
        || is("text")
    {
        // Delimiters belong to what they delimit; the rest are containers
        // with no color of their own.
        return None;
    } else if is("comment.block.documentation") || is("comment.line.documentation") {
        "comment.doc"
    } else if is("comment") {
        "comment"
    } else if is("constant.character.escape") {
        "string.escape"
    } else if is("string.regexp") {
        "string.regex"
    } else if is("string") {
        "string"
    } else if is("constant.numeric") {
        "number"
    } else if is("constant.language.boolean") {
        "boolean"
    } else if is("constant") || is("support.constant") || is("entity.name.constant") {
        "constant"
    } else if is("keyword.operator") {
        "operator"
    } else if is("keyword.control.import") || is("meta.preprocessor") {
        "preproc"
    } else if is("keyword") || is("storage.modifier") {
        "keyword"
    } else if is("storage.type") {
        // `fn`, `class` and `let` are declarations, `int` and `bool` are types.
        if [
            "class",
            "enum",
            "function",
            "impl",
            "interface",
            "module",
            "namespace",
            "struct",
            "trait",
            "type",
            "let",
            "var",
            "const",
        ]
        .iter()
        .any(|word| scope.contains(word))
        {
            "keyword"
        } else {
            "type"
        }
    } else if is("entity.name.function")
        || is("support.function")
        || is("variable.function")
        || is("meta.function-call")
    {
        "function"
    } else if is("entity.name.type")
        || is("entity.name.class")
        || is("entity.name.struct")
        || is("entity.name.enum")
        || is("entity.name.trait")
        || is("entity.name.interface")
        || is("entity.other.inherited-class")
        || is("support.type")
        || is("support.class")
    {
        "type"
    } else if is("entity.name.tag") {
        "tag"
    } else if is("entity.other.attribute-name") {
        "attribute"
    } else if is("entity.name.label") {
        "label"
    } else if is("entity.name.section") || is("markup.heading") {
        "title"
    } else if is("variable.language") {
        "variable.special"
    } else if is("variable.other.member")
        || is("variable.other.property")
        || is("support.variable.property")
        || is("meta.mapping.key")
    {
        "property"
    } else if is("variable") {
        "variable"
    } else if is("markup.bold") {
        "emphasis.strong"
    } else if is("markup.italic") {
        "emphasis"
    } else if is("markup.underline.link") {
        "link_uri"
    } else if is("markup.raw") {
        "text.literal"
    } else if is("markup.list") {
        "punctuation.list_marker"
    } else if is("punctuation.section") || is("punctuation.definition.block") {
        "punctuation.bracket"
    } else if is("punctuation.separator") || is("punctuation.terminator") {
        "punctuation.delimiter"
    } else if is("punctuation") {
        "punctuation"
    } else {
        return None;
    };
    Some(name)
}

/// Turns the highlights overlapping `range` into gap-free style runs.
///
/// `highlights` is ordered and non-overlapping, so the first candidate is found
/// by binary search instead of scanning the whole document on every frame.
fn resolve_styles(
    highlights: &[(Range<usize>, &'static str)],
    range: &Range<usize>,
    resolver: &dyn HighlightStyleResolver,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let first = highlights.partition_point(|(highlight, _)| highlight.end <= range.start);
    let mut runs = Vec::new();
    let mut cursor = range.start;

    for (highlight, name) in &highlights[first..] {
        if highlight.start >= range.end {
            break;
        }
        let start = highlight.start.max(range.start);
        let end = highlight.end.min(range.end);
        if start >= end || end <= cursor {
            continue;
        }
        if cursor < start {
            runs.push((cursor..start, HighlightStyle::default()));
        }
        runs.push((start..end, resolver.style(name).unwrap_or_default()));
        cursor = end;
    }

    if cursor < range.end {
        runs.push((cursor..range.end, HighlightStyle::default()));
    }
    runs
}

/// Moves the highlights after an edit by its length and drops the ones it
/// touched, so the text that did not change keeps its colors while a
/// background parse catches up.
fn shift(highlights: &mut Highlights, edit: &InputEdit) {
    let delta = edit.new_end_byte as isize - edit.old_end_byte as isize;
    highlights.retain_mut(|(range, _)| {
        if range.end <= edit.start_byte {
            true
        } else if range.start >= edit.old_end_byte {
            range.start = range.start.saturating_add_signed(delta);
            range.end = range.end.saturating_add_signed(delta);
            true
        } else {
            false
        }
    });
}

struct SyntectInputHighlighter {
    language: SharedString,
    syntax: &'static SyntaxReference,
    highlights: Rc<RefCell<Highlights>>,
    folds: Rc<RefCell<Vec<FoldRange>>>,
    parse_task: Option<Task<()>>,
    lines: LineParse,
    bg_state: Arc<Mutex<(LineParse, Highlights)>>,
    pending_edits: Rc<RefCell<Vec<InputEdit>>>,
}

impl SyntectInputHighlighter {
    fn new(language: &str, syntax: &'static SyntaxReference) -> Self {
        Self {
            language: language.to_owned().into(),
            syntax,
            highlights: Rc::default(),
            folds: Rc::default(),
            parse_task: None,
            lines: LineParse::default(),
            bg_state: Arc::default(),
            pending_edits: Rc::default(),
        }
    }
}

impl InputHighlighter for SyntectInputHighlighter {
    fn language(&self) -> SharedString {
        self.language.clone()
    }

    fn update(
        &mut self,
        edit: Option<InputEdit>,
        text: &Rope,
        folding: bool,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<EditorState>,
    ) {
        if text.len() <= SYNC_PARSE_MAX_BYTES {
            self.parse_task = None;
            self.pending_edits.borrow_mut().clear();
            if let Ok(mut bg) = self.bg_state.lock()
                && bg.0.len > 0
            {
                *bg = Default::default();
            }
            self.lines.update(
                self.syntax,
                edit.as_ref(),
                text,
                &mut self.highlights.borrow_mut(),
            );
            *self.folds.borrow_mut() = if folding {
                brace_fold_ranges(&text.to_string())
            } else {
                Vec::new()
            };
            return;
        }

        // Keep large documents incremental on the background thread using bg_state.
        self.lines = LineParse::default();
        match edit {
            Some(edit) => {
                shift(&mut self.highlights.borrow_mut(), &edit);
                self.pending_edits.borrow_mut().push(edit);
            }
            None => {
                self.pending_edits.borrow_mut().clear();
                if let Ok(mut bg) = self.bg_state.lock() {
                    *bg = Default::default();
                }
            }
        }
        let single_edit = {
            let edits = self.pending_edits.borrow();
            (edits.len() == 1).then(|| edits[0].clone())
        };

        let syntax = self.syntax;
        let highlights = self.highlights.clone();
        let folds = self.folds.clone();
        let bg_state = self.bg_state.clone();
        let pending_edits = self.pending_edits.clone();
        let text = text.clone();
        // Replacing the task drops the previous one, which cancels a parse
        // still waiting out its debounce.
        self.parse_task = Some(cx.spawn_in(window, async move |entity, cx| {
            cx.background_executor().timer(PARSE_DEBOUNCE).await;
            let (parsed, folded) = cx
                .background_executor()
                .spawn(async move {
                    let parsed = {
                        let mut guard = bg_state.lock().unwrap_or_else(|e| e.into_inner());
                        let (ref mut bg_lines, ref mut bg_highlights) = *guard;
                        bg_lines.update(syntax, single_edit.as_ref(), &text, bg_highlights);
                        bg_highlights.clone()
                    };
                    let folded = if folding && text.len() <= MAX_FOLD_BYTES {
                        brace_fold_ranges(&text.to_string())
                    } else {
                        Vec::new()
                    };
                    (parsed, folded)
                })
                .await;
            *highlights.borrow_mut() = parsed;
            *folds.borrow_mut() = folded.clone();
            pending_edits.borrow_mut().clear();
            let _ = entity.update(cx, |state, cx| {
                state.apply_highlighter_fold_candidates(folded, cx);
            });
        }));
    }

    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        resolve_styles(&self.highlights.borrow(), range, resolver)
    }

    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        self.folds.borrow().clone()
    }
}

/// Fold candidates from matching braces outside strings and line comments.
///
/// Without a syntax tree this is the structure most languages share; a
/// language that nests by indentation alone gets no candidates.
fn brace_fold_ranges(text: &str) -> Vec<FoldRange> {
    if text.len() > MAX_FOLD_BYTES {
        return Vec::new();
    }
    let mut starts = Vec::new();
    let mut ranges = Vec::new();

    for (line_number, line) in text.lines().enumerate() {
        let b = line.as_bytes();
        let mut i = 0;
        let mut quote = None;
        let mut escaped = false;
        while i < b.len() {
            let ch = b[i];
            match quote {
                Some(open) => {
                    if escaped {
                        escaped = false;
                    } else if ch == b'\\' {
                        escaped = true;
                    } else if ch == open {
                        quote = None;
                    }
                }
                None => match ch {
                    b'/' if b.get(i + 1) == Some(&b'/') => break,
                    b'#' => break,
                    b'"' | b'\'' | b'`' => quote = Some(ch),
                    b'{' => starts.push(line_number),
                    b'}' => {
                        if let Some(start) = starts.pop()
                            && line_number.saturating_sub(start) >= 2
                        {
                            ranges.push(FoldRange::new(start, line_number));
                        }
                    }
                    _ => {}
                },
            }
            i += 1;
        }
    }
    ranges.sort_by_key(|range| range.start_line);
    ranges.dedup_by_key(|range| range.start_line);
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(language: &str, code: &str) -> Vec<(&'static str, String)> {
        let syntax = find_syntax(language).expect("a known language");
        parse(syntax, code)
            .into_iter()
            .map(|(range, name)| (name, code[range].to_owned()))
            .collect()
    }

    #[test]
    fn common_fence_names_find_a_syntax() {
        for language in [
            "rust",
            "rs",
            "toml",
            "typescript",
            "ts",
            "tsx",
            "javascript",
            "js",
            "python",
            "py",
            "bash",
            "sh",
            "yaml",
            "yml",
            "json",
            "jsonc",
            "markdown",
            "md",
            "go",
            "c",
            "cpp",
            "csharp",
            "java",
            "kotlin",
            "swift",
            "ruby",
            "html",
            "css",
            "sql",
            "diff",
            "make",
            "dockerfile",
            "zig",
            "lua",
            "php",
        ] {
            assert!(
                find_syntax(language).is_some(),
                "`{language}` has no syntax"
            );
        }
        assert!(
            find_syntax("text").is_none(),
            "plain text is not highlighted"
        );
        assert!(find_syntax("no-such-language").is_none());
    }

    #[test]
    fn scopes_map_onto_the_themes_highlight_names() {
        let toml = names(
            "toml",
            "[package]\nname = \"demo\" # the name\nversion = 1\n",
        );
        assert!(
            toml.contains(&("string", "\"demo\"".to_owned())),
            "{toml:?}"
        );
        assert!(
            toml.iter()
                .any(|(name, text)| *name == "comment" && text.contains("the name"))
        );
        assert!(toml.contains(&("number", "1".to_owned())), "{toml:?}");

        let rust = names("rust", "fn main() { let x = 42; }\n");
        assert!(rust.contains(&("keyword", "fn".to_owned())), "{rust:?}");
        assert!(rust.contains(&("function", "main".to_owned())), "{rust:?}");
        assert!(rust.contains(&("number", "42".to_owned())), "{rust:?}");
    }

    #[test]
    fn style_runs_cover_the_range_without_gaps() {
        struct Named;
        impl HighlightStyleResolver for Named {
            fn style(&self, _: &str) -> Option<HighlightStyle> {
                Some(HighlightStyle::default())
            }
        }
        let runs = resolve_styles(&[(2..4, "keyword"), (6..8, "string")], &(0..10), &Named);
        let ranges: Vec<_> = runs.into_iter().map(|(range, _)| range).collect();
        assert_eq!(ranges, vec![0..2, 2..4, 4..6, 6..8, 8..10]);
    }

    #[test]
    fn an_edit_shifts_later_highlights_and_drops_touched_ones() {
        let mut highlights: Highlights =
            vec![(0..2, "keyword"), (4..6, "string"), (8..10, "number")];
        shift(
            &mut highlights,
            &InputEdit {
                start_byte: 5,
                old_end_byte: 5,
                new_end_byte: 8,
                start_position: gpui_base::input::Point::new(0, 0),
                old_end_position: gpui_base::input::Point::new(0, 0),
                new_end_position: gpui_base::input::Point::new(0, 0),
            },
        );
        assert_eq!(highlights, vec![(0..2, "keyword"), (11..13, "number")]);
    }

    #[test]
    fn braces_outside_strings_and_comments_fold() {
        let folds = brace_fold_ranges("fn a() {\n  let s = \"{\";\n  // }\n}\n");
        assert_eq!(folds, vec![FoldRange::new(0, 3)]);
    }

    /// About 150 lines of JavaScript, the size of file that lagged in issue #92.
    fn sample_js() -> String {
        let block = "// Store {n}\nclass Store{n} extends Base {\n  constructor(key) {\n    super();\n    this.key = `items-${key}-{n}`;\n    this.re = /[a-z]+\\d*/gi;\n  }\n  /* cached */\n  async load(url, { retries = 3 } = {}) {\n    const res = await fetch(url, { headers: { Accept: 'application/json' } });\n    return res.ok ? res.json() : [];\n  }\n}\n\n";
        (0..11)
            .map(|n| block.replace("{n}", &n.to_string()))
            .collect()
    }

    /// Applies `new` over `range` of `text` the way the editor reports it.
    fn edit(text: &mut String, range: Range<usize>, new: &str) -> InputEdit {
        text.replace_range(range.clone(), new);
        let point = gpui_base::input::Point::new(0, 0);
        InputEdit {
            start_byte: range.start,
            old_end_byte: range.end,
            new_end_byte: range.start + new.len(),
            start_position: point,
            old_end_position: point,
            new_end_position: point,
        }
    }

    #[test]
    fn a_line_by_line_reparse_matches_a_full_parse() {
        let syntax = find_syntax("js").unwrap();
        let mut text = sample_js();
        let mut lines = LineParse::default();
        let mut highlights = Highlights::new();
        lines.update(syntax, None, &Rope::from(text.as_str()), &mut highlights);
        assert_eq!(highlights, parse(syntax, &text));

        fn at(text: &str, needle: &str) -> Range<usize> {
            let start = text.find(needle).unwrap();
            start..start
        }
        let edits: [fn(&str) -> (Range<usize>, &'static str); 12] = [
            |t| (at(t, "super"), "x"),
            |t| (at(t, "load(url"), "\n"),
            // Opens a comment that runs to the end, then closes it again.
            |t| (at(t, "// Store 3"), "/*"),
            |t| (at(t, "// Store 5"), "*/"),
            // Opens a template string, then takes it back.
            |t| (at(t, "this.re"), "`"),
            |t| (at(t, "`this.re").start..at(t, "this.re").start, ""),
            |t| (at(t, "class Store2").start..at(t, "class Store4").start, ""),
            |t| (at(t, "fetch").start..at(t, "Accept").start, "go\n(\n"),
            |t| (t.len()..t.len(), "let tail = 1;"),
            |t| (t.len() - 1..t.len(), "\n"),
            |t| (0..t.len(), ""),
            |_| (0..0, "const a = '1';\nconst b = 2;\n"),
        ];
        for (step, make) in edits.iter().enumerate() {
            let (range, new) = make(&text);
            let input_edit = edit(&mut text, range, new);
            lines.update(
                syntax,
                Some(&input_edit),
                &Rope::from(text.as_str()),
                &mut highlights,
            );
            assert_eq!(highlights, parse(syntax, &text), "after edit {step}");
        }

        // A change the highlighter never heard about, then an edit that does
        // not add up against it: the whole text is parsed again.
        text = sample_js();
        let stale = edit(&mut text, 0..0, "/*");
        let parsed = lines.update(
            syntax,
            Some(&stale),
            &Rope::from(text.as_str()),
            &mut highlights,
        );
        assert_eq!(parsed, Rope::from(text.as_str()).len_lines(LineType::LF));
        assert_eq!(highlights, parse(syntax, &text));
    }

    /// Random edits of the kinds that change how later lines parse (quotes,
    /// comment marks, braces, newlines, deletions) always end where a full
    /// parse does.
    #[test]
    fn random_edits_match_a_full_parse() {
        let syntax = find_syntax("js").unwrap();
        let mut text = sample_js();
        let mut lines = LineParse::default();
        let mut highlights = Highlights::new();
        lines.update(syntax, None, &Rope::from(text.as_str()), &mut highlights);

        let pieces = [
            "x", " ", "\n", "\"", "'", "`", "/*", "*/", "//", "{", "}", "${", "\\", "/", "é",
            "\r\n", "abc\ndef", "",
        ];
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = move |bound: usize| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) as usize) % bound.max(1)
        };
        let boundary = |text: &str, mut at: usize| {
            while !text.is_char_boundary(at) {
                at -= 1;
            }
            at
        };
        for step in 0..400 {
            let start = boundary(&text, next(text.len() + 1));
            let end = boundary(&text, (start + next(4) * next(8)).min(text.len())).max(start);
            let new = pieces[next(pieces.len())];
            let input_edit = edit(&mut text, start..end, new);
            lines.update(
                syntax,
                Some(&input_edit),
                &Rope::from(text.as_str()),
                &mut highlights,
            );
            if step % 20 == 0 || step == 399 {
                assert_eq!(highlights, parse(syntax, &text), "after random edit {step}");
            }
            if text.len() > 6 * 1024 {
                text.truncate(boundary(&text, 3 * 1024));
                lines.update(syntax, None, &Rope::from(text.as_str()), &mut highlights);
            }
        }
    }

    #[test]
    fn a_keystroke_reparses_only_its_own_lines() {
        let syntax = find_syntax("js").unwrap();
        let mut text = sample_js();
        let mut lines = LineParse::default();
        let mut highlights = Highlights::new();
        let count = Rope::from(text.as_str()).len_lines(LineType::LF);
        assert!(count >= 150);
        assert_eq!(
            lines.update(syntax, None, &Rope::from(text.as_str()), &mut highlights),
            count
        );

        // Typing in the middle of the file, then Enter.
        let mut at = text.find("const res = await fetch").unwrap() + 5;
        for key in ["x", "y", "z", "1", "\n", "a", "b", " ", "\n"] {
            let input_edit = edit(&mut text, at..at, key);
            at += key.len();
            let parsed = lines.update(
                syntax,
                Some(&input_edit),
                &Rope::from(text.as_str()),
                &mut highlights,
            );
            assert!(parsed <= 2, "{key:?} parsed {parsed} of {count} lines");
        }
        assert_eq!(highlights, parse(syntax, &text));
    }

    #[test]
    fn long_minified_lines_skip_without_breaking_following_lines() {
        fn assert_send<T: Send>() {}
        assert_send::<LineParse>();

        let syntax = find_syntax("js").unwrap();
        let huge = format!("const big = \"{}\";\nconst answer = 42;\n", "a".repeat(5000));
        let mut lines = LineParse::default();
        let mut highlights = Highlights::new();
        lines.update(syntax, None, &Rope::from(huge.as_str()), &mut highlights);
        let second_line_start = huge.find("const answer").unwrap();
        assert!(highlights.iter().all(|(r, _)| r.start >= second_line_start));
        assert!(highlights.iter().any(|(r, name)| *name == "keyword" && &huge[r.clone()] == "const"));
        assert!(highlights.iter().any(|(r, name)| *name == "number" && &huge[r.clone()] == "42"));
    }
}
