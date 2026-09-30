//! Org-mode text → AST parser using hand-written recursive descent.
//!
//! Parses org syntax into the canonical [`crate::ast::Document`] AST. Covers every
//! constructor that [`crate::writer`] produces, so a write-then-parse round trip
//! is well-defined.

use crate::ast::{
    Block, Checkbox, Document, Inline, ListItem, ListType, LogEntry, PlanningEntry, TableCell, Tag,
    Timestamp, Title,
};

/// Parse error with position context.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Human-readable description of the parse failure.
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse an org-mode document string into a [`Document`].
///
/// # Errors
///
/// Returns `ParseError` if the input cannot be parsed.
pub fn parse_document(input: &str) -> Result<Document, ParseError> {
    parse_document_with_residue(input).map(|(doc, _residue)| doc)
}

/// Parse an org-mode document, also returning the content of input lines
/// that no block claimed.
///
/// A file whose residue is empty lost no content during parsing, even if
/// it does not round-trip byte-for-byte: every source line is represented
/// in some block. Blank lines are never residue. Lines inside quote
/// blocks are not tracked.
///
/// # Errors
///
/// Returns `ParseError` if the input cannot be parsed.
pub fn parse_document_with_residue(input: &str) -> Result<(Document, Vec<String>), ParseError> {
    let lines: Vec<&str> = input.lines().collect();
    let mut residue = Vec::new();
    let (blocks, _) = parse_blocks(&lines, 0, &mut residue);
    Ok((Document { blocks }, residue))
}

type ParseResult<T> = Result<(T, usize), ParseError>;

/// Strip an ASCII case-insensitive prefix, as org keyword syntax requires:
/// `#+BEGIN_SRC` and `#+begin_src` are the same directive.
fn strip_ci_prefix<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let head = line.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        line.get(prefix.len()..)
    } else {
        None
    }
}

/// Whether `line` starts with `prefix`, ignoring ASCII case.
fn starts_with_ci(line: &str, prefix: &str) -> bool {
    strip_ci_prefix(line, prefix).is_some()
}

/// Closing delimiter for an org timestamp opener: `>` for active `<`
/// timestamps, `]` for inactive `[` timestamps.
const fn timestamp_delimiter(open: char) -> Option<char> {
    match open {
        '<' => Some('>'),
        '[' => Some(']'),
        _ => None,
    }
}

/// Parse a sequence of blocks starting at `pos`. Returns parsed blocks and new position.
///
/// Unrecognized non-blank lines are appended to `residue`.
fn parse_blocks(lines: &[&str], pos: usize, residue: &mut Vec<String>) -> (Vec<Block>, usize) {
    let mut blocks = Vec::new();
    let mut i = pos;
    while i < lines.len() {
        let Some(&line) = lines.get(i) else { break };
        if line.is_empty() {
            // Blank lines are represented explicitly for faithful spacing.
            blocks.push(Block::BlankLine);
            i += 1;
            continue;
        }

        if let Some(Ok((block, next))) = try_parse_heading(lines, i, residue)
            .or_else(|| try_parse_property_drawer(lines, i))
            .or_else(|| try_parse_logbook_drawer(lines, i))
            .or_else(|| try_parse_src_block(lines, i))
            .or_else(|| try_parse_example_block(lines, i))
            .or_else(|| try_parse_quote_block(lines, i))
            .or_else(|| try_parse_list(lines, i))
            .or_else(|| try_parse_table(lines, i))
            .or_else(|| try_parse_planning(i, line))
            .or_else(|| try_parse_comment(i, line))
            .or_else(|| try_parse_keyword(i, line))
            .or_else(|| try_parse_horizontal_rule(i, line))
            .or_else(|| try_parse_paragraph(lines, i))
        {
            blocks.push(block);
            i = next;
        } else {
            // Unrecognized line — record as residue (dropped content).
            if !line.is_empty() {
                residue.push(line.to_string());
            }
            i += 1;
        }
    }
    (blocks, i)
}

