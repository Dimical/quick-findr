#![windows_subsystem = "windows"] // Cache la console au lancement

mod engine; // Import du module engine.rs
mod favorites; // Import du module favorites.rs

use slint::{VecModel, ComponentHandle};
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::process::Command;
use engine::SearchResult as EngineSearchResult; // Assurez-vous que engine.rs expose ces types
use favorites::FavoritesManager;

#[cfg(target_os = "windows")]
use window_vibrancy::apply_mica;
#[cfg(target_os = "windows")]
use i_slint_backend_winit::WinitWindowAccessor;

// Import des composants générés par Slint
slint::include_modules!();

// Gestionnaire d'état global au thread UI pour le modèle de données
thread_local! {
    static RESULTS_MODEL: RefCell<Rc<VecModel<SearchResult>>> = RefCell::new(Rc::new(VecModel::default()));
    static REMAINING_RESULTS: RefCell<Vec<SearchResult>> = RefCell::new(Vec::new());
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = AppWindow::new()?;
    let window_weak = main_window.as_weak();

    // Détection du thème système (Dark/Light)
    let mode = dark_light::detect();
    let is_dark = match mode {
        dark_light::Mode::Dark => true,
        dark_light::Mode::Light => false,
        _ => true, // Default to Dark
    };
    main_window.set_dark_mode(is_dark);

    // --- Windows 11 Mica Effect Integration ---
    #[cfg(target_os = "windows")]
    {
        let _ = WinitWindowAccessor::with_winit_window(main_window.window(), |winit_window| {
            let _ = apply_mica(winit_window, Some(is_dark));
        });
    }
    // ------------------------------------------

    // Initialisation du path par défaut (home directory)
    let home_dir = dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    main_window.set_current_path(home_dir.to_string_lossy().to_string().into());
    
    // État partagé pour le dossier de recherche
    let search_path = Rc::new(RefCell::new(home_dir.clone()));
    
    // Chargement des favoris
    let favorites_manager = Rc::new(RefCell::new(FavoritesManager::load()));
    
    // Initialisation de l'UI avec les favoris
    {
        let manager = favorites_manager.borrow();
        let fav_vec: Vec<FavoriteFolder> = manager.favorites.iter().map(|f| {
            FavoriteFolder {
                path: f.path.clone().into(),
                name: f.name.clone().into(),
                is_favorite: true,
            }
        }).collect();
        main_window.set_favorites(slint::ModelRc::new(slint::VecModel::from(fav_vec)));
        
        let recent_vec: Vec<FavoriteFolder> = manager.recent_folders.iter().map(|f| {
            FavoriteFolder {
                path: f.path.clone().into(),
                name: f.name.clone().into(),
                is_favorite: false,
            }
        }).collect();
        main_window.set_recent_folders(slint::ModelRc::new(slint::VecModel::from(recent_vec)));
    }
    
    // Ajouter le dossier courant aux récents
    favorites_manager.borrow_mut().add_recent(home_dir.to_string_lossy().to_string());

    // 1. Initialisation du modèle de données
    RESULTS_MODEL.with(|model| {
        main_window.set_results(model.borrow().clone().into());
    });

    // Flag atomique pour stopper un scan en cours
    let is_searching = Arc::new(AtomicBool::new(false));

    // 2. Binding : Sélection du dossier
    main_window.on_select_directory({
        let window_weak = window_weak.clone();
        let search_path = search_path.clone();
        let favorites_manager = favorites_manager.clone();
        move || {
            let window = window_weak.unwrap();
            // Ouvre la boîte de dialogue native
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                let path_str = folder.to_string_lossy().to_string();
                window.set_current_path(path_str.clone().into());
                *search_path.borrow_mut() = folder;
                
                // Ajouter aux récents
                favorites_manager.borrow_mut().add_recent(path_str.clone());
                
                // Mettre à jour l'UI
                let manager = favorites_manager.borrow();
                let recent_vec: Vec<FavoriteFolder> = manager.recent_folders.iter().map(|f| {
                    FavoriteFolder {
                        path: f.path.clone().into(),
                        name: f.name.clone().into(),
                        is_favorite: false,
                    }
                }).collect();
                window.set_recent_folders(slint::ModelRc::new(slint::VecModel::from(recent_vec)));
            }
        }
    });

    // 3. Binding : Lancement de la recherche
    main_window.on_request_search({
        let window_weak = window_weak.clone();
        let is_searching = is_searching.clone();
        let search_path = search_path.clone();
        
        move |query, case_sensitive, use_regex, search_content, respect_gitignore, exclude_extensions, language_filter| {
            let window = window_weak.unwrap();
            
            // Nettoyage de l'UI avant nouveau scan
            RESULTS_MODEL.with(|model| model.borrow().set_vec(vec![]));
            window.set_total_results(0);
            window.set_status_text("Scan en cours...".into());
            window.set_active_threads(num_cpus::get() as i32);

            // Gestion de l'état "Searching" (Stop previous if any)
            is_searching.store(true, Ordering::Relaxed);

            // Lancement du moteur (Engine)
            let path = search_path.borrow().clone();
            engine::spawn_search(
                query.into(), 
                path, 
                window_weak.clone(), 
                is_searching.clone(),
                case_sensitive,
                use_regex,
                search_content,
                respect_gitignore,
                exclude_extensions.into(),
                if language_filter.is_empty() { None } else { Some(language_filter.to_string()) }
            );
        }
    });

    // 4. Binding : Ouverture de fichier (Double-click / Entrée)
    main_window.on_open_item(|item| {
        let _ = Command::new("cmd")
            .args(["/C", "start", "", &item.file_path])
            .spawn();
    });

    // 5. Binding : Ouvrir le dossier (Ctrl + O)
    main_window.on_open_item_folder(|item| {
        let _ = Command::new("explorer")
            .args(["/select,", &item.file_path])
            .spawn();
    });

    // 6. Binding : Copie dans le presse-papier (Ctrl + C - Default to absolute)
    main_window.on_copy_item_path(|item| {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(item.file_path.to_string());
        }
    });

    // 7. Bindings : Context Menu Actions
    main_window.on_copy_absolute_path(|item| {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(item.file_path.to_string());
        }
    });

    main_window.on_copy_relative_path(|item| {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(item.relative_path.to_string());
        }
    });

    main_window.on_copy_filename(|item| {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(item.file_name.to_string());
        }
    });

    // 8. Binding : Effacer la recherche
    main_window.on_clear_search({
        let window_weak = window_weak.clone();
        move || {
            RESULTS_MODEL.with(|model| model.borrow().set_vec(vec![]));
            REMAINING_RESULTS.with(|remaining| *remaining.borrow_mut() = vec![]);
            if let Some(window) = window_weak.upgrade() {
                window.set_total_results(0);
                window.set_status_text("Prêt".into());
                window.set_active_threads(0);
            }
        }
    });

    // 9. Binding : Charger plus de résultats
    main_window.on_load_more_results({
        move || {
            REMAINING_RESULTS.with(|remaining| {
                let mut remaining_vec = remaining.borrow_mut();
                if remaining_vec.is_empty() {
                    return;
                }
                let batch_size = 50;
                let min_count = std::cmp::min(batch_size, remaining_vec.len());
                let batch: Vec<SearchResult> = remaining_vec.drain(0..min_count).collect();
                RESULTS_MODEL.with(|model| {
                    let model_ref = model.borrow_mut();
                    for result in batch {
                        model_ref.push(result);
                    }
                });
            });
        }
    });

    // 10. Binding : Sélectionner un favori
    main_window.on_select_favorite({
        let window_weak = window_weak.clone();
        let search_path = search_path.clone();
        let favorites_manager = favorites_manager.clone();
        move |path_str| {
            let window = window_weak.unwrap();
            let path = std::path::PathBuf::from(path_str.as_str());
            window.set_current_path(path_str.clone());
            *search_path.borrow_mut() = path;
            
            // Mettre à jour last_used
            favorites_manager.borrow_mut().update_last_used(path_str.as_str());
        }
    });

    // 11. Binding : Ajouter aux favoris
    main_window.on_add_to_favorites({
        let window_weak = window_weak.clone();
        let search_path = search_path.clone();
        let favorites_manager = favorites_manager.clone();
        move || {
            let window = window_weak.unwrap();
            let path = search_path.borrow().clone();
            let path_str = path.to_string_lossy().to_string();
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path_str)
                .to_string();
            
            favorites_manager.borrow_mut().add_favorite(path_str, name);
            
            // Mettre à jour l'UI
            let manager = favorites_manager.borrow();
            let fav_vec: Vec<FavoriteFolder> = manager.favorites.iter().map(|f| {
                FavoriteFolder {
                    path: f.path.clone().into(),
                    name: f.name.clone().into(),
                    is_favorite: true,
                }
            }).collect();
            window.set_favorites(slint::ModelRc::new(slint::VecModel::from(fav_vec)));
        }
    });

    // 12. Binding : Retirer des favoris
    main_window.on_remove_from_favorites({
        let window_weak = window_weak.clone();
        let favorites_manager = favorites_manager.clone();
        move |path_str| {
            println!("Tentative de suppression du favori: {}", path_str);
            let window = window_weak.unwrap();
            
            // Vérifier si le favori existe avant suppression
            let before_count = favorites_manager.borrow().favorites.len();
            favorites_manager.borrow_mut().remove_favorite(path_str.as_str());
            let after_count = favorites_manager.borrow().favorites.len();
            
            println!("Favoris avant: {}, après: {}", before_count, after_count);
            
            // Mettre à jour l'UI
            let manager = favorites_manager.borrow();
            let fav_vec: Vec<FavoriteFolder> = manager.favorites.iter().map(|f| {
                FavoriteFolder {
                    path: f.path.clone().into(),
                    name: f.name.clone().into(),
                    is_favorite: true,
                }
            }).collect();
            let fav_count = fav_vec.len();
            window.set_favorites(slint::ModelRc::new(slint::VecModel::from(fav_vec)));
            println!("UI mise à jour avec {} favoris", fav_count);
        }
    });

    main_window.run()
}

