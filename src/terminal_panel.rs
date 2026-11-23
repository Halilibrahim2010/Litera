use gtk4::gdk;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use vte4::prelude::*;
use vte4::Terminal;

pub struct TerminalPanel {
    pub widget: gtk4::Box,
    pub terminal: Terminal,
    pty_child_pid: Rc<RefCell<Option<glib::Pid>>>,
}

impl TerminalPanel {
    pub fn new<F>(initial_dir: &Path, on_cwd_changed: F) -> Self
    where
        F: Fn(PathBuf) + 'static,
    {
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        container.set_widget_name("litera-terminal-panel");
        container.add_css_class("litera-panel");
        container.set_hexpand(true);
        container.set_vexpand(true);
        container.set_size_request(240, -1); // Min width so it doesn't disappear

        // Header for Terminal Panel
        let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        header.add_css_class("litera-panel-header");

        let icon = gtk4::Image::from_icon_name("utilities-terminal-symbolic");
        let label = gtk4::Label::new(Some("TERMINAL"));
        label.add_css_class("litera-panel-title");
        header.append(&icon);
        header.append(&label);

        container.append(&header);

        let terminal = Terminal::new();
        terminal.set_hexpand(true);
        terminal.set_vexpand(true);

        // Styling Terminal Colors
        let fg = gdk::RGBA::new(0.941, 0.933, 0.968, 1.0); // #f0eef7
        let bg = gdk::RGBA::new(0.094, 0.082, 0.145, 0.0); // Completely transparent terminal background
        let cursor = gdk::RGBA::new(0.478, 0.400, 1.0, 1.0); // #7a66ff
        let highlight = gdk::RGBA::new(0.231, 0.211, 0.333, 1.0); // #3b3655

        terminal.set_color_foreground(&fg);
        terminal.set_color_background(&bg);
        terminal.set_clear_background(false);
        terminal.set_color_cursor_foreground(Some(&fg));
        terminal.set_color_cursor(Some(&cursor));
        terminal.set_color_highlight(Some(&highlight));
        terminal.set_cursor_blink_mode(vte4::CursorBlinkMode::On);
        terminal.set_cursor_shape(vte4::CursorShape::Block);
        terminal.set_scroll_on_output(true);
        terminal.set_scroll_on_keystroke(true);
        terminal.set_mouse_autohide(true);

        container.append(&terminal);

        let pty_child_pid = Rc::new(RefCell::new(None));

        // Setup drop target for files dragged into terminal!
        let drop_target = gtk4::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
        let terminal_ref = terminal.clone();
        drop_target.connect_drop(move |_target, value, _x, _y| {
            if let Ok(file) = value.get::<gio::File>() {
                if let Some(path) = file.path() {
                    let path_str = path.to_string_lossy().to_string();
                    let escaped = shell_words_escape(&path_str);
                    terminal_ref.feed_child(format!("{} ", escaped).as_bytes());
                    return true;
                }
            }
            false
        });
        terminal.add_controller(drop_target);

        // Connect OSC 7 notification / window title / cwd
        let on_cwd_changed = Rc::new(on_cwd_changed);
        let on_cwd_ref = on_cwd_changed.clone();

        terminal.connect_current_directory_uri_notify(move |term| {
            if let Some(uri) = term.current_directory_uri() {
                if let Ok(url) = url::Url::parse(uri.as_str()) {
                    if let Ok(path) = url.to_file_path() {
                        on_cwd_ref(path);
                    }
                }
            }
        });

        // Spawn interactive shell
        Self::spawn_shell(&terminal, initial_dir, pty_child_pid.clone());

        Self {
            widget: container,
            terminal,
            pty_child_pid,
        }
    }

    fn spawn_shell(
        terminal: &Terminal,
        initial_dir: &Path,
        pid_storage: Rc<RefCell<Option<glib::Pid>>>,
    ) {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let dir_str = initial_dir.to_string_lossy().to_string();

        let mut envv = std::env::vars()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>();
        envv.push("LITERA_ACTIVE=1".to_string());

        // Bash / Zsh prompt hook for OSC 7
        // OSC 7 emits: \e]7;file://hostname/path\e\\
        let bash_prompt_cmd = r#"__litera_osc7() { printf "\033]7;file://%s%s\033\\" "$HOSTNAME" "$PWD"; }; PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND; }__litera_osc7""#;
        envv.push(format!("PROMPT_COMMAND={}", bash_prompt_cmd));

        let argv = vec![shell.as_str()];

        terminal.spawn_async(
            vte4::PtyFlags::DEFAULT,
            Some(&dir_str),
            &argv,
            &envv.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            glib::SpawnFlags::SEARCH_PATH,
            || {},
            -1,
            gio::Cancellable::NONE,
            move |res| {
                if let Ok(pid) = res {
                    *pid_storage.borrow_mut() = Some(pid);
                }
            },
        );
    }

    pub fn cd_to(&self, path: &Path) {
        let path_str = path.to_string_lossy();
        let escaped = shell_words_escape(&path_str);
        // Send Ctrl+U to clear current line, then cd, then Enter
        let cmd = format!("\x15cd {}\n", escaped);
        self.terminal.feed_child(cmd.as_bytes());
    }

    pub fn execute_command(&self, cmd: &str) {
        let full = format!("\x15{}\n", cmd);
        self.terminal.feed_child(full.as_bytes());
    }

    pub fn get_foreground_cwd(&self) -> Option<PathBuf> {
        // First try terminal.current_directory_uri()
        if let Some(uri) = self.terminal.current_directory_uri() {
            if let Ok(url) = url::Url::parse(uri.as_str()) {
                if let Ok(path) = url.to_file_path() {
                    return Some(path);
                }
            }
        }

        // Fallback: Check /proc/<pid>/cwd if child pid is known
        if let Some(pid) = *self.pty_child_pid.borrow() {
            // Find active foreground process if bash spawned a child
            let proc_cwd = format!("/proc/{}/cwd", pid.0);
            if let Ok(target) = std::fs::read_link(&proc_cwd) {
                return Some(target);
            }
        }

        None
    }
}

pub fn shell_words_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '.' || c == '_' || c == '-') {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}
