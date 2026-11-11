use eframe::egui::{self, Color32, RichText};
use pan_model::{LibrarySource, PanState};
use rfd::FileDialog;
use std::path::PathBuf;

mod vlc;
use vlc::Vlc;

const BACKGROUND: Color32 = Color32::from_rgb(15, 16, 19);
const SIDEBAR: Color32 = Color32::from_rgb(21, 22, 26);
const TRANSPORT: Color32 = Color32::from_rgb(25, 26, 30);
const SELECTED: Color32 = Color32::from_rgb(47, 48, 55);
const MUTED: Color32 = Color32::from_rgb(151, 153, 161);
const ACCENT: Color32 = Color32::from_rgb(104, 96, 232);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Videos,
    Music,
    Images,
    Playlists,
    Network,
}

struct PanPreview {
    state: PanState,
    page: Page,
    search: String,
    url: String,
    playlist_name: String,
    notice: String,
    logo: Option<egui::TextureHandle>,
    vlc: Option<Vlc>,
    engine_status: String,
}

impl PanPreview {
    fn new(ctx: &egui::Context) -> Self {
        let logo = image::load_from_memory(include_bytes!("../assets/pan-logo.png"))
            .ok()
            .map(|image| {
                let rgba = image.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                ctx.load_texture("pan-logo", color, egui::TextureOptions::NEAREST)
            });
        let (vlc, engine_status) = match Vlc::load() {
            Ok(vlc) => (Some(vlc), "libVLC 4 conectado".to_owned()),
            Err(error) => (None, error),
        };
        Self {
            state: PanState::default(),
            page: Page::Home,
            search: String::new(),
            url: String::new(),
            playlist_name: String::new(),
            notice: String::new(),
            logo,
            vlc,
            engine_status,
        }
    }

    fn nav_item(&mut self, ui: &mut egui::Ui, page: Page, label: &str) {
        let selected = self.page == page && self.search.is_empty();
        let button = egui::Button::new(RichText::new(label).size(14.0))
            .fill(if selected { SELECTED } else { SIDEBAR })
            .frame(selected);
        if ui.add_sized([ui.available_width(), 34.0], button).clicked() {
            self.search.clear();
            self.page = page;
        }
    }

    fn play_path(&mut self, path: PathBuf) {
        self.state.open_file(path.clone());
        self.notice = match &mut self.vlc {
            Some(vlc) => match vlc.play_file(&path) {
                Ok(()) => "Reproduciendo con libVLC".into(),
                Err(error) => error,
            },
            None => "Archivo seleccionado; libVLC 4 no está disponible.".into(),
        };
        self.page = Page::Home;
        self.search.clear();
    }

    fn open_file(&mut self) {
        if let Some(path) = FileDialog::new().pick_file() {
            self.play_path(path);
        }
    }

    fn add_folder(&mut self) {
        if let Some(path) = FileDialog::new().pick_folder() {
            if self
                .state
                .library
                .add_source(LibrarySource::LocalFolder(path))
            {
                self.notice = "Carpeta añadida a la biblioteca de esta vista previa.".into();
            } else {
                self.notice = "Esa carpeta ya está en la biblioteca.".into();
            }
            self.page = Page::Videos;
            self.search.clear();
        }
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        let offset = ((ui.available_height() - 340.0) / 2.0).max(48.0);
        ui.add_space(offset);
        ui.vertical_centered(|ui| {
            if let Some(logo) = &self.logo {
                ui.image((logo.id(), egui::vec2(88.0, 88.0)));
            }
            ui.add_space(10.0);
            ui.label(RichText::new("PAN").size(32.0).strong());
            ui.add_space(4.0);
            if let Some(item) = self.state.playback.current() {
                ui.label(RichText::new(&item.title).size(16.0));
                ui.label(RichText::new("El vídeo se abre en una ventana de libVLC").color(MUTED));
            } else {
                ui.label(RichText::new("Abre un archivo para empezar").color(MUTED));
            }
            ui.add_space(26.0);
            if ui
                .add_sized(
                    [220.0, 42.0],
                    egui::Button::new("Abrir archivo").fill(ACCENT),
                )
                .clicked()
            {
                self.open_file();
            }
            ui.add_space(10.0);
            ui.label(RichText::new("O arrastra un archivo a esta ventana").color(MUTED));
            ui.add_space(18.0);
            ui.label(
                RichText::new("Los archivos abiertos no se añaden a la biblioteca.")
                    .size(12.0)
                    .color(MUTED),
            );
        });
    }