// ...
// Doit être publique pour être accessible par le module engine
pub fn add_result_to_ui(_window: &AppWindow, result: EngineSearchResult) {
    let color = get_icon_color(&result.extension);

    // Conversion du résultat Rust vers le struct Slint
    let ui_result = SearchResult {
        file_name: result.file_name.into(),
        file_path: result.file_path.into(),
        relative_path: result.relative_path.into(),
        extension: result.extension.into(),
        line_match: result.line_match.into(),
        icon_color: color,
    };

    // Ajout au modèle (Thread-Local permet l'accès safe)
    RESULTS_MODEL.with(|model| {
        model.borrow_mut().push(ui_result);
    });
}

// Fonction utilitaire appelée par engine.rs pour ajouter un lot de résultats
pub fn add_results_batch_to_ui(_window: &AppWindow, results: Vec<EngineSearchResult>) {
    RESULTS_MODEL.with(|model| {
        let model_ref = model.borrow_mut();
        for result in results {
            let color = get_icon_color(&result.extension);
            let ui_result = SearchResult {
                file_name: result.file_name.into(),
                file_path: result.file_path.into(),
                relative_path: result.relative_path.into(),
                extension: result.extension.into(),
                line_match: result.line_match.into(),
                icon_color: color,
            };
            model_ref.push(ui_result);
        }
    });
}

