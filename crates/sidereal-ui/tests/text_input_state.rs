use sidereal_ui::widgets::{
    SelectionDirection, TextInputDelete, TextInputKind, TextInputMovement, TextInputState,
};

#[test]
fn typing_replaces_active_selection() {
    let mut input = TextInputState::new("hello world");
    input.move_cursor(TextInputMovement::PreviousWord, false);
    input.move_cursor(TextInputMovement::End, true);

    assert_eq!(input.selected_text(), "world");
    assert!(input.insert_text("there"));

    assert_eq!(input.text, "hello there");
    assert_eq!(input.cursor, input.text.len());
    assert!(!input.has_selection());
}

#[test]
fn shift_selection_preserves_anchor_and_direction() {
    let mut input = TextInputState::new("hello");
    input.move_cursor(TextInputMovement::Start, false);
    input.move_cursor(TextInputMovement::NextGrapheme, true);
    input.move_cursor(TextInputMovement::NextGrapheme, true);

    assert_eq!(input.selected_text(), "he");
    assert_eq!(input.selection_direction, SelectionDirection::Forward);

    input.move_cursor(TextInputMovement::PreviousGrapheme, true);

    assert_eq!(input.selected_text(), "h");
    assert_eq!(input.selection_direction, SelectionDirection::Forward);
}

#[test]
fn word_movement_skips_spaces_and_words() {
    let mut input = TextInputState::new("hello world test");
    input.move_cursor(TextInputMovement::PreviousWord, false);

    assert_eq!(input.cursor, "hello world ".len());

    input.move_cursor(TextInputMovement::PreviousWord, false);

    assert_eq!(input.cursor, "hello ".len());

    input.move_cursor(TextInputMovement::NextWord, false);

    assert_eq!(input.cursor, "hello world ".len());
}

#[test]
fn backspace_and_delete_are_grapheme_safe() {
    let mut input = TextInputState::new("a👍🏽b");
    input.move_cursor(TextInputMovement::End, false);
    input.move_cursor(TextInputMovement::PreviousGrapheme, false);
    assert!(input.delete(TextInputDelete::PreviousGrapheme));

    assert_eq!(input.text, "ab");

    input.move_cursor(TextInputMovement::Start, false);
    assert!(input.delete(TextInputDelete::NextGrapheme));

    assert_eq!(input.text, "b");
}

#[test]
fn undo_and_redo_restore_text_and_selection() {
    let mut input = TextInputState::new("hello");
    assert!(input.insert_text(" world"));
    assert_eq!(input.text, "hello world");

    assert!(input.undo());
    assert_eq!(input.text, "hello");

    assert!(input.redo());
    assert_eq!(input.text, "hello world");
}

#[test]
fn password_display_masks_by_grapheme() {
    let input = TextInputState::new("a👍🏽b");
    let display = input.display_segments(TextInputKind::password());

    assert_eq!(display.before_selection, "***");
}

#[test]
fn max_graphemes_clamps_inserted_text() {
    let mut input = TextInputState::default().with_max_graphemes(4);
    assert!(input.insert_text("ab👍🏽cd"));

    assert_eq!(input.text, "ab👍🏽c");
}
