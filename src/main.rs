mod file_entry;
mod file_manager_panel;
mod terminal_panel;
mod watcher;

use file_manager_panel::FileManagerPanel;
use terminal_panel::TerminalPanel;
use watcher::FsWatcher;

use gtk4::gdk;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::channel;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Hybrid,
    PureTerminal,
    PureFileManager,
}

struct AppState {
    mode: ViewMode,
    current_dir: PathBuf,
}

fn main() {
    let app = gtk4::Application::builder()
        .application_id("org.litera.Litera")
        .build();

    app.connect_startup(|_| {
        load_css();
    });

    app.connect_activate(build_ui);

    app.run();
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    let css_bytes = include_bytes!("style.css");
    provider.load_from_data(std::str::from_utf8(css_bytes).unwrap_or(""));

    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_ui(app: &gtk4::Application) {
    let initial_path = std::env::current_dir().unwrap_or_else(|_| {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    });

    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("Litera")
        .default_width(1280)
        .default_height(800)
        .build();

    window.add_css_class("litera-window");

    let root_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root_box.add_css_class("litera-root");
    root_box.set_hexpand(true);
    root_box.set_vexpand(true);

    let state = Rc::new(RefCell::new(AppState {
        mode: ViewMode::Hybrid,
        current_dir: initial_path.clone(),
    }));

    // Main Paned Workspace (Split 50-50 across entire window, no outer margin)
    let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
    paned.add_css_class("litera-paned");
    paned.set_hexpand(true);
    paned.set_vexpand(true);
    paned.set_wide_handle(true);
    paned.set_position(600);

    // Inotify Watcher Setup
    let mut fs_watcher = FsWatcher::new();
    let (tx_notify, rx_notify) = channel();
    let _ = fs_watcher.watch(&initial_path, tx_notify.clone());
    let fs_watcher = Rc::new(RefCell::new(fs_watcher));
    let tx_notify = Rc::new(tx_notify);

    // Shared references container
    let terminal_panel_cell: Rc<RefCell<Option<Rc<TerminalPanel>>>> = Rc::new(RefCell::new(None));
    let fm_panel_cell: Rc<RefCell<Option<Rc<FileManagerPanel>>>> = Rc::new(RefCell::new(None));

    // Terminal -> File Manager Callback
    let fm_cell_for_term = fm_panel_cell.clone();
    let state_for_term = state.clone();
    let watcher_for_term = fs_watcher.clone();
    let tx_for_term = tx_notify.clone();

    let on_terminal_cwd_changed = move |new_path: PathBuf| {
        let mut st = state_for_term.borrow_mut();
        if st.current_dir != new_path {
            st.current_dir = new_path.clone();
            let _ = watcher_for_term.borrow_mut().watch(&new_path, (*tx_for_term).clone());
            if let Some(fm) = fm_cell_for_term.borrow().as_ref() {
                fm.set_current_dir(new_path);
            }
        }
    };

    let term_panel = Rc::new(TerminalPanel::new(&initial_path, on_terminal_cwd_changed));
    *terminal_panel_cell.borrow_mut() = Some(term_panel.clone());

    // File Manager -> Terminal Callback (Navigating folders sends cd)
    let term_cell_for_fm = terminal_panel_cell.clone();
    let fm_cell_for_nav = fm_panel_cell.clone();
    let state_for_fm = state.clone();
    let watcher_for_fm = fs_watcher.clone();
    let tx_for_fm = tx_notify.clone();

    let on_fm_dir_navigated = move |new_path: PathBuf| {
        let mut st = state_for_fm.borrow_mut();
        st.current_dir = new_path.clone();
        let _ = watcher_for_fm.borrow_mut().watch(&new_path, (*tx_for_fm).clone());

        if let Some(term) = term_cell_for_fm.borrow().as_ref() {
            term.cd_to(&new_path);
        }
        if let Some(fm) = fm_cell_for_nav.borrow().as_ref() {
            fm.set_current_dir(new_path);
        }
    };

    let term_cell_for_cmd = terminal_panel_cell.clone();
    let on_run_cmd = move |cmd: String| {
        if let Some(term) = term_cell_for_cmd.borrow().as_ref() {
            term.execute_command(&cmd);
        }
    };

    let fm_panel = Rc::new(FileManagerPanel::new(
        &initial_path,
        on_fm_dir_navigated,
        on_run_cmd,
    ));
    *fm_panel_cell.borrow_mut() = Some(fm_panel.clone());

    // Pack panels into Paned (with minimum widths 240px configured in widgets)
    paned.set_start_child(Some(&fm_panel.widget));
    paned.set_end_child(Some(&term_panel.widget));

    // Mode Switcher Logic
    let paned_ref = paned.clone();
    let fm_widget = fm_panel.widget.clone();
    let term_widget = term_panel.widget.clone();
    let state_mode = state.clone();

    let update_mode = Rc::new(move |mode: ViewMode| {
        state_mode.borrow_mut().mode = mode;
        match mode {
            ViewMode::Hybrid => {
                fm_widget.set_visible(true);
                term_widget.set_visible(true);
                paned_ref.set_wide_handle(true);
            }
            ViewMode::PureTerminal => {
                fm_widget.set_visible(false);
                term_widget.set_visible(true);
                paned_ref.set_wide_handle(false);
            }
            ViewMode::PureFileManager => {
                fm_widget.set_visible(true);
                term_widget.set_visible(false);
                paned_ref.set_wide_handle(false);
            }
        }
    });

    // Top Menu Bar: [Dosya] [Görünüm] [Çalışma Alanı] [Pencere]
    let menu_bar = create_menu_bar(
        &window,
        fm_panel.clone(),
        term_panel.clone(),
        update_mode.clone(),
        state.clone(),
    );

    root_box.append(&menu_bar);
    root_box.append(&paned);
    window.set_child(Some(&root_box));

    // Keyboard Shortcuts (F1, F2, F3, F5)
    let key_controller = gtk4::EventControllerKey::new();
    let mode_updater_key = update_mode.clone();
    let fm_ref_for_key = fm_panel.clone();
    key_controller.connect_key_pressed(move |_ctrl, key, _code, _modifier| {
        match key {
            gdk::Key::F1 => {
                mode_updater_key(ViewMode::Hybrid);
                glib::Propagation::Stop
            }
            gdk::Key::F2 => {
                mode_updater_key(ViewMode::PureTerminal);
                glib::Propagation::Stop
            }
            gdk::Key::F3 => {
                mode_updater_key(ViewMode::PureFileManager);
                glib::Propagation::Stop
            }
            gdk::Key::F5 => {
                fm_ref_for_key.refresh();
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    window.add_controller(key_controller);

    // Inotify event polling on GTK main loop (live sync)
    let fm_ref_for_notify = fm_panel.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        let mut refreshed = false;
        while let Ok(_) = rx_notify.try_recv() {
            refreshed = true;
        }
        if refreshed {
            fm_ref_for_notify.refresh();
        }
        glib::ControlFlow::Continue
    });

    // CWD fallback poll (handles any child proc or manual prompt cd)
    let term_poll_ref = term_panel.clone();
    let fm_poll_ref = fm_panel.clone();
    let state_poll_ref = state.clone();
    let watcher_poll_ref = fs_watcher.clone();
    let tx_poll_ref = tx_notify.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(600), move || {
        if let Some(active_cwd) = term_poll_ref.get_foreground_cwd() {
            let mut st = state_poll_ref.borrow_mut();
            if st.current_dir != active_cwd {
                st.current_dir = active_cwd.clone();
                let _ = watcher_poll_ref.borrow_mut().watch(&active_cwd, (*tx_poll_ref).clone());
                fm_poll_ref.set_current_dir(active_cwd);
            }
        }
        glib::ControlFlow::Continue
    });

    window.present();
}

fn create_menu_bar(
    window: &gtk4::ApplicationWindow,
    fm_panel: Rc<FileManagerPanel>,
    term_panel: Rc<TerminalPanel>,
    update_mode: Rc<dyn Fn(ViewMode)>,
    state: Rc<RefCell<AppState>>,
) -> gtk4::Box {
    let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    bar.add_css_class("litera-menubar");

    // 1. Dosya Menüsü
    let btn_file = gtk4::MenuButton::new();
    btn_file.set_label("Dosya");
    btn_file.add_css_class("litera-menu-btn");

    let pop_file = gtk4::Popover::new();
    pop_file.set_has_arrow(false);
    pop_file.add_css_class("litera-popover");
    let box_file = gtk4::Box::new(gtk4::Orientation::Vertical, 2);

    let m_new_folder = gtk4::Button::with_label("Yeni Klasör Oluştur (Terminal)");
    let pop_f1 = pop_file.clone();
    let term_f1 = term_panel.clone();
    m_new_folder.connect_clicked(move |_| {
        pop_f1.popdown();
        term_f1.execute_command("mkdir -p yeni_klasor");
    });
    box_file.append(&m_new_folder);

    let m_new_file = gtk4::Button::with_label("Yeni Dosya Oluştur (Terminal)");
    let pop_f2 = pop_file.clone();
    let term_f2 = term_panel.clone();
    m_new_file.connect_clicked(move |_| {
        pop_f2.popdown();
        term_f2.execute_command("touch yeni_dosya.txt");
    });
    box_file.append(&m_new_file);

    let m_quit = gtk4::Button::with_label("Çıkış");
    let win_q = window.clone();
    m_quit.connect_clicked(move |_| {
        win_q.close();
    });
    box_file.append(&m_quit);

    pop_file.set_child(Some(&box_file));
    btn_file.set_popover(Some(&pop_file));
    bar.append(&btn_file);

    // 2. Görünüm Menüsü
    let btn_view = gtk4::MenuButton::new();
    btn_view.set_label("Görünüm");
    btn_view.add_css_class("litera-menu-btn");

    let pop_view = gtk4::Popover::new();
    pop_view.set_has_arrow(false);
    pop_view.add_css_class("litera-popover");
    let box_view = gtk4::Box::new(gtk4::Orientation::Vertical, 2);

    let m_hybrid = gtk4::Button::with_label("Hibrit Mod (F1)");
    let pop_v1 = pop_view.clone();
    let mode_v1 = update_mode.clone();
    m_hybrid.connect_clicked(move |_| {
        pop_v1.popdown();
        mode_v1(ViewMode::Hybrid);
    });
    box_view.append(&m_hybrid);

    let m_term = gtk4::Button::with_label("Saf Terminal Modu (F2)");
    let pop_v2 = pop_view.clone();
    let mode_v2 = update_mode.clone();
    m_term.connect_clicked(move |_| {
        pop_v2.popdown();
        mode_v2(ViewMode::PureTerminal);
    });
    box_view.append(&m_term);

    let m_fm = gtk4::Button::with_label("Saf Dosya Yöneticisi Modu (F3)");
    let pop_v3 = pop_view.clone();
    let mode_v3 = update_mode.clone();
    m_fm.connect_clicked(move |_| {
        pop_v3.popdown();
        mode_v3(ViewMode::PureFileManager);
    });
    box_view.append(&m_fm);

    let m_hidden = gtk4::Button::with_label("Gizli Dosyaları Göster / Gizle (Ctrl+H)");
    let pop_v4 = pop_view.clone();
    let fm_v4 = fm_panel.clone();
    m_hidden.connect_clicked(move |_| {
        pop_v4.popdown();
        fm_v4.toggle_hidden();
    });
    box_view.append(&m_hidden);

    let m_refresh = gtk4::Button::with_label("Yenile (F5)");
    let pop_v5 = pop_view.clone();
    let fm_v5 = fm_panel.clone();
    m_refresh.connect_clicked(move |_| {
        pop_v5.popdown();
        fm_v5.refresh();
    });
    box_view.append(&m_refresh);

    pop_view.set_child(Some(&box_view));
    btn_view.set_popover(Some(&pop_view));
    bar.append(&btn_view);

    // 3. Çalışma Alanı Menüsü
    let btn_work = gtk4::MenuButton::new();
    btn_work.set_label("Çalışma Alanı");
    btn_work.add_css_class("litera-menu-btn");

    let pop_work = gtk4::Popover::new();
    pop_work.set_has_arrow(false);
    pop_work.add_css_class("litera-popover");
    let box_work = gtk4::Box::new(gtk4::Orientation::Vertical, 2);

    let m_home = gtk4::Button::with_label("Ev Dizinine Git (~)");
    let pop_w1 = pop_work.clone();
    let fm_w1 = fm_panel.clone();
    let term_w1 = term_panel.clone();
    m_home.connect_clicked(move |_| {
        pop_w1.popdown();
        if let Some(home) = dirs::home_dir() {
            term_w1.cd_to(&home);
            fm_w1.set_current_dir(home);
        }
    });
    box_work.append(&m_home);

    let m_root = gtk4::Button::with_label("Kök Dizinine Git (/)");
    let pop_w2 = pop_work.clone();
    let fm_w2 = fm_panel.clone();
    let term_w2 = term_panel.clone();
    m_root.connect_clicked(move |_| {
        pop_w2.popdown();
        let root = PathBuf::from("/");
        term_w2.cd_to(&root);
        fm_w2.set_current_dir(root);
    });
    box_work.append(&m_root);

    let m_clear_term = gtk4::Button::with_label("Terminali Temizle (clear)");
    let pop_w3 = pop_work.clone();
    let term_w3 = term_panel.clone();
    m_clear_term.connect_clicked(move |_| {
        pop_w3.popdown();
        term_w3.execute_command("clear");
    });
    box_work.append(&m_clear_term);

    pop_work.set_child(Some(&box_work));
    btn_work.set_popover(Some(&pop_work));
    bar.append(&btn_work);

    // 4. Pencere Menüsü
    let btn_win = gtk4::MenuButton::new();
    btn_win.set_label("Pencere");
    btn_win.add_css_class("litera-menu-btn");

    let pop_win = gtk4::Popover::new();
    pop_win.set_has_arrow(false);
    pop_win.add_css_class("litera-popover");
    let box_win = gtk4::Box::new(gtk4::Orientation::Vertical, 2);

    let m_fs = gtk4::Button::with_label("Tam Ekran / Normal");
    let pop_win1 = pop_win.clone();
    let win_fs = window.clone();
    m_fs.connect_clicked(move |_| {
        pop_win1.popdown();
        if win_fs.is_fullscreen() {
            win_fs.unfullscreen();
        } else {
            win_fs.fullscreen();
        }
    });
    box_win.append(&m_fs);

    let m_info = gtk4::Button::with_label("Litera Hakkında");
    let pop_win2 = pop_win.clone();
    let win_parent = window.clone();
    let state_about = state.clone();
    m_info.connect_clicked(move |_| {
        pop_win2.popdown();
        let cwd = state_about.borrow().current_dir.display().to_string();
        let dialog = gtk4::AlertDialog::builder()
            .modal(true)
            .message("Litera — Hybrid File Manager & Terminal")
            .detail(&format!("Linux (Wayland) GTK4 + Rust mimarisiyle geliştirildi.\nAktif Konum: {}", cwd))
            .build();
        dialog.show(Some(&win_parent));
    });
    box_win.append(&m_info);

    pop_win.set_child(Some(&box_win));
    btn_win.set_popover(Some(&pop_win));
    bar.append(&btn_win);

    bar
}

mod dirs {
    use std::path::PathBuf;
    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