// Fonction pour stocker les résultats restants
pub fn set_remaining_results(results: Vec<EngineSearchResult>) {
    REMAINING_RESULTS.with(|remaining| {
        *remaining.borrow_mut() = results.into_iter().map(|r| {
            let color = get_icon_color(&r.extension);
            SearchResult {
                file_name: r.file_name.into(),
                file_path: r.file_path.into(),
                relative_path: r.relative_path.into(),
                extension: r.extension.into(),
                line_match: r.line_match.into(),
                icon_color: color,
            }
        }).collect();
    });
}

fn get_icon_color(extension: &str) -> slint::Color {
    match extension.to_lowercase().as_str() {
        "rs" => slint::Color::from_rgb_u8(222, 165, 132), // Rust
        "js" | "ts" | "jsx" | "tsx" => slint::Color::from_rgb_u8(241, 224, 90), // JS/TS
        "html" | "css" | "scss" => slint::Color::from_rgb_u8(227, 76, 38), // Web
        "json" | "toml" | "yaml" | "yml" => slint::Color::from_rgb_u8(133, 76, 199), // Config
        "md" | "txt" => slint::Color::from_rgb_u8(0, 122, 204), // Docs
        "pdf" => slint::Color::from_rgb_u8(180, 15, 15), // PDF
        "zip" | "tar" | "gz" => slint::Color::from_rgb_u8(255, 200, 0), // Archive
        "png" | "jpg" | "jpeg" | "svg" => slint::Color::from_rgb_u8(100, 200, 100), // Images
        "java" | "kt" => slint::Color::from_rgb_u8(180, 100, 50), // JVM
        "py" => slint::Color::from_rgb_u8(53, 114, 165), // Python
        "c" | "cpp" | "h" => slint::Color::from_rgb_u8(85, 85, 85), // C/C++
        "exe" | "dll" | "bat" | "ps1" => slint::Color::from_rgb_u8(0, 120, 212), // System
        _ => slint::Color::from_rgb_u8(128, 128, 128), // Default
    }
}