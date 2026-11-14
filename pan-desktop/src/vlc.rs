use crate::video::{self, Frames};
use eframe::egui::ColorImage;
use libloading::Library;
use std::{
    ffi::{c_char, c_float, c_int, c_uint, c_void, CStr, CString},
    path::{Path, PathBuf},
    ptr::NonNull,
};

type Handle = *mut c_void;

#[repr(C)]
struct TrackDescription {
    id: c_int,
    name: *const c_char,
    next: *mut TrackDescription,
}

// The signatures below use the public libVLC 3 C ABI. Keep the library resident
// for at least as long as the instance, player, and their callbacks.
macro_rules! libvlc {
    ($($name:ident($($arg:ty),*) $(-> $output:ty)?;)+) => {
        struct Api {
            $($name: unsafe extern "C" fn($($arg),*) $(-> $output)?,)+
            _library: Library,
        }

        impl Api {
            unsafe fn load(path: &Path) -> Result<Self, libloading::Error> {
                #[cfg(not(windows))]
                let library = Library::new(path)?;
                #[cfg(windows)]
                let library: Library = {
                    use libloading::os::windows::{Library as WindowsLibrary,
                        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS};
                    WindowsLibrary::load_with_flags(
                        path, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS
                    )?.into()
                };
                Ok(Self {
                    $($name: *library.get(concat!("libvlc_", stringify!($name), "\0").as_bytes())?,)+
                    _library: library,
                })
            }
        }
    };
}

libvlc! {
    get_version() -> *const c_char;
    new(c_int, *const *const c_char) -> Handle;
    release(Handle);
    free(Handle);
    media_parse_with_options(Handle, c_int, c_int) -> c_int;
    media_get_parsed_status(Handle) -> c_int;
    media_get_duration(Handle) -> i64;
    media_get_meta(Handle, c_int) -> *mut c_char;
    media_new_location(Handle, *const c_char) -> Handle;
    media_release(Handle);
    media_player_new(Handle) -> Handle;
    media_player_release(Handle);
    media_player_set_media(Handle, Handle);
    media_player_play(Handle) -> c_int;
    media_player_set_pause(Handle, c_int);
    media_player_stop(Handle);
    media_player_get_state(Handle) -> c_int;
    media_player_get_time(Handle) -> i64;
    media_player_get_length(Handle) -> i64;
    media_player_set_time(Handle, i64);
    media_player_is_seekable(Handle) -> c_int;
    media_player_set_rate(Handle, c_float) -> c_int;
    media_player_add_slave(Handle, c_int, *const c_char, bool) -> c_int;
    video_set_callbacks(Handle, Option<video::Lock>, Option<video::Unlock>, Option<video::Display>, Handle);
    video_set_format_callbacks(Handle, Option<video::Setup>, Option<video::Cleanup>);
    video_get_spu_description(Handle) -> *mut TrackDescription;
    video_set_spu_delay(Handle, i64) -> c_int;
    video_get_spu(Handle) -> c_int;
    video_set_spu(Handle, c_int) -> c_int;
    track_description_list_release(*mut TrackDescription);
    audio_get_track_description(Handle) -> *mut TrackDescription;
    audio_get_track(Handle) -> c_int;
    audio_set_track(Handle, c_int) -> c_int;
    audio_set_delay(Handle, i64) -> c_int;
    audio_set_volume(Handle, c_int) -> c_int;
    audio_set_mute(Handle, c_int);
    video_get_size(Handle, c_uint, *mut c_uint, *mut c_uint) -> c_int;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Ended,
    Error,
}

pub struct Snapshot {
    pub status: Status,
    pub time: i64,
    pub duration: i64,
    pub seekable: bool,
}

pub struct Player {
    instance: NonNull<c_void>,
    player: NonNull<c_void>,
    frames: Box<Frames>,
    api: Api,
}

