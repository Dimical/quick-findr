use ignore::WalkBuilder;
use rayon::prelude::*;
use regex::RegexBuilder;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

// Result sent to UI thread (main.rs) via invoke_from_event_loop.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_name: String,
    pub file_path: String,
    pub relative_path: String,
    pub extension: String,
    pub line_match: String, // Empty if match on filename
}

pub struct SearchContext {
    pub query: String,
    pub query_lower: String,
    pub regex: Option<regex::Regex>,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub search_content: bool,
    pub root_path: PathBuf,
    pub exclude_extensions: Vec<String>,
    pub respect_gitignore: bool,
}

impl SearchContext {
    // Creates a new search context.
    pub fn new(
        query: String,
        case_sensitive: bool,
        use_regex: bool,
        search_content: bool,
        root_path: PathBuf,
        exclude_extensions: String,
        respect_gitignore: bool,
        _language_filter: Option<String>,
    ) -> Option<Self> {
        // Wildcards (*, ?) enable regex mode for convenience.
        let has_wildcards = query.contains('*') || query.contains('?');
        let should_use_regex = use_regex || has_wildcards;

        let regex = if should_use_regex {
            let pattern = if has_wildcards && !use_regex {
                // Convert wildcards to regex (escaping the rest).
                let escaped = regex::escape(&query);
                let pattern = escaped
                    .replace(r"\*", ".*") // * becomes .*
                    .replace(r"\?", "."); // ? becomes .
                format!("^{}$", pattern)
            } else {
                query.clone()
            };

            match RegexBuilder::new(&pattern)
                .case_insensitive(!case_sensitive)
                .build()
            {
                Ok(re) => Some(re),
                Err(_) => return None, // Invalid regex
            }
        } else {
            None
        };

        let exclude_list: Vec<String> = exclude_extensions
            .split(',')
            .map(|s| {
                let trimmed = s.trim();
                // Keep dot if present, otherwise add it
                if trimmed.starts_with('.') {
                    trimmed.to_lowercase()
                } else if !trimmed.is_empty() {
                    format!(".{}", trimmed.to_lowercase())
                } else {
                    String::new()
                }
            })
            .filter(|s| !s.is_empty())
            .collect();

        Some(Self {
            query: query.clone(),
            query_lower: query.to_lowercase(),
            regex,
            case_sensitive,
            use_regex: should_use_regex,
            search_content,
            root_path,
            exclude_extensions: exclude_list,
            respect_gitignore,
        })
    }

    // Checks if the given string matches the query.
    pub fn is_match(&self, text: &str) -> bool {
        if self.use_regex {
            if let Some(re) = &self.regex {
                return re.is_match(text);
            }
            return false;
        }

        // CamelCase matching: if query is uppercase/digits (e.g., "UC"), match uppercase letters in name.
        if self.is_camelcase_query() {
            if self.camelcase_match(text) {
                return true;
            }
        }

        // Substring matching.
        if self.case_sensitive {
            text.contains(&self.query)
        } else {
            text.to_lowercase().contains(&self.query_lower)
        }
    }

    // Checks if the query is a CamelCase pattern (e.g., "UC", "UCS", "U2C")
    fn is_camelcase_query(&self) -> bool {
        // CamelCase pattern: at least 2 characters, all uppercase or digits
        self.query.len() >= 2
            && self
                .query
                .chars()
                .all(|c| c.is_uppercase() || c.is_numeric())
    }

    // CamelCase matching: "UC" matches "UserController", "U2C" matches "User2Controller"
    fn camelcase_match(&self, text: &str) -> bool {
        let query_chars: Vec<char> = self.query.chars().collect();
        let mut query_idx = 0;

        for ch in text.chars() {
            if query_idx >= query_chars.len() {
                return true;
            }

            // Match uppercase letters and digits from query with those in text
            if (ch.is_uppercase() || ch.is_numeric()) && ch == query_chars[query_idx] {
                query_idx += 1;
            }
        }

        query_idx >= query_chars.len()
    }
}

// Default exclusions (keeps scan fast and avoids build/vendor directories).
const IGNORED_DIRS: &[&str] = &[
    "target",
    ".git",
    "node_modules",
    "vendor",
    ".idea",
    ".vscode",
];

