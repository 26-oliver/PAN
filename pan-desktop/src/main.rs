#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[macro_use]
mod locale;
mod artwork;
mod catalog;
mod details;
mod input;
mod library;
mod library_view;
mod media;
mod playback;
mod theme;
mod video;
mod vlc;

use eframe::egui::{self, *};
use media::Media;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use theme::*;
use vlc::{Player, Snapshot, Status};

struct Pan {
    player: Option<Player>,
    engine_error: Option<String>,
    error: Option<String>,
    queue: Vec<Media>,
    current: usize,
    volume: i32,
    muted: bool,
    speed: f32,
    repeat: bool,
    queue_open: bool,
    library: library::Library,
    library_view: library_view::View,
    library_open: bool,
    network_open: bool,
    network: String,
    fullscreen: bool,
    native_toolbar: bool,
    toolbar_language: locale::Language,
    video: Option<TextureHandle>,
    logo: TextureHandle,
    seek: Option<f32>,
    catalog: catalog::Catalog,
    preferences: details::Preferences,
    editor: Option<details::Editor>,
    active: Option<playback::Active>,
    engine_preferences: (String, u32),
    probe: Option<mpsc::Receiver<(String, Option<catalog::Entry>)>>,
    probes: HashSet<String>,
    last_scan: Instant,
}

impl Pan {
    fn new(cc: &eframe::CreationContext<'_>, logo: &IconData, initial: Vec<PathBuf>) -> Self {
        let ctx = &cc.egui_ctx;
        theme::install(ctx);
        let mut catalog = catalog::Catalog::load();
        locale::set(catalog.data.settings.ui_language);
        catalog
            .data
            .settings
            .keys
            .resize_with(catalog::SHORTCUT_KEYS.len(), String::new);
        for (index, key) in catalog.data.settings.keys.iter_mut().enumerate() {
            if Key::from_name(key).is_none() {
                *key = catalog::SHORTCUT_KEYS[index].into();
            }
        }
        let settings = &catalog.data.settings;
        let engine_preferences = (settings.language.clone(), settings.subtitle_size);
        let (player, engine_error) = match Player::new(&engine_preferences.0, engine_preferences.1)
        {
            Ok(player) => (Some(player), None),
            Err(error) => (None, Some(error)),
        };
        let mut app = Self {
            player,
            engine_error,
            error: None,
            queue: Vec::new(),
            current: 0,
            volume: 80,
            muted: false,
            speed: 1.0,
            repeat: false,
            queue_open: false,
            library: library::Library::new(catalog.data.location.clone()),
            library_view: library_view::View::default(),
            library_open: false,
            network_open: false,
            network: String::new(),
            fullscreen: false,
            native_toolbar: input::glass_toolbar(cc),
            toolbar_language: locale::current(),
            video: None,
            logo: ctx.load_texture(
                "PAN",
                ColorImage::from_rgba_unmultiplied(
                    [logo.width as usize, logo.height as usize],
                    &logo.rgba,
                ),
                TextureOptions::NEAREST,
            ),
            seek: None,
            catalog,
            preferences: details::Preferences::default(),
            editor: None,
            active: None,
            engine_preferences,
            probe: None,
            probes: HashSet::new(),
            last_scan: Instant::now(),
        };
        app.open_paths(initial);
        app
    }

    fn open_paths(&mut self, paths: Vec<PathBuf>) {
        let mut media = Vec::new();
        let mut failure = None;
        for path in paths {
            match Media::from_path(&path) {
                Ok(item) => media.push(item),
                Err(error) => failure = Some(error),
            }
        }
        if !media.is_empty() {
            self.library_open = false;
            self.replace_queue(media, 0);
        }
        if let Some(error) = failure {
            self.error = Some(error);
        }
    }

    fn replace_queue(&mut self, items: Vec<Media>, index: usize) {
        self.finish_position();
        self.active = None;
        self.queue = items;
        self.select(index);
    }

    fn select(&mut self, index: usize) {
        let Some(item) = self.queue.get(index).cloned() else {
            return;
        };
        self.finish_position();
        self.current = index;
        self.error = None;
        self.video = None;
        self.seek = None;
        self.begin_playback(
            &item,
            !self
                .active
                .as_ref()
                .is_some_and(|active| active.ended && self.repeat),
        );
    }

