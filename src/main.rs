#![windows_subsystem = "windows"]

mod engine;
mod favorites;

use engine::SearchResult as EngineSearchResult;
use favorites::FavoritesManager;
use slint::{ComponentHandle, VecModel};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[cfg(target_os = "windows")]
use i_slint_backend_winit::WinitWindowAccessor;
#[cfg(target_os = "windows")]
use window_vibrancy::apply_mica;

slint::include_modules!();

// UI-thread models: Slint models are not Send/Sync; keep them on the UI thread.
thread_local! {
    static RESULTS_MODEL: RefCell<Rc<VecModel<SearchResult>>> = RefCell::new(Rc::new(VecModel::default()));
    static REMAINING_RESULTS: RefCell<Vec<SearchResult>> = RefCell::new(Vec::new());
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = AppWindow::new()?;
    let window_weak = main_window.as_weak();

    // System theme detection (Dark/Light).
    let mode = dark_light::detect();

    let is_dark = match mode {
        dark_light::Mode::Dark => true,
        dark_light::Mode::Light => false,
        _ => true, // Default to Dark
    };
    main_window.set_dark_mode(is_dark);

    // Windows 11 Mica effect.
    #[cfg(target_os = "windows")]
    {
        let _ = WinitWindowAccessor::with_winit_window(main_window.window(), |winit_window| {
            let _ = apply_mica(winit_window, Some(is_dark));
        });
    }

    // Default search directory: user's home.
    let home_dir = dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    main_window.set_current_path(home_dir.to_string_lossy().to_string().into());

    // Shared selected directory (UI updates this when you pick a folder).
    let search_path = Rc::new(RefCell::new(home_dir.clone()));

    // Favorites/recents persistence.
    let favorites_manager = Rc::new(RefCell::new(FavoritesManager::load()));

    // Populate UI models with persisted favorites/recents.
    {
        let manager = favorites_manager.borrow();

        let fav_vec: Vec<FavoriteFolder> = manager
            .favorites
            .iter()
            .map(|f| FavoriteFolder {
                path: f.path.clone().into(),
                name: f.name.clone().into(),
                is_favorite: true,
            })
            .collect();
        main_window.set_favorites(slint::ModelRc::new(slint::VecModel::from(fav_vec)));

        let recent_vec: Vec<FavoriteFolder> = manager
            .recent_folders
            .iter()
            .map(|f| FavoriteFolder {
                path: f.path.clone().into(),
                name: f.name.clone().into(),
                is_favorite: false,
            })
            .collect();
        main_window.set_recent_folders(slint::ModelRc::new(slint::VecModel::from(recent_vec)));
    }

    // Track app start directory in recents.
    favorites_manager
        .borrow_mut()
        .add_recent(home_dir.to_string_lossy().to_string());

    // Attach the results model to the UI.
    RESULTS_MODEL.with(|model| {
        main_window.set_results(model.borrow().clone().into());
    });

    // Cancel flag for background search threads.
    let is_searching = Arc::new(AtomicBool::new(false));

    // Folder picker.
    main_window.on_select_directory({
        let window_weak = window_weak.clone();
        let search_path = search_path.clone();
        let favorites_manager = favorites_manager.clone();
        move || {
            let window = window_weak.unwrap();
            // Open native folder picker.
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                let path_str = folder.to_string_lossy().to_string();
                window.set_current_path(path_str.clone().into());
                *search_path.borrow_mut() = folder;

                // Persist in recents.
                favorites_manager.borrow_mut().add_recent(path_str.clone());

                // Refresh UI model.
                let manager = favorites_manager.borrow();

                let recent_vec: Vec<FavoriteFolder> = manager
                    .recent_folders
                    .iter()
                    .map(|f| FavoriteFolder {
                        path: f.path.clone().into(),
                        name: f.name.clone().into(),
                        is_favorite: false,
                    })
                    .collect();
                window.set_recent_folders(slint::ModelRc::new(slint::VecModel::from(recent_vec)));
            }
        }
    });

    // Start search.
    main_window.on_request_search({
        let window_weak = window_weak.clone();
        let is_searching = is_searching.clone();
        let search_path = search_path.clone();

        move |query,
              case_sensitive,
              use_regex,
              search_content,
              respect_gitignore,
              exclude_extensions,
              language_filter| {
            let window = window_weak.unwrap();

            // Clear UI state for a new scan.
            RESULTS_MODEL.with(|model| model.borrow().set_vec(vec![]));

            window.set_total_results(0);
            window.set_status_text("Scan en cours...".into());
            window.set_active_threads(num_cpus::get() as i32);

            // Mark search as active (used by worker threads to stop early).
            is_searching.store(true, Ordering::Relaxed);

            // Spawn the search worker.
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
                if language_filter.is_empty() {
                    None
                } else {
                    Some(language_filter.to_string())
                },
            );
        }
    });

    // Open a file.
    main_window.on_open_item(|item| {
        let _ = Command::new("cmd")
            .args(["/C", "start", "", &item.file_path])
            .spawn();
    });

    // Reveal in Explorer.
    main_window.on_open_item_folder(|item| {
        let _ = Command::new("explorer")
            .args(["/select,", &item.file_path])
            .spawn();
    });

    // Copy absolute path.
    main_window.on_copy_item_path(|item| {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(item.file_path.to_string());
        }
    });

    // Context menu actions.
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

    // Reset UI.
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

    // Pagination: append a batch of results.
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

    // Favorites: selecting an entry updates the current search path.
    main_window.on_select_favorite({
        let window_weak = window_weak.clone();
        let search_path = search_path.clone();
        let favorites_manager = favorites_manager.clone();
        move |path_str| {
            let window = window_weak.unwrap();
            let path = std::path::PathBuf::from(path_str.as_str());
            window.set_current_path(path_str.clone());
            *search_path.borrow_mut() = path;

            // Persist last_used for sorting/recents.
            favorites_manager
                .borrow_mut()
                .update_last_used(path_str.as_str());
        }
    });

    // Favorites: add current folder.
    main_window.on_add_to_favorites({
        let window_weak = window_weak.clone();
        let search_path = search_path.clone();
        let favorites_manager = favorites_manager.clone();
        move || {
            let window = window_weak.unwrap();
            let path = search_path.borrow().clone();
            let path_str = path.to_string_lossy().to_string();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path_str)
                .to_string();

            favorites_manager.borrow_mut().add_favorite(path_str, name);

            // Refresh favorites model in the UI.
            let manager = favorites_manager.borrow();
            let fav_vec: Vec<FavoriteFolder> = manager
                .favorites
                .iter()
                .map(|f| FavoriteFolder {
                    path: f.path.clone().into(),
                    name: f.name.clone().into(),
                    is_favorite: true,
                })
                .collect();

            window.set_favorites(slint::ModelRc::new(slint::VecModel::from(fav_vec)));
        }
    });

    // Favorites: remove selected entry.
    main_window.on_remove_from_favorites({
        let window_weak = window_weak.clone();
        let favorites_manager = favorites_manager.clone();
        move |path_str| {
            let window = window_weak.unwrap();
            favorites_manager
                .borrow_mut()
                .remove_favorite(path_str.as_str());

            // Refresh favorites model in the UI.
            let manager = favorites_manager.borrow();
            let fav_vec: Vec<FavoriteFolder> = manager
                .favorites
                .iter()
                .map(|f| FavoriteFolder {
                    path: f.path.clone().into(),
                    name: f.name.clone().into(),
                    is_favorite: true,
                })
                .collect();
            window.set_favorites(slint::ModelRc::new(slint::VecModel::from(fav_vec)));
        }
    });

    main_window.run()
}

