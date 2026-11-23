use crate::file_entry::{list_directory, FileEntry};
use gtk4::gdk;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub struct FileManagerPanel {
    pub widget: gtk4::Box,
    current_dir: Rc<RefCell<PathBuf>>,
    entries: Rc<RefCell<Vec<FileEntry>>>,
    list_box: gtk4::ListBox,
    path_entry: gtk4::Entry,
    status_label: gtk4::Label,
    show_hidden: Rc<RefCell<bool>>,
    on_dir_navigated: Rc<dyn Fn(PathBuf)>,
    on_run_in_terminal: Rc<dyn Fn(String)>,
}

impl FileManagerPanel {
    pub fn new<FDir, FTerm>(
        initial_dir: &Path,
        on_dir_navigated: FDir,
        on_run_in_terminal: FTerm,
    ) -> Self
    where
        FDir: Fn(PathBuf) + 'static,
        FTerm: Fn(String) + 'static,
    {
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        container.set_widget_name("litera-fm-panel");
        container.add_css_class("litera-panel");
        container.set_hexpand(true);
        container.set_vexpand(true);
        container.set_size_request(240, -1); // Min width

        let current_dir = Rc::new(RefCell::new(initial_dir.to_path_buf()));
        let entries = Rc::new(RefCell::new(Vec::new()));
        let show_hidden = Rc::new(RefCell::new(false));
        let on_dir_navigated = Rc::new(on_dir_navigated);
        let on_run_in_terminal = Rc::new(on_run_in_terminal);

        // Header / Breadcrumb Toolbar
        let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        header.add_css_class("litera-panel-header");

        // Back / Up button
        let back_btn = gtk4::Button::from_icon_name("go-up-symbolic");
        back_btn.add_css_class("litera-icon-btn");
        back_btn.set_tooltip_text(Some("Üst Dizine Çık (Alt+Up)"));

        let curr_dir_clone = current_dir.clone();
        let on_nav_clone = on_dir_navigated.clone();
        back_btn.connect_clicked(move |_| {
            let parent_opt = {
                let curr = curr_dir_clone.borrow();
                curr.parent().map(|p| p.to_path_buf())
            };
            if let Some(parent) = parent_opt {
                on_nav_clone(parent);
            }
        });
        header.append(&back_btn);

        // Refresh button
        let refresh_btn = gtk4::Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.add_css_class("litera-icon-btn");
        refresh_btn.set_tooltip_text(Some("Yenile (F5)"));

        let curr_dir_refresh = current_dir.clone();
        let on_nav_refresh = on_dir_navigated.clone();
        refresh_btn.connect_clicked(move |_| {
            let current = curr_dir_refresh.borrow().clone();
            on_nav_refresh(current);
        });
        header.append(&refresh_btn);

        // Toggle hidden files button
        let hidden_btn = gtk4::Button::from_icon_name("view-conceal-symbolic");
        hidden_btn.add_css_class("litera-icon-btn");
        hidden_btn.set_tooltip_text(Some("Gizli Dosyaları Göster/Gizle (Ctrl+H)"));

        let hidden_state = show_hidden.clone();
        let curr_dir_hidden = current_dir.clone();
        let on_nav_hidden = on_dir_navigated.clone();
        hidden_btn.connect_clicked(move |_| {
            let mut val = hidden_state.borrow_mut();
            *val = !*val;
            drop(val);
            let current = curr_dir_hidden.borrow().clone();
            on_nav_hidden(current);
        });
        header.append(&hidden_btn);

        // Path entry - editable text entry so user can manually type path!
        let path_entry = gtk4::Entry::new();
        path_entry.set_text(&initial_dir.to_string_lossy());
        path_entry.set_hexpand(true);
        path_entry.set_halign(gtk4::Align::Fill);
        path_entry.add_css_class("litera-path-bar");

        let on_nav_entry = on_dir_navigated.clone();
        path_entry.connect_activate(move |entry| {
            let text = entry.text().trim().to_string();
            let expanded_path = if text.starts_with('~') {
                if let Some(home) = dirs_home() {
                    let rest = text.trim_start_matches('~').trim_start_matches('/');
                    home.join(rest)
                } else {
                    PathBuf::from(text)
                }
            } else {
                PathBuf::from(text)
            };

            if expanded_path.exists() && expanded_path.is_dir() {
                on_nav_entry(expanded_path);
            }
        });
        header.append(&path_entry);

        container.append(&header);

        // File List in a ScrolledWindow
        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_hexpand(true);
        scrolled.set_vexpand(true);

        let list_box = gtk4::ListBox::new();
        list_box.add_css_class("litera-file-list");
        list_box.set_selection_mode(gtk4::SelectionMode::Single);
        // Do NOT activate on single click. User wants 1-click select, 2-click open!
        list_box.set_activate_on_single_click(false);

        scrolled.set_child(Some(&list_box));
        container.append(&scrolled);

        // Status bar
        let status_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        status_box.add_css_class("litera-status-bar");
        let status_label = gtk4::Label::new(Some("Hazır"));
        status_label.set_halign(gtk4::Align::Start);
        status_box.append(&status_label);
        container.append(&status_box);

        let panel = Self {
            widget: container,
            current_dir,
            entries,
            list_box,
            path_entry,
            status_label,
            show_hidden,
            on_dir_navigated,
            on_run_in_terminal,
        };

        panel.refresh();
        panel.setup_interactions();

        panel
    }

