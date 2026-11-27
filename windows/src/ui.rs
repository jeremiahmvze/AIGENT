// The AIGENT window: Favorites / Apps / Profiles, add, edit, launch, restart, settings.
use crate::core::*;
use crate::{icon, win};
use eframe::egui::{self, Align, Color32, Id, Layout, RichText, Sense, TextureHandle, UiBuilder, Vec2};
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const QUICK_EMOJI: [&str; 40] = [
    "💼", "🏠", "💻", "🧪", "🛠️", "📚", "🎓", "🧠", "💡", "🚀", "⭐️", "❤️", "🔥", "⚡️", "🌙", "☀️", "🌈", "🌊", "🌲", "🍀",
    "🐶", "🐱", "🦊", "🐻", "🐼", "🦉", "🐙", "🦄", "🤖", "👾", "🎨", "🎵", "🎮", "📷", "✈️", "💰", "📈", "🏋️", "🛒", "🔒",
];

pub fn run() {
    let px = icon::render_app_icon(256);
    let rgba = px.pixels().iter().flat_map(|p| { let c = p.demultiply(); [c.red(), c.green(), c.blue(), c.alpha()] }).collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("AIGENT").with_inner_size([560.0, 560.0]).with_min_inner_size([480.0, 420.0])
            .with_icon(egui::IconData { rgba, width: 256, height: 256 }),
        ..Default::default()
    };
    let _ = eframe::run_native("AIGENT", options, Box::new(|cc| { set_fonts(&cc.egui_ctx); Ok(Box::new(App::new())) }));
}

/// Segoe UI for text (with egui's own fonts as fallback for symbols and emoji).
fn set_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    for (name, file, first) in [("segoe", "segoeui.ttf", true), ("segoe-emoji", "seguiemj.ttf", false)] {
        let Ok(data) = std::fs::read(format!(r"{windir}\Fonts\{file}")) else { continue };
        fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        let list = fonts.families.entry(egui::FontFamily::Proportional).or_default();
        if first { list.insert(0, name.into()) } else { list.push(name.into()) }
    }
    ctx.set_fonts(fonts);
}

#[derive(Clone)]
enum Item { App(usize), Profile(Profile) }

impl Item {
    fn key(&self, installs: &[Install]) -> String {
        match self { Item::App(i) => app_key(&installs[*i].name), Item::Profile(p) => p.key() }
    }
}

enum Action {
    Launch(Item), Show(Item, Vec<u32>), Restart(String), ForceQuit(Vec<u32>), ToggleFavorite(String), MoveFavorite(usize, usize),
    Edit(Profile), Rebuild(Profile), ShowData(Profile), AskDelete(Profile), ToggleSection(&'static str),
}

enum Dialog { Edit(Editor), Delete(Profile), CloseAll(Vec<(String, Vec<u32>)>), Settings, Error(String) }

struct Editor { profile: Profile, name: String, look: Look, color: [u8; 3], error: Option<String> }

struct App {
    store: Store,
    installs: Vec<Install>,
    running: Vec<Running>,
    refreshed: Instant,
    installs_refreshed: Instant,
    base_icons: HashMap<String, Option<tiny_skia::Pixmap>>,
    textures: HashMap<String, TextureHandle>,
    new_name: String,
    new_source: usize,
    busy: Option<(String, mpsc::Receiver<Result<(), String>>)>,
    dialog: Option<Dialog>,
    open_at_login: bool,
}

impl App {
    fn new() -> App {
        let installs = win::installed();
        App {
            running: crate::running(&installs), installs, store: Store::load(), refreshed: Instant::now(), installs_refreshed: Instant::now(),
            base_icons: HashMap::new(), textures: HashMap::new(), new_name: String::new(), new_source: 0, busy: None, dialog: None,
            open_at_login: win::open_at_login(),
        }
        .dev_open_editor()
    }

    /// Dev builds only: AIGENT_DEV_EDIT=1 opens the editor for the first profile (for screenshots).
    fn dev_open_editor(mut self) -> Self {
        if cfg!(not(windows)) && std::env::var_os("AIGENT_DEV_EDIT").is_some() {
            if let Some(p) = self.store.profiles.first().cloned() { self.apply(&egui::Context::default(), Action::Edit(p)) }
        }
        self
    }