// Spawns a search in a separate thread.
pub fn spawn_search(
    query: String,
    root_path: PathBuf,
    sender: slint::Weak<crate::AppWindow>,
    is_searching: Arc<AtomicBool>,
    case_sensitive: bool,
    use_regex: bool,
    search_content: bool,
    respect_gitignore: bool,
    exclude_extensions: String,
    language_filter: Option<String>,
) {
    let root_path_clone = root_path.clone();
    std::thread::spawn(move || {
        let start_time = Instant::now();

        // Context preparation (regex compilation, exclusion parsing, etc.)
        let context = match SearchContext::new(
            query,
            case_sensitive,
            use_regex,
            search_content,
            root_path_clone.clone(),
            exclude_extensions,
            respect_gitignore,
            language_filter,
        ) {
            Some(ctx) => ctx,
            None => {
                let _ = slint::invoke_from_event_loop({
                    let sender_clone = sender.clone();
                    move || {
                        if let Some(window) = sender_clone.upgrade() {
                            window.set_status_text("Error: Invalid regular expression".into());
                            window.set_active_threads(0);
                        }
                    }
                });
                return;
            }
        };

        // Pipeline: WalkBuilder -> process_file() -> results -> push UI.
        let mut builder = WalkBuilder::new(&root_path);
        builder
            .hidden(true)
            .git_ignore(context.respect_gitignore)
            .threads(num_cpus::get());

        // Manual exclusions.
        for dir in IGNORED_DIRS {
            builder.add_ignore(format!("**/{}/**", dir));
        }

        // Pre-filtering by excluded extensions.
        if !context.exclude_extensions.is_empty() {
            for ext in context.exclude_extensions.iter() {
                if !ext.is_empty() {
                    builder.add_ignore(format!("**/*{}", ext));
                }
            }
        }

        let all_results: Vec<SearchResult> = builder
            .build()
            .par_bridge()
            .filter_map(|entry| {
                if !is_searching.load(Ordering::Relaxed) {
                    return None;
                }

                match entry {
                    Ok(dir_entry) => {
                        let path = dir_entry.path();

                        if path.is_file() {
                            process_file(path, &context)
                        } else {
                            None
                        }
                    }
                    Err(err) => {
                        eprintln!("Access error: {}", err);
                        None
                    }
                }
            })
            .collect();

        // 3. Send results in pages (pagination)
        let total_results_count = all_results.len();
        let page_size = 50;
        let first_batch: Vec<SearchResult> = all_results.iter().take(page_size).cloned().collect();
        let remaining: Vec<SearchResult> = all_results.iter().skip(page_size).cloned().collect();

        let _ = slint::invoke_from_event_loop({
            let sender_clone = sender.clone();
            let first_batch_clone = first_batch.clone();
            let remaining_clone = remaining.clone();
            move || {
                if let Some(window) = sender_clone.upgrade() {
                    // Note: These functions are implemented in main.rs
                    #[cfg(not(test))]
                    {
                        crate::add_results_batch_to_ui(&window, first_batch_clone);
                        crate::set_remaining_results(remaining_clone);
                        window.set_total_results(total_results_count as i32);
                    }
                }
            }
        });

        // 4. End of scan
        let duration = start_time.elapsed().as_millis() as u64;
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = sender.upgrade() {
                window.set_status_text(
                    format!(
                        "Completed: {} results in {}ms",
                        total_results_count, duration
                    )
                    .into(),
                );
                window.set_active_threads(0);
            }
        });
    });
}

