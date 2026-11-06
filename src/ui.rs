use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Result;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::gio::ApplicationFlags;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GBox, Button, ContentFit, CssProvider, FlowBox,
    FlowBoxChild, HeaderBar, Label, Orientation, Picture, PolicyType, ScrolledWindow,
    SelectionMode, Stack,
};

use crate::library::{self, Wallpaper};

const APP_ID: &str = "com.breadway.breadpaper";
const THUMB_W: i32 = 240;
const THUMB_H: i32 = 135;

const APP_CSS: &str = "\
headerbar {\
  background-color: @bg; color: @on-bg; box-shadow: none;\
  border-bottom: 1px solid alpha(@on-bg, 0.08);\
}\n\
.library-chrome { padding: 12px 16px 8px 16px; }\n\
.library-grid { padding: 8px 12px 16px 12px; }\n\
.library-empty { padding: 32px 24px; }\n\
.wallpaper-tile {\
  padding: 0; background-color: @surface; color: @on-surface;\
  border-radius: 8px;\
}\n\
.wallpaper-tile:hover { background-color: alpha(@on-surface, 0.14); }\n\
.wallpaper-tile.current {\
  box-shadow: inset 0 0 0 2px @accent; background-color: alpha(@accent, 0.10);\
}\n\
.wallpaper-tile.applied { box-shadow: inset 0 0 0 3px @accent; }\n\
.wallpaper-caption { padding: 6px 10px 8px 10px; }\n\
.wallpaper-name { font-size: 12px; }\n\
.wallpaper-badge {\
  padding: 0 7px; border-radius: 999px; font-size: 10px; font-weight: 600;\
  background-color: @accent; color: @on-accent;\
}\n\
";

thread_local! {
    static APP_PROVIDER: RefCell<Option<CssProvider>> = const { RefCell::new(None) };
}

pub fn run(dirs: Vec<PathBuf>) -> Result<()> {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(ApplicationFlags::empty())
        .build();

    app.connect_activate(move |app| present(app, dirs.clone()));
    // Clap already consumed argv; do not let GApplication re-parse `library --dir`.
    let _ = app.run_with_args(&["breadpaper"]);
    Ok(())
}

