use crate::library;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Data {
    pub location: Option<PathBuf>,
    pub entries: BTreeMap<String, Entry>,
    pub history: BTreeMap<String, Playback>,
    pub lists: Vec<Playlist>,
    pub settings: Settings,
}

#[derive(Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Entry {
    pub filename: String,
    pub title: String,
    pub description: String,
    pub creator: String,
    pub duration: i64,
    pub scanned: bool,
    pub artwork_source: String,
}

#[derive(Default, Deserialize, Serialize)]
pub struct Playlist {
    pub name: String,
    pub items: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Playback {
    pub time: i64,
    pub last_played: u64,
    pub volume: i32,
    pub muted: bool,
    pub speed: f32,
    pub subtitle: Option<String>,
    pub subtitle_id: Option<i32>,
    pub subtitle_delay: i64,
    pub audio_delay: i64,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            time: 0,
            last_played: 0,
            volume: 80,
            muted: false,
            speed: 1.0,
            subtitle: None,
            subtitle_id: None,
            subtitle_delay: 0,
            audio_delay: 0,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub language: String,
    pub subtitle_size: u32,
    pub keys: Vec<String>,
}

pub const SHORTCUTS: [(&str, &str); 9] = [
    ("Reproducir / pausar", "Space"),
    ("Pantalla completa", "F"),
    ("Silenciar", "M"),
    ("Retroceder 5 s", "ArrowLeft"),
    ("Avanzar 5 s", "ArrowRight"),
    ("Bajar volumen", "ArrowDown"),
    ("Subir volumen", "ArrowUp"),
    ("Anterior", "PageUp"),
    ("Siguiente", "PageDown"),
];

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: String::new(),
            subtitle_size: 16,
            keys: SHORTCUTS.map(|(_, key)| key.to_owned()).to_vec(),
        }
    }
}

pub struct Catalog {
    pub data: Data,
    file: Option<PathBuf>,
    error: Option<String>,
    saved: Value,
}

impl Catalog {
    pub fn load() -> Self {
        let file = library::storage_root().map(|root| {
            if std::env::var_os("PAN_LIBRARY_DIR").is_some() {
                root.join(".pan-catalog.json")
            } else {
                root.with_file_name("catalog.json")
            }
        });
        let (data, error) = match file.as_ref().map(fs::read) {
            Some(Ok(bytes)) => match serde_json::from_slice(&bytes) {
                Ok(data) => (data, None),
                Err(err) => (
                    Data::default(),
                    Some(format!("PAN no pudo leer sus datos: {err}")),
                ),
            },
            Some(Err(err)) if err.kind() != io::ErrorKind::NotFound => (
                Data::default(),
                Some(format!("PAN no pudo abrir sus datos: {err}")),
            ),
            _ => (Data::default(), None),
        };
        let saved = serde_json::to_value(&data).unwrap_or_default();
        Self {
            data,
            file,
            error,
            saved,
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(error) = &self.error {
            return Err(io::Error::other(error.clone()));
        }
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| io::Error::other("Carpeta de datos no disponible"))?;
        let folder = file.parent().unwrap_or(Path::new("."));
        let current = serde_json::to_value(&self.data).map_err(io::Error::other)?;
        if current == self.saved {
            return Ok(());
        }
        fs::create_dir_all(folder)?;
        let lock = fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(file.with_extension("lock"))?;
        lock.lock()?;
        let mut disk = match fs::read(file) {
            Ok(bytes) => serde_json::from_slice::<Value>(&bytes).map_err(io::Error::other)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.saved.clone(),
            Err(error) => return Err(error),
        };
        merge_changes(&self.saved, &current, &mut disk);
        atomic_write(folder, file, |file| {
            serde_json::to_writer(file, &disk).map_err(io::Error::other)
        })?;
        self.saved = current;
        Ok(())
    }
}

// Apply only the fields changed by this window; arrays are edited as a unit.
fn merge_changes(previous: &Value, current: &Value, disk: &mut Value) {
    if previous == current {
        return;
    }
    match (previous, current, &mut *disk) {
        (Value::Object(previous), Value::Object(current), Value::Object(disk)) => {
            for name in previous.keys().filter(|name| !current.contains_key(*name)) {
                disk.remove(name);
            }
            for (name, value) in current {
                match (previous.get(name), disk.get_mut(name)) {
                    (Some(old), Some(saved)) => merge_changes(old, value, saved),
                    (old, _) if old != Some(value) => {
                        disk.insert(name.clone(), value.clone());
                    }
                    _ => {}
                }
            }
        }
        _ => *disk = current.clone(),
    }
}

pub fn atomic_write(
    folder: &Path,
    file: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    let mut temp = tempfile::NamedTempFile::new_in(folder)?;
    write(temp.as_file_mut())?;
    temp.as_file().sync_all()?;
    temp.persist(file).map_err(|err| err.error)?;
    Ok(())
}

pub fn unix_seconds(now: std::time::SystemTime) -> u64 {
    now.duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
