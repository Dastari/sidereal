/// TODO: add symmetric fade-out on relevance loss via a short-lived visual ghost entity.
#[allow(clippy::type_complexity)]
pub(super) fn update_entity_visibility_fade_in_system(
    time: Res<'_, Time>,
    mut commands: Commands<'_, '_>,
    mut parents: Query<'_, '_, (Entity, &'_ Children, &'_ mut PendingVisibilityFadeIn)>,
    mut visual_children: Query<'_, '_, &'_ mut Sprite, With<StreamedVisualChild>>,
) {
    let dt_s = time.delta_secs().max(0.0);
    for (entity, children, mut fade) in &mut parents {
        fade.elapsed_s += dt_s;
        let alpha = if fade.duration_s <= 0.0 {
            1.0
        } else {
            (fade.elapsed_s / fade.duration_s).clamp(0.0, 1.0)
        };
        let mut touched_any = false;
        for child in children.iter() {
            if let Ok(mut sprite) = visual_children.get_mut(child) {
                touched_any = true;
                let mut srgba = sprite.color.to_srgba();
                srgba.alpha = alpha;
                sprite.color = Color::Srgba(srgba);
            }
        }
        if (alpha >= 0.999 || !touched_any)
            && let Ok(mut entity_commands) = commands.get_entity(entity)
        {
            entity_commands.remove::<PendingVisibilityFadeIn>();
        }
    }
}

