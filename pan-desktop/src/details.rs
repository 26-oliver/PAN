use crate::{
    artwork,
    catalog::{self, Entry, Settings, SHORTCUTS},
    library::Library,
    library_view::size_label,
};
use eframe::egui::*;
use std::sync::mpsc::{self, Receiver};

#[derive(Default)]
pub struct Preferences {
    pub open: bool,
    capturing: Option<usize>,
    pub message: Option<String>,
}

impl Preferences {
    pub fn show(&mut self, ctx: &Context, settings: &mut Settings, library: &mut Library) -> bool {
        let mut changed = false;
        if !self.open {
            self.capturing = None;
            return false;
        }
        let mut open = self.open;
        Window::new("Ajustes de PAN").open(&mut open).resizable(true).default_width(460.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().max_height(490.0).show(ui, |ui| {
                    ui.heading("Almacenamiento");
                    if let Some(root) = &library.root {
                        ui.label(root.display().to_string());
                    }
                    ui.label(format!("Archivos multimedia: {}", size_label(library.items.iter().map(|item| item.size).sum())));
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(!library.busy(), Button::new("Cambiar carpeta…")).clicked() {
                            crate::input::storage_picker();
                        }
                        if ui.button("Abrir carpeta").clicked() { library.reveal(); }
                        if ui.add_enabled(!library.busy(), Button::new("Comprobar archivos")).clicked() {
                            library.reload();
                        }
                    });
                    ui.label("PAN copia la biblioteca a la ubicación nueva y conserva la carpeta anterior.");
                    if let Some((message, _)) = &library.message { ui.label(message); }
                    ui.separator();
                    ui.heading("Subtítulos");
                    let previous = settings.language.clone();
                    ui.horizontal(|ui| {
                        ui.label("Idioma preferido");
                        let languages = [("", "Automático"), ("es", "Español"), ("en", "Inglés"), ("fr", "Francés"),
                            ("ca", "Catalán"), ("pt", "Portugués"), ("it", "Italiano"), ("de", "Alemán"), ("ja", "Japonés"), ("und", "Indeterminado")];
                        ComboBox::from_id_salt("subtitle-language")
                            .selected_text(languages.iter().find(|(code, _)| *code == settings.language).map_or("Automático", |(_, name)| *name))
                            .show_ui(ui, |ui| {
                                for (code, label) in languages {
                                    ui.selectable_value(&mut settings.language, code.to_owned(), label);
                                }
                            });
                    });
                    changed |= previous != settings.language;
                    ui.horizontal(|ui| {
                        ui.label("Tamaño de subtítulos de texto");
                        let previous = settings.subtitle_size;
                        ComboBox::from_id_salt("subtitle-size")
                            .selected_text(match previous { 20 => "Pequeño", 12 => "Grande", 8 => "Muy grande", _ => "Normal" })
                            .show_ui(ui, |ui| {
                                for (size, label) in [(20, "Pequeño"), (16, "Normal"), (12, "Grande"), (8, "Muy grande")] {
                                    ui.selectable_value(&mut settings.subtitle_size, size, label);
                                }
                            });
                        changed |= previous != settings.subtitle_size;
                    });
                    ui.label("El idioma y el tamaño se aplican al abrir el siguiente archivo.");
                    ui.separator();
                    ui.heading("Atajos");
                    ui.label(if cfg!(target_os = "macos") { "⌘O abrir · ⌘L biblioteca · ⌘, ajustes" } else { "Ctrl+O abrir · Ctrl+L biblioteca · Ctrl+, ajustes" });
                    Grid::new("shortcut-keys").min_col_width(160.0).show(ui, |ui| {
                        for (index, (label, default)) in SHORTCUTS.iter().enumerate() {
                            ui.label(*label);
                            if ui.button(if self.capturing == Some(index) { "Pulsa una tecla…" } else {
                                settings.keys.get(index).map_or(*default, String::as_str)
                            }).clicked() { self.capturing = Some(index); }
                            ui.end_row();
                        }
                    });
                    if ui.button("Restablecer atajos").clicked() {
                        settings.keys = Settings::default().keys;
                        self.capturing = None;
                        self.message = None;
                        changed = true;
                    }
                    if let Some(index) = self.capturing {
                        let key = ctx.input(|i| i.events.iter().find_map(|event| match event {
                            Event::Key { key, pressed: true, repeat: false, modifiers, .. } if !modifiers.any() => Some(*key),
                            _ => None,
                        }));
                        if let Some(key) = key {
                            self.capturing = None;
                            if key != Key::Escape {
                                let name = key.name();
                                if settings.keys.iter().enumerate().any(|(i, key)| i != index && key == name) {
                                    self.message = Some("Esa tecla ya está asignada.".into());
                                } else {
                                    settings.keys[index] = name.into();
                                    self.message = None;
                                    changed = true;
                                }
                            }
                        }
                    }
                    if let Some(message) = &self.message { ui.label(message); }
                });
            });
        self.open = open;
        changed
    }
}