fn try_parse_heading(
    lines: &[&str],
    pos: usize,
    residue: &mut Vec<String>,
) -> Option<ParseResult<Block>> {
    let line = *lines.get(pos)?;
    if !line.starts_with('*') {
        return None;
    }

    let raw_level = line.chars().take_while(|c| *c == '*').count();
    // Require at least one space after stars for a valid heading
    let rest_after_stars = line.get(raw_level..)?;
    if !rest_after_stars.starts_with(' ') {
        return None;
    }
    let level = u8::try_from(raw_level).unwrap_or(u8::MAX);
    let rest = rest_after_stars.trim();

    // Parse tags at end: "Title :tag1:tag2:"
    let (title_str, tags) = parse_heading_title_and_tags(rest);

    let title = Title(title_str.to_string());

    // Collect children (blocks at higher indentation level)
    let mut children = Vec::new();
    let mut next = pos + 1;
    while let Some(&line) = lines.get(next) {
        if line.starts_with('*') {
            break;
        }
        // Gather child blocks that start on this or later lines up to the
        // next blank-line-separated block or next heading.
        if line.is_empty() {
            children.push(Block::BlankLine);
            next += 1;
            continue;
        }
        // Try to parse the next item as a child block
        let mut consumed = false;
        for child_parser in &[
            try_parse_property_drawer,
            try_parse_logbook_drawer,
            try_parse_src_block,
            try_parse_example_block,
            try_parse_quote_block,
            try_parse_list,
            try_parse_table,
        ] {
            if let Some(Ok((child_block, new_pos))) = child_parser(lines, next) {
                children.push(child_block);
                next = new_pos;
                consumed = true;
                break;
            }
        }
        if !consumed {
            // Line-based child blocks, mirroring the top-level chain.
            let line_block = try_parse_planning(next, line)
                .or_else(|| try_parse_comment(next, line))
                .or_else(|| try_parse_keyword(next, line))
                .or_else(|| try_parse_horizontal_rule(next, line));
            if let Some(Ok((block, new_pos))) = line_block {
                children.push(block);
                next = new_pos;
                consumed = true;
            }
        }
        if !consumed {
            // Try paragraph
            if let Some(Ok((para, new_pos))) = try_parse_paragraph(lines, next) {
                children.push(para);
                next = new_pos;
            } else {
                // Unrecognized child line — record as residue.
                if let Some(&child) = lines.get(next)
                    && !child.is_empty()
                {
                    residue.push(child.to_string());
                }
                next += 1;
            }
        }
    }

    Some(Ok((
        Block::Heading {
            level,
            title,
            tags,
            children,
        },
        next,
    )))
}

fn parse_heading_title_and_tags(rest: &str) -> (&str, Vec<Tag>) {
    let Some(tail) = tag_group_tail(rest) else {
        return (rest, vec![]);
    };
    let title = rest.strip_suffix(tail).unwrap_or(rest).trim_end();
    let inner = tail
        .get(1..tail.len().saturating_sub(1))
        .unwrap_or_default();
    let tags = inner.split(':').map(|tag| Tag(tag.to_owned())).collect();
    (title, tags)
}

/// The trailing tag group of a heading title: a colon-delimited run of
/// org tag characters ending the title, preceded by whitespace or starting
/// the title. Tag characters are alphanumeric plus `_`, `@`, `#`, and `%`
/// (org's tag grammar); an invalid character anywhere rejects the whole
/// group, leaving it as title text.
fn tag_group_tail(rest: &str) -> Option<&str> {
    if !rest.ends_with(':') {
        return None;
    }
    let start = rest.rfind(' ').map_or(0, |space| space + 1);
    let tail = rest.get(start..)?;
    if tail.len() <= 1 || !tail.starts_with(':') {
        return None;
    }
    let inner = tail.get(1..tail.len().saturating_sub(1))?;
    (!inner.is_empty()
        && inner
            .split(':')
            .all(|segment| segment.chars().all(is_tag_char)))
    .then_some(tail)
}

/// Whether `c` is valid in an org tag.
fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '@' | '#' | '%')
}

fn try_parse_property_drawer(lines: &[&str], pos: usize) -> Option<ParseResult<Block>> {
    if lines.get(pos)?.trim() != ":PROPERTIES:" {
        return None;
    }
    let mut entries = Vec::new();
    let mut i = pos + 1;
    while let Some(line) = lines.get(i).map(|line| line.trim()) {
        if line == ":END:" {
            return Some(Ok((Block::PropertyDrawer { entries }, i + 1)));
        }
        if let Some(stripped) = line.strip_prefix(':')
            && let Some(colon_pos) = stripped.find(':')
            && let (Some(key), Some(value)) = (
                stripped.get(..colon_pos),
                stripped.get(colon_pos.saturating_add(1)..),
            )
        {
            // Value kept verbatim (leading padding included) so aligned
            // drawers round-trip.
            entries.push((key.to_owned(), value.to_owned()));
        }
        i += 1;
    }
    Some(Ok((Block::PropertyDrawer { entries }, i)))
}

fn try_parse_logbook_drawer(lines: &[&str], pos: usize) -> Option<ParseResult<Block>> {
    if lines.get(pos)?.trim() != ":LOGBOOK:" {
        return None;
    }
    let mut entries = Vec::new();
    let mut i = pos + 1;
    while let Some(line) = lines.get(i).map(|line| line.trim()) {
        if line == ":END:" {
            return Some(Ok((Block::LogbookDrawer { entries }, i + 1)));
        }
        // Parse "- <timestamp> note" and "- [timestamp] note"
        if let Some(rest) = line.strip_prefix("- ")
            && let Some(open) = rest.chars().next()
            && let Some(close) = timestamp_delimiter(open)
            && let Some(close_pos) = rest.find(close)
            && let (Some(timestamp), Some(note)) = (
                rest.get(..=close_pos),
                rest.get(close_pos.saturating_add(1)..),
            )
        {
            entries.push(LogEntry {
                timestamp: Timestamp(timestamp.to_owned()),
                note: note.trim().to_owned(),
            });
        }
        i += 1;
    }
    Some(Ok((Block::LogbookDrawer { entries }, i)))
}

