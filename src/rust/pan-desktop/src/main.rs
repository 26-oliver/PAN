use eframe::egui::{self, Color32, RichText};
use pan_model::{LibrarySource, PanState};
use rfd::FileDialog;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Library,
    Playlists,
    Internet,
}

struct PanPreview {
    state: PanState,
    page: Page,
    url: String,
    playlist_name: String,
    notice: String,
    logo: Option<egui::TextureHandle>,
}

impl Default for PanPreview {
    fn default() -> Self {
        Self {
            state: PanState::default(),
            page: Page::Home,
            url: String::new(),
            playlist_name: String::new(),
            notice: String::new(),
            logo: None,
        }
    }
}

impl PanPreview {
    fn new(ctx: &egui::Context) -> Self {
        let mut app = Self::default();
        if let Ok(image) = image::load_from_memory(include_bytes!("../assets/pan-logo.png")) {
            let rgba = image.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            app.logo = Some(ctx.load_texture("pan-logo", color, egui::TextureOptions::NEAREST));
        }
        app
    }

    fn nav_item(&mut self, ui: &mut egui::Ui, page: Page, label: &str) {
        if ui.selectable_label(self.page == page, label).clicked() {
            self.page = page;
        }
    }

    fn open_file(&mut self) {
        if let Some(path) = FileDialog::new().pick_file() {
            self.state.open_file(path);
            self.notice = "Archivo seleccionado. La reproducción llegará con libVLC.".into();
            self.page = Page::Home;
        }
    }

    fn add_folder(&mut self) {
        if let Some(path) = FileDialog::new().pick_folder() {
            self.state.library.add_source(LibrarySource::LocalFolder(path));
            self.notice = "Carpeta añadida a la biblioteca de esta vista previa.".into();
            self.page = Page::Library;
        }
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        ui.heading("Reproduce lo que quieras");
        ui.label("Abre un archivo directamente. Tu biblioteca solo cambia cuando tú lo decides.");
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            if ui.button("Abrir archivo").clicked() {
                self.open_file();
            }
            if ui.button("Añadir carpeta a la biblioteca").clicked() {
                self.add_folder();
            }
        });
        ui.add_space(32.0);
        ui.heading("Ahora");
        if let Some(item) = self.state.playback.current() {
            ui.label(RichText::new(&item.title).size(22.0));
            ui.label("Vista previa de la interfaz · reproducción aún no disponible");
        } else {
            ui.label("No hay ningún archivo abierto");
        }
    }

    fn library(&mut self, ui: &mut egui::Ui) {
        ui.heading("Biblioteca");
        ui.label("Solo aparecen las fuentes que añadas expresamente.");
        ui.add_space(16.0);
        if ui.button("Añadir carpeta").clicked() {
            self.add_folder();
        }
        ui.separator();
        if self.state.library.sources().is_empty() {
            ui.label("Tu biblioteca está vacía.");
        } else {
            let sources = self.state.library.sources().to_vec();
            for source in sources {
                ui.horizontal(|ui| {
                    match &source {
                        LibrarySource::LocalFolder(path) => {
                            ui.label(format!("📁 {}", path.display()));
                        }
                        LibrarySource::LocalFile(path) => {
                            ui.label(format!("♫ {}", path.display()));
                        }
                        LibrarySource::Provider { provider, source_id } => {
                            ui.label(format!("{provider}: {source_id}"));
                        }
                    }
                    if ui.small_button("Quitar").clicked() {
                        self.state.library.remove_source(&source);
                    }
                });
            }
        }
    }

    fn playlists(&mut self, ui: &mut egui::Ui) {
        ui.heading("Playlists");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.playlist_name);
            if ui.button("Crear").clicked()
                && self.state.library.create_playlist(self.playlist_name.clone())
            {
                self.notice = format!("Playlist «{}» creada.", self.playlist_name);
                self.playlist_name.clear();
            }
        });
        ui.label("La edición y reproducción de playlists se conectarán en una versión posterior.");
    }

    fn internet(&mut self, ui: &mut egui::Ui) {
        ui.heading("Internet");
        ui.label("Abre una URL multimedia directa compatible con VLC.");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.url);
            if ui.button("Abrir URL").clicked() && !self.url.trim().is_empty() {
                self.state.open_url(self.url.trim().to_owned());
                self.notice = "URL seleccionada. La reproducción llegará con libVLC.".into();
                self.page = Page::Home;
            }
        });
        ui.add_space(16.0);
        ui.label("Google Drive y otros servicios necesitarán conectores con inicio de sesión.");
    }
}

impl eframe::App for PanPreview {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());

        egui::SidePanel::left("pan_navigation")
            .exact_width(220.0)
            .show(ctx, |ui| {
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if let Some(logo) = &self.logo {
                        ui.image((logo.id(), egui::vec2(48.0, 48.0)));
                    }
                    ui.label(RichText::new("PAN").size(32.0).strong().color(Color32::from_rgb(155, 130, 255)));
                });
                ui.label("Play Anything Now");
                ui.add_space(30.0);
                self.nav_item(ui, Page::Home, "⌂  Inicio");
                self.nav_item(ui, Page::Library, "▦  Biblioteca");
                self.nav_item(ui, Page::Playlists, "≡  Playlists");
                self.nav_item(ui, Page::Internet, "◎  Internet");
                ui.add_space(24.0);
                ui.separator();
                ui.label("Vista previa de PAN");
            });

        egui::TopBottomPanel::bottom("pan_transport")
            .min_height(72.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label("◀");
                    ui.label("▶");
                    ui.label("■");
                    ui.separator();
                    if let Some(item) = self.state.playback.current() {
                        ui.label(&item.title);
                    } else {
                        ui.label("Nada seleccionado");
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(22.0);
            ui.horizontal(|ui| {
                ui.heading(match self.page {
                    Page::Home => "Inicio",
                    Page::Library => "Biblioteca",
                    Page::Playlists => "Playlists",
                    Page::Internet => "Internet",
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Abrir archivo").clicked() {
                        self.open_file();
                    }
                });
            });
            ui.separator();
            ui.add_space(24.0);
            match self.page {
                Page::Home => self.home(ui),
                Page::Library => self.library(ui),
                Page::Playlists => self.playlists(ui),
                Page::Internet => self.internet(ui),
            }
            if !self.notice.is_empty() {
                ui.add_space(32.0);
                ui.colored_label(Color32::from_rgb(180, 170, 255), &self.notice);
            }
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
