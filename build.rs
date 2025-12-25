fn main() {
    slint_build::compile("ui/app_window.slint").unwrap();

    // Windows icon configuration (optional)
    #[cfg(target_os = "windows")]
    {
        use std::path::Path;
        let icon_path = "assets/icon.ico";

        if Path::new(icon_path).exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon(icon_path);
            if let Err(e) = res.compile() {
                println!("cargo:warning=Unable to compile icon: {}", e);
            }
        } else {
            println!("cargo:warning=Icon not found: {}. Application will be compiled without custom icon.", icon_path);
        }
    }
}