/// Unit scan function (executed by Rayon threads)
fn process_file(path: &Path, context: &SearchContext) -> Option<SearchResult> {
    let file_name = path.file_name()?.to_string_lossy();
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Filter by excluded extension
    let ext_lower = extension.to_lowercase();
    if !context.exclude_extensions.is_empty() {
        for excluded in &context.exclude_extensions {
            if excluded.starts_with('.') && ext_lower == excluded[1..] {
                return None;
            } else if ext_lower == *excluded {
                return None;
            }
            // Support for patterns like "node_modules"
            if path.to_string_lossy().contains(excluded) {
                return None;
            }
        }
    }

    // Calculate relative path
    let relative_path = path
        .strip_prefix(&context.root_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    // A. Match on filename (Absolute priority & Fast)
    // If query contains wildcards, match on name without extension
    let match_target = if context.query.contains('*') || context.query.contains('?') {
        // For wildcards, match on name without extension (Eclipse style)
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| file_name.to_string())
    } else {
        // For normal searches, match on full name
        file_name.to_string()
    };

    if context.is_match(&match_target) {
        return Some(SearchResult {
            file_name: file_name.to_string(),
            file_path: path.to_string_lossy().to_string(),
            relative_path,
            extension: extension.clone(),
            line_match: String::new(), // No excerpt needed
        });
    }

    // If not searching content, stop here
    if !context.search_content {
        return None;
    }

    // B. Match on content (Slower, requires reading)
    // Ignore common binaries to avoid reading arbitrary data
    if is_likely_binary(&extension) {
        return None;
    }

    if let Ok(file) = File::open(path) {
        // Use BufReader for I/O performance
        let reader = BufReader::new(file);

        // Scan line by line with an index
        for (i, line) in reader.lines().enumerate() {
            if let Ok(content) = line {
                if context.is_match(&content) {
                    // Early return: stop at first match
                    return Some(SearchResult {
                        file_name: file_name.to_string(),
                        file_path: path.to_string_lossy().to_string(),
                        relative_path,
                        extension,
                        line_match: format!("L{}: {}", i + 1, content.trim()),
                    });
                }
            }
            // Safety: stop reading if file is too large or no match after N lines
            if i > 5000 {
                break;
            }
        }
    }

    None
}

