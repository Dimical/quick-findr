fn main() {
    slint_build::compile("ui/app_window.slint").unwrap();

    // Configuration de l'icône Windows (optionnel)
    #[cfg(target_os = "windows")]
    {
        use std::path::Path;
        let icon_path = "assets/icon.png";

        if Path::new(icon_path).exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon(icon_path);
            if let Err(e) = res.compile() {
                println!("cargo:warning=Impossible de compiler l'icône: {}", e);
            }
        } else {
            println!("cargo:warning=Icône non trouvée: {}. L'application sera compilée sans icône personnalisée.", icon_path);
        }
    }
}
