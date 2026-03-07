use sidereal_game::{
    ProceduralSprite, compute_collision_half_extents_from_procedural_sprite,
    generate_procedural_sprite_image_set, generate_rdp_collision_outline_from_procedural_sprite,
};

fn test_sprite() -> ProceduralSprite {
    ProceduralSprite {
        generator_id: "asteroid_rocky_v1".to_string(),
        resolution_px: 128,
        edge_noise: 0.03,
        lobe_amplitude: 0.12,
        crater_count: 6,
        palette_dark_rgb: [0.18, 0.16, 0.14],
        palette_light_rgb: [0.54, 0.48, 0.42],
    }
}

#[test]
fn procedural_asteroid_generates_albedo_normal_and_outline() {
    let sprite = test_sprite();
    let images =
        generate_procedural_sprite_image_set("00000000-0000-0000-0000-000000000123", &sprite)
            .expect("generate images");
    assert_eq!(images.width, 128);
    assert_eq!(images.height, 128);
    assert_eq!(images.albedo_rgba.len(), (128 * 128 * 4) as usize);
    assert_eq!(images.normal_rgba.len(), (128 * 128 * 4) as usize);
    assert!(
        images.albedo_rgba.chunks_exact(4).any(|px| px[3] > 0),
        "expected at least one opaque pixel in generated albedo"
    );
    assert!(
        images.normal_rgba.chunks_exact(4).any(|px| px[3] > 0),
        "expected at least one opaque pixel in generated normal map"
    );

    let (half_x, half_y) = compute_collision_half_extents_from_procedural_sprite(
        "00000000-0000-0000-0000-000000000123",
        &sprite,
        20.0,
    )
    .expect("compute half extents");
    assert!(half_x > 0.0);
    assert!(half_y > 0.0);

    let outline = generate_rdp_collision_outline_from_procedural_sprite(
        "00000000-0000-0000-0000-000000000123",
        &sprite,
        half_x,
        half_y,
    )
    .expect("generate outline");
    assert!(outline.points.len() >= 3);
}