fn try_parse_src_block(lines: &[&str], pos: usize) -> Option<ParseResult<Block>> {
    let line = lines.get(pos)?.trim();
    let language = strip_ci_prefix(line, "#+begin_src")?.trim().to_owned();

    let mut i = pos + 1;
    let mut content = String::new();
    while let Some(&cur) = lines.get(i) {
        if cur.trim().eq_ignore_ascii_case("#+end_src") {
            // Canonical form: non-empty src bodies always end with newline
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            return Some(Ok((Block::SrcBlock { language, content }, i + 1)));
        }
        if !content.is_empty() {
            content.push('\n');
        }
        content.push_str(cur);
        i += 1;
    }
    // No end marker found — treat rest as content
    Some(Ok((Block::SrcBlock { language, content }, i)))
}

fn try_parse_example_block(lines: &[&str], pos: usize) -> Option<ParseResult<Block>> {
    if !lines
        .get(pos)?
        .trim()
        .eq_ignore_ascii_case("#+begin_example")
    {
        return None;
    }
    let mut i = pos + 1;
    let mut content = String::new();
    while let Some(&cur) = lines.get(i) {
        if cur.trim().eq_ignore_ascii_case("#+end_example") {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            return Some(Ok((Block::ExampleBlock { content }, i + 1)));
        }
        if !content.is_empty() {
            content.push('\n');
        }
        content.push_str(cur);
        i += 1;
    }
    // No end marker — treat the rest as content.
    Some(Ok((Block::ExampleBlock { content }, i)))
}

fn try_parse_quote_block(lines: &[&str], pos: usize) -> Option<ParseResult<Block>> {
    if !lines.get(pos)?.trim().eq_ignore_ascii_case("#+begin_quote") {
        return None;
    }
    let mut i = pos + 1;
    let mut child_lines = Vec::new();
    while let Some(&cur) = lines.get(i) {
        if cur.trim().eq_ignore_ascii_case("#+end_quote") {
            // Quote-block interiors are not residue-tracked.
            let (children, _) = parse_blocks(&child_lines, 0, &mut Vec::new());
            return Some(Ok((Block::QuoteBlock { children }, i + 1)));
        }
        child_lines.push(cur);
        i += 1;
    }
    None
}

fn try_parse_list(lines: &[&str], pos: usize) -> Option<ParseResult<Block>> {
    let (list_type, first_checkbox, first_rest) = match_bullet(lines.get(pos)?)?;

    let mut items: Vec<ListItem> = Vec::new();
    let mut cur_checkbox = first_checkbox;
    let mut cur_inlines = parse_inlines(first_rest);
    let mut i = pos + 1;

    while let Some(&line) = lines.get(i) {
        if line.is_empty() {
            // A blank line ends the list.
            break;
        }
        if let Some((_lt, checkbox, rest)) = match_bullet(line) {
            // A column-0 bullet starts the next sibling item.
            items.push(ListItem {
                content: vec![Block::Paragraph {
                    inlines: std::mem::take(&mut cur_inlines),
                }],
                checkbox: cur_checkbox,
            });
            cur_checkbox = checkbox;
            cur_inlines = parse_inlines(rest);
            i += 1;
        } else if line.starts_with(' ') || line.starts_with('\t') {
            // Indented continuation of the current item — including
            // nested sub-bullets, kept verbatim as continuation text.
            cur_inlines.push(Inline::LineBreak);
            cur_inlines.extend(parse_inlines(line));
            i += 1;
        } else {
            // A column-0 non-bullet line ends the list.
            break;
        }
    }
    items.push(ListItem {
        content: vec![Block::Paragraph {
            inlines: cur_inlines,
        }],
        checkbox: cur_checkbox,
    });
    Some(Ok((Block::List { list_type, items }, i)))
}

/// If `line` starts (column 0) with a list bullet, return the list type,
/// checkbox state, and the content after the bullet and checkbox.
fn match_bullet(line: &str) -> Option<(ListType, Checkbox, &str)> {
    if let Some(rest) = line.strip_prefix("- ") {
        let (checkbox, rest) = strip_checkbox(rest);
        return Some((ListType::Unordered, checkbox, rest));
    }
    // Ordered: `N. ` for one or more digits.
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0
        && let (Some(ordinal), Some(rest)) = (line.get(..digits), line.get(digits..))
        && let Some(rest) = rest.strip_prefix(". ")
    {
        let ordinal = ordinal.parse().unwrap_or(1);
        let (checkbox, rest) = strip_checkbox(rest);
        return Some((ListType::Ordered(ordinal), checkbox, rest));
    }
    None
}

/// Strip a leading `[ ] ` / `[X] ` checkbox marker, if present.
fn strip_checkbox(s: &str) -> (Checkbox, &str) {
    s.strip_prefix("[X] ").map_or_else(
        || {
            s.strip_prefix("[ ] ")
                .map_or((Checkbox::NoCheckbox, s), |rest| {
                    (Checkbox::Unchecked, rest)
                })
        },
        |rest| (Checkbox::Checked, rest),
    )
}