fn present(app: &Application, dirs: Vec<PathBuf>) {
    bread_theme::gtk::apply_shared();
    APP_PROVIDER.with(|cell| bread_theme::gtk::apply_css(APP_CSS, cell));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Wallpapers")
        .default_width(960)
        .default_height(640)
        .build();

    let header = HeaderBar::new();
    window.set_titlebar(Some(&header));

    let refresh = Button::with_label("Refresh");
    header.pack_end(&refresh);

    let root = GBox::new(Orientation::Vertical, 0);

    let chrome = GBox::new(Orientation::Vertical, 6);
    chrome.add_css_class("library-chrome");

    let summary = Label::new(None);
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    summary.add_css_class("dim");
    chrome.append(&summary);

    let status = Label::new(Some("Click a wallpaper to apply it."));
    status.set_xalign(0.0);
    status.set_wrap(true);
    chrome.append(&status);
    root.append(&chrome);

    let flow = FlowBox::new();
    flow.set_selection_mode(SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_max_children_per_line(6);
    flow.set_min_children_per_line(2);
    flow.set_row_spacing(12);
    flow.set_column_spacing(12);
    flow.set_halign(Align::Fill);
    // The picker is a scrolling grid: rows must keep their natural height
    // instead of being stretched to the viewport. Without this a single tile
    // is allocated the whole window height (PAPER-19).
    flow.set_valign(Align::Start);
    flow.set_vexpand(false);
    flow.add_css_class("library-grid");

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .child(&flow)
        .build();

    let empty = Label::new(None);
    empty.set_wrap(true);
    empty.set_justify(gtk4::Justification::Center);
    empty.add_css_class("dim");
    empty.add_css_class("library-empty");
    empty.set_hexpand(true);
    empty.set_vexpand(true);

    let stack = Stack::new();
    stack.set_vexpand(true);
    stack.add_named(&scrolled, Some("grid"));
    stack.add_named(&empty, Some("empty"));
    root.append(&stack);

    window.set_child(Some(&root));
    bread_theme::gtk::bind_window_auto(&window);

    let dirs = Rc::new(dirs);
    let reload = {
        let dirs = dirs.clone();
        let flow = flow.clone();
        let summary = summary.clone();
        let status = status.clone();
        let stack = stack.clone();
        let empty = empty.clone();
        let window = window.clone();
        Rc::new(move || {
            let papers = library::scan(&dirs);
            summary.set_text(&dirs_summary(&dirs, papers.len()));
            empty.set_text(&empty_message(&dirs));
            if papers.is_empty() {
                stack.set_visible_child_name("empty");
            } else {
                stack.set_visible_child_name("grid");
            }
            fill_grid(&flow, &papers, &status, &window);
        })
    };

    window.present();
    reload();
    {
        let reload = reload.clone();
        refresh.connect_clicked(move |_| reload());
    }

    if std::env::var_os("BREADPAPER_UI_MEASURE").is_some() {
        measure_and_exit(
            window.clone(),
            flow.clone(),
            summary.clone(),
            status.clone(),
            empty.clone(),
        );
    }
}

/// Development hook for the headless layout check: print the header copy and
/// the allocated window, tile and thumbnail sizes, then quit. Without
/// `BREADPAPER_UI_MEASURE` this is dead code. See
/// `ci/measure-library-window.sh`.
fn measure_and_exit(
    window: ApplicationWindow,
    flow: FlowBox,
    summary: Label,
    status: Label,
    empty: Label,
) {
    let click_first = std::env::var_os("BREADPAPER_UI_MEASURE_CLICK").is_some();
    if click_first {
        let flow = flow.clone();
        gtk4::glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            if let Some(button) = flow
                .child_at_index(0)
                .and_then(|wrapper| wrapper.downcast_ref::<FlowBoxChild>().cloned())
                .and_then(|child| child.child())
                .and_then(|widget| widget.downcast::<Button>().ok())
            {
                button.emit_clicked();
            }
        });
    }
    let delay = if click_first { 2200 } else { 700 };
    gtk4::glib::timeout_add_local_once(std::time::Duration::from_millis(delay), move || {
        let current = crate::get().ok();
        println!("MEASURE summary={}", summary.text());
        println!("MEASURE status={}", status.text());
        println!("MEASURE empty={}", empty.text().replace('\n', " | "));
        println!(
            "MEASURE window={}x{} tiles={}",
            window.width(),
            window.height(),
            flow.observe_children().n_items()
        );
        let mut i = 0;
        while let Some(wrapper) = flow.child_at_index(i) {
            if let Some(child) = wrapper.downcast_ref::<FlowBoxChild>() {
                let button = child.child().and_then(|w| w.downcast::<Button>().ok());
                let name = button
                    .as_ref()
                    .map(|b| b.widget_name().to_string())
                    .unwrap_or_default();
                let current_mark = match button
                    .as_ref()
                    .map(|b| b.has_css_class("current"))
                    .unwrap_or(false)
                {
                    true => " current",
                    false => "",
                };
                let badge_mark = match button
                    .as_ref()
                    .and_then(tile_badge)
                    .map(|badge| badge.is_visible())
                    .unwrap_or(false)
                {
                    true => " badge=visible",
                    false => " badge=hidden",
                };
                println!(
                    "MEASURE tile={i} box={}x{} button={}x{}{current_mark}{badge_mark} path={name}",
                    child.width(),
                    child.height(),
                    button.as_ref().map(|b| b.width()).unwrap_or(0),
                    button.as_ref().map(|b| b.height()).unwrap_or(0),
                );
                if let Some(button) = button
                    && let Some(col) = button.child().and_then(|w| w.downcast::<GBox>().ok())
                    && let Some(picture) =
                        col.first_child().and_then(|w| w.downcast::<Picture>().ok())
                {
                    println!(
                        "MEASURE picture={i} {}x{}",
                        picture.width(),
                        picture.height()
                    );
                }
            }
            i += 1;
        }
        if let Some(path) = current {
            println!("MEASURE current={}", path.display());
        }
        std::process::exit(0);
    });
}

fn fill_grid(flow: &FlowBox, papers: &[Wallpaper], status: &Label, host: &impl IsA<gtk4::Widget>) {
    while let Some(child) = flow.first_child() {
        flow.remove(&child);
    }
    let current = current_path_for(host);
    for paper in papers {
        let is_current = is_current_path(current.as_deref(), &paper.path);
        flow.insert(&tile(paper, is_current, flow, status), -1);
    }
}

fn current_path_for(widget: &impl IsA<gtk4::Widget>) -> Option<PathBuf> {
    target_output(widget)
        .and_then(|output| {
            crate::current::Current::load()
                .get_output(&output)
                .map(Path::to_path_buf)
        })
        .or_else(|| crate::get().ok())
}

fn target_output(widget: &impl IsA<gtk4::Widget>) -> Option<String> {
    bread_theme::gtk::output_for_widget(widget)
        .or_else(|| bread_utils::hypr::focused_monitor().map(|m| m.name))
}