pub struct Editor {
    pub id: String,
    entry: Entry,
    query: String,
    language: &'static str,
    result_language: &'static str,
    pending_source: Option<String>,
    results: Vec<artwork::Match>,
    search: Option<Receiver<Result<Vec<artwork::Match>, String>>>,
    download: Option<Receiver<Result<Vec<u8>, String>>>,
    image: Option<TextureHandle>,
    cover: Option<Vec<u8>>,
    remove_cover: bool,
    message: Option<String>,
    open: bool,
}

pub enum EditAction {
    Keep,
    Close,
    Save(Entry),
}

impl Editor {
    pub fn new(id: String, entry: Entry, folder: &std::path::Path, ctx: &Context) -> Self {
        let query = if entry.title.is_empty() {
            std::path::Path::new(&entry.filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&entry.filename)
                .to_owned()
        } else {
            entry.title.clone()
        };
        let image = artwork::load(&artwork::cover_path(folder))
            .map(|image| ctx.load_texture("editor-cover", image, TextureOptions::LINEAR));
        Self {
            id,
            entry,
            query,
            language: "es",
            result_language: "es",
            pending_source: None,
            results: Vec::new(),
            search: None,
            download: None,
            image,
            cover: None,
            remove_cover: false,
            message: None,
            open: true,
        }
    }

    pub fn pick_cover(&mut self, path: &std::path::Path, ctx: &Context) {
        let result = artwork::local_file(path);
        self.set_cover(result, ctx);
        if self.message.is_none() {
            self.entry.artwork_source.clear();
        }
    }

    fn set_cover(&mut self, result: Result<Vec<u8>, String>, ctx: &Context) {
        match result.and_then(|bytes| artwork::preview(&bytes).map(|image| (bytes, image))) {
            Ok((bytes, image)) => {
                self.image = Some(ctx.load_texture("editor-cover", image, TextureOptions::LINEAR));
                self.cover = Some(bytes);
                self.remove_cover = false;
                self.message = None;
            }
            Err(error) => self.message = Some(error),
        }
    }

