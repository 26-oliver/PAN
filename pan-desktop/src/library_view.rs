use crate::{
    catalog::{unix_seconds, Data, Entry, Playlist},
    library::{Item, Kind, Library},
    media::Media,
    theme::{button, text, AMBER, BASE, BORDER, CREAM, INK, PANEL, SOFT},
};
use eframe::egui::*;
use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    time::SystemTime,
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Sort {
    #[default]
    List,
    Recent,
    Played,
    Name,
    Size,
    Duration,
}

impl Sort {
    fn label(self) -> &'static str {
        match self {
            Self::List => "Orden de lista",
            Self::Recent => "Fecha de importación",
            Self::Played => "Última reproducción",
            Self::Name => "Nombre",
            Self::Size => "Tamaño",
            Self::Duration => "Duración",
        }
    }
}

#[derive(Default)]
pub struct View {
    query: String,
    filter: Option<Kind>,
    sort: Sort,
    duration: u8,
    date: u8,
    selected_list: Option<usize>,
    list_name: String,
    list_dialog: Option<ListDialog>,
    missing_open: bool,
    covers: HashMap<String, TextureHandle>,
    checked_covers: HashSet<String>,
}

#[derive(Clone, Copy)]
enum ListDialog {
    Create,
    Rename(usize),
}

pub enum Action {
    Player,
    Import,
    Settings,
    Edit(String),
    Save,
    Play(Vec<Media>, usize),
}

pub fn display_title<'a>(item: &'a Item, entry: Option<&'a Entry>) -> &'a str {
    entry
        .filter(|entry| !entry.title.is_empty() && entry.title != entry.filename)
        .map(|entry| entry.title.as_str())
        .unwrap_or_else(|| {
            item.path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&item.media.title)
        })
}

fn title<'a>(item: &'a Item, data: &'a Data) -> &'a str {
    display_title(item, data.entries.get(&item.id))
}