fn tile(paper: &Wallpaper, is_current: bool, flow: &FlowBox, status: &Label) -> Button {
    let btn = Button::new();
    btn.add_css_class("wallpaper-tile");
    // Keep the tile at its image-plus-label height even when the FlowBox
    // allocates a taller cell for the row.
    btn.set_valign(Align::Start);
    btn.set_vexpand(false);
    btn.set_widget_name(&paper.path.to_string_lossy());
    btn.set_tooltip_text(Some(&paper.path.to_string_lossy()));
    if is_current {
        btn.add_css_class("current");
    }

    let col = GBox::new(Orientation::Vertical, 0);
    col.set_valign(Align::Start);
    col.set_vexpand(false);
    col.append(&thumbnail(&paper.path));

    let caption = GBox::new(Orientation::Horizontal, 6);
    caption.add_css_class("wallpaper-caption");
    let name = Label::new(Some(&paper.name));
    name.add_css_class("wallpaper-name");
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    name.set_max_width_chars(24);
    caption.append(&name);

    let badge = Label::new(Some("Current"));
    badge.add_css_class("wallpaper-badge");
    badge.set_valign(Align::Center);
    badge.set_visible(is_current);
    caption.append(&badge);
    col.append(&caption);
    btn.set_child(Some(&col));

    let path = paper.path.clone();
    let pretty = paper.name.clone();
    let status = status.clone();
    let flow = flow.clone();
    btn.connect_clicked(move |clicked| {
        if !clicked.is_sensitive() {
            return;
        }
        clicked.set_sensitive(false);
        let output = target_output(clicked);
        status.set_text(&match output.as_deref() {
            Some(name) => format!("Applying {pretty} on {name}…"),
            None => format!("Applying {pretty}…"),
        });
        let path = path.clone();
        let pretty = pretty.clone();
        let status = status.clone();
        let flow = flow.clone();
        let clicked = clicked.clone();
        gtk4::glib::spawn_future_local(async move {
            let path_thread = path.clone();
            let output_thread = output.clone();
            let result = gtk4::gio::spawn_blocking(move || match output_thread.as_deref() {
                Some(name) => crate::set_on(&path_thread, name),
                None => crate::set(&path_thread),
            })
            .await;
            clicked.set_sensitive(true);
            match result {
                Ok(Ok(())) => {
                    status.set_text(&match output.as_deref() {
                        Some(name) => format!("Applied {pretty} on {name}"),
                        None => format!("Applied {pretty}"),
                    });
                    // Brief, high-contrast confirmation on the tile itself,
                    // so a click is visible even when it is already current.
                    clicked.add_css_class("applied");
                    let flash = clicked.clone();
                    gtk4::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(1100),
                        move || flash.remove_css_class("applied"),
                    );
                    mark_current(&flow, &path);
                }
                Ok(Err(e)) => status.set_text(&format!("{e:#}")),
                Err(_) => status.set_text("Failed to apply wallpaper"),
            }
        });
    });

    btn
}

fn thumbnail(path: &Path) -> Picture {
    let picture = match Pixbuf::from_file_at_scale(path, THUMB_W, THUMB_H, true) {
        Ok(pb) => Picture::for_paintable(&gtk4::gdk::Texture::for_pixbuf(&pb)),
        Err(_) => Picture::for_filename(path),
    };
    picture.set_content_fit(ContentFit::Cover);
    picture.set_size_request(THUMB_W, THUMB_H);
    picture.set_can_shrink(true);
    picture.set_valign(Align::Start);
    picture.set_vexpand(false);
    picture.set_hexpand(true);
    picture
}