    fn refresh(&mut self) {
        if self.installs_refreshed.elapsed() > Duration::from_secs(30) {
            self.installs = win::installed();
            self.installs_refreshed = Instant::now();
            self.new_source = self.new_source.min(self.installs.len().saturating_sub(1));
        }
        self.running = crate::running(&self.installs);
        self.refreshed = Instant::now();
    }

    fn app_index(&self, source: &str) -> Option<usize> { self.installs.iter().position(|i| i.name == source) }

    fn pids(&self, item: &Item) -> Vec<u32> {
        match item {
            Item::App(i) => pids_for(&self.running, *i, None),
            Item::Profile(p) => self.app_index(&p.source).map_or(vec![], |i| pids_for(&self.running, i, Some(&p.data_dir))),
        }
    }

    fn work(&mut self, label: &str, ctx: &egui::Context, job: impl FnOnce() -> Result<(), String> + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || { let _ = tx.send(job()); ctx.request_repaint(); });
        self.busy = Some((label.into(), rx));
    }

    fn base_icon(&mut self, source: &str) -> Option<tiny_skia::Pixmap> {
        if !self.base_icons.contains_key(source) {
            let px = self.installs.iter().find(|i| i.name == source).and_then(win::app_icon);
            self.base_icons.insert(source.into(), px);
        }
        self.base_icons[source].clone()
    }

    fn texture(&mut self, ctx: &egui::Context, key: String, make: impl FnOnce(&mut Self) -> Option<tiny_skia::Pixmap>) -> Option<TextureHandle> {
        if !self.textures.contains_key(&key) {
            let px = make(self)?;
            let img = egui::ColorImage::from_rgba_premultiplied([px.width() as usize, px.height() as usize], px.data());
            self.textures.insert(key.clone(), ctx.load_texture(&key, img, egui::TextureOptions::LINEAR));
        }
        self.textures.get(&key).cloned()
    }

    fn icon_for(&mut self, ctx: &egui::Context, item: &Item) -> Option<TextureHandle> {
        match item {
            Item::App(i) => { let name = self.installs[*i].name.clone(); self.texture(ctx, format!("app:{name}"), |s| s.base_icon(&name)) }
            Item::Profile(p) => {
                let p = p.clone();
                self.texture(ctx, format!("{:?}", p.icon_path()), |s| { let b = s.base_icon(&p.source); Some(icon::render_icon(b.as_ref(), &p.look, 64)) })
            }
        }
    }

    fn save(&mut self) {
        if let Err(e) = self.store.save() { self.dialog = Some(Dialog::Error(e)) }
    }

    // MARK: Rows