impl View {
    pub fn refresh_cover(&mut self, id: &str) {
        self.covers.remove(id);
        self.checked_covers.remove(id);
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        library: &Library,
        data: &mut Data,
        logo: &TextureHandle,
        current: Option<&Media>,
    ) -> Option<Action> {
        let present: HashSet<_> = library.items.iter().map(|item| item.id.as_str()).collect();
        let missing = data
            .entries
            .iter()
            .filter(|(id, _)| !present.contains(id.as_str()))
            .collect::<Vec<_>>();
        let mut missing_count = missing.len();
        let mut action = None;
        if self.missing_open {
            let mut forget = None;
            Window::new("Archivos que faltan").open(&mut self.missing_open).show(ctx, |ui| {
                ui.label("Comprueba que el disco esté conectado o que el archivo siga en la carpeta de PAN.");
                ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for (id, entry) in &missing {
                        ui.horizontal(|ui| {
                            ui.label(if entry.title.is_empty() { &entry.filename } else { &entry.title });
                            if ui.button("Quitar del catálogo").clicked() {
                                forget = Some((*id).clone());
                            }
                        });
                    }
                });
            });
            if let Some(id) = forget {
                missing_count -= 1;
                data.entries.remove(&id);
                data.history.remove(&format!("library:{id}"));
                for list in &mut data.lists {
                    list.items.retain(|item| item != &id);
                }
                action = Some(Action::Save);
            }
        }

        CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(BASE)
                    .inner_margin(Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .corner_radius(14)
                    .inner_margin(16)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        ui.horizontal(|ui| {
                            if ui.add(button("‹ Reproductor", false)).clicked() {
                                action = Some(Action::Player);
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add_enabled(!library.busy(), button("+ Añadir archivos", true))
                                    .clicked()
                                {
                                    action = Some(Action::Import);
                                }
                                if ui.add(button("Ajustes", false)).clicked() {
                                    action = Some(Action::Settings);
                                }
                            });
                        });
                        ui.add_space(8.0);
                        ui.label(text("Tu biblioteca", 29.0, CREAM).strong());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(text(
                                format!(
                                    "{} {} · {}",
                                    library.items.len(),
                                    if library.items.len() == 1 {
                                        "archivo"
                                    } else {
                                        "archivos"
                                    },
                                    size_label(library.items.iter().map(|i| i.size).sum())
                                ),
                                13.0,
                                SOFT,
                            ));
                            if missing_count > 0
                                && ui
                                    .button(format!("{missing_count} sin localizar"))
                                    .clicked()
                            {
                                self.missing_open = true;
                            }
                        });
                        ui.add_space(8.0);
                        if self.playlists(ui, data) {
                            action = Some(Action::Save);
                        }
                        self.filters(ui);
                        ui.add_space(8.0);

                        if library.busy() {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(if library.moving() {
                                    "Copiando la biblioteca…"
                                } else {
                                    "Importando archivos…"
                                });
                            });
                        } else if let Some((message, error)) = &library.message {
                            ui.label(text(
                                message,
                                13.0,
                                if *error { Color32::LIGHT_RED } else { AMBER },
                            ));
                        }

                        let visible = self.items(library, data);
                        if visible.is_empty() {
                            empty(
                                ui,
                                logo,
                                !library.items.is_empty() || self.selected_list.is_some(),
                            );
                        } else {
                            if let Some(result) = tiles(
                                ui,
                                &visible,
                                current,
                                library,
                                data,
                                self.selected_list,
                                &mut self.covers,
                                &mut self.checked_covers,
                            ) {
                                action = Some(result);
                            }
                        }
                    });
            });
        if self.edit_list(ctx, data) {
            action = Some(Action::Save);
        }
        action
    }

    fn edit_list(&mut self, ctx: &Context, data: &mut Data) -> bool {
        let Some(dialog) = self.list_dialog else {
            return false;
        };
        let (title, button_label) = match dialog {
            ListDialog::Create => ("Nueva lista", "Crear"),
            ListDialog::Rename(_) => ("Cambiar nombre", "Guardar"),
        };
        let mut open = true;
        let mut submit = false;
        Window::new(title)
            .id(Id::new("playlist-name"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                let name = ui.add(
                    TextEdit::singleline(&mut self.list_name)
                        .desired_width(240.0)
                        .hint_text("Nombre de la lista"),
                );
                if !name.has_focus() && !name.lost_focus() {
                    name.request_focus();
                }
                let enter = name.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                let valid = !self.list_name.trim().is_empty();
                submit =
                    (ui.add_enabled(valid, button(button_label, true)).clicked() || enter) && valid;
            });
        if !open || submit {
            self.list_dialog = None;
        }
        if !submit {
            return false;
        }
        let name = self.list_name.trim().to_owned();
        match dialog {
            ListDialog::Create => {
                data.lists.push(Playlist {
                    name,
                    items: Vec::new(),
                });
                self.selected_list = Some(data.lists.len() - 1);
                self.sort = Sort::List;
            }
            ListDialog::Rename(index) => data.lists[index].name = name,
        }
        true
    }

    fn playlists(&mut self, ui: &mut Ui, data: &mut Data) -> bool {
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            ComboBox::from_id_salt("lists")
                .selected_text(
                    self.selected_list
                        .map(|i| data.lists[i].name.as_str())
                        .unwrap_or("Todos los archivos"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected_list, None, "Todos los archivos");
                    for (index, list) in data.lists.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_list, Some(index), &list.name);
                    }
                });
            if ui.button("+ Nueva lista").clicked() {
                self.list_name.clear();
                self.list_dialog = Some(ListDialog::Create);
            }
            if let Some(index) = self.selected_list {
                ui.menu_button("Gestionar lista", |ui| {
                    if ui.button("Cambiar nombre…").clicked() {
                        self.list_name.clone_from(&data.lists[index].name);
                        self.list_dialog = Some(ListDialog::Rename(index));
                        ui.close();
                    }
                    if ui
                        .button("Eliminar lista")
                        .on_hover_text("Los archivos se conservan en PAN")
                        .clicked()
                    {
                        data.lists.remove(index);
                        self.selected_list = None;
                        changed = true;
                        ui.close();
                    }
                });
            }
        });
        changed
    }

    fn filters(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            for (filter, label) in [
                (None, "Todo"),
                (Some(Kind::Audio), "Audio"),
                (Some(Kind::Video), "Vídeo"),
                (Some(Kind::Other), "Otros"),
            ] {
                let active = self.filter == filter;
                if ui
                    .add(
                        Button::new(text(label, 13.0, if active { INK } else { CREAM }))
                            .fill(if active {
                                AMBER
                            } else {
                                Color32::from_rgb(40, 41, 46)
                            })
                            .corner_radius(15),
                    )
                    .clicked()
                {
                    self.filter = filter;
                }
            }
            ui.add(
                TextEdit::singleline(&mut self.query)
                    .desired_width(165.0)
                    .hint_text("Buscar en tu biblioteca"),
            );
            let label = if self.sort == Sort::List && self.selected_list.is_none() {
                Sort::Recent.label()
            } else {
                self.sort.label()
            };
            ComboBox::from_id_salt("library-sort")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    if self.selected_list.is_some() {
                        ui.selectable_value(&mut self.sort, Sort::List, Sort::List.label());
                    }
                    for option in [
                        Sort::Recent,
                        Sort::Played,
                        Sort::Name,
                        Sort::Size,
                        Sort::Duration,
                    ] {
                        ui.selectable_value(&mut self.sort, option, option.label());
                    }
                });
            ui.menu_button(
                if self.duration != 0 || self.date != 0 {
                    "Filtros activos"
                } else {
                    "Filtros"
                },
                |ui| {
                    for (id, name, value, labels) in [
                        (
                            "duration",
                            "Duración",
                            &mut self.duration,
                            ["Todas", "Menos de 10 min", "10 min o más", "Desconocida"],
                        ),
                        (
                            "date",
                            "Importado",
                            &mut self.date,
                            [
                                "Cualquier fecha",
                                "Últimos 7 días",
                                "Últimos 30 días",
                                "Hace más de 30 días",
                            ],
                        ),
                    ] {
                        ui.label(name);
                        ComboBox::from_id_salt(id)
                            .selected_text(labels[*value as usize])
                            .show_ui(ui, |ui| {
                                for (i, label) in labels.into_iter().enumerate() {
                                    ui.selectable_value(value, i as u8, label);
                                }
                            });
                    }
                },
            );
        });
    }

    fn items<'a>(&self, library: &'a Library, data: &Data) -> Vec<&'a Item> {
        let query = self.query.trim().to_lowercase();
        let list = self.selected_list.map(|i| &data.lists[i]);
        let date = unix_seconds(SystemTime::now());
        let duration = |item: &Item| data.entries.get(&item.id).map_or(0, |e| e.duration);
        let mut visible = library
            .items
            .iter()
            .filter(|item| {
                let age = date.saturating_sub(item.added);
                let ms = duration(item);
                self.filter.is_none_or(|kind| kind == item.kind)
                    && list.is_none_or(|list| list.items.contains(&item.id))
                    && match self.duration {
                        1 => ms > 0 && ms < 600_000,
                        2 => ms >= 600_000,
                        3 => ms <= 0,
                        _ => true,
                    }
                    && match self.date {
                        1 => age < 604_800,
                        2 => age < 2_592_000,
                        3 => age >= 2_592_000,
                        _ => true,
                    }
                    && (query.is_empty()
                        || title(item, data).to_lowercase().contains(&query)
                        || item.media.title.to_lowercase().contains(&query)
                        || data.entries.get(&item.id).is_some_and(|e| {
                            e.creator.to_lowercase().contains(&query)
                                || e.description.to_lowercase().contains(&query)
                        }))
            })
            .collect::<Vec<_>>();
        use std::cmp::Reverse;
        match self.sort {
            Sort::List => {
                if let Some(list) = list {
                    visible
                        .sort_by_cached_key(|item| list.items.iter().position(|id| id == &item.id));
                }
            }
            Sort::Recent => visible.sort_by_key(|item| Reverse(item.added)),
            Sort::Played => visible.sort_by_cached_key(|item| {
                Reverse(
                    data.history
                        .get(&format!("library:{}", item.id))
                        .map_or(0, |item| item.last_played),
                )
            }),
            Sort::Name => visible.sort_by_cached_key(|item| title(item, data).to_lowercase()),
            Sort::Size => visible.sort_by_key(|item| Reverse(item.size)),
            Sort::Duration => visible.sort_by_key(|item| Reverse(duration(item))),
        }
        visible
    }
}