    fn save_catalog(&mut self) {
        if let Err(error) = self.catalog.save() {
            self.library.message = Some((
                tf!(
                    "Could not save PAN data: {error}",
                    "No se pudieron guardar los datos de PAN: {error}",
                    "Impossible d’enregistrer les données de PAN : {error}"
                ),
                true,
            ));
        }
    }

    fn catalog_poll(&mut self, ctx: &Context) {
        let mut changed = false;
        if let Some(path) = self.library.poll_move() {
            let previous = self.catalog.data.location.replace(path.clone());
            if let Err(error) = self.catalog.save() {
                self.catalog.data.location = previous;
                self.library.message = Some((
                    tf!("The files were copied, but PAN could not save the new location: {error}", "Se hizo la copia, pero PAN no pudo guardar la ubicación: {error}", "Les fichiers ont été copiés, mais PAN n’a pas pu enregistrer le nouvel emplacement : {error}"),
                    true,
                ));
            } else {
                if let Some(old_root) = self.library.root.replace(path.clone()) {
                    for item in &mut self.queue {
                        if let Some(relative) =
                            item.url.to_file_path().ok().and_then(|file| {
                                file.strip_prefix(&old_root).ok().map(PathBuf::from)
                            })
                        {
                            if let Ok(url) = url::Url::from_file_path(path.join(relative)) {
                                item.url = url;
                            }
                        }
                    }
                }
                self.library.reload();
                self.library.message = Some((
                    tr!("PAN is using the new folder. The previous copy is still available.", "PAN ya usa la carpeta nueva. La copia anterior sigue en su sitio.", "PAN utilise le nouveau dossier. La copie précédente est toujours disponible.").into(),
                    false,
                ));
            }
        }
        if self.library_open
            && !self.library.busy()
            && self.last_scan.elapsed() > Duration::from_secs(4)
        {
            self.library.reload();
            self.last_scan = Instant::now();
        }
        for item in &self.library.items {
            let entry = self
                .catalog
                .data
                .entries
                .entry(item.id.clone())
                .or_default();
            if entry.filename != item.media.title {
                entry.filename.clone_from(&item.media.title);
                changed = true;
            }
        }
        if let Some(task) = &self.probe {
            loop {
                match task.try_recv() {
                    Ok((id, info)) => {
                        let Some(entry) = self.catalog.data.entries.get_mut(&id) else {
                            continue;
                        };
                        if let Some(info) = info {
                            if self.editor.as_ref().is_none_or(|editor| editor.id != id) {
                                if entry.title.is_empty() && info.title != entry.filename {
                                    entry.title = info.title;
                                }
                                if entry.creator.is_empty() {
                                    entry.creator = info.creator;
                                }
                                if entry.description.is_empty() {
                                    entry.description = info.description;
                                }
                            }
                            entry.duration = info.duration;
                        }
                        entry.scanned = true;
                        changed = true;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.probe = None;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if changed {
            self.save_catalog();
        }

        if self.probe.is_none() && self.player.is_some() && !self.library.busy() {
            let items = self
                .library
                .items
                .iter()
                .filter(|item| {
                    self.catalog
                        .data
                        .entries
                        .get(&item.id)
                        .is_some_and(|entry| !entry.scanned)
                        && !self.probes.contains(&item.id)
                })
                .take(24)
                .map(|item| (item.id.clone(), item.media.url.clone()))
                .collect::<Vec<_>>();
            if !items.is_empty() {
                self.probes.extend(items.iter().map(|(id, _)| id.clone()));
                let (tx, rx) = mpsc::channel();
                self.probe = Some(rx);
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    if let Ok(player) = Player::new("", 16) {
                        for (id, url) in items {
                            if tx.send((id, player.inspect(&url))).is_err() {
                                break;
                            }
                            ctx.request_repaint();
                        }
                    }
                });
            }
        }
    }

    fn toggle_play(&mut self, state: Status) {
        match (self.player.as_ref(), self.queue.is_empty(), state) {
            (Some(player), false, Status::Playing | Status::Loading) => player.pause(true),
            (Some(player), false, Status::Paused) => player.pause(false),
            (Some(_), false, _) => self.select(self.current),
            (_, true, _) => input::picker(false),
            _ => {}
        }
    }

    fn skip(&mut self, offset: isize) {
        if let Some(index) = self.current.checked_add_signed(offset) {
            self.select(index);
        }
    }

    fn full(&mut self, ctx: &Context, fullscreen: bool) {
        if fullscreen {
            self.library_open = false;
        }
        self.fullscreen = fullscreen;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(fullscreen));
    }

    fn shortcuts(&mut self, ctx: &Context, state: &Snapshot) {
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::O)) {
            input::picker(false);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::L)) {
            self.toggle_library();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Comma)) {
            self.preferences.open = !self.preferences.open;
        }
        if self.network_open
            || self.preferences.open
            || self.editor.is_some()
            || ctx.wants_keyboard_input()
        {
            return;
        }
        let pressed = |index| {
            details::shortcut(&self.catalog.data.settings, index)
                .is_some_and(|key| ctx.input(|i| i.key_pressed(key) && !i.modifiers.any()))
        };
        let keys = std::array::from_fn::<_, 9, _>(pressed);
        if keys[0] {
            self.toggle_play(state.status);
        }
        if keys[1] {
            self.full(ctx, !self.fullscreen);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.fullscreen {
                self.full(ctx, false);
            } else {
                self.library_open = false;
            }
        }
        if keys[2] {
            self.muted = !self.muted;
            self.apply_audio();
        }
        if let Some(player) = &self.player {
            if state.seekable {
                if keys[3] {
                    player.seek((state.time - 5_000).max(0));
                }
                if keys[4] {
                    player.seek((state.time + 5_000).min(state.duration));
                }
            }
        }
        if keys[5] || keys[6] {
            self.volume = (self.volume + if keys[5] { -5 } else { 5 }).clamp(0, 100);
            self.apply_audio();
        }
        if keys[7] {
            self.skip(-1);
        }
        if keys[8] {
            self.skip(1);
        }
    }

    fn apply_audio(&self) {
        if let Some(player) = &self.player {
            player.audio(self.volume, self.muted);
        }
    }

    fn toggle_library(&mut self) {
        self.library_open = !self.library_open;
        if self.library_open {
            self.library.reload();
        }
    }

    fn header(&mut self, ctx: &Context) {
        TopBottomPanel::top("header")
            .frame(
                Frame::new()
                    .fill(BASE)
                    .inner_margin(Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 11.0, Color32::from_rgb(59, 43, 28));
                    ui.painter().image(
                        self.logo.id(),
                        rect.shrink(6.0),
                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    ui.add_space(5.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(text("PAN", 21.0, CREAM).strong());
                        ui.label(text("Play Anything Now", 12.0, SOFT));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(
                            ui,
                            Icon::Queue,
                            34.0,
                            tr!("Playback queue", "Cola de reproducción", "File d’attente"),
                            false,
                        )
                        .clicked()
                        {
                            self.queue_open = !self.queue_open;
                        }
                        let library_label = if self.library_open {
                            tr!("Player", "Reproductor", "Lecteur")
                        } else {
                            tr!("Library", "Biblioteca", "Bibliothèque")
                        };
                        if icon_button(ui, Icon::Library, 34.0, library_label, self.library_open)
                            .clicked()
                        {
                            self.toggle_library();
                        }
                        if icon_button(
                            ui,
                            Icon::Link,
                            34.0,
                            tr!("Open URL", "Abrir URL", "Ouvrir une URL"),
                            false,
                        )
                        .clicked()
                        {
                            self.network_open = true;
                        }
                        if ui
                            .add_sized(
                                [121.0, 34.0],
                                button(
                                    tr!("Open file", "Abrir archivo", "Ouvrir un fichier"),
                                    false,
                                ),
                            )
                            .clicked()
                        {
                            input::picker(false);
                        }
                    });
                });
            });
    }

    fn queue(&mut self, ctx: &Context) {
        if !self.queue_open || self.fullscreen {
            return;
        }
        SidePanel::right("queue")
            .resizable(true)
            .default_width(252.0)
            .width_range(190.0..=340.0)
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::same(16))
                    .stroke(Stroke::new(1.0_f32, BORDER)),
            )
            .show(ctx, |ui| {
                ui.heading(tr!("Queue", "Cola", "File d’attente"));
                ui.add_space(10.0);
                if self.queue.is_empty() {
                    ui.label(text(
                        tr!(
                            "Open multiple files to play them in order.",
                            "Abre varios archivos para reproducirlos en orden.",
                            "Ouvrez plusieurs fichiers pour les lire dans l’ordre."
                        ),
                        13.0,
                        SOFT,
                    ));
                }
                let mut selected = None;
                ScrollArea::vertical().show(ui, |ui| {
                    for (index, media) in self.queue.iter().enumerate() {
                        if ui
                            .add(Button::selectable(index == self.current, &media.title).truncate())
                            .on_hover_text(&media.title)
                            .clicked()
                        {
                            selected = Some(index);
                        }
                    }
                });
                if let Some(index) = selected {
                    self.select(index);
                }
            });
    }

    fn stage(&mut self, ctx: &Context, state: &Snapshot) {
        let margin = if self.fullscreen {
            Margin::ZERO
        } else {
            Margin {
                left: 24,
                right: 24,
                top: 4,
                bottom: 6,
            }
        };
        CentralPanel::default()
            .frame(Frame::new().fill(BASE).inner_margin(margin))
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click());
                let pointer = ctx.input(|i| !i.raw.hovered_files.is_empty());
                let bg = Color32::from_rgb(12, 13, 15);
                let round = if self.fullscreen { 0.0 } else { 15.0 };
                ui.painter().rect_filled(rect, round, bg);
                if let Some(video) = &self.video {
                    let source = video.size_vec2();
                    let size = source * (rect.width() / source.x).min(rect.height() / source.y);
                    ui.painter().image(
                        video.id(),
                        Rect::from_center_size(rect.center(), size),
                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    if response.double_clicked() {
                        self.full(ctx, !self.fullscreen);
                    }
                } else {
                    let empty = self.queue.is_empty();
                    let message = if empty {
                        tr!("Ready to play", "Todo listo para reproducir", "Prêt à lire")
                    } else if state.status == Status::Loading {
                        tr!("Loading…", "Cargando…", "Chargement…")
                    } else if state.status == Status::Error {
                        tr!(
                            "Could not play the file",
                            "No se pudo reproducir el archivo",
                            "Impossible de lire le fichier"
                        )
                    } else {
                        tr!("Playback", "Reproducción", "Lecture")
                    };
                    let hint = if empty {
                        Some(tr!(
                            "Drop a video or audio file here",
                            "Suelta un vídeo o audio aquí",
                            "Déposez un fichier audio ou vidéo ici"
                        ))
                    } else {
                        self.queue
                            .get(self.current)
                            .map(|media| media.title.as_str())
                    };
                    let mut content = ui.new_child(
                        UiBuilder::new()
                            .max_rect(Rect::from_center_size(
                                rect.center(),
                                vec2((rect.width() - 38.0).max(0.0), 178.0),
                            ))
                            .layout(Layout::top_down(Align::Center)),
                    );
                    let (logo_rect, _) =
                        content.allocate_exact_size(Vec2::splat(62.0), Sense::hover());
                    content
                        .painter()
                        .rect_filled(logo_rect, 17.0, Color32::from_rgb(52, 38, 24));
                    content.painter().rect_stroke(
                        logo_rect,
                        17.0,
                        Stroke::new(1.0_f32, Color32::from_rgb(78, 58, 36)),
                        StrokeKind::Inside,
                    );
                    content.painter().image(
                        self.logo.id(),
                        logo_rect.shrink(11.0),
                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    content.add_space(13.0);
                    content.add(Label::new(text(message, 27.0, CREAM).strong()).truncate());
                    if let Some(hint) = hint {
                        content.add(Label::new(text(hint, 14.0, SOFT)).truncate());
                    }
                    if empty {
                        content.add_space(10.0);
                        if content
                            .add_sized(
                                [132.0, 36.0],
                                button(
                                    tr!("Open file", "Abrir archivo", "Ouvrir un fichier"),
                                    true,
                                ),
                            )
                            .clicked()
                        {
                            input::picker(false);
                        }
                    }
                }
                if !self.fullscreen || pointer {
                    ui.painter().rect_stroke(
                        rect,
                        round,
                        Stroke::new(1.0_f32, if pointer { AMBER } else { BORDER }),
                        StrokeKind::Inside,
                    );
                }
            });
    }

    fn controls(&mut self, ctx: &Context, state: &Snapshot) {
        TopBottomPanel::bottom("transport")
            .frame(Frame::new().fill(BASE).inner_margin(Margin {
                left: 24,
                right: 24,
                top: 7,
                bottom: 14,
            }))
            .show(ctx, |ui| {
                if let Some(error) = self.error.as_ref().or(self.engine_error.as_ref()) {
                    ui.label(text(error, 13.0, Color32::from_rgb(249, 166, 146)));
                    ui.add_space(5.0);
                }
                self.timeline(ui, state);
                ui.add_space(6.0);
                let (bar, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::hover());
                let (now, controls, options) = split_bar(bar);
                self.now_playing(ui, now);
                let mut transport = ui.new_child(
                    UiBuilder::new()
                        .max_rect(controls)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                transport.spacing_mut().item_spacing.x = 5.0;
                if transport
                    .add_enabled_ui(self.current > 0, |ui| {
                        icon_button(
                            ui,
                            Icon::Back,
                            33.0,
                            tr!("Previous", "Anterior", "Précédent"),
                            false,
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.skip(-1);
                }
                let playing = matches!(state.status, Status::Playing | Status::Loading);
                if icon_button(
                    &mut transport,
                    if playing { Icon::Pause } else { Icon::Play },
                    44.0,
                    if playing {
                        tr!("Pause", "Pausar", "Pause")
                    } else {
                        tr!("Play", "Reproducir", "Lire")
                    },
                    true,
                )
                .clicked()
                {
                    self.toggle_play(state.status);
                }
                if transport
                    .add_enabled_ui(self.current + 1 < self.queue.len(), |ui| {
                        icon_button(
                            ui,
                            Icon::Forward,
                            33.0,
                            tr!("Next", "Siguiente", "Suivant"),
                            false,
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.skip(1);
                }
                if transport
                    .add_enabled_ui(!self.queue.is_empty(), |ui| {
                        icon_button(
                            ui,
                            Icon::Stop,
                            33.0,
                            tr!("Stop", "Detener", "Arrêter"),
                            false,
                        )
                    })
                    .inner
                    .clicked()
                {
                    self.finish_position();
                    self.active = None;
                    if let Some(player) = &self.player {
                        player.stop();
                    }
                    self.video = None;
                }

                let mut right = ui.new_child(
                    UiBuilder::new()
                        .max_rect(options)
                        .layout(Layout::right_to_left(Align::Center)),
                );
                right.spacing_mut().item_spacing.x = 3.0;
                if icon_button(
                    &mut right,
                    Icon::Fullscreen,
                    34.0,
                    tr!("Fullscreen", "Pantalla completa", "Plein écran"),
                    false,
                )
                .clicked()
                {
                    self.full(ctx, !self.fullscreen);
                }
                right
                    .menu_button(format!("{}×", self.speed), |ui| {
                        for speed in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
                            if ui
                                .selectable_label(self.speed == speed, format!("{speed}×"))
                                .clicked()
                            {
                                if self.player.as_ref().is_none_or(|player| player.rate(speed)) {
                                    self.speed = speed;
                                }
                                ui.close();
                            }
                        }
                        ui.separator();
                        ui.checkbox(
                            &mut self.repeat,
                            tr!("Repeat file", "Repetir archivo", "Répéter le fichier"),
                        );
                    })
                    .response
                    .on_hover_text(tr!(
                        "Speed and repeat",
                        "Velocidad y repetición",
                        "Vitesse et répétition"
                    ));
                self.track_menu(&mut right, true);
                self.track_menu(&mut right, false);
                if options.width() > 224.0 {
                    right.spacing_mut().slider_width = (options.width() - 188.0).min(74.0);
                    if right
                        .add(Slider::new(&mut self.volume, 0..=100).show_value(false))
                        .on_hover_text(tr!("Volume", "Volumen", "Volume"))
                        .changed()
                    {
                        self.apply_audio();
                    }
                }
                if icon_button(
                    &mut right,
                    Icon::Volume(self.muted),
                    32.0,
                    tr!("Mute", "Silenciar", "Couper le son"),
                    false,
                )
                .clicked()
                {
                    self.muted = !self.muted;
                    self.apply_audio();
                }
            });
    }

    fn timeline(&mut self, ui: &mut Ui, state: &Snapshot) {
        ui.horizontal(|ui| {
            let pos = self
                .seek
                .unwrap_or_else(|| {
                    if state.duration > 0 {
                        state.time as f32 / state.duration as f32
                    } else {
                        0.0
                    }
                })
                .clamp(0.0, 1.0);
            let time = self.seek.map_or(state.time, |fraction| {
                (fraction * state.duration as f32) as i64
            });
            let width = if state.duration >= 3_600_000 {
                66.0
            } else {
                42.0
            };
            ui.add_sized(
                [width, 18.0],
                Label::new(text(clock(time), 12.0, SOFT).monospace()),
            );
            let bar_width = (ui.available_width() - width - ui.spacing().item_spacing.x).max(30.0);
            let (rect, response) =
                ui.allocate_exact_size(vec2(bar_width, 18.0), Sense::click_and_drag());
            let rail = Rect::from_center_size(rect.center(), vec2(rect.width(), 4.0));
            let mut shown = pos;
            if state.seekable && state.duration > 0 {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let fraction = ((pointer.x - rail.left()) / rail.width()).clamp(0.0, 1.0);
                    if response.dragged() {
                        self.seek = Some(fraction);
                        shown = fraction;
                    }
                    if response.clicked() || response.drag_stopped() {
                        self.seek = None;
                        shown = fraction;
                        if let Some(player) = &self.player {
                            player.seek((fraction * state.duration as f32) as i64);
                        }
                    }
                } else if response.drag_stopped() {
                    self.seek = None;
                }
            }
            ui.painter().rect_filled(rail, 2.0, BORDER);
            let end = rail.left() + rail.width() * shown;
            if shown > 0.0 {
                ui.painter().rect_filled(
                    Rect::from_min_max(rail.min, pos2(end, rail.bottom())),
                    2.0,
                    AMBER,
                );
            }
            if state.seekable {
                ui.painter().circle_filled(
                    pos2(end, rail.center().y),
                    if response.hovered() { 5.0 } else { 3.5 },
                    AMBER,
                );
            }
            ui.add_sized(
                [width, 18.0],
                Label::new(text(clock(state.duration), 12.0, SOFT).monospace()),
            );
        });
    }

    fn now_playing(&self, ui: &mut Ui, rect: Rect) {
        let mut ui = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let (tile, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
        ui.painter().rect_filled(tile, 8.0, PANEL);
        ui.painter()
            .rect_stroke(tile, 8.0, Stroke::new(1.0_f32, BORDER), StrokeKind::Inside);
        self.logo.paint_at(&ui, tile.shrink(8.0));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let item = self.queue.get(self.current);
            let title = item.map_or(tr!("Ready", "En espera", "En attente"), |m| {
                m.title.as_str()
            });
            ui.add(Label::new(text(title, 14.0, CREAM).strong()).truncate())
                .on_hover_text(title);
            if let Some(item) = item {
                let details = match self.player.as_ref().and_then(Player::size) {
                    Some([w, h]) => format!("{}  ·  {w} × {h}", item.tag),
                    None => item.tag.clone(),
                };
                ui.add(Label::new(text(details, 11.0, SOFT)).truncate());
            }
        });
    }

    fn track_menu(&mut self, ui: &mut Ui, subtitles: bool) {
        let (label, icon) = if subtitles {
            (
                tr!("Subtitles", "Subtítulos", "Sous-titres"),
                Icon::Captions,
            )
        } else {
            (
                tr!("Audio tracks", "Pistas de audio", "Pistes audio"),
                Icon::Audio,
            )
        };
        egui::Popup::menu(&icon_button(ui, icon, 32.0, label, false)).show(|ui| {
            ui.label(text(label, 13.0, SOFT));
            if let Some(player) = &self.player {
                let (selected, tracks) = player.tracks(subtitles);
                if tracks.is_empty() {
                    ui.label(tr!("No tracks", "Sin pistas", "Aucune piste"));
                }
                for (id, name) in tracks {
                    if ui.selectable_label(selected == id, name).clicked() {
                        player.select_track(subtitles, id);
                        if subtitles {
                            if let Some(active) = &mut self.active {
                                active.settings.subtitle_id = Some(id);
                                active.subtitle_pending = None;
                                if id == -1 {
                                    active.settings.subtitle = None;
                                }
                            }
                        }
                        ui.close();
                    }
                }
                if let Some(active) = &mut self.active {
                    let delay = if subtitles {
                        &mut active.settings.subtitle_delay
                    } else {
                        &mut active.settings.audio_delay
                    };
                    ui.separator();
                    ui.label(tr!(
                        "Timing (ms)",
                        "Sincronización (ms)",
                        "Synchronisation (ms)"
                    ));
                    ui.horizontal(|ui| {
                        let before = *delay;
                        if ui.button("−100").clicked() {
                            *delay -= 100;
                        }
                        ui.add(DragValue::new(delay).range(-60_000..=60_000).suffix(" ms"));
                        if ui.button("+100").clicked() {
                            *delay += 100;
                        }
                        if ui
                            .button(tr!("Reset", "Restablecer", "Réinitialiser"))
                            .clicked()
                        {
                            *delay = 0;
                        }
                        *delay = (*delay).clamp(-60_000, 60_000);
                        if *delay != before {
                            player.delay(subtitles, *delay);
                        }
                    });
                    if subtitles
                        && ui
                            .button(tr!(
                                "Subtitle settings…",
                                "Ajustes de subtítulos…",
                                "Paramètres des sous-titres…"
                            ))
                            .clicked()
                    {
                        self.preferences.open = true;
                        ui.close();
                    }
                }
                if subtitles
                    && !self.queue.is_empty()
                    && ui
                        .button(tr!(
                            "Open subtitles…",
                            "Abrir subtítulos…",
                            "Ouvrir des sous-titres…"
                        ))
                        .clicked()
                {
                    input::picker(true);
                    ui.close();
                }
            }
        });
    }

    fn network_dialog(&mut self, ctx: &Context) {
        let mut open = self.network_open;
        let mut submit = false;
        Window::new(tr!("Open URL", "Abrir URL", "Ouvrir une URL"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                let entry = ui.add(
                    TextEdit::singleline(&mut self.network)
                        .desired_width(360.0)
                        .hint_text("https://…"),
                );
                if ctx.input(|i| i.key_pressed(Key::Enter))
                    && (entry.has_focus() || entry.lost_focus())
                {
                    submit = true;
                }
                if ui
                    .add_sized(
                        [110.0, 32.0],
                        button(tr!("Play", "Reproducir", "Lire"), true),
                    )
                    .clicked()
                {
                    submit = true;
                }
            });
        if submit {
            match Media::from_address(&self.network) {
                Ok(media) => {
                    self.replace_queue(vec![media], 0);
                    open = false;
                }
                Err(error) => self.error = Some(error),
            }
        }
        self.network_open = open;
    }
}

trait PaintTexture {
    fn paint_at(&self, ui: &Ui, rect: Rect);
}

impl PaintTexture for TextureHandle {
    fn paint_at(&self, ui: &Ui, rect: Rect) {
        ui.painter().image(
            self.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

fn split_bar(bar: Rect) -> (Rect, Rect, Rect) {
    let center = bar.center().x
        - if bar.width() < 760.0 {
            (760.0 - bar.width()) * 0.19
        } else {
            0.0
        };
    let transport = Rect::from_center_size(pos2(center, bar.center().y), vec2(158.0, bar.height()));
    let now = Rect::from_min_max(bar.min, pos2(transport.left() - 12.0, bar.bottom()));
    let tools = Rect::from_min_max(pos2(transport.right() + 10.0, bar.top()), bar.max);
    (now, transport, tools)
}

fn clock(ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

impl eframe::App for Pan {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        if self.toolbar_language != locale::current() {
            self.toolbar_language = locale::current();
            if self.native_toolbar {
                input::refresh_toolbar(frame);
            }
        }
        self.library.poll();
        self.catalog_poll(ctx);
        let mut incoming = input::drain();
        if let Some(folder) = incoming.folders.pop() {
            self.library.request_move(folder);
        }
        if let (Some(editor), Some(cover)) = (&mut self.editor, incoming.covers.pop()) {
            editor.pick_cover(&cover, ctx);
        }
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        if self.library_open {
            incoming.imports.extend(dropped);
        } else {
            incoming.files.extend(dropped);
        }
        self.library.start_import(incoming.imports);
        self.open_paths(incoming.files);
        for action in incoming.actions {
            match action {
                1 => self.network_open = true,
                2 => self.queue_open = !self.queue_open,
                3 => self.toggle_library(),
                _ => {}
            }
        }
        if let Some(fullscreen) = ctx.input(|input| input.viewport().fullscreen) {
            self.fullscreen = fullscreen;
        }
        let mut state = Snapshot {
            status: Status::Idle,
            time: 0,
            duration: 0,
            seekable: false,
        };
        if let Some(player) = &self.player {
            for path in incoming.subtitles {
                let success =
                    Media::from_path(&path).is_ok_and(|media| player.subtitle(&media.url));
                if success {
                    if let Some(active) = &mut self.active {
                        active.settings.subtitle = Media::from_path(&path)
                            .ok()
                            .map(|media| media.url.to_string());
                        active.settings.subtitle_id = None;
                        active.subtitle_pending = None;
                    }
                } else {
                    self.error = Some(
                        tr!(
                            "Could not load subtitles.",
                            "No se pudieron cargar los subtítulos.",
                            "Impossible de charger les sous-titres."
                        )
                        .into(),
                    );
                }
            }
            if let Some(frame) = player.frame() {
                match &mut self.video {
                    Some(video) => video.set(frame, TextureOptions::LINEAR),
                    None => {
                        self.video = Some(ctx.load_texture(
                            tr!("Video", "Vídeo", "Vidéo"),
                            frame,
                            TextureOptions::LINEAR,
                        ))
                    }
                }
            }
            state = player.state();
        }
        self.progress(&state);
        if state.status == Status::Ended {
            self.save_position(&state, true);
            if self.repeat {
                self.select(self.current);
            } else if self.current + 1 < self.queue.len() {
                self.skip(1);
            }
        }
        if state.status == Status::Error {
            self.error.get_or_insert_with(|| {
                tr!(
                    "LibVLC could not play this media.",
                    "LibVLC no pudo reproducir este medio.",
                    "LibVLC n’a pas pu lire ce média."
                )
                .into()
            });
        }
        self.shortcuts(ctx, &state);
        if !self.fullscreen && !self.native_toolbar {
            self.header(ctx);
        }
        let show_full_controls = ctx.input(|i| {
            i.pointer
                .hover_pos()
                .is_some_and(|p| i.viewport_rect().bottom() - p.y < 108.0)
        });
        if !self.fullscreen || show_full_controls {
            self.controls(ctx, &state);
        }
        self.queue(ctx);
        if self.library_open && !self.fullscreen {
            match self.library_view.show(
                ctx,
                &self.library,
                &mut self.catalog.data,
                &self.logo,
                self.queue.get(self.current),
            ) {
                Some(library_view::Action::Play(items, index)) => {
                    self.replace_queue(items, index);
                    if self.queue[index]
                        .url
                        .to_file_path()
                        .is_ok_and(|path| library::Kind::of(&path) == library::Kind::Video)
                    {
                        self.library_open = false;
                    }
                }
                Some(library_view::Action::Player) => self.library_open = false,
                Some(library_view::Action::Import) => input::import_picker(),
                Some(library_view::Action::Settings) => self.preferences.open = true,
                Some(library_view::Action::Save) => self.save_catalog(),
                Some(library_view::Action::Edit(id)) => {
                    if let (Some(entry), Some(folder)) = (
                        self.catalog.data.entries.get(&id),
                        self.library.entry_folder(&id),
                    ) {
                        self.editor = Some(details::Editor::new(id, entry.clone(), &folder, ctx));
                    }
                }
                None => {}
            }
        } else {
            self.stage(ctx, &state);
        }
        if self.network_open {
            self.network_dialog(ctx);
        }
        if self
            .preferences
            .show(ctx, &mut self.catalog.data.settings, &mut self.library)
        {
            self.save_catalog();
        }
        if let Some(editor) = &mut self.editor {
            match editor.show(ctx, &self.library) {
                details::EditAction::Keep => {}
                details::EditAction::Close => self.editor = None,
                details::EditAction::Save(mut entry) => {
                    let id = editor.id.clone();
                    if let Some(old) = self.catalog.data.entries.get(&id) {
                        entry.scanned = old.scanned;
                        entry.duration = old.duration;
                    }
                    self.catalog.data.entries.insert(id.clone(), entry);
                    self.editor = None;
                    self.library_view.refresh_cover(&id);
                    self.save_catalog();
                }
            }
        }
        let interval = if state.status == Status::Playing && self.video.is_some() {
            12
        } else if self.library.busy() {
            50
        } else {
            100
        };
        ctx.request_repaint_after(Duration::from_millis(interval));
    }
}

impl Drop for Pan {
    fn drop(&mut self) {
        self.finish_position();
    }
}

fn main() -> eframe::Result {
    input::register();
    let initial = std::env::args_os()
        .skip(1)
        .filter(|a| !a.to_string_lossy().starts_with("-psn_"))
        .map(PathBuf::from)
        .collect();
    let logo =
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/pan.png")).expect("PNG de PAN");
    let native = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("PAN · Play Anything Now")
            .with_inner_size([960.0, 625.0])
            .with_min_inner_size([635.0, 435.0])
            .with_icon(Arc::new(logo.clone())),
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "PAN · Play Anything Now",
        native,
        Box::new(|cc| Ok(Box::new(Pan::new(cc, &logo, initial)))),
    )
}
