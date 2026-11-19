fn main() {
    println!("cargo:rerun-if-changed=assets/rocket.png");

    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;

        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        let ico_path = out_dir.join("app.ico");
        write_ico("assets/rocket.png", &ico_path);

        let mut resource = winres::WindowsResource::new();
        resource.set_icon(ico_path.to_str().expect("icon path must be valid UTF-8"));
        resource.compile().expect("failed to compile Windows resources");
    }
}

#[cfg(target_os = "windows")]
fn write_ico(png_path: &str, ico_path: &std::path::Path) {
    use std::{fs::File, io::BufWriter};

    let image = image::open(png_path).expect("failed to open assets/rocket.png");
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for size in [16u32, 24, 32, 48, 64, 128, 256] {
        let resized = image.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let icon_image =
            ico::IconImage::from_rgba_data(size, size, rgba.into_raw());
        let entry = ico::IconDirEntry::encode(&icon_image).expect("failed to encode icon");
        icon_dir.add_entry(entry);
    }

    let file = File::create(ico_path).expect("failed to create app.ico");
    icon_dir
        .write(BufWriter::new(file))
        .expect("failed to write app.ico");
}