pub fn size_label(bytes: u64) -> String {
    let mut size = bytes as f64;
    for unit in ["B", "KiB", "MiB", "GiB", "TiB"] {
        if size < 1024.0 || unit == "TiB" {
            return format!("{size:.1} {unit}");
        }
        size /= 1024.0;
    }
    unreachable!()
}

fn empty(ui: &mut Ui, logo: &TextureHandle, filtered: bool) {
    let height = ui.available_height().max(124.0);
    ui.add_space(((height - 108.0) / 2.0).max(0.0));
    ui.vertical_centered(|ui| {
        if !filtered {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(54.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, 14, Color32::from_rgb(59, 43, 28));
            ui.painter().image(
                logo.id(),
                rect.shrink(9.0),
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            ui.add_space(7.0);
        }
        let (title, hint) = if filtered {
            ("No hay coincidencias", "Prueba con otra búsqueda o filtro.")
        } else {
            (
                "Tu biblioteca está vacía",
                "Arrastra archivos aquí o usa Añadir archivos.",
            )
        };
        ui.label(text(title, 20.0, CREAM).strong());
        ui.label(text(hint, 13.0, SOFT));
    });
}

fn tiles(
    ui: &mut Ui,
    items: &[&Item],
    current: Option<&Media>,
    library: &Library,
    data: &mut Data,
    list: Option<usize>,
    covers: &mut HashMap<String, TextureHandle>,
    checked: &mut HashSet<String>,
) -> Option<Action> {
    let gap = 12.0;
    let width = ui.available_width();
    let columns = (((width + gap) / (128.0 + gap)).floor() as usize).max(1);
    let width = ((width + gap) / columns as f32 - gap).min(190.0).max(1.0);
    let height = width + 47.0;
    let mut clicked = None;
    ScrollArea::vertical()
        .id_salt("library-grid")
        .auto_shrink([false, false])
        .show_rows(ui, height, items.len().div_ceil(columns), |ui, rows| {
            for row in rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (col, item) in items[row * columns..].iter().take(columns).enumerate() {
                        let selected = current.is_some_and(|media| media.url == item.media.url);
                        if checked.insert(item.id.clone()) {
                            if let Some(image) = library.entry_folder(&item.id).and_then(|folder| {
                                crate::artwork::load(&crate::artwork::cover_path(&folder))
                            }) {
                                covers.insert(
                                    item.id.clone(),
                                    ui.ctx()
                                        .load_texture(&item.id, image, TextureOptions::LINEAR),
                                );
                            }
                        }
                        let cover = covers.get(&item.id);
                        let entry = data.entries.get(&item.id);
                        let response = tile(ui, item, entry, cover, vec2(width, height), selected);
                        if response.clicked() {
                            clicked = Some(Action::Play(
                                items
                                    .iter()
                                    .map(|item| {
                                        let mut media = item.media.clone();
                                        media.title = title(item, data).to_owned();
                                        media
                                    })
                                    .collect(),
                                row * columns + col,
                            ));
                        }
                        response.context_menu(|ui| {
                            if ui
                                .add_enabled(
                                    !library.busy(),
                                    Button::new("Editar ficha y carátula…"),
                                )
                                .clicked()
                            {
                                clicked = Some(Action::Edit(item.id.clone()));
                                ui.close();
                            }
                            if let Some(index) = list {
                                let ids = &mut data.lists[index].items;
                                let index = ids.iter().position(|id| id == &item.id).unwrap();
                                for (label, offset) in
                                    [("Subir en la lista", -1), ("Bajar en la lista", 1)]
                                {
                                    if let Some(other) =
                                        index.checked_add_signed(offset).filter(|i| *i < ids.len())
                                    {
                                        if ui.button(label).clicked() {
                                            ids.swap(index, other);
                                            clicked = Some(Action::Save);
                                            ui.close();
                                        }
                                    }
                                }
                                if ui.button("Quitar de esta lista").clicked() {
                                    ids.remove(index);
                                    clicked = Some(Action::Save);
                                    ui.close();
                                }
                            }
                            if !data.lists.is_empty() {
                                ui.menu_button("Añadir a una lista", |ui| {
                                    for playlist in &mut data.lists {
                                        if ui
                                            .add_enabled(
                                                !playlist.items.contains(&item.id),
                                                Button::new(&playlist.name),
                                            )
                                            .clicked()
                                        {
                                            playlist.items.push(item.id.clone());
                                            clicked = Some(Action::Save);
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        });
                    }
                });
            }
        });
    clicked
}

fn tile(
    ui: &mut Ui,
    item: &Item,
    entry: Option<&Entry>,
    cover: Option<&TextureHandle>,
    size: Vec2,
    selected: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let response = response.on_hover_text(format!(
        "{}\nMenú contextual: editar o añadir a una lista",
        &item.media.title
    ));
    let painter = ui.painter();
    let hover = response.hovered() || response.has_focus();
    if hover || selected {
        painter.rect_filled(rect, 11, Color32::from_rgb(39, 39, 45));
    }
    let art = Rect::from_min_size(rect.min + vec2(7.0, 7.0), Vec2::splat(size.x - 14.0));
    let mut hasher = DefaultHasher::new();
    item.media.title.hash(&mut hasher);
    let colors = [
        (
            Color32::from_rgb(111, 63, 31),
            Color32::from_rgb(50, 27, 18),
        ),
        (
            Color32::from_rgb(76, 62, 122),
            Color32::from_rgb(32, 28, 54),
        ),
        (Color32::from_rgb(39, 91, 91), Color32::from_rgb(17, 43, 54)),
        (
            Color32::from_rgb(119, 56, 77),
            Color32::from_rgb(51, 27, 47),
        ),
        (
            Color32::from_rgb(49, 72, 122),
            Color32::from_rgb(23, 37, 65),
        ),
    ];
    let (bright, dark) = colors[hasher.finish() as usize % colors.len()];
    painter.rect_filled(art, 7, dark);
    let art_painter = ui.painter().with_clip_rect(art.shrink(7.0));
    let center = art.center();
    let motif = match item.kind {
        Kind::Audio => {
            for n in [0.44, 0.64, 0.85] {
                art_painter.circle_stroke(
                    center,
                    art.width() * n / 2.0,
                    Stroke::new(2.0_f32, bright),
                );
            }
            "♪"
        }
        Kind::Video => {
            for offset in [-0.11, 0.0, 0.11] {
                let tile = Rect::from_center_size(
                    center + vec2(offset * art.width(), -offset * art.width()),
                    art.size() * 0.53,
                );
                art_painter.rect_stroke(tile, 8, Stroke::new(2.0_f32, bright), StrokeKind::Inside);
            }
            "▶"
        }
        Kind::Other => "◇",
    };
    painter.text(
        center,
        Align2::CENTER_CENTER,
        motif,
        FontId::proportional(30.0),
        CREAM,
    );
    if let Some(cover) = cover {
        let ratio = cover.size_vec2();
        let size = ratio * (art.width() / ratio.x.max(ratio.y));
        painter.image(
            cover.id(),
            Rect::from_center_size(art.center(), size),
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    if hover || selected {
        let badge = art.right_bottom() - Vec2::splat(19.0);
        painter.circle_filled(badge, 13.0, AMBER);
        painter.add(Shape::convex_polygon(
            vec![
                badge + vec2(-3.0, -5.0),
                badge + vec2(5.0, 0.0),
                badge + vec2(-3.0, 5.0),
            ],
            INK,
            Stroke::NONE,
        ));
    }

    let title = display_title(item, entry);
    let subtitle = if let Some(entry) = entry.filter(|e| !e.creator.is_empty()) {
        entry.creator.clone()
    } else {
        format!("{} · {}", item.kind.label(), item.media.tag)
    };
    let mut labels = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(
                pos2(art.left(), art.bottom() + 6.0),
                pos2(art.right(), rect.bottom()),
            ))
            .layout(Layout::top_down(Align::Min)),
    );
    labels.spacing_mut().item_spacing.y = 1.0;
    labels.add(
        Label::new(text(title, 14.0, if selected { AMBER } else { CREAM }).strong()).truncate(),
    );
    labels.add(Label::new(text(subtitle, 12.0, SOFT)).truncate());
    response
}