/// Helper to ignore binary extensions (non-exhaustive list)
fn is_likely_binary(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "exe" | "dll" | "png" | "jpg" | "pdf" | "zip" | "class" | "jar" | "ico" | "mp3" | "mp4"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ============================================================================
    // Tests de SearchContext::new
    // ============================================================================

    #[test]
    fn test_search_context_creation_valid() {
        let ctx = SearchContext::new(
            "test".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            ".exe,.dll".to_string(),
            true,
            None,
        );
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.query, "test");
        assert_eq!(ctx.query_lower, "test");
        assert_eq!(ctx.exclude_extensions, vec![".exe", ".dll"]);
        assert!(!ctx.case_sensitive);
        assert!(!ctx.use_regex);
    }

    #[test]
    fn test_search_context_invalid_regex() {
        let ctx = SearchContext::new(
            "[invalid".to_string(),
            false,
            true,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        );
        assert!(ctx.is_none(), "Invalid regex should return None");
    }

    #[test]
    fn test_search_context_valid_regex() {
        let ctx = SearchContext::new(
            r"\d+".to_string(),
            false,
            true,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        );
        assert!(ctx.is_some(), "Valid regex should return Some");
    }

    #[test]
    fn test_exclude_extensions_parsing() {
        let ctx = SearchContext::new(
            "test".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            ".exe, .dll ,.jpg, .png".to_string(),
            true,
            None,
        )
        .unwrap();
        assert_eq!(ctx.exclude_extensions, vec![".exe", ".dll", ".jpg", ".png"]);
    }

    #[test]
    fn test_exclude_extensions_empty() {
        let ctx = SearchContext::new(
            "test".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();
        assert!(ctx.exclude_extensions.is_empty());
    }

    // ============================================================================
    // Tests de is_match - Recherche simple
    // ============================================================================

    #[test]
    fn test_is_match_case_insensitive() {
        let ctx = SearchContext::new(
            "Test".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("Test"));
        assert!(ctx.is_match("test"));
        assert!(ctx.is_match("TEST"));
        assert!(ctx.is_match("This is a Test"));
        assert!(!ctx.is_match("No match here"));
    }

    #[test]
    fn test_is_match_case_sensitive() {
        let ctx = SearchContext::new(
            "Test".to_string(),
            true,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("Test"));
        assert!(ctx.is_match("This is a Test"));
        assert!(!ctx.is_match("test"));
        assert!(!ctx.is_match("TEST"));
    }

    #[test]
    fn test_is_match_empty_query() {
        let ctx = SearchContext::new(
            "".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("anything"));
        assert!(ctx.is_match(""));
    }

    // ============================================================================
    // Tests de is_match - Regex
    // ============================================================================

    #[test]
    fn test_is_match_regex_digits() {
        let ctx = SearchContext::new(
            r"\d+".to_string(),
            false,
            true,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("123"));
        assert!(ctx.is_match("file123"));
        assert!(!ctx.is_match("abc"));
    }

    #[test]
    fn test_is_match_regex_word_boundary() {
        let ctx = SearchContext::new(
            r"\btest\b".to_string(),
            false,
            true,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("test"));
        assert!(ctx.is_match("a test file"));
        assert!(!ctx.is_match("testing"));
    }

    #[test]
    fn test_is_match_regex_case_sensitive() {
        let ctx = SearchContext::new(
            r"Test".to_string(),
            true,
            true,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("Test"));
        assert!(!ctx.is_match("test"));
    }

    #[test]
    fn test_is_match_regex_complex_pattern() {
        let ctx = SearchContext::new(
            r"(TODO|FIXME|HACK):\s*.+".to_string(),
            false,
            true,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("TODO: Fix this bug"));
        assert!(ctx.is_match("FIXME: Refactor"));
        assert!(!ctx.is_match("NOTE: This is fine"));
    }

    // ============================================================================
    // Tests Edge Cases
    // ============================================================================

    #[test]
    fn test_is_match_unicode() {
        let ctx = SearchContext::new(
            "café".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("café"));
        assert!(ctx.is_match("CAFÉ"));
    }

    #[test]
    fn test_is_match_very_long_string() {
        let ctx = SearchContext::new(
            "needle".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        let haystack = "a".repeat(10000) + "needle" + &"b".repeat(10000);
        assert!(ctx.is_match(&haystack));
    }

    #[test]
    fn test_exclude_extensions_normalization() {
        let ctx = SearchContext::new(
            "test".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            ".EXE, .DLL, .Jpg".to_string(),
            true,
            None,
        )
        .unwrap();

        assert_eq!(ctx.exclude_extensions, vec![".exe", ".dll", ".jpg"]);
    }

    #[test]
    fn test_is_likely_binary() {
        assert!(is_likely_binary("exe"));
        assert!(is_likely_binary("CLASS"));
        assert!(!is_likely_binary("txt"));
        assert!(!is_likely_binary("rs"));
    }

    #[test]
    fn test_regex_with_anchors() {
        let ctx = SearchContext::new(
            r"^test$".to_string(),
            false,
            true,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("test"));
        assert!(!ctx.is_match("test "));
        assert!(!ctx.is_match("testing"));
    }

    #[test]
    fn test_multiple_spaces_in_exclude_extensions() {
        let ctx = SearchContext::new(
            "test".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "  .exe  ,  .dll  ".to_string(),
            true,
            None,
        )
        .unwrap();

        assert_eq!(ctx.exclude_extensions, vec![".exe", ".dll"]);
    }

    #[test]
    fn test_empty_extension_in_list() {
        let ctx = SearchContext::new(
            "test".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            ".exe,,.dll".to_string(),
            true,
            None,
        )
        .unwrap();

        assert_eq!(ctx.exclude_extensions, vec![".exe", ".dll"]);
    }

    #[test]
    fn test_context_properties_preserved() {
        let ctx = SearchContext::new(
            "MyQuery".to_string(),
            true,
            false,
            true,
            PathBuf::from("/custom/path"),
            ".rs,.toml".to_string(),
            false,
            None,
        )
        .unwrap();

        assert_eq!(ctx.query, "MyQuery");
        assert_eq!(ctx.query_lower, "myquery");
        assert!(ctx.case_sensitive);
        assert!(!ctx.use_regex);
        assert!(ctx.search_content);
        assert_eq!(ctx.root_path, PathBuf::from("/custom/path"));
        assert!(!ctx.respect_gitignore);
    }

    // ============================================================================
    // Tests de recherche avec wildcards (style Eclipse)
    // ============================================================================

    #[test]
    fn test_wildcard_star_suffix() {
        let ctx = SearchContext::new(
            "*controller".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        // Should match filenames (without extension) ending with "controller"
        assert!(ctx.is_match("UserController"));
        assert!(ctx.is_match("TotoController"));
        assert!(ctx.is_match("MyController"));
        assert!(ctx.is_match("controller"));

        // Should not match
        assert!(!ctx.is_match("ControllerService"));
        assert!(!ctx.is_match("MyService"));
    }

    #[test]
    fn test_wildcard_star_prefix() {
        let ctx = SearchContext::new(
            "User*".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("UserController"));
        assert!(ctx.is_match("UserService"));
        assert!(ctx.is_match("User"));
        assert!(!ctx.is_match("MyUser"));
    }

    #[test]
    fn test_wildcard_star_middle() {
        let ctx = SearchContext::new(
            "User*Service".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("UserService"));
        assert!(ctx.is_match("UserAuthService"));
        assert!(ctx.is_match("UserManagementService"));
        assert!(!ctx.is_match("UserController"));
    }

    #[test]
    fn test_wildcard_question_mark() {
        let ctx = SearchContext::new(
            "User?".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("User1"));
        assert!(ctx.is_match("UserA"));
        assert!(ctx.is_match("Users"));
        assert!(!ctx.is_match("User"));
        assert!(!ctx.is_match("User12"));
    }

    #[test]
    fn test_wildcard_multiple_stars() {
        let ctx = SearchContext::new(
            "*User*Controller*".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("MyUserController"));
        assert!(ctx.is_match("AdminUserControllerImpl"));
        assert!(ctx.is_match("UserController"));
        assert!(!ctx.is_match("UserService"));
    }

    #[test]
    fn test_wildcard_case_insensitive() {
        let ctx = SearchContext::new(
            "*CONTROLLER".to_string(),
            false, // case insensitive
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("UserController"));
        assert!(ctx.is_match("usercontroller"));
        assert!(ctx.is_match("MyController"));
    }

    #[test]
    fn test_wildcard_with_special_chars() {
        let ctx = SearchContext::new(
            "User*.java".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        // The dot in .java should be escaped
        assert!(ctx.is_match("UserController.java"));
        assert!(ctx.is_match("User.java"));
        assert!(!ctx.is_match("UserControllerXjava")); // The dot is literal
    }

    #[test]
    fn test_no_wildcard_still_works() {
        let ctx = SearchContext::new(
            "Controller".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        // Without wildcard, should work as before (contains)
        assert!(ctx.is_match("UserController.java"));
        assert!(ctx.is_match("Controller"));
        assert!(ctx.is_match("MyControllerService"));
    }

    // ============================================================================
    // Tests de recherche avec wildcards (style Eclipse)
    // ============================================================================

    // ============================================================================
    // Tests de CamelCase Matching
    // ============================================================================

    #[test]
    fn test_camelcase_basic() {
        let ctx = SearchContext::new(
            "UC".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("UserController"));
        assert!(ctx.is_match("UsersController"));
        assert!(ctx.is_match("UpdateController"));
        assert!(!ctx.is_match("usercontroller"));
        assert!(!ctx.is_match("Usercontroller"));
    }

    #[test]
    fn test_camelcase_three_letters() {
        let ctx = SearchContext::new(
            "UCS".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("UserControllerService"));
        assert!(ctx.is_match("UpdateCustomerService"));
        assert!(!ctx.is_match("UserController"));
        assert!(!ctx.is_match("UserService"));
    }

    #[test]
    fn test_camelcase_with_numbers() {
        let ctx = SearchContext::new(
            "U2C".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("User2Controller"));
        assert!(!ctx.is_match("UserController"));
    }

    #[test]
    fn test_camelcase_fallback_to_normal() {
        let ctx = SearchContext::new(
            "UC".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        // If no CamelCase match, should fallback to normal search
        assert!(ctx.is_match("ABUC"));
        assert!(ctx.is_match("testUCvalue"));
    }

    #[test]
    fn test_not_camelcase_query() {
        let ctx = SearchContext::new(
            "User".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        // "User" is not a CamelCase pattern (not all uppercase)
        // Should perform normal search
        assert!(ctx.is_match("UserController"));
        assert!(ctx.is_match("user"));
        assert!(ctx.is_match("MyUser"));
    }

    #[test]
    fn test_camelcase_single_letter() {
        let ctx = SearchContext::new(
            "U".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        // A single letter is not a CamelCase pattern
        // Should perform normal search
        assert!(ctx.is_match("UserController"));
        assert!(ctx.is_match("user"));
    }

    #[test]
    fn test_camelcase_long_pattern() {
        let ctx = SearchContext::new(
            "UACS".to_string(),
            false,
            false,
            false,
            PathBuf::from("/tmp"),
            "".to_string(),
            true,
            None,
        )
        .unwrap();

        assert!(ctx.is_match("UserAuthenticationControllerService"));
        assert!(ctx.is_match("UpdateAccountCustomerService"));
        assert!(!ctx.is_match("UserController"));
    }
}