impl Player {
    pub fn new(language: &str, size: u32) -> Result<Self, String> {
        let mut errors = Vec::new();
        for path in candidates() {
            match unsafe { Self::load(&path, language, size) } {
                Ok(player) => return Ok(player),
                Err(error) => errors.push(format!("{}: {error}", path.display())),
            }
        }
        let message = tr!(
            "Install VideoLAN VLC 3 to enable playback.",
            "Instala VLC 3 de VideoLAN para activar la reproducción.",
            "Installez VLC 3 de VideoLAN pour activer la lecture."
        );
        eprintln!("PAN · LibVLC: {}", errors.join("\n"));
        Err(message.into())
    }

    unsafe fn load(path: &Path, language: &str, size: u32) -> Result<Self, String> {
        let api = Api::load(path).map_err(|error| error.to_string())?;
        let version = CStr::from_ptr((api.get_version)()).to_string_lossy();
        if !version.starts_with("3.") {
            return Err(tf!(
                "LibVLC {version}; PAN requires ABI version 3.",
                "LibVLC {version}; PAN usa la ABI 3.",
                "LibVLC {version} ; PAN nécessite l’ABI 3."
            ));
        }

        #[cfg(any(target_os = "macos", windows))]
        {
            static PLUGINS: std::sync::Once = std::sync::Once::new();
            PLUGINS.call_once(|| {
                let directory = path.parent().and_then(|p| {
                    if cfg!(target_os = "macos") {
                        p.parent()
                    } else {
                        Some(p)
                    }
                });
                if let Some(plugins) = directory.map(|p| p.join("plugins")).filter(|p| p.is_dir()) {
                    std::env::set_var("VLC_PLUGIN_PATH", plugins);
                }
            });
        }
        let language =
            CString::new(format!("--sub-language={language}")).map_err(|e| e.to_string())?;
        let size =
            CString::new(format!("--freetype-rel-fontsize={size}")).map_err(|e| e.to_string())?;
        // Ignore the user's VLC settings. PAN owns these preferences.
        let flags = [
            c"--ignore-config",
            c"--no-video-title-show",
            c"--no-osd",
            c"--no-interact",
            language.as_c_str(),
            size.as_c_str(),
        ];
        let pointers = flags.map(CStr::as_ptr);
        let instance =
            NonNull::new((api.new)(pointers.len() as c_int, pointers.as_ptr())).ok_or(tr!(
                "Could not start LibVLC.",
                "No se inició LibVLC.",
                "Impossible de démarrer LibVLC."
            ))?;
        let Some(player) = NonNull::new((api.media_player_new)(instance.as_ptr())) else {
            (api.release)(instance.as_ptr());
            return Err(tr!(
                "Could not start the player.",
                "No se inició el reproductor.",
                "Impossible de démarrer le lecteur."
            )
            .into());
        };
        let mut frames: Box<Frames> = Box::default();
        (api.video_set_callbacks)(
            player.as_ptr(),
            Some(video::lock),
            None,
            Some(video::display),
            (&mut *frames as *mut Frames).cast(),
        );
        (api.video_set_format_callbacks)(player.as_ptr(), Some(video::setup), Some(video::cleanup));
        Ok(Self {
            instance,
            player,
            frames,
            api,
        })
    }

    pub fn open(&self, url: &url::Url) -> Result<(), String> {
        let location = CString::new(url.as_str()).map_err(|_| {
            tr!(
                "Invalid address.",
                "Dirección inválida.",
                "Adresse non valide."
            )
        })?;
        unsafe {
            (self.api.media_player_stop)(self.player.as_ptr());
            self.frames.lock().unwrap().take();
            let media = (self.api.media_new_location)(self.instance.as_ptr(), location.as_ptr());
            if media.is_null() {
                return Err(tr!(
                    "Could not open media.",
                    "No se pudo abrir el medio.",
                    "Impossible d’ouvrir le média."
                )
                .into());
            }
            (self.api.media_player_set_media)(self.player.as_ptr(), media);
            (self.api.media_release)(media);
        }
        self.play()
    }