    fn library(&mut self, ui: &mut egui::Ui) {
        let title = match self.page {
            Page::Videos => "Vídeos",
            Page::Music => "Música",
            Page::Images => "Imágenes",
            _ => "Biblioteca",
        };
        ui.heading(title);
        ui.label(RichText::new("Tus carpetas, añadidas por ti.").color(MUTED));
        ui.add_space(18.0);
        if ui.button("Añadir carpeta").clicked() {
            self.add_folder();
        }
        ui.add_space(22.0);
        ui.separator();
        let sources = self.state.library.sources().to_vec();
        if sources.is_empty() {
            ui.add_space(18.0);
            ui.label("Tu biblioteca está vacía.");
        }
        for source in sources {
            ui.horizontal(|ui| {
                match &source {
                    LibrarySource::LocalFolder(path) | LibrarySource::LocalFile(path) => {
                        ui.label(path.display().to_string());
                    }
                    LibrarySource::Provider {
                        provider,
                        source_id,
                    } => {
                        ui.label(format!("{provider}: {source_id}"));
                    }
                }
                if ui.small_button("Quitar").clicked() {
                    self.state.library.remove_source(&source);
                }
            });
        }
        ui.add_space(14.0);
        ui.label(
            RichText::new("La exploración de medios de estas carpetas aún está en desarrollo.")
                .size(12.0)
                .color(MUTED),
        );
    }

    fn playlists(&mut self, ui: &mut egui::Ui) {
        ui.heading("Playlists");
        ui.label(RichText::new("Organiza lo que quieras volver a reproducir.").color(MUTED));
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [240.0, 32.0],
                egui::TextEdit::singleline(&mut self.playlist_name)
                    .hint_text("Nombre de la playlist"),
            );
            if ui.button("Crear").clicked()
                && self
                    .state
                    .library
                    .create_playlist(self.playlist_name.clone())
            {
                self.playlist_name.clear();
            }
        });
        ui.add_space(18.0);
        let names: Vec<String> = self
            .state
            .library
            .playlists()
            .map(|playlist| playlist.name.clone())
            .collect();
        if names.is_empty() {
            ui.label("Todavía no hay playlists.");
        } else {
            for name in names {
                ui.label(name);
            }
        }
    }

    fn network(&mut self, ui: &mut egui::Ui) {
        ui.heading("Red");
        ui.label(RichText::new("Abre un enlace directo compatible con VLC.").color(MUTED));
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [360.0, 32.0],
                egui::TextEdit::singleline(&mut self.url).hint_text("https://…"),
            );
            if ui.button("Abrir URL").clicked() && !self.url.trim().is_empty() {
                let url = self.url.trim().to_owned();
                self.state.open_url(url.clone());
                self.notice = match &mut self.vlc {
                    Some(vlc) => match vlc.play_url(&url) {
                        Ok(()) => "Reproduciendo URL con libVLC".into(),
                        Err(error) => error,
                    },
                    None => "URL seleccionada; libVLC 4 no está disponible.".into(),
                };
                self.page = Page::Home;
            }
        });
        ui.add_space(22.0);
        ui.label(
            RichText::new("Los servicios con cuenta, como Google Drive, requieren un conector.")
                .size(12.0)
                .color(MUTED),
        );
    }

    fn search_results(&mut self, ui: &mut egui::Ui) {
        let query = self.search.trim().to_lowercase();
        ui.heading(format!("Buscar: {}", self.search.trim()));
        ui.add_space(18.0);
        let mut found = false;
        for source in self.state.library.sources() {
            let name = match source {
                LibrarySource::LocalFolder(path) | LibrarySource::LocalFile(path) => {
                    path.display().to_string()
                }
                LibrarySource::Provider {
                    provider,
                    source_id,
                } => format!("{provider}: {source_id}"),
            };
            if name.to_lowercase().contains(&query) {
                ui.label(name);
                found = true;
            }
        }
        for playlist in self.state.library.playlists() {
            if playlist.name.to_lowercase().contains(&query) {
                ui.label(format!("Playlist · {}", playlist.name));
                found = true;
            }
        }
        if !found {
            ui.label("No hay resultados en tu biblioteca.");
        }
    }
}