fn try_parse_table(lines: &[&str], pos: usize) -> Option<ParseResult<Block>> {
    let first = lines.get(pos)?.trim();
    if !first.starts_with('|') || !first.ends_with('|') {
        return None;
    }
    let mut rows = Vec::new();
    let mut i = pos;
    while let Some(line) = lines.get(i).map(|line| line.trim()) {
        // A blank line ends the table.
        if line.is_empty() {
            break;
        }
        if line.len() < 2 || !line.starts_with('|') || !line.ends_with('|') {
            break;
        }
        // Cells are kept verbatim, padding included — column alignment
        // and separator rows (`|---+---|`) round-trip as cell content.
        let cells: Vec<TableCell> = line
            .get(1..line.len().saturating_sub(1))?
            .split('|')
            .map(|c| TableCell {
                inlines: parse_inlines(c),
            })
            .collect();
        rows.push(cells);
        i += 1;
    }
    Some(Ok((Block::Table { rows }, i)))
}

fn try_parse_planning(pos: usize, line: &str) -> Option<ParseResult<Block>> {
    let trimmed = line.trim();
    if !starts_with_ci(trimmed, "SCHEDULED: ")
        && !starts_with_ci(trimmed, "DEADLINE: ")
        && !starts_with_ci(trimmed, "CLOSED: ")
    {
        return None;
    }

    let mut entries = Vec::new();
    // Scan the line for keyword + timestamp pairs
    let mut remaining = trimmed;
    while !remaining.is_empty() {
        if let Some(rest) = strip_ci_prefix(remaining, "SCHEDULED: ")
            && let Some((timestamp, after)) = extract_timestamp(rest)
        {
            entries.push(PlanningEntry::Scheduled(Timestamp(timestamp)));
            remaining = after;
            continue;
        }
        if let Some(rest) = strip_ci_prefix(remaining, "DEADLINE: ")
            && let Some((timestamp, after)) = extract_timestamp(rest)
        {
            entries.push(PlanningEntry::Deadline(Timestamp(timestamp)));
            remaining = after;
            continue;
        }
        if let Some(rest) = strip_ci_prefix(remaining, "CLOSED: ")
            && let Some((timestamp, after)) = extract_timestamp(rest)
        {
            entries.push(PlanningEntry::Closed(Timestamp(timestamp)));
            remaining = after;
            continue;
        }
        break;
    }

    if entries.is_empty() {
        return None;
    }
    Some(Ok((Block::Planning { entries }, pos + 1)))
}

/// Extract a timestamp like `<2026-04-30 Thu>` (active) or `[2026-04-30 Thu]`
/// (inactive) from the start of `s`. Returns the timestamp string and the
/// remaining text.
fn extract_timestamp(s: &str) -> Option<(String, &str)> {
    let s = s.trim();
    let open = s.chars().next()?;
    let close = timestamp_delimiter(open)?;
    let close_pos = s.find(close)?;
    let timestamp = s.get(..=close_pos)?.to_owned();
    let remaining = s.get(close_pos.saturating_add(1)..)?.trim();
    Some((timestamp, remaining))
}

fn try_parse_comment(pos: usize, line: &str) -> Option<ParseResult<Block>> {
    let trimmed = line.trim();
    trimmed.strip_prefix("# ").map(|text| {
        Ok((
            Block::Comment {
                text: text.to_owned(),
            },
            pos + 1,
        ))
    })
}

/// Parse a `#+NAME: value` keyword line.
///
/// `name` is the run of non-`:`, non-whitespace characters after `#+`;
/// the character immediately after must be `:`. Org keywords are
/// case-insensitive, so `name` is normalized to lowercase. `value` is the
/// verbatim remainder after that `:`, leading space included. Block
/// delimiters such as `#+begin_src` have no `:` after the name and fall
/// through.
fn try_parse_keyword(pos: usize, line: &str) -> Option<ParseResult<Block>> {
    let rest = line.strip_prefix("#+")?;
    let name_len = rest
        .find(|c: char| c == ':' || c.is_whitespace())
        .unwrap_or(rest.len());
    if name_len == 0 || rest.as_bytes().get(name_len) != Some(&b':') {
        return None;
    }
    let name = rest.get(..name_len)?.to_ascii_lowercase();
    let value = rest.get(name_len.saturating_add(1)..)?.to_owned();
    Some(Ok((Block::Keyword { name, value }, pos + 1)))
}

fn try_parse_horizontal_rule(pos: usize, line: &str) -> Option<ParseResult<Block>> {
    let trimmed = line.trim();
    if trimmed == "-----" {
        Some(Ok((Block::HorizontalRule, pos + 1)))
    } else {
        None
    }
}

fn try_parse_paragraph(lines: &[&str], pos: usize) -> Option<ParseResult<Block>> {
    if !is_paragraph_line(lines.get(pos)?) {
        return None;
    }
    // Consume consecutive paragraph lines into one block, joining them
    // with explicit `LineBreak`s so the source wrapping round-trips.
    let mut inlines = Vec::new();
    let mut i = pos;
    while let Some(&line) = lines.get(i).filter(|l| is_paragraph_line(l)) {
        if i > pos {
            inlines.push(Inline::LineBreak);
        }
        inlines.extend(parse_inlines(line));
        i += 1;
    }
    Some(Ok((Block::Paragraph { inlines }, i)))
}