    pub fn show(&mut self, ctx: &Context, library: &Library) -> EditAction {
        if let Some(result) = completed(&mut self.search) {
            match result {
                Ok(results) => {
                    self.message = results
                        .is_empty()
                        .then(|| "Wikipedia no encontró coincidencias.".into());
                    self.results = results;
                }
                Err(error) => self.message = Some(format!("No se pudo buscar: {error}")),
            }
        }
        if let Some(result) = completed(&mut self.download) {
            self.set_cover(result, ctx);
            if let Some(source) = self
                .pending_source
                .take()
                .filter(|_| self.message.is_none())
            {
                self.entry.artwork_source = source;
            }
        }
        let mut open = self.open;
        let mut save = false;
        Window::new("Ficha del archivo")
            .open(&mut open)
            .default_width(450.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().max_height(500.0).show(ui, |ui| {
                    ui.label(&self.entry.filename);
                    for (label, field) in [
                        ("Título", &mut self.entry.title),
                        ("Artista o autor", &mut self.entry.creator),
                    ] {
                        ui.label(label);
                        ui.text_edit_singleline(field);
                    }
                    ui.label("Descripción");
                    ui.add(
                        TextEdit::multiline(&mut self.entry.description)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    ui.horizontal(|ui| {
                        if let Some(cover) = &self.image {
                            ui.add(Image::new(cover).max_size(vec2(80.0, 80.0)));
                        }
                        ui.vertical(|ui| {
                            if ui
                                .add_enabled(
                                    self.download.is_none(),
                                    Button::new("Elegir carátula…"),
                                )
                                .clicked()
                            {
                                crate::input::cover_picker();
                            }
                            if self.image.is_some()
                                && ui
                                    .add_enabled(
                                        self.download.is_none(),
                                        Button::new("Quitar carátula"),
                                    )
                                    .clicked()
                            {
                                self.remove_cover = true;
                                self.image = None;
                                self.cover = None;
                                self.entry.artwork_source.clear();
                            }
                            if !self.entry.artwork_source.is_empty() {
                                ui.hyperlink_to("Fuente en Wikipedia", &self.entry.artwork_source);
                            }
                        });
                    });
                    ui.separator();
                    ui.label("Buscar en Wikipedia");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.query);
                        ComboBox::from_id_salt("wiki-language")
                            .selected_text(self.language)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.language, "es", "es");
                                ui.selectable_value(&mut self.language, "en", "en");
                            });
                        if ui
                            .add_enabled(
                                self.search.is_none() && !self.query.trim().is_empty(),
                                Button::new("Buscar"),
                            )
                            .clicked()
                        {
                            self.message = None;
                            self.results.clear();
                            let query = self.query.trim().to_owned();
                            let language = self.language;
                            self.result_language = language;
                            self.search =
                                Some(background(ctx, move || artwork::search(&query, language)));
                        }
                    });
                    if self.search.is_some() || self.download.is_some() {
                        ui.spinner();
                    }
                    if let Some(message) = &self.message {
                        ui.label(message);
                    }
                    ui.label("Los resultados pueden incluir logotipos o imágenes del artículo.");
                    for result in &self.results {
                        ui.group(|ui| {
                            ui.strong(&result.title);
                            if let Some(description) = &result.description {
                                ui.label(description);
                            }
                            if ui.button("Usar ficha").clicked() {
                                self.entry.title = result.title.clone();
                                self.entry.description =
                                    result.description.clone().unwrap_or_default();
                            }
                            if result.has_image()
                                && ui
                                    .add_enabled(
                                        self.download.is_none(),
                                        Button::new("Usar carátula"),
                                    )
                                    .clicked()
                            {
                                let result = result.clone();
                                let source = result.source(self.result_language);
                                self.download = Some(background(ctx, move || result.download()));
                                self.pending_source = Some(source);
                            }
                        });
                    }
                    ui.separator();
                    save = ui
                        .add_enabled(
                            !library.busy() && self.download.is_none(),
                            Button::new("Guardar cambios"),
                        )
                        .clicked();
                });
            });
        self.open = open;
        if !open {
            return EditAction::Close;
        }
        if !save {
            return EditAction::Keep;
        }
        let Some(folder) = library.entry_folder(&self.id).filter(|p| p.is_dir()) else {
            self.message = Some("La carpeta de este archivo ya no está disponible.".into());
            return EditAction::Keep;
        };
        let result = if let Some(bytes) = &self.cover {
            artwork::save(&folder, bytes)
        } else if self.remove_cover {
            std::fs::remove_file(artwork::cover_path(&folder))
                .or_else(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        Ok(())
                    } else {
                        Err(e)
                    }
                })
                .map_err(|e| e.to_string())
        } else {
            Ok(())
        };
        match result {
            Ok(()) => EditAction::Save(self.entry.clone()),
            Err(error) => {
                self.message = Some(error);
                EditAction::Keep
            }
        }
    }
}

fn background<T: Send + 'static>(
    ctx: &Context,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Receiver<Result<T, String>> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(work());
        ctx.request_repaint();
    });
    rx
}

fn completed<T>(pending: &mut Option<Receiver<Result<T, String>>>) -> Option<Result<T, String>> {
    let result = match pending.as_ref()?.try_recv() {
        Ok(result) => result,
        Err(mpsc::TryRecvError::Empty) => return None,
        Err(mpsc::TryRecvError::Disconnected) => Err("Se interrumpió la operación.".into()),
    };
    *pending = None;
    Some(result)
}

pub fn shortcut(settings: &catalog::Settings, index: usize) -> Option<Key> {
    Key::from_name(
        settings
            .keys
            .get(index)
            .map_or(SHORTCUTS[index].1, String::as_str),
    )
}
