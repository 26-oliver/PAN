//! Minimal LibVLC 4 bridge for PAN. Symbols are loaded at runtime so the
//! interface preview still opens when the matching VLC engine is unavailable.

use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr;

type Handle = *mut c_void;

#[derive(Clone, Copy)]
struct Functions {
    release: unsafe extern "C" fn(Handle),
    media_new_path: unsafe extern "C" fn(*const c_char) -> Handle,
    media_new_location: unsafe extern "C" fn(*const c_char) -> Handle,
    media_release: unsafe extern "C" fn(Handle),
    player_new: unsafe extern "C" fn(Handle, *const c_void, *mut c_void) -> Handle,
    player_release: unsafe extern "C" fn(Handle),
    player_set_media: unsafe extern "C" fn(Handle, Handle),
    player_play: unsafe extern "C" fn(Handle) -> c_int,
    player_set_pause: unsafe extern "C" fn(Handle, c_int),
    player_stop: unsafe extern "C" fn(Handle) -> c_int,
    player_is_playing: unsafe extern "C" fn(Handle) -> bool,
}

pub struct Vlc {
    // The library must outlive every function pointer and VLC object.
    _library: Library,
    instance: Handle,
    player: Handle,
    functions: Functions,
}

// PAN calls this bridge only from the eframe application thread. LibVLC owns
// its internal worker threads and no PAN callback receives these pointers.
unsafe impl Send for Vlc {}

unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    library
        .get::<T>(name)
        .map(|symbol| *symbol)
        .map_err(|error| format!("Missing LibVLC symbol: {error}"))
}

impl Vlc {
    pub fn load() -> Result<Self, String> {
        let candidates = library_candidates();
        let mut last_error = String::from("No se encontró libVLC 4");
        for path in candidates {
            // Loading a shared library executes its initializers. Only the
            // user-specified path or standard VLC installation paths are used.
            let library = match unsafe { Library::new(&path) } {
                Ok(library) => library,
                Err(error) => {
                    last_error = error.to_string();
                    continue;
                }
            };
            match unsafe { Self::from_library(library) } {
                Ok(vlc) => return Ok(vlc),
                Err(error) => last_error = error,
            }
        }
        Err(format!("{last_error}. Instala una compilación de este fork de VLC 4 o define PAN_LIBVLC_PATH."))
    }

    unsafe fn from_library(library: Library) -> Result<Self, String> {
        let get_version: unsafe extern "C" fn() -> *const c_char =
            symbol(&library, b"libvlc_get_version")?;
        let version_ptr = get_version();
        if version_ptr.is_null() {
            return Err("libVLC no devolvió su versión".into());
        }
        let version = CStr::from_ptr(version_ptr).to_string_lossy();
        if !version.starts_with("4.") {
            return Err(format!("PAN necesita libVLC 4; se encontró {version}"));
        }

        let new: unsafe extern "C" fn(c_int, *const *const c_char) -> Handle =
            symbol(&library, b"libvlc_new")?;
        let functions = Functions {
            release: symbol(&library, b"libvlc_release")?,
            media_new_path: symbol(&library, b"libvlc_media_new_path")?,
            media_new_location: symbol(&library, b"libvlc_media_new_location")?,
            media_release: symbol(&library, b"libvlc_media_release")?,
            player_new: symbol(&library, b"libvlc_media_player_new")?,
            player_release: symbol(&library, b"libvlc_media_player_release")?,
            player_set_media: symbol(&library, b"libvlc_media_player_set_media")?,
            player_play: symbol(&library, b"libvlc_media_player_play")?,
            player_set_pause: symbol(&library, b"libvlc_media_player_set_pause")?,
            player_stop: symbol(&library, b"libvlc_media_player_stop_async")?,
            player_is_playing: symbol(&library, b"libvlc_media_player_is_playing")?,
        };
        let instance = new(0, ptr::null());
        if instance.is_null() {
            return Err("No se pudo iniciar libVLC".into());
        }
        let player = (functions.player_new)(instance, ptr::null(), ptr::null_mut());
        if player.is_null() {
            (functions.release)(instance);
            return Err("No se pudo crear el reproductor de libVLC".into());
        }
        Ok(Self {
            _library: library,
            instance,
            player,
            functions,
        })
    }

    pub fn play_file(&mut self, path: &Path) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or("La ruta contiene caracteres que libVLC no puede abrir")?;
        let path = CString::new(path).map_err(|_| "La ruta contiene un carácter nulo")?;
        self.play_media(path.as_ptr(), self.functions.media_new_path)
    }

    pub fn play_url(&mut self, url: &str) -> Result<(), String> {
        let url = CString::new(url).map_err(|_| "La URL contiene un carácter nulo")?;
        self.play_media(url.as_ptr(), self.functions.media_new_location)
    }

    fn play_media(
        &mut self,
        location: *const c_char,
        make_media: unsafe extern "C" fn(*const c_char) -> Handle,
    ) -> Result<(), String> {
        unsafe {
            let media = make_media(location);
            if media.is_null() {
                return Err("libVLC no pudo abrir esta fuente".into());
            }
            (self.functions.player_set_media)(self.player, media);
            (self.functions.media_release)(media);
            if (self.functions.player_play)(self.player) != 0 {
                return Err("libVLC no pudo iniciar la reproducción".into());
            }
        }
        Ok(())
    }

    pub fn toggle_pause(&mut self) {
        let playing = unsafe { (self.functions.player_is_playing)(self.player) };
        unsafe { (self.functions.player_set_pause)(self.player, if playing { 1 } else { 0 }) };
    }

    pub fn stop(&mut self) {
        unsafe { (self.functions.player_stop)(self.player) };
    }
}

impl Drop for Vlc {
    fn drop(&mut self) {
        unsafe {
            (self.functions.player_release)(self.player);
            (self.functions.release)(self.instance);
        }
    }
}

fn library_candidates() -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os("PAN_LIBVLC_PATH") {
        return vec![PathBuf::from(path)];
    }

    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/Applications/VLC.app/Contents/MacOS/lib/libvlc.dylib"),
            PathBuf::from("libvlc.dylib"),
        ]
    }
    #[cfg(target_os = "windows")]
    {
        vec![PathBuf::from("libvlc.dll")]
    }
    #[cfg(target_os = "linux")]
    {
        vec![PathBuf::from("libvlc.so.12"), PathBuf::from("libvlc.so")]
    }
}
