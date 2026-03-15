use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

pub fn spawn_scanline_overlay(
    parent: &mut ChildSpawnerCommands,
    images: &mut Assets<Image>,
    primary_line_color: Color,
    secondary_line_color: Color,
    inset_px: f32,
    line_stride_px: f32,
    line_thickness_px: usize,
) {
    let stripe_texture = images.add(make_scanline_texture(
        primary_line_color,
        secondary_line_color,
        line_stride_px,
        line_thickness_px,
    ));
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(inset_px),
                right: Val::Px(inset_px),
                bottom: Val::Px(inset_px),
                left: Val::Px(inset_px),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            FocusPolicy::Pass,
        ))
        .with_children(|scanlines| {
            scanlines.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    right: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    left: Val::Px(0.0),
                    ..default()
                },
                ImageNode::new(stripe_texture).with_mode(NodeImageMode::Tiled {
                    tile_x: false,
                    tile_y: true,
                    stretch_value: 1.0,
                }),
                FocusPolicy::Pass,
            ));
        });
}

fn make_scanline_texture(
    primary: Color,
    secondary: Color,
    line_stride_px: f32,
    line_thickness_px: usize,
) -> Image {
    let stride = line_stride_px.max(2.0).round() as usize;
    let thickness = line_thickness_px.max(1);
    let height = stride * 2;
    let mut data = vec![0_u8; height * 4];

    let primary_rgba = color_to_rgba8(primary);
    let secondary_rgba = color_to_rgba8(secondary);

    for y in 0..thickness.min(height) {
        write_row_rgba(&mut data, y, primary_rgba);
    }

    for y in stride..(stride + thickness).min(height) {
        write_row_rgba(&mut data, y, secondary_rgba);
    }

    let mut image = Image::new_fill(
        Extent3d {
            width: 1,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );

    image.sampler = ImageSampler::nearest();
    image
        .sampler
        .get_or_init_descriptor()
        .set_address_mode(ImageAddressMode::Repeat);

    image
}

fn write_row_rgba(data: &mut [u8], row: usize, rgba: [u8; 4]) {
    let offset = row * 4;
    data[offset..offset + 4].copy_from_slice(&rgba);
}

fn color_to_rgba8(color: Color) -> [u8; 4] {
    let srgba = color.to_srgba();
    [
        (srgba.red.clamp(0.0, 1.0) * 255.0).round() as u8,
        (srgba.green.clamp(0.0, 1.0) * 255.0).round() as u8,
        (srgba.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
        (srgba.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}