fn is_current_path(current: Option<&Path>, target: &Path) -> bool {
    let Some(current) = current else {
        return false;
    };
    if current == target {
        return true;
    }
    match (current.canonicalize(), target.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn tile_badge(button: &Button) -> Option<Label> {
    fn find_in(widget: &gtk4::Widget) -> Option<Label> {
        let mut child = widget.first_child();
        while let Some(widget) = child {
            if let Some(label) = widget.downcast_ref::<Label>()
                && label.has_css_class("wallpaper-badge")
            {
                return Some(label.clone());
            }
            if let Some(found) = find_in(&widget) {
                return Some(found);
            }
            child = widget.next_sibling();
        }
        None
    }
    find_in(button.upcast_ref())
}

fn mark_current(flow: &FlowBox, current: &Path) {
    let current = current
        .canonicalize()
        .unwrap_or_else(|_| current.to_path_buf());
    let current = current.to_string_lossy();
    let mut i = 0;
    while let Some(wrapper) = flow.child_at_index(i) {
        if let Some(btn) = wrapper
            .downcast_ref::<FlowBoxChild>()
            .and_then(|c| c.child())
            .and_then(|w| w.downcast::<Button>().ok())
        {
            let is_current = btn.widget_name() == current.as_ref();
            if is_current {
                btn.add_css_class("current");
            } else {
                btn.remove_css_class("current");
            }
            if let Some(badge) = tile_badge(&btn) {
                badge.set_visible(is_current);
            }
        }
        i += 1;
    }
}

/// Human-facing folder name: no absolute home prefix, and the packaged BOS
/// collection gets a plain-language label.
fn friendly_dir(dir: &Path) -> String {
    if dir == Path::new(crate::DEFAULT_SYSTEM_LIBRARY) {
        return "System backgrounds".to_string();
    }
    match dir.strip_prefix(bread_utils::xdg::home_dir()) {
        Ok(rest) if !rest.as_os_str().is_empty() => rest.display().to_string(),
        _ => dir.display().to_string(),
    }
}

fn join_human(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

fn existing_dirs(dirs: &[PathBuf]) -> Vec<String> {
    dirs.iter()
        .filter(|dir| dir.is_dir())
        .map(|dir| friendly_dir(dir))
        .collect()
}

fn dirs_summary(dirs: &[PathBuf], count: usize) -> String {
    let found = existing_dirs(dirs);
    let locations = join_human(&found);
    let noun = if count == 1 {
        "wallpaper"
    } else {
        "wallpapers"
    };
    match (count, found.is_empty()) {
        (0, true) => "No wallpaper folders found yet.".to_string(),
        (0, false) => format!("No wallpapers found in {locations}."),
        (_, true) => format!("{count} {noun} found."),
        _ => format!("{count} {noun} in {locations}."),
    }
}

fn empty_message(dirs: &[PathBuf]) -> String {
    let found = existing_dirs(dirs);
    match found.is_empty() {
        true => "No wallpaper folders yet.\n\
             Add PNG, JPG, WebP, GIF or BMP images to Pictures/Backgrounds, \
             then press Refresh.\n\n\
             You can change the folders in breadpaper's config.toml."
            .to_string(),
        false => format!(
            "No wallpapers in {} yet.\n\
             Add PNG, JPG, WebP, GIF or BMP images, then press Refresh.\n\n\
             You can change the folders in breadpaper's config.toml.",
            join_human(&found)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "breadpaper-ui-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn friendly_dir_shortens_home_and_labels_the_system_collection() {
        let home = bread_utils::xdg::home_dir();
        assert_eq!(
            friendly_dir(&home.join(crate::DEFAULT_USER_LIBRARY)),
            crate::DEFAULT_USER_LIBRARY
        );
        assert_eq!(
            friendly_dir(Path::new(crate::DEFAULT_SYSTEM_LIBRARY)),
            "System backgrounds"
        );
        assert_eq!(friendly_dir(Path::new("/opt/walls")), "/opt/walls");
    }

    #[test]
    fn summary_names_a_single_wallpaper_without_developer_text() {
        let dir = tmp_dir("summary-one");
        let text = dirs_summary(std::slice::from_ref(&dir), 1);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(text.starts_with("1 wallpaper in "), "{text}");
        assert!(!text.contains("wallpaper(s)"), "{text}");
        assert!(!text.contains("(missing)"), "{text}");
    }

    #[test]
    fn summary_omits_missing_folders_and_absolute_home_paths() {
        let dir = tmp_dir("summary-present");
        let missing = PathBuf::from("/no/such/breadpaper-wallpapers");
        let text = dirs_summary(&[dir.clone(), missing], 2);
        let _ = std::fs::remove_dir_all(&dir);
        let home = bread_utils::xdg::home_dir();
        assert!(text.contains("2 wallpapers in "), "{text}");
        assert!(!text.contains("/no/such"), "{text}");
        assert!(!text.contains("(missing)"), "{text}");
        assert!(
            !text.contains(&home.to_string_lossy().to_string()),
            "{text}"
        );
    }

    #[test]
    fn summary_handles_no_existing_folders() {
        let text = dirs_summary(&[PathBuf::from("/no/such/breadpaper-wallpapers")], 0);
        assert_eq!(text, "No wallpaper folders found yet.");
        assert!(!text.contains("(missing)"));
    }

    #[test]
    fn empty_message_is_friendly_and_omits_missing_folders() {
        let dir = tmp_dir("empty-message");
        let text = empty_message(&[dir.clone(), PathBuf::from("/no/such/breadpaper-wallpapers")]);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(text.contains("No wallpapers in "), "{text}");
        assert!(text.contains("press Refresh"), "{text}");
        assert!(!text.contains("/no/such"), "{text}");
        assert!(!text.contains("(missing)"), "{text}");
    }

    #[test]
    fn selected_state_matches_same_file_and_rejects_another() {
        let dir = tmp_dir("selected");
        let a = dir.join("a.png");
        let b = dir.join("b.png");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();

        assert!(is_current_path(Some(&a), &a));
        assert!(!is_current_path(Some(&a), &b));
        assert!(!is_current_path(None, &a));

        let link = dir.join("link.png");
        std::os::unix::fs::symlink(&a, &link).unwrap();
        assert!(is_current_path(Some(&link), &a));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
