//! The macOS menu bar: Blockwise (About, Settings, Hide, Quit), File (new,
//! import, open, close and save a project, auto save, export), View (theme, phone)
//! and Window. Other systems get no native menu; the same commands stay on
//! their keyboard shortcuts and in the window.

/// What a menu item asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Settings,
    NewProject,
    ImportProject,
    OpenProject(u32),
    CloseProject,
    Save,
    ToggleAutoSave,
    Export,
    ToggleDark,
    PreviewIphone(bool),
}

/// What the menu shows: kept in step with the app every frame.
pub struct State<'a> {
    pub in_project: bool,
    pub auto_save: bool,
    pub dark: bool,
    pub iphone: bool,
    /// (id, name) of every project, newest first.
    pub projects: &'a [(u32, String)],
}

fn parse(id: &str) -> Option<Cmd> {
    Some(match id {
        "settings" => Cmd::Settings,
        "new" => Cmd::NewProject,
        "import" => Cmd::ImportProject,
        "close" => Cmd::CloseProject,
        "save" => Cmd::Save,
        "autosave" => Cmd::ToggleAutoSave,
        "export" => Cmd::Export,
        "dark" => Cmd::ToggleDark,
        "iphone" => Cmd::PreviewIphone(true),
        "android" => Cmd::PreviewIphone(false),
        _ => Cmd::OpenProject(id.strip_prefix("open:")?.parse().ok()?),
    })
}

#[cfg(target_os = "macos")]
pub use mac::AppMenu;

#[cfg(target_os = "macos")]
mod mac {
    use super::{parse, Cmd, State};
    use muda::accelerator::{Accelerator, Code, Modifiers};
    use muda::{AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
    use std::sync::mpsc;

    pub struct AppMenu {
        _menu: Menu,
        open: Submenu,
        close: MenuItem,
        save: MenuItem,
        export: MenuItem,
        auto_save: CheckMenuItem,
        dark: CheckMenuItem,
        iphone: CheckMenuItem,
        android: CheckMenuItem,
        shown: Option<(bool, bool, bool, bool, Vec<(u32, String)>)>,
        rx: mpsc::Receiver<String>,
    }

    fn key(code: Code) -> Option<Accelerator> {
        Some(Accelerator::new(Some(Modifiers::SUPER), code))
    }

    impl AppMenu {
        /// Build the menu bar and put it on the app. Menu clicks wake the app.
        pub fn install(ctx: &egui::Context) -> Option<Self> {
            let about = AboutMetadata {
                name: Some("Blockwise".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                comments: Some("Build Android and iOS apps with blocks.".into()),
                copyright: Some("Copyright 2026 Aissam Abouqassime El Edrissi. Apache License 2.0.".into()),
                ..Default::default()
            };
            let settings = MenuItem::with_id("settings", "Settings…", true, key(Code::Comma));
            let app = Submenu::with_items(
                "Blockwise",
                true,
                &[
                    &PredefinedMenuItem::about(Some("About Blockwise"), Some(about)),
                    &PredefinedMenuItem::separator(),
                    &settings,
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::services(None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::hide(Some("Hide Blockwise")),
                    &PredefinedMenuItem::hide_others(None),
                    &PredefinedMenuItem::show_all(None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::quit(Some("Quit Blockwise")),
                ],
            )
            .ok()?;
            let open = Submenu::new("Open Project", true);
            let close = MenuItem::with_id("close", "Close Project", false, key(Code::KeyW));
            let save = MenuItem::with_id("save", "Save", false, key(Code::KeyS));
            let export = MenuItem::with_id("export", "Export…", false, key(Code::KeyE));
            let auto_save = CheckMenuItem::with_id("autosave", "Auto Save", true, false, None);
            let file = Submenu::with_items(
                "File",
                true,
                &[
                    &MenuItem::with_id("new", "New Project…", true, key(Code::KeyN)),
                    &MenuItem::with_id("import", "Import Project…", true, key(Code::KeyO)),
                    &open,
                    &PredefinedMenuItem::separator(),
                    &close,
                    &PredefinedMenuItem::separator(),
                    &save,
                    &auto_save,
                    &PredefinedMenuItem::separator(),
                    &export,
                ],
            )
            .ok()?;
            let dark = CheckMenuItem::with_id("dark", "Dark Theme", true, false, None);
            let iphone = CheckMenuItem::with_id("iphone", "iOS Preview", true, true, None);
            let android = CheckMenuItem::with_id("android", "Android Preview", true, false, None);
            let view = Submenu::with_items("View", true, &[&dark, &PredefinedMenuItem::separator(), &iphone, &android, &PredefinedMenuItem::separator(), &PredefinedMenuItem::fullscreen(None)]).ok()?;
            let window = Submenu::with_items("Window", true, &[&PredefinedMenuItem::minimize(None), &PredefinedMenuItem::maximize(Some("Zoom"))]).ok()?;
            let menu = Menu::with_items(&[&app, &file, &view, &window]).ok()?;
            menu.init_for_nsapp();
            window.set_as_windows_menu_for_nsapp();

            let (tx, rx) = mpsc::channel();
            let ctx = ctx.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let _ = tx.send(e.id().0.clone());
                ctx.request_repaint();
            }));
            Some(AppMenu { _menu: menu, open, close, save, export, auto_save, dark, iphone, android, shown: None, rx })
        }

        /// Bring checks, enabled items and the project list up to date.
        pub fn sync(&mut self, st: &State) {
            let now = (st.in_project, st.auto_save, st.dark, st.iphone, st.projects.to_vec());
            if self.shown.as_ref() == Some(&now) {
                return;
            }
            let list_changed = self.shown.as_ref().map(|s| &s.4) != Some(&now.4);
            self.close.set_enabled(st.in_project);
            self.save.set_enabled(st.in_project);
            self.export.set_enabled(st.in_project);
            self.auto_save.set_checked(st.auto_save);
            self.dark.set_checked(st.dark);
            self.iphone.set_checked(st.iphone);
            self.android.set_checked(!st.iphone);
            if list_changed {
                for it in self.open.items() {
                    if let Some(item) = it.as_menuitem() {
                        let _ = self.open.remove(item);
                    }
                }
                if st.projects.is_empty() {
                    let _ = self.open.append(&MenuItem::new("No projects yet", false, None));
                }
                for (id, name) in st.projects {
                    let _ = self.open.append(&MenuItem::with_id(format!("open:{id}"), name, true, None));
                }
            }
            self.shown = Some(now);
        }

        /// The commands picked since the last call.
        pub fn poll(&self) -> Vec<Cmd> {
            self.rx.try_iter().filter_map(|id| parse(&id)).collect()
        }
    }
}

/// No native menu outside macOS.
#[cfg(not(target_os = "macos"))]
pub struct AppMenu;

#[cfg(not(target_os = "macos"))]
impl AppMenu {
    pub fn install(_ctx: &egui::Context) -> Option<Self> {
        None
    }
    pub fn sync(&mut self, _st: &State) {}
    pub fn poll(&self) -> Vec<Cmd> {
        vec![]
    }
}

/// The menu bar holds Settings and the theme on macOS, so the window
/// doesn't repeat them there.
pub const NATIVE: bool = cfg!(target_os = "macos");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_turn_into_commands() {
        assert_eq!(parse("open:612"), Some(Cmd::OpenProject(612)));
        assert_eq!(parse("android"), Some(Cmd::PreviewIphone(false)));
        assert_eq!(parse("save"), Some(Cmd::Save));
        assert_eq!(parse("open:x"), None);
        assert_eq!(parse("about"), None);
    }
}