    pub fn set_current_dir(&self, dir: PathBuf) {
        *self.current_dir.borrow_mut() = dir;
        self.refresh();
    }

    pub fn toggle_hidden(&self) {
        let mut val = self.show_hidden.borrow_mut();
        *val = !*val;
        drop(val);
        self.refresh();
    }

    pub fn refresh(&self) {
        let dir = self.current_dir.borrow().clone();
        self.path_entry.set_text(&dir.to_string_lossy());

        let show_hidden = *self.show_hidden.borrow();
        let items = list_directory(&dir, show_hidden);

        let dir_count = items.iter().filter(|i| i.is_dir).count();
        let file_count = items.len() - dir_count;
        self.status_label.set_text(&format!(
            "{} klasör, {} dosya | {}",
            dir_count,
            file_count,
            dir.display()
        ));

        // Clear existing list rows
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        // Clone entries for generating UI rows
        let items_copy = items.clone();
        *self.entries.borrow_mut() = items;

        for entry in items_copy.iter() {
            let row = gtk4::ListBoxRow::new();
            row.add_css_class("litera-file-row");

            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
            hbox.set_margin_start(4);
            hbox.set_margin_end(4);

            let icon = gtk4::Image::from_icon_name(&entry.icon_name);
            icon.set_icon_size(gtk4::IconSize::Large);
            icon.add_css_class("litera-file-icon");

            let name_label = gtk4::Label::new(Some(&entry.name));
            name_label.set_halign(gtk4::Align::Start);
            name_label.set_hexpand(true);
            name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            name_label.add_css_class("litera-file-name");

            let meta_label = gtk4::Label::new(Some(&entry.formatted_size()));
            meta_label.set_halign(gtk4::Align::End);
            meta_label.add_css_class("litera-file-meta");

            hbox.append(&icon);
            hbox.append(&name_label);
            hbox.append(&meta_label);

            row.set_child(Some(&hbox));

            // Setup Drag Source so files can be dragged to the terminal
            let drag_source = gtk4::DragSource::new();
            let file_g = gio::File::for_path(&entry.path);
            let content_provider = gdk::ContentProvider::for_value(&file_g.to_value());
            drag_source.set_content(Some(&content_provider));
            drag_source.set_actions(gdk::DragAction::COPY);
            row.add_controller(drag_source);

            // Double Click Gesture: 1 click selects row, 2 clicks activate/open!
            let click_gesture = gtk4::GestureClick::new();
            click_gesture.set_button(1); // Left click
            let entry_target_path = entry.path.clone();
            let is_directory = entry.is_dir;
            let on_nav_click = self.on_dir_navigated.clone();

            click_gesture.connect_pressed(move |gesture, n_press, _x, _y| {
                if n_press == 2 {
                    gesture.set_state(gtk4::EventSequenceState::Claimed);
                    if is_directory {
                        on_nav_click(entry_target_path.clone());
                    } else {
                        let _ = open::that(&entry_target_path);
                    }
                }
            });
            row.add_controller(click_gesture);

            // Right click context menu
            let right_click = gtk4::GestureClick::new();
            right_click.set_button(3); // Secondary mouse button
            let entry_path = entry.path.clone();
            let entry_is_dir = entry.is_dir;
            let on_term_cmd = self.on_run_in_terminal.clone();
            let on_nav = self.on_dir_navigated.clone();

            let row_clone = row.clone();
            right_click.connect_pressed(move |gesture, _n_press, x, y| {
                gesture.set_state(gtk4::EventSequenceState::Claimed);
                Self::show_context_menu(
                    &row_clone,
                    x,
                    y,
                    &entry_path,
                    entry_is_dir,
                    on_term_cmd.clone(),
                    on_nav.clone(),
                );
            });
            row.add_controller(right_click);

            self.list_box.append(&row);
        }
    }

