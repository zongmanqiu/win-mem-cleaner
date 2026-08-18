use std::fs;

fn main() {
    if std::env::var("CARGO_CFG_WINDOWS").is_err() {
        return;
    }
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=image/logo.png");
    println!("cargo:rerun-if-changed=image/WeChatPay.svg");
    println!("cargo:rerun-if-changed=image/ALiPay.svg");

    let out_dir = std::env::var("OUT_DIR").unwrap();

    // ---- 1. 从 PNG 生成多尺寸 ICO ----
    let ico_path = format!("{out_dir}\\logo.ico");
    let img = image::open("image/logo.png").expect("failed to open image/logo.png");
    let sizes: &[u32] = &[16, 32, 48, 256];
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in sizes {
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let icon_image = ico::IconImage::from_rgba_data(size, size, rgba.into_raw());
        let entry = ico::IconDirEntry::encode(&icon_image).expect("failed to encode icon entry");
        icon_dir.add_entry(entry);
    }
    let mut ico_file = fs::File::create(&ico_path).expect("failed to create ICO");
    icon_dir.write(&mut ico_file).expect("failed to write ICO");

    // ---- 2. 将 SVG 二维码转为 PNG（供 include_bytes! 使用）----
    let qr_size = 200u32;
    for (name, file) in &[("WeChatPay", "image/WeChatPay.svg"), ("ALiPay", "image/ALiPay.svg")] {
        let svg_data = fs::read(file).unwrap_or_else(|e| panic!("failed to read {file}: {e}"));
        let svg_str = std::str::from_utf8(&svg_data).unwrap_or_else(|e| panic!("invalid UTF-8 in {file}: {e}"));
        let tree = usvg::Tree::from_str(svg_str, &usvg::Options::default())
            .unwrap_or_else(|e| panic!("failed to parse SVG {file}: {e}"));
        let pixmap_size = tree.size().to_int_size();
        let scale = qr_size as f32 / pixmap_size.width().max(pixmap_size.height()) as f32;
        let transform = tiny_skia::Transform::from_scale(scale, scale);
        let mut pixmap = tiny_skia::Pixmap::new(qr_size, qr_size).unwrap();
        pixmap.fill(tiny_skia::Color::WHITE);
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        let png_bytes = pixmap.encode_png().unwrap();
        let png_path = format!("image\\{name}.png");
        fs::write(&png_path, &png_bytes).unwrap();
    }

    // ---- 3. 用 winres 嵌入 manifest + 应用图标 ----
    // 测试构建时跳过 manifest 嵌入（避免 requireAdministrator 导致测试无法运行）
    let is_test = std::env::var("CARGO_CFG_DEBUG_ASSERTIONS").is_ok();
    if !is_test {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("app.manifest");
        res.set_icon(&ico_path);
        res.compile().expect("winres compile failed");
    }
}
