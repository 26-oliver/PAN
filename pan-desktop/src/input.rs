use std::{path::PathBuf, sync::Mutex};

pub struct Inputs {
    pub files: Vec<PathBuf>,
    pub subtitles: Vec<PathBuf>,
    pub imports: Vec<PathBuf>,
    pub actions: Vec<i32>,
    pub folders: Vec<PathBuf>,
    pub covers: Vec<PathBuf>,
}

static PENDING: Mutex<Inputs> = Mutex::new(Inputs {
    files: Vec::new(),
    subtitles: Vec::new(),
    imports: Vec::new(),
    actions: Vec::new(),
    folders: Vec::new(),
    covers: Vec::new(),
});

pub fn drain() -> Inputs {
    let mut pending = PENDING.lock().unwrap();
    Inputs {
        files: std::mem::take(&mut pending.files),
        subtitles: std::mem::take(&mut pending.subtitles),
        imports: std::mem::take(&mut pending.imports),
        actions: std::mem::take(&mut pending.actions),
        folders: std::mem::take(&mut pending.folders),
        covers: std::mem::take(&mut pending.covers),
    }
}

fn received(path: PathBuf, subtitles: bool) {
    let mut pending = PENDING.lock().unwrap();
    if subtitles {
        pending.subtitles.push(path);
    } else {
        pending.files.push(path);
    }
}

#[cfg(target_os = "macos")]
extern "C" {
    fn pan_register_open_events();
    fn pan_install_glass_toolbar(view: *mut std::ffi::c_void) -> i32;
    fn pan_localize_glass_toolbar(view: *mut std::ffi::c_void);
    fn pan_pick_file(kind: std::ffi::c_int);
}

#[cfg(target_os = "macos")]
#[no_mangle]
unsafe extern "C" fn pan_open_path(path: *const std::ffi::c_char, kind: std::ffi::c_int) {
    use std::{
        ffi::{CStr, OsStr},
        os::unix::ffi::OsStrExt,
    };

    if !path.is_null() {
        let path = OsStr::from_bytes(CStr::from_ptr(path).to_bytes()).into();
        match kind {
            2 => PENDING.lock().unwrap().imports.push(path),
            3 => PENDING.lock().unwrap().folders.push(path),
            4 => PENDING.lock().unwrap().covers.push(path),
            _ => received(path, kind == 1),
        }
    }
}

pub fn register() {
    #[cfg(target_os = "macos")]
    unsafe {
        pan_register_open_events()
    }
}

pub fn picker(subtitles: bool) {
    #[cfg(target_os = "macos")]
    unsafe {
        pan_pick_file(i32::from(subtitles))
    }

    #[cfg(not(target_os = "macos"))]
    std::thread::spawn(move || {
        let dialog = rfd::FileDialog::new();
        if subtitles {
            let file = dialog
                .set_title(tr!(
                    "Open subtitles",
                    "Abrir subtítulos",
                    "Ouvrir des sous-titres"
                ))
                .add_filter(
                    tr!("Subtitles", "Subtítulos", "Sous-titres"),
                    &["srt", "ass", "ssa", "vtt", "sub"],
                )
                .pick_file();
            if let Some(path) = file {
                received(path, true);
            }
        } else if let Some(files) = dialog
            .set_title(tr!("Open file", "Abrir archivo", "Ouvrir un fichier"))
            .pick_files()
        {
            PENDING.lock().unwrap().files.extend(files);
        }
    });
}

#[cfg(target_os = "macos")]
#[no_mangle]
extern "C" fn pan_toolbar_action(index: i32) {
    PENDING.lock().unwrap().actions.push(index);
}

pub fn glass_toolbar(_cc: &eframe::CreationContext<'_>) -> bool {
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = _cc.window_handle() {
            if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
                return unsafe { pan_install_glass_toolbar(handle.ns_view.as_ptr()) != 0 };
            }
        }
    }
    false
}

pub fn import_picker() {
    #[cfg(target_os = "macos")]
    unsafe {
        pan_pick_file(2)
    }

    #[cfg(not(target_os = "macos"))]
    std::thread::spawn(|| {
        if let Some(files) = rfd::FileDialog::new()
            .set_title(tr!("Add to PAN", "Añadir a PAN", "Ajouter à PAN"))
            .pick_files()
        {
            PENDING.lock().unwrap().imports.extend(files);
        }
    });
}

pub fn storage_picker() {
    #[cfg(target_os = "macos")]
    unsafe {
        pan_pick_file(3)
    }
    #[cfg(not(target_os = "macos"))]
    std::thread::spawn(|| {
        if let Some(path) = rfd::FileDialog::new()
            .set_title(tr!(
                "Choose library location",
                "Elegir ubicación de la biblioteca",
                "Choisir l’emplacement de la bibliothèque"
            ))
            .pick_folder()
        {
            PENDING.lock().unwrap().folders.push(path);
        }
    });
}

pub fn cover_picker() {
    #[cfg(target_os = "macos")]
    unsafe {
        pan_pick_file(4)
    }
    #[cfg(not(target_os = "macos"))]
    std::thread::spawn(|| {
        if let Some(path) = rfd::FileDialog::new()
            .set_title(tr!(
                "Choose artwork",
                "Elegir carátula",
                "Choisir une pochette"
            ))
            .add_filter(
                tr!("Images", "Imágenes", "Images"),
                &["jpg", "jpeg", "png", "webp"],
            )
            .pick_file()
        {
            PENDING.lock().unwrap().covers.push(path);
        }
    });
}

pub fn refresh_toolbar(_frame: &eframe::Frame) {
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = _frame.window_handle() {
            if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
                unsafe { pan_localize_glass_toolbar(handle.ns_view.as_ptr()) };
            }
        }
    }
}