// Helpers called by `engine.rs` via `slint::invoke_from_event_loop`.
pub fn add_result_to_ui(_window: &AppWindow, result: EngineSearchResult) {
    let color = get_icon_color(&result.extension);

    // Convert engine result to the Slint struct.
    let ui_result = SearchResult {
        file_name: result.file_name.into(),
        file_path: result.file_path.into(),
        relative_path: result.relative_path.into(),
        extension: result.extension.into(),
        line_match: result.line_match.into(),
        icon_color: color,
    };

    // Push into the UI-thread model.
    RESULTS_MODEL.with(|model| {
        model.borrow_mut().push(ui_result);
    });
}

// Batch insert to reduce event-loop calls.
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

// Store remaining results for pagination ("Load more").
pub fn set_remaining_results(results: Vec<EngineSearchResult>) {
    REMAINING_RESULTS.with(|remaining| {
        *remaining.borrow_mut() = results
            .into_iter()
            .map(|r| {
                let color = get_icon_color(&r.extension);
                SearchResult {
                    file_name: r.file_name.into(),
                    file_path: r.file_path.into(),
                    relative_path: r.relative_path.into(),
                    extension: r.extension.into(),
                    line_match: r.line_match.into(),
                    icon_color: color,
                }
            })
            .collect();
    });
}

fn get_icon_color(extension: &str) -> slint::Color {
    match extension.to_lowercase().as_str() {
        "rs" => slint::Color::from_rgb_u8(222, 165, 132), // Rust
        "js" | "ts" | "jsx" | "tsx" => slint::Color::from_rgb_u8(241, 224, 90), // JS/TS
        "html" | "css" | "scss" => slint::Color::from_rgb_u8(227, 76, 38), // Web
        "json" | "toml" | "yaml" | "yml" => slint::Color::from_rgb_u8(133, 76, 199), // Config
        "md" | "txt" => slint::Color::from_rgb_u8(0, 122, 204), // Docs
        "pdf" => slint::Color::from_rgb_u8(180, 15, 15),  // PDF
        "zip" | "tar" | "gz" => slint::Color::from_rgb_u8(255, 200, 0), // Archive
        "png" | "jpg" | "jpeg" | "svg" => slint::Color::from_rgb_u8(100, 200, 100), // Images
        "java" | "kt" => slint::Color::from_rgb_u8(180, 100, 50), // JVM
        "py" => slint::Color::from_rgb_u8(53, 114, 165),  // Python
        "c" | "cpp" | "h" => slint::Color::from_rgb_u8(85, 85, 85), // C/C++
        "exe" | "dll" | "bat" | "ps1" => slint::Color::from_rgb_u8(0, 120, 212), // System
        _ => slint::Color::from_rgb_u8(128, 128, 128),    // Default
    }
}
