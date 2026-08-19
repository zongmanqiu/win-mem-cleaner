use std::fs;

fn main() {
    if std::env::var("CARGO_CFG_WINDOWS").is_err() {
        return;
    }
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=image/logo.png");

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

    // ---- 2. 用 winres 嵌入 manifest + 应用图标 ----
    let mut res = winres::WindowsResource::new();
    
    // 测试构建时跳过 manifest 嵌入（避免 requireAdministrator 导致测试无法运行）
    let is_test = std::env::var("CARGO_CFG_DEBUG_ASSERTIONS").is_ok();
    if !is_test {
        res.set_manifest_file("app.manifest");
    }
    
    res.set_icon(&ico_path);
    
    // 设置 VERSIONINFO 资源，让任务管理器显示正确的程序名称
    res.set("FileDescription", "WinMemCleaner - 轻量内存清理工具");
    res.set("ProductName", "WinMemCleaner");
    res.set("ProductVersion", "1.0.0");
    res.set("FileVersion", "1.0.0");
    res.set("CompanyName", "邱宗满");
    res.set("OriginalFilename", "WinMemCleaner_x64.exe");
    res.set("InternalName", "WinMemCleaner");
    
    res.compile().expect("winres compile failed");
}