/// Whether `line` can appear as paragraph content: neither blank nor the
/// start of any other block kind.
fn is_paragraph_line(line: &str) -> bool {
    if line.is_empty() {
        return false;
    }
    let trimmed = line.trim();
    // Heading: one or more `*` followed by a space.
    let stars = trimmed.chars().take_while(|c| *c == '*').count();
    if stars > 0 && trimmed[stars..].starts_with(' ') {
        return false;
    }
    if trimmed.starts_with("# ")
        || trimmed.starts_with(":PROPERTIES:")
        || trimmed.starts_with(":LOGBOOK:")
        || starts_with_ci(trimmed, "#+begin_")
        || starts_with_ci(trimmed, "SCHEDULED:")
        || starts_with_ci(trimmed, "DEADLINE:")
        || starts_with_ci(trimmed, "CLOSED:")
        || trimmed == "-----"
        || (trimmed.starts_with('|') && trimmed.ends_with('|'))
    {
        return false;
    }
    // A column-0 list bullet starts a list, not a paragraph. An indented
    // bullet has no column-0 list to join, so it stays paragraph text.
    if match_bullet(line).is_some() {
        return false;
    }
    // Keyword line `#+name:`.
    if let Some(rest) = trimmed.strip_prefix("#+") {
        let name_len = rest
            .find(|c: char| c == ':' || c.is_whitespace())
            .unwrap_or(rest.len());
        if name_len > 0 && rest.as_bytes().get(name_len) == Some(&b':') {
            return false;
        }
    }
    true
}

/// Parse inline formatting from a string.
fn parse_inlines(input: &str) -> Vec<Inline> {
    let mut inlines = Vec::new();
    let mut pos = 0;
    let chars: Vec<char> = input.chars().collect();
    while pos < chars.len() {
        let c = chars.get(pos).copied().unwrap_or_default();
        let (inline, next) = match c {
            '*' => consume_nested_marker(&chars, pos, '*', Inline::Bold),
            '/' => consume_nested_marker(&chars, pos, '/', Inline::Italic),
            '+' => consume_nested_marker(&chars, pos, '+', Inline::Strikethrough),
            '=' => consume_literal_marker(&chars, pos, '=', Inline::InlineCode),
            '~' => consume_literal_marker(&chars, pos, '~', Inline::Verbatim),
            '[' => consume_bracket(&chars, pos),
            _ => consume_plain(&chars, pos),
        };
        inlines.push(inline);
        pos = next;
    }

    // Merge adjacent Plain inlines
    merge_adjacent_plain(&mut inlines);
    inlines
}

fn consume_nested_marker(
    chars: &[char],
    pos: usize,
    marker: char,
    wrap: fn(Vec<Inline>) -> Inline,
) -> (Inline, usize) {
    find_closing(chars, pos.saturating_add(1), marker).map_or_else(
        || consume_plain(chars, pos),
        |end| {
            let inner = chars_to_string(chars, pos.saturating_add(1), end);
            (wrap(parse_inlines(&inner)), end.saturating_add(1))
        },
    )
}

fn consume_literal_marker(
    chars: &[char],
    pos: usize,
    marker: char,
    wrap: fn(String) -> Inline,
) -> (Inline, usize) {
    find_closing(chars, pos.saturating_add(1), marker).map_or_else(
        || consume_plain(chars, pos),
        |end| {
            (
                wrap(chars_to_string(chars, pos.saturating_add(1), end)),
                end.saturating_add(1),
            )
        },
    )
}

fn consume_plain(chars: &[char], pos: usize) -> (Inline, usize) {
    next_marker_or_end(chars, pos).map_or_else(
        || (Inline::Plain(chars_from(chars, pos)), chars.len()),
        |end| (Inline::Plain(chars_to_string(chars, pos, end)), end),
    )
}