    pub fn play(&self) -> Result<(), String> {
        let result = unsafe { (self.api.media_player_play)(self.player.as_ptr()) };
        if result == 0 {
            Ok(())
        } else {
            Err(tr!(
                "Could not start playback.",
                "No se pudo iniciar la reproducción.",
                "Impossible de démarrer la lecture."
            )
            .into())
        }
    }

    pub fn pause(&self, pause: bool) {
        unsafe { (self.api.media_player_set_pause)(self.player.as_ptr(), pause.into()) };
    }

    pub fn stop(&self) {
        unsafe { (self.api.media_player_stop)(self.player.as_ptr()) };
        self.frames.lock().unwrap().take();
    }

    pub fn state(&self) -> Snapshot {
        let p = self.player.as_ptr();
        unsafe {
            let status = match (self.api.media_player_get_state)(p) {
                1 | 2 => Status::Loading,
                3 => Status::Playing,
                4 => Status::Paused,
                5 => Status::Stopped,
                6 => Status::Ended,
                7 => Status::Error,
                _ => Status::Idle,
            };
            Snapshot {
                status,
                time: (self.api.media_player_get_time)(p).max(0),
                duration: (self.api.media_player_get_length)(p).max(0),
                seekable: (self.api.media_player_is_seekable)(p) != 0,
            }
        }
    }

    pub fn seek(&self, time: i64) {
        unsafe { (self.api.media_player_set_time)(self.player.as_ptr(), time) };
    }

    pub fn audio(&self, volume: i32, muted: bool) {
        unsafe {
            (self.api.audio_set_volume)(self.player.as_ptr(), volume);
            (self.api.audio_set_mute)(self.player.as_ptr(), muted.into());
        }
    }

    pub fn rate(&self, rate: f32) -> bool {
        unsafe { (self.api.media_player_set_rate)(self.player.as_ptr(), rate) == 0 }
    }

    pub fn subtitle(&self, url: &url::Url) -> bool {
        let Ok(location) = CString::new(url.as_str()) else {
            return false;
        };
        unsafe {
            (self.api.media_player_add_slave)(self.player.as_ptr(), 0, location.as_ptr(), true) == 0
        }
    }

    pub fn tracks(&self, subtitles: bool) -> (i32, Vec<(i32, String)>) {
        let player = self.player.as_ptr();
        unsafe {
            let (selected, head) = if subtitles {
                (
                    (self.api.video_get_spu)(player),
                    (self.api.video_get_spu_description)(player),
                )
            } else {
                (
                    (self.api.audio_get_track)(player),
                    (self.api.audio_get_track_description)(player),
                )
            };
            let mut tracks = Vec::new();
            let mut cursor = head;
            while let Some(track) = cursor.as_ref() {
                let name = if track.name.is_null() {
                    tf!("Track {}", "Pista {}", "Piste {}"; track.id)
                } else {
                    CStr::from_ptr(track.name).to_string_lossy().into_owned()
                };
                tracks.push((track.id, name));
                cursor = track.next;
            }
            if !head.is_null() {
                (self.api.track_description_list_release)(head);
            }
            (selected, tracks)
        }
    }

    pub fn delay(&self, subtitles: bool, milliseconds: i64) -> bool {
        let set = if subtitles {
            self.api.video_set_spu_delay
        } else {
            self.api.audio_set_delay
        };
        unsafe { set(self.player.as_ptr(), milliseconds * 1_000) == 0 }
    }

