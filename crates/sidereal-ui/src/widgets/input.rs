use bevy::prelude::*;
use unicode_segmentation::UnicodeSegmentation;

use crate::theme::{UiTheme, color, with_alpha};

use super::shadow::glow_box_shadow;

const DEFAULT_UNDO_LIMIT: usize = 100;

pub fn input_surface(
    theme: UiTheme,
    focused: bool,
    glow_intensity: f32,
) -> (BackgroundColor, BorderColor, BoxShadow) {
    let colors = theme.colors;
    if focused {
        (
            BackgroundColor(color(with_alpha(colors.input, 0.98))),
            BorderColor::all(colors.ring_color()),
            glow_box_shadow(colors.glow, 0.06, 1.0, 4.0, glow_intensity),
        )
    } else {
        (
            BackgroundColor(color(with_alpha(colors.input, 0.88))),
            BorderColor::all(color(with_alpha(colors.border, 0.75))),
            glow_box_shadow(colors.glow_muted, 0.025, 0.75, 3.0, glow_intensity),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAdornment {
    Icon(String),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputAdornments {
    pub start: Option<InputAdornment>,
    pub end: Option<InputAdornment>,
}

impl InputAdornments {
    #[must_use]
    pub fn has_start(&self) -> bool {
        self.start.is_some()
    }

    #[must_use]
    pub fn has_end(&self) -> bool {
        self.end.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextInputKind {
    #[default]
    Text,
    Password {
        mask: char,
    },
}

impl TextInputKind {
    #[must_use]
    pub fn password() -> Self {
        Self::Password { mask: '*' }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionDirection {
    Forward,
    Backward,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputMovement {
    PreviousGrapheme,
    NextGrapheme,
    PreviousWord,
    NextWord,
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputDelete {
    PreviousGrapheme,
    NextGrapheme,
    PreviousWord,
    NextWord,
    ToStart,
    ToEnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextInputDisplaySegments {
    pub before_selection: String,
    pub selected: String,
    pub after_selection: String,
    pub caret_at_selection_start: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextInputSnapshot {
    text: String,
    selection_anchor: Option<usize>,
    selection_focus: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextInputState {
    pub text: String,
    pub cursor: usize,
    pub selection_start: usize,
    pub selection_end: usize,
    pub selection_direction: SelectionDirection,
    selection_anchor: Option<usize>,
    selection_focus: usize,
    max_graphemes: Option<usize>,
    readonly: bool,
    undo_stack: Vec<TextInputSnapshot>,
    redo_stack: Vec<TextInputSnapshot>,
    undo_limit: usize,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            selection_start: 0,
            selection_end: 0,
            selection_direction: SelectionDirection::None,
            selection_anchor: None,
            selection_focus: 0,
            max_graphemes: None,
            readonly: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_limit: DEFAULT_UNDO_LIMIT,
        }
    }
}

impl TextInputState {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let mut state = Self::default();
        state.set_text(text);
        state
    }

    #[must_use]
    pub fn with_max_graphemes(mut self, max_graphemes: usize) -> Self {
        self.max_graphemes = Some(max_graphemes);
        self.truncate_to_max_graphemes();
        self
    }

    #[must_use]
    pub fn with_readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = normalize_single_line_input(&text.into());
        self.truncate_to_max_graphemes();
        let end = self.text.len();
        self.selection_anchor = None;
        self.selection_focus = end;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.sync_public_selection();
    }

    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selection_start != self.selection_end
    }

    #[must_use]
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection_start..self.selection_end]
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.selection_focus = self.text.len();
        self.sync_public_selection();
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.selection_anchor = None;
        self.selection_focus = self.snap_to_grapheme_boundary(cursor);
        self.sync_public_selection();
    }

    pub fn set_cursor_from_fraction(&mut self, fraction: f32, extend_selection: bool) {
        let cursor = self.cursor_for_fraction(fraction);
        self.set_focus(cursor, extend_selection);
    }

    pub fn select_word_at_cursor(&mut self) {
        if self.text.is_empty() {
            self.set_cursor(0);
            return;
        }

        let mut start = self.selection_focus;
        let mut end = self.selection_focus;
        if start == self.text.len() {
            start = previous_grapheme_boundary(&self.text, start);
            end = self.text.len();
        }

        if self
            .text
            .get(start..next_grapheme_boundary(&self.text, start))
            .is_some_and(is_word_grapheme)
        {
            while start > 0 {
                let previous = previous_grapheme_boundary(&self.text, start);
                if !is_word_grapheme(&self.text[previous..start]) {
                    break;
                }
                start = previous;
            }
            while end < self.text.len() {
                let next = next_grapheme_boundary(&self.text, end);
                if !is_word_grapheme(&self.text[end..next]) {
                    break;
                }
                end = next;
            }
        } else {
            end = next_grapheme_boundary(&self.text, start);
        }

        self.selection_anchor = Some(start);
        self.selection_focus = end;
        self.sync_public_selection();
    }

    pub fn move_cursor(&mut self, movement: TextInputMovement, extend_selection: bool) {
        let target = if !extend_selection && self.has_selection() {
            match movement {
                TextInputMovement::PreviousGrapheme | TextInputMovement::PreviousWord => {
                    self.selection_start
                }
                TextInputMovement::NextGrapheme | TextInputMovement::NextWord => self.selection_end,
                TextInputMovement::Start => 0,
                TextInputMovement::End => self.text.len(),
            }
        } else {
            match movement {
                TextInputMovement::PreviousGrapheme => {
                    previous_grapheme_boundary(&self.text, self.selection_focus)
                }
                TextInputMovement::NextGrapheme => {
                    next_grapheme_boundary(&self.text, self.selection_focus)
                }
                TextInputMovement::PreviousWord => {
                    previous_word_boundary(&self.text, self.selection_focus)
                }
                TextInputMovement::NextWord => next_word_boundary(&self.text, self.selection_focus),
                TextInputMovement::Start => 0,
                TextInputMovement::End => self.text.len(),
            }
        };
        self.set_focus(target, extend_selection);
    }

    pub fn insert_text(&mut self, raw: &str) -> bool {
        if self.readonly {
            return false;
        }

        let normalized = normalize_single_line_input(raw);
        let inserted = self.truncate_insert_to_capacity(&normalized);
        if inserted.is_empty() && !self.has_selection() {
            return false;
        }

        self.push_undo();
        self.replace_selection_without_undo(&inserted);
        true
    }

    pub fn delete(&mut self, delete: TextInputDelete) -> bool {
        if self.readonly {
            return false;
        }

        if self.has_selection() {
            self.push_undo();
            self.replace_selection_without_undo("");
            return true;
        }

        let (start, end) = match delete {
            TextInputDelete::PreviousGrapheme => (
                previous_grapheme_boundary(&self.text, self.cursor),
                self.cursor,
            ),
            TextInputDelete::NextGrapheme => {
                (self.cursor, next_grapheme_boundary(&self.text, self.cursor))
            }
            TextInputDelete::PreviousWord => {
                (previous_word_boundary(&self.text, self.cursor), self.cursor)
            }
            TextInputDelete::NextWord => (self.cursor, next_word_boundary(&self.text, self.cursor)),
            TextInputDelete::ToStart => (0, self.cursor),
            TextInputDelete::ToEnd => (self.cursor, self.text.len()),
        };

        if start == end {
            return false;
        }

        self.push_undo();
        self.text.replace_range(start..end, "");
        self.selection_anchor = None;
        self.selection_focus = start;
        self.sync_public_selection();
        true
    }

    pub fn cut_selection(&mut self) -> Option<String> {
        if !self.has_selection() || self.readonly {
            return None;
        }
        let selected = self.selected_text().to_string();
        self.push_undo();
        self.replace_selection_without_undo("");
        Some(selected)
    }

    #[must_use]
    pub fn copy_selection(&self) -> Option<String> {
        self.has_selection()
            .then(|| self.selected_text().to_string())
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo_stack.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.redo_stack.push(current);
        self.restore(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo_stack.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.undo_stack.push(current);
        self.restore(snapshot);
        true
    }

    #[must_use]
    pub fn display_segments(&self, kind: TextInputKind) -> TextInputDisplaySegments {
        let before = &self.text[..self.selection_start];
        let selected = &self.text[self.selection_start..self.selection_end];
        let after = &self.text[self.selection_end..];
        TextInputDisplaySegments {
            before_selection: display_for_kind(before, kind),
            selected: display_for_kind(selected, kind),
            after_selection: display_for_kind(after, kind),
            caret_at_selection_start: self.selection_focus == self.selection_start,
        }
    }

    #[must_use]
    pub fn cursor_for_fraction(&self, fraction: f32) -> usize {
        let boundaries = grapheme_boundaries(&self.text);
        if boundaries.len() <= 1 {
            return 0;
        }

        let grapheme_count = boundaries.len() - 1;
        let index = (fraction.clamp(0.0, 1.0) * grapheme_count as f32).round() as usize;
        boundaries[index.min(grapheme_count)]
    }

    fn set_focus(&mut self, cursor: usize, extend_selection: bool) {
        let cursor = self.snap_to_grapheme_boundary(cursor);
        if extend_selection {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.selection_focus);
            }
        } else {
            self.selection_anchor = None;
        }
        self.selection_focus = cursor;
        if self.selection_anchor == Some(self.selection_focus) {
            self.selection_anchor = None;
        }
        self.sync_public_selection();
    }

    fn replace_selection_without_undo(&mut self, replacement: &str) {
        let start = self.selection_start;
        let end = self.selection_end;
        self.text.replace_range(start..end, replacement);
        let cursor = start + replacement.len();
        self.selection_anchor = None;
        self.selection_focus = cursor;
        self.sync_public_selection();
    }

    fn push_undo(&mut self) {
        let snapshot = self.snapshot();
        if self.undo_stack.last() != Some(&snapshot) {
            self.undo_stack.push(snapshot);
            if self.undo_stack.len() > self.undo_limit {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();
    }

    fn snapshot(&self) -> TextInputSnapshot {
        TextInputSnapshot {
            text: self.text.clone(),
            selection_anchor: self.selection_anchor,
            selection_focus: self.selection_focus,
        }
    }

    fn restore(&mut self, snapshot: TextInputSnapshot) {
        self.text = snapshot.text;
        self.selection_anchor = snapshot.selection_anchor;
        self.selection_focus = self.snap_to_grapheme_boundary(snapshot.selection_focus);
        self.sync_public_selection();
    }

    fn sync_public_selection(&mut self) {
        self.selection_focus = self.snap_to_grapheme_boundary(self.selection_focus);
        self.selection_anchor = self
            .selection_anchor
            .map(|anchor| self.snap_to_grapheme_boundary(anchor));
        let anchor = self.selection_anchor.unwrap_or(self.selection_focus);
        self.selection_start = anchor.min(self.selection_focus);
        self.selection_end = anchor.max(self.selection_focus);
        self.cursor = self.selection_focus;
        self.selection_direction = match self.selection_anchor {
            None => SelectionDirection::None,
            Some(anchor) if anchor < self.selection_focus => SelectionDirection::Forward,
            Some(anchor) if anchor > self.selection_focus => SelectionDirection::Backward,
            Some(_) => SelectionDirection::None,
        };
    }

    fn snap_to_grapheme_boundary(&self, cursor: usize) -> usize {
        let cursor = cursor.min(self.text.len());
        if self.text.is_char_boundary(cursor) && is_grapheme_boundary(&self.text, cursor) {
            return cursor;
        }
        previous_grapheme_boundary(&self.text, cursor)
    }

    fn truncate_to_max_graphemes(&mut self) {
        let Some(max_graphemes) = self.max_graphemes else {
            return;
        };
        let mut boundaries = self.text.grapheme_indices(true);
        if let Some((byte_index, _)) = boundaries.nth(max_graphemes) {
            self.text.truncate(byte_index);
        }
    }

    fn truncate_insert_to_capacity(&self, text: &str) -> String {
        let Some(max_graphemes) = self.max_graphemes else {
            return text.to_string();
        };
        let selected_count = self.selected_text().graphemes(true).count();
        let existing_count = self.text.graphemes(true).count();
        let remaining_count = existing_count.saturating_sub(selected_count);
        let allowed = max_graphemes.saturating_sub(remaining_count);
        text.graphemes(true).take(allowed).collect()
    }
}

fn normalize_single_line_input(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    let mut last_was_newline = false;
    for chr in raw.chars() {
        if chr == '\r' || chr == '\n' {
            if !last_was_newline {
                normalized.push(' ');
            }
            last_was_newline = true;
            continue;
        }
        last_was_newline = false;
        if chr == '\t' {
            normalized.push(' ');
        } else if !chr.is_control() {
            normalized.push(chr);
        }
    }
    normalized
}

fn display_for_kind(text: &str, kind: TextInputKind) -> String {
    match kind {
        TextInputKind::Text => text.to_string(),
        TextInputKind::Password { mask } => {
            std::iter::repeat_n(mask, text.graphemes(true).count()).collect()
        }
    }
}

fn grapheme_boundaries(text: &str) -> Vec<usize> {
    let mut boundaries = vec![0];
    boundaries.extend(text.grapheme_indices(true).map(|(index, _)| index).skip(1));
    boundaries.push(text.len());
    boundaries.dedup();
    boundaries
}

fn is_grapheme_boundary(text: &str, cursor: usize) -> bool {
    grapheme_boundaries(text).binary_search(&cursor).is_ok()
}

fn previous_grapheme_boundary(text: &str, cursor: usize) -> usize {
    grapheme_boundaries(text)
        .into_iter()
        .take_while(|boundary| *boundary < cursor)
        .last()
        .unwrap_or(0)
}

fn next_grapheme_boundary(text: &str, cursor: usize) -> usize {
    grapheme_boundaries(text)
        .into_iter()
        .find(|boundary| *boundary > cursor)
        .unwrap_or(text.len())
}

fn previous_word_boundary(text: &str, cursor: usize) -> usize {
    let mut index = cursor.min(text.len());
    while index > 0 {
        let previous = previous_grapheme_boundary(text, index);
        if !text[previous..index].chars().all(char::is_whitespace) {
            break;
        }
        index = previous;
    }
    while index > 0 {
        let previous = previous_grapheme_boundary(text, index);
        if text[previous..index].chars().all(char::is_whitespace) {
            break;
        }
        index = previous;
    }
    index
}

fn next_word_boundary(text: &str, cursor: usize) -> usize {
    let mut index = cursor.min(text.len());
    while index < text.len() {
        let next = next_grapheme_boundary(text, index);
        if text[index..next].chars().all(char::is_whitespace) {
            break;
        }
        index = next;
    }
    while index < text.len() {
        let next = next_grapheme_boundary(text, index);
        if !text[index..next].chars().all(char::is_whitespace) {
            break;
        }
        index = next;
    }
    index
}

fn is_word_grapheme(value: &str) -> bool {
    value.chars().any(|chr| chr.is_alphanumeric() || chr == '_')
}
