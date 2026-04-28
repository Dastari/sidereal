fn auth_svg_icon_kind(role: AuthUiSvgIconRole, reveal_password: bool) -> AuthUiSvgIconKind {
    match role {
        AuthUiSvgIconRole::Alert => AuthUiSvgIconKind::CircleAlert,
        AuthUiSvgIconRole::Email => AuthUiSvgIconKind::Email,
        AuthUiSvgIconRole::Password => AuthUiSvgIconKind::Password,
        AuthUiSvgIconRole::PasswordVisibilityToggle if reveal_password => AuthUiSvgIconKind::EyeOff,
        AuthUiSvgIconRole::PasswordVisibilityToggle => AuthUiSvgIconKind::Eye,
    }
}

fn auth_svg_icon_color_key(
    role: AuthUiSvgIconRole,
    status_icon: Option<&AuthUiStatusIconSlot>,
) -> AuthUiSvgIconColor {
    match role {
        AuthUiSvgIconRole::Alert => AuthUiSvgIconColor::SemanticForeground(
            status_icon
                .map(|icon| icon.tone)
                .unwrap_or(UiSemanticTone::Danger),
        ),
        AuthUiSvgIconRole::Email
        | AuthUiSvgIconRole::Password
        | AuthUiSvgIconRole::PasswordVisibilityToggle => AuthUiSvgIconColor::Primary,
    }
}

fn auth_svg_icon_color(
    role: AuthUiSvgIconRole,
    status_icon: Option<&AuthUiStatusIconSlot>,
    theme: sidereal_ui::theme::UiTheme,
) -> Color {
    match auth_svg_icon_color_key(role, status_icon) {
        AuthUiSvgIconColor::Primary => theme.colors.primary_color(),
        AuthUiSvgIconColor::SemanticForeground(tone) => tone.foreground_color(theme),
    }
}

fn auth_svg_icon_handle(
    key: AuthUiSvgIconCacheKey,
    color: Color,
    cache: &mut AuthSvgIconHandleCache,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    if let Some(handle) = cache.handles_by_key.get(&key) {
        return Some(handle.clone());
    }

    let (bytes, _) = auth_svg_icon_bytes(key.kind);
    let image = auth_svg_icon_image(bytes, color)?;
    let handle = images.add(image);
    cache.handles_by_key.insert(key, handle.clone());
    Some(handle)
}

fn auth_svg_icon_bytes(kind: AuthUiSvgIconKind) -> (&'static [u8], &'static str) {
    match kind {
        AuthUiSvgIconKind::CircleAlert => (
            include_bytes!("../../../../../data/icons/circle-alert.svg"),
            "embedded-auth-circle-alert.svg",
        ),
        AuthUiSvgIconKind::Email => (
            include_bytes!("../../../../../data/icons/email.svg"),
            "embedded-auth-email.svg",
        ),
        AuthUiSvgIconKind::Password => (
            include_bytes!("../../../../../data/icons/password.svg"),
            "embedded-auth-password.svg",
        ),
        AuthUiSvgIconKind::Eye => (
            include_bytes!("../../../../../data/icons/eye.svg"),
            "embedded-auth-eye.svg",
        ),
        AuthUiSvgIconKind::EyeOff => (
            include_bytes!("../../../../../data/icons/eye-off.svg"),
            "embedded-auth-eye-off.svg",
        ),
    }
}

fn auth_svg_icon_image(bytes: &[u8], color: Color) -> Option<Image> {
    let source = std::str::from_utf8(bytes)
        .ok()?
        .replace("currentColor", &color_to_svg_hex(color));
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(source.as_bytes(), &options).ok()?;
    let size = tree.size();
    let natural_width = size.width().max(1.0);
    let natural_height = size.height().max(1.0);
    let scale = AUTH_INPUT_ICON_RASTER_PX as f32 / natural_width.max(natural_height);
    let width = (natural_width * scale).ceil().max(1.0) as u32;
    let height = (natural_height * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut data = pixmap.data().to_vec();
    demultiply_rgba(&mut data);
    Some(Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    ))
}

fn color_to_svg_hex(color: Color) -> String {
    let srgba = color.to_srgba();
    let r = (srgba.red.clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (srgba.green.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (srgba.blue.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn demultiply_rgba(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * 255) / alpha).min(255) as u8;
        }
    }
}

