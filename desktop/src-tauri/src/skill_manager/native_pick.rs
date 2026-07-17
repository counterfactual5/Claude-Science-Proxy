//! Native Skill source picker.
//!
//! On macOS, `rfd` only exposes exclusive `pick_file` / `pick_folder` modes.
//! We use `NSOpenPanel` directly so one panel can choose either a Skill folder
//! or a `.zip` archive.

use std::path::PathBuf;

/// Open a native chooser for a local Skill folder or `.zip` file.
pub fn pick_skill_source(title: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        macos::pick_folder_or_zip(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Non-macOS: rfd cannot mix files + folders; prefer folders (primary
        // Skill layout). ZIP paths remain available via the text field.
        rfd::FileDialog::new().set_title(title).pick_folder()
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::PathBuf;

    use dispatch2::run_on_main;
    use objc2::rc::autoreleasepool;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::{NSArray, NSString};

    pub(super) fn pick_folder_or_zip(title: &str) -> Option<PathBuf> {
        let title = title.to_owned();
        autoreleasepool(|_| run_on_main(move |mtm| show_panel(mtm, &title)))
    }

    fn show_panel(mtm: MainThreadMarker, title: &str) -> Option<PathBuf> {
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseDirectories(true);
        panel.setCanChooseFiles(true);
        panel.setAllowsMultipleSelection(false);
        panel.setCanCreateDirectories(false);
        panel.setMessage(Some(&NSString::from_str(title)));
        // Directories remain selectable even with a file-type filter.
        let zip = NSString::from_str("zip");
        let types = NSArray::from_retained_slice(&[zip]);
        #[allow(deprecated)]
        panel.setAllowedFileTypes(Some(&types));

        let response = panel.runModal();
        if response != NSModalResponseOK {
            return None;
        }

        let url = panel.URL()?;
        let path = PathBuf::from(url.path()?.to_string());
        if path.is_dir() {
            return Some(path);
        }
        let is_zip = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("zip"))
            .unwrap_or(false);
        if is_zip {
            Some(path)
        } else {
            None
        }
    }
}
