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
.wallpaper-tile.current { box-shadow: inset 0 0 0 2px @accent; }\n\
.wallpaper-name { padding: 8px 10px; font-size: 12px; }\n\
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

    let dirs = Rc::new(dirs);
    let reload = {
        let dirs = dirs.clone();
        let flow = flow.clone();
        let summary = summary.clone();
        let status = status.clone();
        let stack = stack.clone();
        let empty = empty.clone();
        Rc::new(move || {
            let papers = library::scan(&dirs);
            summary.set_text(&dirs_summary(&dirs, papers.len()));
            empty.set_text(&empty_message(&dirs));
            if papers.is_empty() {
                stack.set_visible_child_name("empty");
            } else {
                stack.set_visible_child_name("grid");
            }
            fill_grid(&flow, &papers, &status);
        })
    };

    reload();
    {
        let reload = reload.clone();
        refresh.connect_clicked(move |_| reload());
    }

    window.present();
}

fn fill_grid(flow: &FlowBox, papers: &[Wallpaper], status: &Label) {
    while let Some(child) = flow.first_child() {
        flow.remove(&child);
    }
    let current = crate::get().ok();
    for paper in papers {
        let is_current = current.as_deref() == Some(paper.path.as_path());
        flow.insert(&tile(paper, is_current, flow, status), -1);
    }
}

fn tile(paper: &Wallpaper, is_current: bool, flow: &FlowBox, status: &Label) -> Button {
    let btn = Button::new();
    btn.add_css_class("wallpaper-tile");
    btn.set_widget_name(&paper.path.to_string_lossy());
    btn.set_tooltip_text(Some(&paper.path.to_string_lossy()));
    if is_current {
        btn.add_css_class("current");
    }

    let col = GBox::new(Orientation::Vertical, 0);
    col.append(&thumbnail(&paper.path));

    let name = Label::new(Some(&paper.name));
    name.add_css_class("wallpaper-name");
    name.set_xalign(0.0);
    name.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    name.set_max_width_chars(24);
    col.append(&name);
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
        status.set_text(&format!("Applying {pretty}…"));
        let path = path.clone();
        let pretty = pretty.clone();
        let status = status.clone();
        let flow = flow.clone();
        let clicked = clicked.clone();
        gtk4::glib::spawn_future_local(async move {
            let path_thread = path.clone();
            let result = gtk4::gio::spawn_blocking(move || crate::set(&path_thread)).await;
            clicked.set_sensitive(true);
            match result {
                Ok(Ok(())) => {
                    status.set_text(&format!("Applied {pretty}"));
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
    picture.set_hexpand(true);
    picture
}

fn mark_current(flow: &FlowBox, current: &Path) {
    let current = current.to_string_lossy();
    let mut i = 0;
    while let Some(wrapper) = flow.child_at_index(i) {
        if let Some(btn) = wrapper
            .downcast_ref::<FlowBoxChild>()
            .and_then(|c| c.child())
            .and_then(|w| w.downcast::<Button>().ok())
        {
            if btn.widget_name() == current.as_ref() {
                btn.add_css_class("current");
            } else {
                btn.remove_css_class("current");
            }
        }
        i += 1;
    }
}

fn dirs_summary(dirs: &[PathBuf], count: usize) -> String {
    let listed = dirs
        .iter()
        .map(|d| {
            if d.is_dir() {
                d.display().to_string()
            } else {
                format!("{} (missing)", d.display())
            }
        })
        .collect::<Vec<_>>()
        .join(" · ");
    format!("{count} wallpaper(s) · {listed}")
}

fn empty_message(dirs: &[PathBuf]) -> String {
    let listed = dirs
        .iter()
        .map(|d| d.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "No wallpapers found.\nAdd png/jpg/webp/gif/bmp files under:\n{listed}\n\nOr set library_dirs in {}",
        crate::config::Config::path().display()
    )
}
