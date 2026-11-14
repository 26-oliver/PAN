use crate::locale::{self, Language};
use crate::{
    artwork,
    catalog::{self, Entry, Settings, SHORTCUT_KEYS},
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
        Window::new(tr!("PAN Settings", "Ajustes de PAN", "Paramètres de PAN")).id(Id::new("pan-settings")).open(&mut open).resizable(true).default_width(460.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().max_height(490.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(tr!("Interface language", "Idioma de la interfaz", "Langue de l’interface"));
                        ComboBox::from_id_salt("interface-language")
                            .selected_text(settings.ui_language.native_name())
                            .show_ui(ui, |ui| {
                                for language in Language::ALL {
                                    if ui.selectable_value(&mut settings.ui_language, language, language.native_name()).changed() {
                                        locale::set(language);
                                        changed = true;
                                        ctx.request_repaint();
                                    }
                                }
                            });
                    });
                    ui.separator();
                    ui.heading(tr!("Storage", "Almacenamiento", "Stockage"));
                    if let Some(root) = &library.root {
                        ui.label(root.display().to_string());
                    }
                    ui.label(tf!("Media files: {}", "Archivos multimedia: {}", "Fichiers multimédias : {}"; size_label(library.items.iter().map(|item| item.size).sum())));
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(!library.busy(), Button::new(tr!("Change folder…", "Cambiar carpeta…", "Changer de dossier…"))).clicked() {
                            crate::input::storage_picker();
                        }
                        if ui.button(tr!("Open folder", "Abrir carpeta", "Ouvrir le dossier")).clicked() { library.reveal(); }
                        if ui.add_enabled(!library.busy(), Button::new(tr!("Check files", "Comprobar archivos", "Vérifier les fichiers"))).clicked() {
                            library.reload();
                        }
                    });
                    ui.label(tr!("PAN copies the library to the new location and keeps the previous folder.", "PAN copia la biblioteca a la ubicación nueva y conserva la carpeta anterior.", "PAN copie la bibliothèque au nouvel emplacement et conserve le dossier précédent."));
                    if let Some((message, _)) = &library.message { ui.label(message); }
                    ui.separator();
                    ui.heading(tr!("Subtitles", "Subtítulos", "Sous-titres"));
                    let previous = settings.language.clone();
                    ui.horizontal(|ui| {
                        ui.label(tr!("Preferred language", "Idioma preferido", "Langue préférée"));
                        let languages = [("", tr!("Automatic", "Automático", "Automatique")), ("es", tr!("Spanish", "Español", "Espagnol")), ("en", tr!("English", "Inglés", "Anglais")), ("fr", tr!("French", "Francés", "Français")),
                            ("ca", tr!("Catalan", "Catalán", "Catalan")), ("pt", tr!("Portuguese", "Portugués", "Portugais")), ("it", tr!("Italian", "Italiano", "Italien")), ("de", tr!("German", "Alemán", "Allemand")), ("ja", tr!("Japanese", "Japonés", "Japonais")), ("und", tr!("Undetermined", "Indeterminado", "Indéterminée"))];
                        ComboBox::from_id_salt("subtitle-language")
                            .selected_text(languages.iter().find(|(code, _)| *code == settings.language).map_or(tr!("Automatic", "Automático", "Automatique"), |(_, name)| *name))
                            .show_ui(ui, |ui| {
                                for (code, label) in languages {
                                    ui.selectable_value(&mut settings.language, code.to_owned(), label);
                                }
                            });
                    });
                    changed |= previous != settings.language;
                    ui.horizontal(|ui| {
                        ui.label(tr!("Subtitle text size", "Tamaño de subtítulos de texto", "Taille du texte des sous-titres"));
                        let previous = settings.subtitle_size;
                        ComboBox::from_id_salt("subtitle-size")
                            .selected_text(match previous { 20 => tr!("Small", "Pequeño", "Petit"), 12 => tr!("Large", "Grande", "Grand"), 8 => tr!("Extra large", "Muy grande", "Très grand"), _ => tr!("Normal", "Normal", "Normal") })
                            .show_ui(ui, |ui| {
                                for (size, label) in [(20, tr!("Small", "Pequeño", "Petit")), (16, tr!("Normal", "Normal", "Normal")), (12, tr!("Large", "Grande", "Grand")), (8, tr!("Extra large", "Muy grande", "Très grand"))] {
                                    ui.selectable_value(&mut settings.subtitle_size, size, label);
                                }
                            });
                        changed |= previous != settings.subtitle_size;
                    });
                    ui.label(tr!("Language and size apply to the next file you open.", "El idioma y el tamaño se aplican al abrir el siguiente archivo.", "La langue et la taille s’appliquent au prochain fichier ouvert."));
                    ui.separator();
                    ui.heading(tr!("Shortcuts", "Atajos", "Raccourcis"));
                    ui.label(if cfg!(target_os = "macos") { tr!("⌘O open · ⌘L library · ⌘, settings", "⌘O abrir · ⌘L biblioteca · ⌘, ajustes", "⌘O ouvrir · ⌘L bibliothèque · ⌘, paramètres") } else { tr!("Ctrl+O open · Ctrl+L library · Ctrl+, settings", "Ctrl+O abrir · Ctrl+L biblioteca · Ctrl+, ajustes", "Ctrl+O ouvrir · Ctrl+L bibliothèque · Ctrl+, paramètres") });
                    Grid::new("shortcut-keys").min_col_width(160.0).show(ui, |ui| {
                        for (index, (label, default)) in catalog::shortcut_labels().into_iter().zip(SHORTCUT_KEYS).enumerate() {
                            ui.label(label);
                            if ui.button(if self.capturing == Some(index) { tr!("Press a key…", "Pulsa una tecla…", "Appuyez sur une touche…") } else {
                                settings.keys.get(index).map_or(default, String::as_str)
                            }).clicked() { self.capturing = Some(index); }
                            ui.end_row();
                        }
                    });
                    if ui.button(tr!("Reset shortcuts", "Restablecer atajos", "Réinitialiser les raccourcis")).clicked() {
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
                                    self.message = Some(tr!("That key is already assigned.", "Esa tecla ya está asignada.", "Cette touche est déjà attribuée.").into());
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
            language: locale::current().code(),
            result_language: locale::current().code(),
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
                    self.message = results.is_empty().then(|| {
                        tr!(
                            "Wikipedia found no matches.",
                            "Wikipedia no encontró coincidencias.",
                            "Wikipédia n’a trouvé aucun résultat."
                        )
                        .into()
                    });
                    self.results = results;
                }
                Err(error) => {
                    self.message = Some(tf!(
                        "Search failed: {error}",
                        "No se pudo buscar: {error}",
                        "Échec de la recherche : {error}"
                    ))
                }
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
        Window::new(tr!(
            "File details",
            "Ficha del archivo",
            "Détails du fichier"
        ))
        .open(&mut open)
        .default_width(450.0)
        .show(ctx, |ui| {
            ScrollArea::vertical().max_height(500.0).show(ui, |ui| {
                ui.label(&self.entry.filename);
                for (label, field) in [
                    (tr!("Title", "Título", "Titre"), &mut self.entry.title),
                    (
                        tr!("Artist or author", "Artista o autor", "Artiste ou auteur"),
                        &mut self.entry.creator,
                    ),
                ] {
                    ui.label(label);
                    ui.text_edit_singleline(field);
                }
                ui.label(tr!("Description", "Descripción", "Description"));
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
                                Button::new(tr!(
                                    "Choose artwork…",
                                    "Elegir carátula…",
                                    "Choisir une pochette…"
                                )),
                            )
                            .clicked()
                        {
                            crate::input::cover_picker();
                        }
                        if self.image.is_some()
                            && ui
                                .add_enabled(
                                    self.download.is_none(),
                                    Button::new(tr!(
                                        "Remove artwork",
                                        "Quitar carátula",
                                        "Retirer la pochette"
                                    )),
                                )
                                .clicked()
                        {
                            self.remove_cover = true;
                            self.image = None;
                            self.cover = None;
                            self.entry.artwork_source.clear();
                        }
                        if !self.entry.artwork_source.is_empty() {
                            ui.hyperlink_to(
                                tr!(
                                    "Wikipedia source",
                                    "Fuente en Wikipedia",
                                    "Source Wikipédia"
                                ),
                                &self.entry.artwork_source,
                            );
                        }
                    });
                });
                ui.separator();
                ui.label(tr!(
                    "Search Wikipedia",
                    "Buscar en Wikipedia",
                    "Rechercher sur Wikipédia"
                ));
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.query);
                    ComboBox::from_id_salt("wiki-language")
                        .selected_text(self.language)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.language, "es", "es");
                            ui.selectable_value(&mut self.language, "en", "en");
                            ui.selectable_value(&mut self.language, "fr", "fr");
                        });
                    if ui
                        .add_enabled(
                            self.search.is_none() && !self.query.trim().is_empty(),
                            Button::new(tr!("Search", "Buscar", "Rechercher")),
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
                ui.label(tr!(
                    "Results may include logos or images from the article.",
                    "Los resultados pueden incluir logotipos o imágenes del artículo.",
                    "Les résultats peuvent inclure des logos ou des images de l’article."
                ));
                for result in &self.results {
                    ui.group(|ui| {
                        ui.strong(&result.title);
                        if let Some(description) = &result.description {
                            ui.label(description);
                        }
                        if ui
                            .button(tr!("Use details", "Usar ficha", "Utiliser les détails"))
                            .clicked()
                        {
                            self.entry.title = result.title.clone();
                            self.entry.description = result.description.clone().unwrap_or_default();
                        }
                        if result.has_image()
                            && ui
                                .add_enabled(
                                    self.download.is_none(),
                                    Button::new(tr!(
                                        "Use artwork",
                                        "Usar carátula",
                                        "Utiliser la pochette"
                                    )),
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
                        Button::new(tr!(
                            "Save changes",
                            "Guardar cambios",
                            "Enregistrer les modifications"
                        )),
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
            self.message = Some(
                tr!(
                    "The folder for this file is no longer available.",
                    "La carpeta de este archivo ya no está disponible.",
                    "Le dossier de ce fichier n’est plus disponible."
                )
                .into(),
            );
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
        Err(mpsc::TryRecvError::Disconnected) => Err(tr!(
            "The operation was interrupted.",
            "Se interrumpió la operación.",
            "L’opération a été interrompue."
        )
        .into()),
    };
    *pending = None;
    Some(result)
}

pub fn shortcut(settings: &catalog::Settings, index: usize) -> Option<Key> {
    Key::from_name(
        settings
            .keys
            .get(index)
            .map_or(SHORTCUT_KEYS[index], String::as_str),
    )
}