    fn row(&mut self, ui: &mut egui::Ui, item: &Item, fav_index: Option<usize>, actions: &mut Vec<Action>) {
        let key = item.key(&self.installs);
        let pids = self.pids(item);
        let on = !pids.is_empty();
        let tex = self.icon_for(ui.ctx(), item);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click_and_drag());
        if resp.hovered() || resp.context_menu_opened() { ui.painter().rect_filled(rect, 6.0, ui.visuals().faint_bg_color); }
        if let Some(i) = fav_index {
            resp.dnd_set_drag_payload(i);
            if resp.dnd_hover_payload::<usize>().is_some() {
                ui.painter().hline(rect.x_range(), rect.top(), ui.visuals().selection.stroke);
            }
            if let Some(from) = resp.dnd_release_payload::<usize>() { actions.push(Action::MoveFavorite(*from, i)) }
        }
        let (name, caption, installed) = match item {
            Item::App(_) => (self.installs.iter().find(|i| app_key(&i.name) == key).unwrap().name.clone(), "Default profile".to_string(), true),
            Item::Profile(p) => {
                let installed = self.app_index(&p.source).is_some();
                (p.name.clone(), format!("{}{}", p.source, if installed { "" } else { " · not installed" }), installed)
            }
        };
        let star = self.store.favorites_in_lists && fav_index.is_none() && self.store.is_favorite(&key);
        let mut row = ui.new_child(UiBuilder::new().max_rect(rect.shrink2(Vec2::new(8.0, 4.0))).layout(Layout::left_to_right(Align::Center)));
        match tex {
            Some(t) => { row.add(egui::Image::from_texture(&t).fit_to_exact_size(Vec2::splat(32.0))); }
            None => { row.allocate_space(Vec2::splat(32.0)); }
        }
        row.vertical(|ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(&name).strong());
                if star { ui.label(RichText::new("★").color(Color32::from_rgb(255, 204, 0))).on_hover_text("Favorite"); }
            });
            ui.label(RichText::new(caption).small().weak());
        });
        row.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add_enabled(installed, egui::Button::new(if on { "Show" } else { "Launch" })).clicked() {
                actions.push(if on { Action::Show(item.clone(), pids.clone()) } else { Action::Launch(item.clone()) });
            }
            if on {
                if ui.button("Restart").on_hover_text("Quit and reopen").clicked() { actions.push(Action::Restart(key.clone())) }
                let (dot, r) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, Color32::from_rgb(48, 209, 88));
                r.on_hover_text("Running");
            }
        });
        resp.context_menu(|ui| {
            let fav = self.store.is_favorite(&key);
            if ui.button(if fav { "Remove from Favorites" } else { "Add to Favorites" }).clicked() { actions.push(Action::ToggleFavorite(key.clone())); ui.close() }
            if on && ui.button("Force Quit").clicked() { actions.push(Action::ForceQuit(pids.clone())); ui.close() }
            if let Item::Profile(p) = item {
                ui.separator();
                if ui.button("Edit Name & Icon…").clicked() { actions.push(Action::Edit(p.clone())); ui.close() }
                if ui.button("Rebuild Shortcut & Icon").clicked() { actions.push(Action::Rebuild(p.clone())); ui.close() }
                if ui.button("Show Data Folder").clicked() { actions.push(Action::ShowData(p.clone())); ui.close() }
                ui.separator();
                if ui.button("Delete Profile…").clicked() { actions.push(Action::AskDelete(p.clone())); ui.close() }
            }
        });
    }

    fn section(&mut self, ui: &mut egui::Ui, title: &'static str, items: Vec<Item>, favorites: bool, actions: &mut Vec<Action>, empty: Option<&str>) {
        let open = !self.store.collapsed.iter().any(|s| s == title);
        let r = egui::CollapsingHeader::new(RichText::new(title).strong()).id_salt(title).open(Some(open)).show(ui, |ui| {
            if items.is_empty() { if let Some(e) = empty { ui.label(RichText::new(e).weak()); } }
            for (i, item) in items.iter().enumerate() { self.row(ui, item, favorites.then_some(i), actions) }
        });
        if r.header_response.clicked() { actions.push(Action::ToggleSection(title)) }
    }

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        let fail = |e: String| Some(Dialog::Error(e));
        match action {
            // No window showing (e.g. it's in the tray): starting it again makes the running copy show itself.
            Action::Show(item, pids) => if !win::focus(&pids) { self.apply(ctx, Action::Launch(item)) },
            Action::Launch(Item::App(i)) => if let Err(e) = win::start(&self.installs[i], None) { self.dialog = fail(e) },
            Action::Launch(Item::Profile(p)) => if let Err(e) = crate::launch_profile(&p.id) { self.dialog = fail(e) },
            Action::Restart(key) => self.work("Restarting…", ctx, move || crate::restart(&key)),
            Action::ForceQuit(pids) => { std::thread::spawn(move || win::quit(&pids, true)); }
            Action::ToggleFavorite(key) => { self.store.toggle_favorite(&key); self.save() }
            Action::MoveFavorite(from, to) => {
                let favs = self.favorites().iter().map(|i| i.key(&self.installs)).collect::<Vec<_>>();
                if from < favs.len() && to < favs.len() {
                    let mut favs = favs;
                    let k = favs.remove(from);
                    favs.insert(to, k);
                    self.store.favorites = favs;
                    self.save();
                }
            }
            Action::Edit(p) => {
                let color = icon::hex(&p.look.color).unwrap_or([10, 132, 255]);
                self.dialog = Some(Dialog::Edit(Editor { name: p.name.clone(), look: p.look.clone(), color, error: None, profile: p }));
            }
            Action::Rebuild(p) => self.work("Rebuilding…", ctx, move || crate::rebuild(&p)),
            Action::ShowData(p) => win::show_folder(&p.data_dir),
            Action::AskDelete(p) => self.dialog = Some(Dialog::Delete(p)),
            Action::ToggleSection(t) => {
                if self.store.collapsed.iter().any(|s| s == t) { self.store.collapsed.retain(|s| s != t) } else { self.store.collapsed.push(t.into()) }
                self.save();
            }
        }
    }

    fn items(&self) -> (Vec<Item>, Vec<Item>) {
        ((0..self.installs.len()).map(Item::App).collect(), self.store.profiles.iter().cloned().map(Item::Profile).collect())
    }

    /// Favorites that still exist, in the user's order.
    fn favorites(&self) -> Vec<Item> {
        let (apps, profiles) = self.items();
        let all: Vec<Item> = apps.into_iter().chain(profiles).collect();
        self.store.favorites.iter().filter_map(|k| all.iter().find(|i| i.key(&self.installs) == *k).cloned()).collect()
    }

    fn bulk_targets(&self) -> Vec<Item> {
        let (apps, profiles) = self.items();
        if self.store.all_includes_defaults { apps.into_iter().chain(profiles).collect() } else { profiles }
    }

    fn add(&mut self, ctx: &egui::Context) {
        let Some(src) = self.installs.get(self.new_source).map(|i| i.name.clone()) else { return };
        let name = std::mem::take(&mut self.new_name);
        self.work("Building…", ctx, move || crate::launch_profile(&crate::add_profile(&src, &name)?.id));
    }

    // MARK: Dialogs

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.take() else { return };
        let mut keep = true;
        let modal = egui::Modal::new(Id::unique("dialog")).show(ctx, |ui| match &dialog {
            Dialog::Error(e) => {
                ui.heading("Something went wrong");
                ui.label(e);
                ui.add_space(8.0);
                if ui.button("OK").clicked() { keep = false }
            }
            Dialog::Delete(p) => {
                ui.set_max_width(380.0);
                ui.heading(format!("Delete “{}”?", p.name));
                ui.label("Its Start Menu shortcut is removed. Its data folder (sign-in, sessions) is kept, so re-adding the same name restores it.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        if let Err(e) = crate::delete_profile(&p.id) { self.dialog = Some(Dialog::Error(e)) }
                        self.store = Store::load();
                        keep = false;
                    }
                    if ui.button("Cancel").clicked() { keep = false }
                });
            }
            Dialog::CloseAll(apps) => {
                ui.heading(format!("Quit {} app{}?", apps.len(), if apps.len() == 1 { "" } else { "s" }));
                for (name, _) in apps { ui.label(name); }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Quit All").clicked() {
                        let pids: Vec<u32> = apps.iter().flat_map(|(_, p)| p.clone()).collect();
                        self.work("Quitting…", ctx, move || { win::quit(&pids, false); Ok(()) });
                        keep = false;
                    }
                    if ui.button("Cancel").clicked() { keep = false }
                });
            }
            Dialog::Settings => {
                ui.heading("Settings");
                ui.add_space(4.0);
                let mut login = self.open_at_login;
                if ui.checkbox(&mut login, "Open at sign-in").changed() {
                    if let Err(e) = win::set_open_at_login(login) { self.dialog = Some(Dialog::Error(e)); keep = false; }
                    self.open_at_login = win::open_at_login();
                }
                let (mut a, mut b) = (self.store.all_includes_defaults, self.store.favorites_in_lists);
                ui.checkbox(&mut a, "Include default Claude and ChatGPT in Launch All / Close All");
                ui.checkbox(&mut b, "Also show Favorites in their Apps / Profiles section");
                if (a, b) != (self.store.all_includes_defaults, self.store.favorites_in_lists) {
                    (self.store.all_includes_defaults, self.store.favorites_in_lists) = (a, b);
                    self.save();
                }
                ui.add_space(8.0);
                if ui.button("Done").clicked() { keep = false }
            }
            Dialog::Edit(_) => {}
        });
        if let Dialog::Edit(mut ed) = dialog {
            let mut save = false;
            let r = egui::Modal::new(Id::unique("editor")).show(ctx, |ui| { keep = self.editor(ui, &mut ed, &mut save); });
            if r.should_close() { keep = false }
            if save {
                let (id, name, look) = (ed.profile.id.clone(), ed.name.trim().to_string(), ed.look.clone());
                self.work("Saving…", ctx, move || crate::edit_profile(&id, &name, look));
                keep = false;
            }
            if keep && self.dialog.is_none() { self.dialog = Some(Dialog::Edit(ed)) }
            return;
        }
        if modal.should_close() && !matches!(dialog, Dialog::CloseAll(_) | Dialog::Delete(_)) { keep = false }
        if keep && self.dialog.is_none() { self.dialog = Some(dialog) }
    }

    /// Returns whether to keep the editor open.
    fn editor(&mut self, ui: &mut egui::Ui, ed: &mut Editor, save: &mut bool) -> bool {
        let mut keep = true;
        ui.set_width(500.0);
        ui.heading("Edit Profile");
        ui.add_space(6.0);
        ed.look.color = icon::to_hex(ed.color);
        let source = ed.profile.source.clone();
        let look = ed.look.clone();
        let preview = self.texture(ui.ctx(), format!("preview:{source}:{look:?}"), |s| { let b = s.base_icon(&source); Some(icon::render_icon(b.as_ref(), &look, 128)) });
        ui.horizontal_top(|ui| {
            if let Some(t) = preview { ui.add(egui::Image::from_texture(&t).fit_to_exact_size(Vec2::splat(128.0))).on_hover_text("Icon preview"); }
            ui.add_space(12.0);
            egui::Grid::new("editor-grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut ed.name).desired_width(240.0));
                ui.end_row();
                ui.label("Emoji or letter");
                ui.horizontal(|ui| {
                    let hint = if ed.look.glyph { "1 character" } else { "1–2 characters, or empty" };
                    let r = ui.add(egui::TextEdit::singleline(&mut ed.look.badge).hint_text(hint).desired_width(150.0));
                    if r.changed() && graphemes(&ed.look.badge) > ed.look.max_badge() { ed.look.badge = last_graphemes(&ed.look.badge, ed.look.max_badge()) }
                    if ui.button("More…").on_hover_text("All emoji and symbols (or press Win + .)").clicked() {
                        r.request_focus();
                        std::thread::spawn(|| { std::thread::sleep(Duration::from_millis(150)); win::emoji_panel() });
                    }
                });
                ui.end_row();
                ui.label("Show as");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut ed.look.glyph, false, "Badge");
                    ui.selectable_value(&mut ed.look.glyph, true, "Whole Icon");
                });
                ui.end_row();
                ui.label("Size");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut ed.look.framed, false, "Full Size");
                    ui.selectable_value(&mut ed.look.framed, true, "Framed");
                });
                ui.end_row();
                ui.label(if ed.look.glyph { "Background" } else { "Badge color" });
                ui.horizontal(|ui| {
                    egui::color_picker::color_edit_button_srgb(ui, &mut ed.color);
                    for hex in PALETTE {
                        let c = icon::hex(hex).unwrap();
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 9.0, Color32::from_rgb(c[0], c[1], c[2]));
                        if resp.on_hover_text(hex).clicked() { ed.color = c }
                    }
                });
                ui.end_row();
                if !ed.look.glyph {
                    ui.label("Image");
                    ui.horizontal(|ui| {
                        if ui.button("Choose Image…").clicked() {
                            if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp", "ico"]).pick_file() {
                                match import_image(&path) { Ok(p) => { ed.look.image_path = Some(p); ed.error = None } Err(e) => ed.error = Some(e) }
                            }
                        }
                        if ed.look.image_path.is_some() && ui.button("Use App Icon").clicked() { ed.look.image_path = None }
                    });
                    ui.end_row();
                }
            });
        });
        if graphemes(&ed.look.badge) > ed.look.max_badge() { ed.look.badge = last_graphemes(&ed.look.badge, ed.look.max_badge()) }
        ui.add_space(8.0);
        egui::Grid::new("emoji").spacing([6.0, 6.0]).show(ui, |ui| {
            for (i, e) in QUICK_EMOJI.iter().enumerate() {
                let selected = ed.look.badge == *e;
                if ui.add(egui::Button::new(RichText::new(*e).size(20.0)).selected(selected).min_size(Vec2::splat(40.0))).on_hover_text(format!("Use {e}")).clicked() {
                    ed.look.badge = e.to_string();
                }
                if i % 10 == 9 { ui.end_row() }
            }
        });
        if let Some(e) = &ed.error { ui.colored_label(Color32::from_rgb(255, 69, 58), e); }
        let name_ok = clean_name(&ed.name);
        if let (Err(e), false) = (&name_ok, ed.name.trim().is_empty()) { ui.colored_label(Color32::from_rgb(255, 69, 58), e); }
        ui.label(RichText::new("Sign-in and sessions stay the same. Changes apply right away, even while it's running.").small().weak());
        ui.add_space(6.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add_enabled(name_ok.is_ok(), egui::Button::new("Save")).clicked() || (name_ok.is_ok() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                *save = true;
            }
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) { keep = false }
        });
        keep
    }
}