    pub fn inspect(&self, url: &url::Url) -> Option<crate::catalog::Entry> {
        use std::time::{Duration, Instant};
        let source = CString::new(url.as_str()).ok()?;
        unsafe {
            let media = NonNull::new((self.api.media_new_location)(
                self.instance.as_ptr(),
                source.as_ptr(),
            ))?;
            let handle = media.as_ptr();
            let result = (|| {
                if (self.api.media_parse_with_options)(handle, 0, 2_000) != 0 {
                    return None;
                }
                let deadline = Instant::now() + Duration::from_millis(2_200);
                loop {
                    match (self.api.media_get_parsed_status)(handle) {
                        4 => break,
                        1..=3 => return None,
                        _ if Instant::now() >= deadline => return None,
                        _ => std::thread::sleep(Duration::from_millis(20)),
                    }
                }
                let meta = |kind| {
                    let ptr = (self.api.media_get_meta)(handle, kind);
                    if ptr.is_null() {
                        return String::new();
                    }
                    let text = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                    (self.api.free)(ptr.cast());
                    text
                };
                Some(crate::catalog::Entry {
                    title: meta(0),
                    creator: meta(1),
                    description: meta(6),
                    duration: (self.api.media_get_duration)(handle).max(0),
                    ..Default::default()
                })
            })();
            (self.api.media_release)(handle);
            result
        }
    }

    pub fn select_track(&self, subtitles: bool, id: i32) {
        unsafe {
            let set = if subtitles {
                self.api.video_set_spu
            } else {
                self.api.audio_set_track
            };
            set(self.player.as_ptr(), id);
        }
    }

    pub fn size(&self) -> Option<[u32; 2]> {
        let (mut w, mut h) = (0, 0);
        let result = unsafe { (self.api.video_get_size)(self.player.as_ptr(), 0, &mut w, &mut h) };
        (result == 0 && w > 0 && h > 0).then_some([w, h])
    }

    pub fn frame(&self) -> Option<ColorImage> {
        self.frames.lock().unwrap().take()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        unsafe {
            (self.api.media_player_stop)(self.player.as_ptr());
            (self.api.media_player_release)(self.player.as_ptr());
            (self.api.release)(self.instance.as_ptr());
        }
    }
}

fn candidates() -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::env::var_os("PAN_LIBVLC")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    #[cfg(target_os = "macos")]
    {
        if let Some(parent) = std::env::current_exe()
            .ok()
            .as_deref()
            .and_then(Path::parent)
        {
            paths.push(parent.join("../Frameworks/vlc/lib/libvlc.dylib"));
        }
        paths.push("/Applications/VLC.app/Contents/MacOS/lib/libvlc.dylib".into());
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(
                PathBuf::from(home).join("Applications/VLC.app/Contents/MacOS/lib/libvlc.dylib"),
            );
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(parent) = std::env::current_exe()
            .ok()
            .as_deref()
            .and_then(Path::parent)
        {
            paths.push(parent.join("vlc/libvlc.dll"));
        }
        for var in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(var) {
                paths.push(PathBuf::from(root).join("VideoLAN/VLC/libvlc.dll"));
            }
        }
    }
    #[cfg(target_os = "linux")]
    paths.push("libvlc.so.5".into());
    paths
}

#[cfg(test)]
mod tests {
    use super::{Player, Status};
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    #[test]
    #[ignore = "requires VLC 3 and its video plugins"]
    fn decodes_video_pauses_and_seeks() {
        fn wait_for(message: &str, ready: impl Fn() -> bool) {
            let deadline = Instant::now() + Duration::from_secs(18);
            while !ready() {
                assert!(Instant::now() < deadline, "{message}");
                std::thread::sleep(Duration::from_millis(30));
            }
        }

        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bars.webm");
        let url = url::Url::from_file_path(path).unwrap();
        let player = Player::new("", 16).expect("load VLC 3");
        let media = player.inspect(&url).expect("read local metadata");
        assert!((5_500..=6_500).contains(&media.duration));
        player.open(&url).expect("play a local file");
        wait_for("LibVLC did not produce a 96×64 video frame", || {
            player.frame().is_some_and(|frame| frame.size == [96, 64])
        });
        wait_for("media is not seekable", || player.state().seekable);
        player.pause(true);
        wait_for("media did not pause", || {
            player.state().status == Status::Paused
        });
        player.seek(3_500);
        player.pause(false);
        wait_for("media did not resume at the new position", || {
            let state = player.state();
            state.status == Status::Playing && state.time >= 3_000
        });
    }
}