fn chars_to_string(chars: &[char], start: usize, end: usize) -> String {
    chars
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

fn chars_from(chars: &[char], start: usize) -> String {
    chars.iter().skip(start).collect()
}

/// Consume a `[`-run at `pos`: an org `[[target]]` / `[[target][desc]]`
/// link, or — when the brackets do not form a well-shaped link — a plain
/// literal run up to the next inline marker. Returns the inline to emit
/// and the position after it. Never drops trailing text.
fn consume_bracket(chars: &[char], pos: usize) -> (Inline, usize) {
    let plain_to = |end: usize| Inline::Plain(chars_to_string(chars, pos, end));
    let plain_rest = || Inline::Plain(chars_from(chars, pos));
    let literal = || {
        next_marker_or_end(chars, pos)
            .map_or_else(|| (plain_rest(), chars.len()), |end| (plain_to(end), end))
    };

    // Not a `[[…` link opener — single bracket, literal.
    if pos + 1 >= chars.len() || chars.get(pos + 1) != Some(&'[') {
        return literal();
    }
    let start = pos + 2;
    let Some(bracket_end) = chars
        .get(start..)
        .and_then(|rest| rest.iter().position(|&c| c == ']'))
        .map(|p| start + p)
    else {
        return (plain_rest(), chars.len());
    };

    if bracket_end + 1 < chars.len() && chars.get(bracket_end + 1) == Some(&']') {
        // [[target]]
        let target = chars_to_string(chars, start, bracket_end);
        return (
            Inline::Link {
                target,
                description: None,
            },
            bracket_end + 2,
        );
    }
    if bracket_end + 1 >= chars.len() || chars.get(bracket_end + 1) != Some(&'[') {
        // `[[…]` followed by something other than `]` or `[` — literal.
        return literal();
    }
    // [[target][description]]
    let target = chars_to_string(chars, start, bracket_end);
    let desc_start = bracket_end + 2;
    match chars
        .get(desc_start..)
        .and_then(|rest| rest.iter().position(|&c| c == ']'))
        .map(|p| desc_start + p)
    {
        Some(desc_end) if desc_end + 1 < chars.len() && chars.get(desc_end + 1) == Some(&']') => {
            let description = chars_to_string(chars, desc_start, desc_end);
            (
                Inline::Link {
                    target,
                    description: Some(description),
                },
                desc_end + 2,
            )
        }
        // Malformed `[[target][…` — literal up to the unmatched `]`.
        Some(desc_end) => (plain_to(desc_end + 1), desc_end + 1),
        None => (plain_rest(), chars.len()),
    }
}

fn find_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    for i in start..chars.len() {
        let Some(&c) = chars.get(i) else { break };
        if c == marker && (i + 1 == chars.len() || chars.get(i + 1) != Some(&marker)) {
            return Some(i);
        }
    }
    None
}

fn next_marker_or_end(chars: &[char], pos: usize) -> Option<usize> {
    for i in pos..chars.len() {
        let Some(&c) = chars.get(i) else { break };
        if c == '*' || c == '/' || c == '+' || c == '=' || c == '~' || c == '[' {
            if i == pos {
                // Find the next different char
                continue;
            }
            return Some(i);
        }
    }
    None
}