impl eframe::App for PanPreview {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BACKGROUND;
        visuals.window_fill = BACKGROUND;
        ctx.set_visuals(visuals);

        for file in ctx.input(|input| input.raw.dropped_files.clone()) {
            self.play_path(file.path().to_path_buf());
        }

        egui::Panel::bottom("pan_transport")
            .exact_size(64.0)
            .frame(egui::Frame::default().fill(TRANSPORT))
            .show(ui, |ui| {
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    if ui
                        .add_enabled(self.vlc.is_some(), egui::Button::new("▶ / ⏸"))
                        .clicked()
                    {
                        if let Some(vlc) = &mut self.vlc {
                            vlc.toggle_pause();
                        }
                    }
                    if ui
                        .add_enabled(self.vlc.is_some(), egui::Button::new("■"))
                        .clicked()
                    {
                        if let Some(vlc) = &mut self.vlc {
                            vlc.stop();
                        }
                    }
                    ui.add_space(18.0);
                    if let Some(item) = self.state.playback.current() {
                        ui.label(&item.title);
                    } else {
                        ui.label(RichText::new("Nada en reproducción").color(MUTED));
                    }
                });
            });

        egui::Panel::left("pan_navigation")
            .exact_size(224.0)
            .frame(egui::Frame::default().fill(SIDEBAR))
            .show(ui, |ui| {
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    if let Some(logo) = &self.logo {
                        ui.image((logo.id(), egui::vec2(30.0, 30.0)));
                    }
                    ui.label(RichText::new("PAN").size(22.0).strong());
                });
                ui.add_space(34.0);
                ui.label(RichText::new("REPRODUCCIÓN").size(11.0).color(MUTED));
                ui.add_space(8.0);
                self.nav_item(ui, Page::Home, "Inicio");
                if ui
                    .add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::new("Abrir archivo").frame(false),
                    )
                    .clicked()
                {
                    self.open_file();
                }
                ui.add_space(24.0);
                ui.label(RichText::new("BIBLIOTECA").size(11.0).color(MUTED));
                ui.add_space(8.0);
                self.nav_item(ui, Page::Videos, "Vídeos");
                self.nav_item(ui, Page::Music, "Música");
                self.nav_item(ui, Page::Images, "Imágenes");
                self.nav_item(ui, Page::Playlists, "Playlists");
                ui.add_space(24.0);
                ui.label(RichText::new("RED").size(11.0).color(MUTED));
                ui.add_space(8.0);
                self.nav_item(ui, Page::Network, "Abrir URL");
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BACKGROUND))
            .show(ui, |ui| {
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(22.0);
                        ui.add_sized(
                            [240.0, 32.0],
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Buscar en tu biblioteca"),
                        );
                    });
                });
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width() - 24.0);
                        if !self.search.trim().is_empty() {
                            self.search_results(ui);
                        } else {
                            match self.page {
                                Page::Home => self.home(ui),
                                Page::Videos | Page::Music | Page::Images => self.library(ui),
                                Page::Playlists => self.playlists(ui),
                                Page::Network => self.network(ui),
                            }
                        }
                        if !self.notice.is_empty() {
                            ui.add_space(22.0);
                            ui.label(RichText::new(&self.notice).size(12.0).color(MUTED));
                        }
                        if self.vlc.is_none() && self.page == Page::Home {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(
                                    "Vista previa: motor VLC no disponible en este paquete",
                                )
                                .size(11.0)
                                .color(MUTED),
                            )
                            .on_hover_text(&self.engine_status);
                        }
                    });
                });
            });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PAN Preview",
        options,
        Box::new(|creation_context| Ok(Box::new(PanPreview::new(&creation_context.egui_ctx)))),
    )
}