    fn setup_interactions(&self) {
        let entries_ref = self.entries.clone();
        let on_nav = self.on_dir_navigated.clone();

        // Key activation (e.g. Enter on selected row)
        self.list_box.connect_row_activated(move |_lb, row| {
            let idx = row.index();
            if idx >= 0 {
                let idx = idx as usize;
                // Scope the borrow to prevent holding borrow during on_nav!
                let entry_opt = {
                    let entries = entries_ref.borrow();
                    entries.get(idx).map(|e| (e.is_dir, e.path.clone()))
                };

                if let Some((is_dir, path)) = entry_opt {
                    if is_dir {
                        on_nav(path);
                    } else {
                        let _ = open::that(&path);
                    }
                }
            }
        });
    }

    fn show_context_menu(
        parent_widget: &gtk4::ListBoxRow,
        x: f64,
        y: f64,
        path: &Path,
        is_dir: bool,
        on_run_in_terminal: Rc<dyn Fn(String)>,
        on_dir_navigated: Rc<dyn Fn(PathBuf)>,
    ) {
        let popover = gtk4::Popover::new();
        popover.set_has_arrow(false);
        popover.add_css_class("litera-popover");
        popover.set_parent(parent_widget);
        popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

        let menu_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);

        // 1. Open / Enter
        let open_btn = gtk4::Button::with_label(if is_dir {
            "Klasöre Gir (Enter)"
        } else {
            "Dosyayı Aç (Varsayılan)"
        });
        open_btn.add_css_class("litera-btn");
        let pop_ref = popover.clone();
        let path_clone = path.to_path_buf();
        let on_nav_clone = on_dir_navigated.clone();
        open_btn.connect_clicked(move |_| {
            pop_ref.popdown();
            if is_dir {
                on_nav_clone(path_clone.clone());
            } else {
                let _ = open::that(&path_clone);
            }
        });
        menu_box.append(&open_btn);

        // 2. "Burada Terminal Aç / cd burası"
        let cd_btn = gtk4::Button::with_label("Terminalde Bu Konuma Geç (cd)");
        cd_btn.add_css_class("litera-btn");
        let pop_ref = popover.clone();
        let target_dir = if is_dir {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        let on_term_clone = on_run_in_terminal.clone();
        cd_btn.connect_clicked(move |_| {
            pop_ref.popdown();
            let escaped = crate::terminal_panel::shell_words_escape(&target_dir.to_string_lossy());
            on_term_clone(format!("cd {}", escaped));
        });
        menu_box.append(&cd_btn);

        // 3. Quick command: ls -la / file info
        let info_btn = gtk4::Button::with_label("Terminalde Bilgilerini Görüntüle");
        info_btn.add_css_class("litera-btn");
        let pop_ref = popover.clone();
        let path_for_info = path.to_path_buf();
        let on_term_clone2 = on_run_in_terminal.clone();
        info_btn.connect_clicked(move |_| {
            pop_ref.popdown();
            let escaped = crate::terminal_panel::shell_words_escape(&path_for_info.to_string_lossy());
            if is_dir {
                on_term_clone2(format!("ls -la {}", escaped));
            } else {
                on_term_cmd_info(&on_term_clone2, &escaped);
            }
        });
        menu_box.append(&info_btn);

        popover.set_child(Some(&menu_box));
        popover.popup();
    }
}

fn on_term_cmd_info(on_term: &Rc<dyn Fn(String)>, escaped: &str) {
    on_term(format!("ls -la {} && file {}", escaped, escaped));
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
