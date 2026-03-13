use bevy::prelude::*;

pub fn text_font(font: Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font,
        font_size: size,
        ..default()
    }
}