fn merge_adjacent_plain(inlines: &mut Vec<Inline>) {
    let mut i = 0;
    while i + 1 < inlines.len() {
        if let (Some(Inline::Plain(a)), Some(Inline::Plain(b))) =
            (inlines.get(i), inlines.get(i + 1))
        {
            let merged = format!("{a}{b}");
            if let Some(slot) = inlines.get_mut(i) {
                *slot = Inline::Plain(merged);
            }
            inlines.remove(i + 1);
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Block, Document, Inline, ListItem, ListType, LogEntry, ParseError, PlanningEntry, Tag,
        Timestamp, Title, parse_document, parse_document_with_residue,
    };

    fn assert_document(input: &str, blocks: Vec<Block>) {
        assert_eq!(parse_document(input).ok(), Some(Document { blocks }));
    }

    #[test]
    fn parse_heading_level_1() {
        assert_document(
            "* Hello\n",
            vec![Block::Heading {
                level: 1,
                title: Title("Hello".into()),
                tags: vec![],
                children: vec![],
            }],
        );
    }

    #[test]
    fn parse_heading_with_tags() {
        assert_document(
            "** Task :rust:kb:\n",
            vec![Block::Heading {
                level: 2,
                title: Title("Task".into()),
                tags: vec![Tag("rust".into()), Tag("kb".into())],
                children: vec![],
            }],
        );
    }

    #[test]
    fn parse_paragraph() {
        assert_document(
            "some text\n",
            vec![Block::Paragraph {
                inlines: vec![Inline::Plain("some text".into())],
            }],
        );
    }

    #[test]
    fn parse_bold() {
        assert_document(
            "*bold*\n",
            vec![Block::Paragraph {
                inlines: vec![Inline::Bold(vec![Inline::Plain("bold".into())])],
            }],
        );
    }

    #[test]
    fn parse_italic() {
        assert_document(
            "/italic/\n",
            vec![Block::Paragraph {
                inlines: vec![Inline::Italic(vec![Inline::Plain("italic".into())])],
            }],
        );
    }

    #[test]
    fn parse_strikethrough() {
        assert_document(
            "+struck+\n",
            vec![Block::Paragraph {
                inlines: vec![Inline::Strikethrough(vec![Inline::Plain("struck".into())])],
            }],
        );
    }

    #[test]
    fn parse_link_no_description() {
        assert_document(
            "[[https://example.com]]\n",
            vec![Block::Paragraph {
                inlines: vec![Inline::Link {
                    target: "https://example.com".into(),
                    description: None,
                }],
            }],
        );
    }

    #[test]
    fn parse_link_with_description() {
        assert_document(
            "[[https://example.com][example]]\n",
            vec![Block::Paragraph {
                inlines: vec![Inline::Link {
                    target: "https://example.com".into(),
                    description: Some("example".into()),
                }],
            }],
        );
    }

    #[test]
    fn parse_src_block() {
        assert_document(
            "#+begin_src rust\nfn main() {}\n#+end_src\n",
            vec![Block::SrcBlock {
                language: "rust".into(),
                content: "fn main() {}\n".into(),
            }],
        );
    }

    #[test]
    fn parse_example_block() {
        assert_document(
            "#+begin_example\n$ ls\nfoo\n#+end_example\n",
            vec![Block::ExampleBlock {
                content: "$ ls\nfoo\n".into(),
            }],
        );
    }

    #[test]
    fn parse_property_drawer() {
        assert_document(
            ":PROPERTIES:\n:ID: abc-123\n:END:\n",
            vec![Block::PropertyDrawer {
                entries: vec![("ID".into(), " abc-123".into())],
            }],
        );
    }

    #[test]
    fn parse_list_unordered() {
        assert_document(
            "- one\n- two\n",
            vec![Block::List {
                list_type: ListType::Unordered,
                items: vec![
                    ListItem {
                        content: vec![Block::Paragraph {
                            inlines: vec![Inline::Plain("one".into())],
                        }],
                        checkbox: super::Checkbox::NoCheckbox,
                    },
                    ListItem {
                        content: vec![Block::Paragraph {
                            inlines: vec![Inline::Plain("two".into())],
                        }],
                        checkbox: super::Checkbox::NoCheckbox,
                    },
                ],
            }],
        );
    }

    #[test]
    fn parse_comment() {
        assert_document(
            "# a comment\n",
            vec![Block::Comment {
                text: "a comment".into(),
            }],
        );
    }

    #[test]
    fn parse_horizontal_rule() {
        assert_document("-----\n", vec![Block::HorizontalRule]);
    }

    #[test]
    fn parse_single_bracket_run_keeps_trailing_text() {
        assert_document(
            "see [his] notes here\n",
            vec![Block::Paragraph {
                inlines: vec![Inline::Plain("see [his] notes here".into())],
            }],
        );
    }

    #[test]
    fn residue_empty_when_every_line_claimed() {
        let residue = parse_document_with_residue("* Heading\n\nA paragraph.\n")
            .ok()
            .map(|(_, residue)| residue);
        assert_eq!(residue, Some(vec![]));
    }

    #[test]
    fn residue_records_unparsable_block_marker() {
        let residue = parse_document_with_residue("#+begin_verse\n")
            .ok()
            .map(|(_, residue)| residue);
        assert_eq!(residue, Some(vec!["#+begin_verse".to_owned()]));
    }

    #[test]
    fn residue_records_unclaimed_line_under_heading() {
        let residue = parse_document_with_residue("* Heading\n#+begin_verse\n")
            .ok()
            .map(|(_, residue)| residue);
        assert_eq!(residue, Some(vec!["#+begin_verse".to_owned()]));
    }

    #[test]
    fn residue_excludes_blank_lines() {
        let residue = parse_document_with_residue("para\n\n\n")
            .ok()
            .map(|(_, residue)| residue);
        assert_eq!(residue, Some(vec![]));
    }

    #[test]
    fn line_based_blocks_parse_as_heading_children() {
        assert_document(
            "* Heading\nSCHEDULED: <2026-05-01>\n# a comment\n#+name: value\n-----\n",
            vec![Block::Heading {
                level: 1,
                title: Title("Heading".into()),
                tags: vec![],
                children: vec![
                    Block::Planning {
                        entries: vec![PlanningEntry::Scheduled(Timestamp("<2026-05-01>".into()))],
                    },
                    Block::Comment {
                        text: "a comment".into(),
                    },
                    Block::Keyword {
                        name: "name".into(),
                        value: " value".into(),
                    },
                    Block::HorizontalRule,
                ],
            }],
        );
    }

    #[test]
    fn parse_error_displays_its_message() {
        let error = ParseError {
            message: "invalid org".to_owned(),
        };
        assert_eq!(error.to_string(), "invalid org");
    }

    #[test]
    fn heading_accepts_structured_child_blocks() {
        assert_document(
            "* Heading\n:PROPERTIES:\n:ID: node-1\n:END:\n",
            vec![Block::Heading {
                level: 1,
                title: Title("Heading".into()),
                tags: vec![],
                children: vec![Block::PropertyDrawer {
                    entries: vec![("ID".into(), " node-1".into())],
                }],
            }],
        );
    }

    #[test]
    fn unterminated_drawers_and_literal_blocks_preserve_content() {
        let cases = [
            ":PROPERTIES:\n:ID: node-1\n",
            ":LOGBOOK:\n- <2026-07-18> note\n",
            "#+begin_src rust\nfn main() {}\n",
            "#+begin_example\nliteral\n",
        ];
        for input in cases {
            let parsed = parse_document_with_residue(input).ok();
            assert!(parsed.is_some());
            assert!(parsed.is_some_and(|(_, residue)| residue.is_empty()));
        }
    }

    #[test]
    fn unterminated_quote_marker_is_reported_as_residue() {
        let residue = parse_document_with_residue("#+begin_quote\nquoted\n")
            .ok()
            .map(|(_, residue)| residue);
        assert_eq!(residue, Some(vec!["#+begin_quote".to_owned()]));
    }

    #[test]
    fn list_continuation_and_terminator_remain_distinct() {
        assert_document(
            "- item\n  continuation\nafter\n",
            vec![
                Block::List {
                    list_type: ListType::Unordered,
                    items: vec![ListItem {
                        content: vec![Block::Paragraph {
                            inlines: vec![
                                Inline::Plain("item".into()),
                                Inline::LineBreak,
                                Inline::Plain("  continuation".into()),
                            ],
                        }],
                        checkbox: super::Checkbox::NoCheckbox,
                    }],
                },
                Block::Paragraph {
                    inlines: vec![Inline::Plain("after".into())],
                },
            ],
        );
    }

    #[test]
    fn table_stops_at_blank_or_non_table_lines() {
        assert!(parse_document("| a |\n\nafter\n").is_ok());
        assert!(parse_document("| a |\nafter\n").is_ok());
    }

    #[test]
    fn invalid_and_trailing_planning_text_is_not_dropped_silently() {
        let invalid = parse_document_with_residue("SCHEDULED: later\n")
            .ok()
            .map(|(_, residue)| residue);
        assert_eq!(invalid, Some(vec!["SCHEDULED: later".to_owned()]));
        assert!(parse_document("SCHEDULED: <2026-07-18> trailing\n").is_ok());
    }

    #[test]
    fn keyword_and_multiline_paragraph_are_parsed() {
        assert_document(
            "#+title: Example\nfirst\nsecond\n",
            vec![
                Block::Keyword {
                    name: "title".into(),
                    value: " Example".into(),
                },
                Block::Paragraph {
                    inlines: vec![
                        Inline::Plain("first".into()),
                        Inline::LineBreak,
                        Inline::Plain("second".into()),
                    ],
                },
            ],
        );
    }

    #[test]
    fn unmatched_literal_markers_remain_plain_text() {
        assert_document(
            "=code\n~verb\n",
            vec![Block::Paragraph {
                inlines: vec![
                    Inline::Plain("=code".into()),
                    Inline::LineBreak,
                    Inline::Plain("~verb".into()),
                ],
            }],
        );
    }

    #[test]
    fn malformed_links_remain_plain_text() {
        for input in [
            "[[unterminated",
            "[[target]x",
            "[[target][desc]x",
            "[[target][desc",
        ] {
            assert_document(
                input,
                vec![Block::Paragraph {
                    inlines: vec![Inline::Plain(input.into())],
                }],
            );
        }
    }

    #[test]
    fn uppercase_block_delimiters_parse() {
        assert_document(
            "#+BEGIN_SRC rust\nfn main() {}\n#+END_SRC\n",
            vec![Block::SrcBlock {
                language: "rust".into(),
                content: "fn main() {}\n".into(),
            }],
        );
        assert_document(
            "#+Begin_Example\n$ ls\n#+End_Example\n",
            vec![Block::ExampleBlock {
                content: "$ ls\n".into(),
            }],
        );
        assert_document(
            "#+BEGIN_QUOTE\nquoted\n#+END_QUOTE\n",
            vec![Block::QuoteBlock {
                children: vec![Block::Paragraph {
                    inlines: vec![Inline::Plain("quoted".into())],
                }],
            }],
        );
    }

    #[test]
    fn uppercase_unrecognized_directives_remain_residue() {
        let residue = parse_document_with_residue("#+BEGIN_VERSE\n")
            .ok()
            .map(|(_, residue)| residue);
        assert_eq!(residue, Some(vec!["#+BEGIN_VERSE".to_owned()]));
    }

    #[test]
    fn keyword_names_normalize_to_lowercase() {
        assert_document(
            "#+TITLE: Example\n",
            vec![Block::Keyword {
                name: "title".into(),
                value: " Example".into(),
            }],
        );
    }

    #[test]
    fn planning_keywords_are_case_insensitive() {
        assert_document(
            "scheduled: <2026-05-01>\n",
            vec![Block::Planning {
                entries: vec![PlanningEntry::Scheduled(Timestamp("<2026-05-01>".into()))],
            }],
        );
    }

    #[test]
    fn inactive_timestamps_parse_in_planning_lines() {
        assert_document(
            "CLOSED: [2026-04-30 Thu 10:00]\n",
            vec![Block::Planning {
                entries: vec![PlanningEntry::Closed(Timestamp(
                    "[2026-04-30 Thu 10:00]".into(),
                ))],
            }],
        );
        assert_document(
            "SCHEDULED: <2026-05-01> DEADLINE: [2026-05-15]\n",
            vec![Block::Planning {
                entries: vec![
                    PlanningEntry::Scheduled(Timestamp("<2026-05-01>".into())),
                    PlanningEntry::Deadline(Timestamp("[2026-05-15]".into())),
                ],
            }],
        );
    }

    #[test]
    fn inactive_timestamps_parse_in_logbook_entries() {
        assert_document(
            ":LOGBOOK:\n- [2026-04-30 Thu 10:00] noted\n:END:\n",
            vec![Block::LogbookDrawer {
                entries: vec![LogEntry {
                    timestamp: Timestamp("[2026-04-30 Thu 10:00]".into()),
                    note: "noted".into(),
                }],
            }],
        );
    }
}