/// Copy a picked image into AIGENT's folder so the profile doesn't depend on the original file.
fn import_image(path: &std::path::Path) -> Result<String, String> {
    let img = image::open(path).map_err(|_| "That file isn't an image.".to_string())?;
    std::fs::create_dir_all(icons_dir()).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dest = icons_dir().join(format!("image-{stamp:x}.png"));
    img.save(&dest).map_err(|e| e.to_string())?;
    Ok(dest.display().to_string())
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some((_, rx)) = &self.busy {
            if let Ok(result) = rx.try_recv() {
                self.busy = None;
                if let Err(e) = result { self.dialog = Some(Dialog::Error(e)) }
                self.store = Store::load();
                self.textures.retain(|k, _| k.starts_with("app:"));
                self.refresh();
            }
        }
        if self.refreshed.elapsed() > Duration::from_secs(2) { self.refresh() }
        ctx.request_repaint_after(Duration::from_secs(2));

        let mut actions = vec![];
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let all = if self.store.all_includes_defaults { " and default app" } else { "" };
                if ui.button("Launch All").on_hover_text(format!("Open every profile{all}")).clicked() {
                    for item in self.bulk_targets() { if self.pids(&item).is_empty() { actions.push(Action::Launch(item)) } }
                }
                if ui.button("Close All").on_hover_text(format!("Quit every running profile{all}")).clicked() {
                    let apps: Vec<(String, Vec<u32>)> = self.bulk_targets().iter().filter_map(|item| {
                        let pids = self.pids(item);
                        let name = match item { Item::App(i) => self.installs[*i].name.clone(), Item::Profile(p) => p.display() };
                        (!pids.is_empty()).then_some((name, pids))
                    }).collect();
                    if !apps.is_empty() { self.dialog = Some(Dialog::CloseAll(apps)) }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Settings…").clicked() { self.open_at_login = win::open_at_login(); self.dialog = Some(Dialog::Settings) }
                });
            });
            ui.add_space(2.0);
        });
        egui::Panel::bottom("add").show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if self.installs.is_empty() {
                    ui.label(RichText::new("Install Claude or ChatGPT to add profiles.").weak());
                    return;
                }
                egui::ComboBox::from_id_salt("source").selected_text(&self.installs[self.new_source].name).show_ui(ui, |ui| {
                    for (i, inst) in self.installs.iter().enumerate() { ui.selectable_value(&mut self.new_source, i, &inst.name); }
                });
                let label = self.busy.as_ref().map_or("Add Profile".to_string(), |(l, _)| l.clone());
                let can_add = self.busy.is_none() && clean_name(&self.new_name).is_ok();
                let button_width = 110.0;
                let r = ui.add(egui::TextEdit::singleline(&mut self.new_name).hint_text("New profile name (e.g. Work)").desired_width(ui.available_width() - button_width));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && can_add { self.add(&ctx) }
                if ui.add_enabled(can_add, egui::Button::new(label).min_size(Vec2::new(button_width - 8.0, 0.0))).clicked() { self.add(&ctx) }
            });
            ui.add_space(8.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                let favorites = self.favorites();
                let (apps, profiles) = self.items();
                let show_all = self.store.favorites_in_lists;
                let not_fav = |items: Vec<Item>, s: &Self| items.into_iter().filter(|i| show_all || !s.store.is_favorite(&i.key(&s.installs))).collect::<Vec<_>>();
                if !favorites.is_empty() { self.section(ui, "Favorites", favorites, true, &mut actions, None) }
                let apps = not_fav(apps, self);
                self.section(ui, "Apps", apps, false, &mut actions, Some("Claude and ChatGPT weren't found. Install one, then reopen AIGENT."));
                let profiles = not_fav(profiles, self);
                self.section(ui, "Profiles", profiles, false, &mut actions, Some("No profiles yet. Add one below."));
            });
        });
        for a in actions { self.apply(&ctx, a) }
        self.dialogs(&ctx);
        #[cfg(not(windows))]
        dev_screenshot(&ctx);
    }
}

/// Dev builds only: AIGENT_SCREENSHOT=out.png saves the window after a few frames and quits.
#[cfg(not(windows))]
fn dev_screenshot(ctx: &egui::Context) {
    static FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let Ok(path) = std::env::var("AIGENT_SCREENSHOT") else { return };
    if FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 10 {
        ctx.request_screenshot(move |img| {
            let _ = image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, img.as_raw().to_vec()).map(|i| i.save(&path));
            std::process::exit(0);
        });
    }
    ctx.request_repaint();
}
