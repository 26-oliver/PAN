use crate::media::Media;
use std::{
    env, fs,
    io::{self, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_IMPORT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Audio,
    Video,
    Other,
}

impl Kind {
    pub fn of(path: &Path) -> Self {
        let tag = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        match tag.as_str() {
            "mp3" | "m4a" | "wav" | "flac" | "ogg" | "oga" | "opus" | "aac" | "aif" | "aiff"
            | "alac" | "wma" | "ac3" | "ape" | "mka" | "mid" | "aifc" => Self::Audio,
            "mp4" | "m4v" | "mov" | "mkv" | "webm" | "avi" | "wmv" | "mpg" | "mpeg" | "ts"
            | "mts" | "m2ts" | "flv" | "vob" | "ogv" | "3gp" | "asf" | "rmvb" => Self::Video,
            _ => Self::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Audio => "Audio",
            Self::Video => tr!("Video", "Vídeo", "Vidéo"),
            Self::Other => tr!("File", "Archivo", "Fichier"),
        }
    }
}

pub struct Item {
    pub id: String,
    pub added: u64,
    pub path: PathBuf,
    pub media: Media,
    pub kind: Kind,
    pub size: u64,
}

pub struct Library {
    pub root: Option<PathBuf>,
    pub items: Vec<Item>,
    pub message: Option<(String, bool)>,
    task: Option<Receiver<ImportReport>>,
    move_task: Option<Receiver<io::Result<PathBuf>>>,
}

#[derive(Default)]
struct ImportReport {
    copied: usize,
    skipped: usize,
    errors: Vec<String>,
}

impl Library {
    pub fn new(location: Option<PathBuf>) -> Self {
        let mut library = Self {
            root: None,
            items: Vec::new(),
            message: None,
            task: None,
            move_task: None,
        };
        let default_location = location.is_none();
        match location.or_else(storage_root) {
            Some(root) => {
                if default_location {
                    if let Err(error) = fs::create_dir_all(&root) {
                        library.message = Some((
                            tf!(
                                "Could not create the library: {error}",
                                "No se pudo crear la biblioteca: {error}",
                                "Impossible de créer la bibliothèque : {error}"
                            ),
                            true,
                        ));
                    }
                }
                library.root = Some(root);
                library.reload();
            }
            None => {
                library.message = Some((
                    tr!(
                        "PAN could not find its data folder.",
                        "No se encontró la carpeta de datos de PAN.",
                        "PAN ne trouve pas son dossier de données."
                    )
                    .into(),
                    true,
                ))
            }
        }
        library
    }

    pub fn reload(&mut self) {
        let Some(root) = &self.root else { return };
        match read_items(root) {
            Ok(items) => self.items = items,
            Err(err) => {
                self.message = Some((
                    tf!(
                        "Could not read the library: {err}",
                        "No se pudo leer la biblioteca: {err}",
                        "Impossible de lire la bibliothèque : {err}"
                    ),
                    true,
                ))
            }
        }
    }

    pub fn busy(&self) -> bool {
        self.task.is_some() || self.move_task.is_some()
    }

    pub fn moving(&self) -> bool {
        self.move_task.is_some()
    }

    pub fn id_for(&self, path: &Path) -> Option<String> {
        let relative = path.strip_prefix(self.root.as_ref()?).ok()?;
        let folder = relative.parent()?;
        (folder.components().count() == 1)
            .then(|| folder.to_str())
            .flatten()
            .filter(|id| id.starts_with("media-"))
            .map(str::to_owned)
    }

    pub fn entry_folder(&self, id: &str) -> Option<PathBuf> {
        if id.starts_with("media-") && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
            self.root.as_ref().map(|root| root.join(id))
        } else {
            None
        }
    }

    pub fn request_move(&mut self, directory: PathBuf) {
        if self.busy() {
            return;
        }
        let Some(source) = self.root.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.move_task = Some(rx);
        self.message = None;
        std::thread::spawn(move || {
            let destination = directory.join("Biblioteca PAN");
            let result = copy_library(&source, &destination).map(|()| destination);
            let _ = tx.send(result);
        });
    }

    pub fn poll_move(&mut self) -> Option<PathBuf> {
        match self.move_task.as_ref()?.try_recv() {
            Ok(Ok(path)) => {
                self.move_task = None;
                Some(path)
            }
            Ok(Err(err)) => {
                self.move_task = None;
                self.message = Some((
                    tf!(
                        "Could not change the library folder: {err}",
                        "No se pudo cambiar la carpeta: {err}",
                        "Impossible de changer le dossier de la bibliothèque : {err}"
                    ),
                    true,
                ));
                None
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.move_task = None;
                self.message = Some((
                    tr!(
                        "The library copy was interrupted.",
                        "Se interrumpió la copia de la biblioteca.",
                        "La copie de la bibliothèque a été interrompue."
                    )
                    .into(),
                    true,
                ));
                None
            }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }

    pub fn start_import(&mut self, files: Vec<PathBuf>) {
        if files.is_empty() {
            return;
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        if self.busy() {
            self.message = Some((
                tr!(
                    "Wait for the current import to finish.",
                    "Espera a que termine la importación actual.",
                    "Attendez la fin de l’importation en cours."
                )
                .into(),
                true,
            ));
            return;
        }
        match std::thread::Builder::new()
            .name("pan-import".into())
            .spawn({
                let (tx, rx) = mpsc::channel();
                let already_saved = self
                    .items
                    .iter()
                    .map(|item| item.path.clone())
                    .collect::<Vec<_>>();
                self.task = Some(rx);
                move || {
                    let _ = tx.send(import_files(&root, &already_saved, files));
                }
            }) {
            Ok(_) => self.message = None,
            Err(err) => {
                self.task = None;
                self.message = Some((
                    tf!(
                        "Could not start the import: {err}",
                        "No se pudo iniciar la importación: {err}",
                        "Impossible de démarrer l’importation : {err}"
                    ),
                    true,
                ));
            }
        }
    }

    pub fn poll(&mut self) {
        let Some(task) = &self.task else { return };
        match task.try_recv() {
            Ok(report) => {
                self.task = None;
                let count = report.copied;
                let mut status = if count == 1 {
                    tr!("1 file saved", "1 archivo guardado", "1 fichier enregistré").to_owned()
                } else {
                    tf!(
                        "{count} files saved",
                        "{count} archivos guardados",
                        "{count} fichiers enregistrés"
                    )
                };
                if report.skipped > 0 {
                    status.push_str(&tf!(" · {} already in PAN", " · {} ya en PAN", " · {} déjà dans PAN"; report.skipped));
                }
                let failed = !report.errors.is_empty();
                if let Some(first) = report.errors.first() {
                    status.push_str(&tf!(" · {} could not be copied. {first}", " · {} sin copiar. {first}", " · {} n’ont pas pu être copiés. {first}"; report.errors.len()));
                }
                self.message = Some((status, failed));
                self.reload();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.task = None;
                self.message = Some((
                    tr!(
                        "The import was interrupted.",
                        "La importación se interrumpió.",
                        "L’importation a été interrompue."
                    )
                    .into(),
                    true,
                ));
                self.reload();
            }
        }
    }

    pub fn reveal(&mut self) {
        let Some(root) = &self.root else { return };
        let result = fs::create_dir_all(root).and_then(|()| reveal(root));
        if let Err(err) = result {
            self.message = Some((
                tf!(
                    "Could not open the folder: {err}",
                    "No se pudo abrir la carpeta: {err}",
                    "Impossible d’ouvrir le dossier : {err}"
                ),
                true,
            ));
        }
    }
}

pub fn storage_root() -> Option<PathBuf> {
    if let Some(absolute) = env::var_os("PAN_LIBRARY_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return Some(absolute);
    }
    #[cfg(not(windows))]
    let user_home = || {
        env::var_os("HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
    };
    #[cfg(target_os = "macos")]
    let data = user_home().map(|p| p.join("Library/Application Support"));
    #[cfg(windows)]
    let data = env::var_os("LOCALAPPDATA")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    #[cfg(not(any(windows, target_os = "macos")))]
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| user_home().map(|p| p.join(".local/share")));
    data.map(|path| path.join("PAN").join("Biblioteca"))
}

fn reveal(root: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let command = "/usr/bin/open";
    #[cfg(windows)]
    let command = "explorer.exe";
    #[cfg(not(any(windows, target_os = "macos")))]
    let command = "xdg-open";
    std::process::Command::new(command)
        .arg(root)
        .spawn()
        .map(|_| ())
}

fn read_items(root: &Path) -> io::Result<Vec<Item>> {
    let mut folders = match fs::read_dir(root) {
        Ok(entries) => entries.collect::<io::Result<Vec<_>>>()?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    folders.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    let mut items = Vec::new();
    for folder in folders {
        if !folder.file_type()?.is_dir()
            || !folder.file_name().to_string_lossy().starts_with("media-")
        {
            continue;
        }
        for entry in fs::read_dir(folder.path())? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            if let Ok(media) = Media::from_path(&path) {
                items.push(Item {
                    id: folder.file_name().to_string_lossy().into_owned(),
                    added: folder
                        .file_name()
                        .to_string_lossy()
                        .split('-')
                        .nth(1)
                        .and_then(|n| n.parse::<u128>().ok())
                        .map(|n| (n / 1_000_000_000) as u64)
                        .unwrap_or_else(|| {
                            crate::catalog::unix_seconds(metadata.modified().unwrap_or(UNIX_EPOCH))
                        }),
                    kind: Kind::of(&path),
                    size: metadata.len(),
                    path,
                    media,
                });
            }
        }
    }
    Ok(items)
}

fn import_files(root: &Path, already_saved: &[PathBuf], files: Vec<PathBuf>) -> ImportReport {
    let mut report = ImportReport::default();
    if let Err(err) = fs::create_dir_all(root) {
        report.errors.push(tf!(
            "Could not create the folder: {err}",
            "No se pudo crear la carpeta: {err}",
            "Impossible de créer le dossier : {err}"
        ));
        return report;
    }
    let lock = match fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".pan-import.lock"))
        .and_then(|lock| {
            lock.lock()?;
            Ok(lock)
        }) {
        Ok(lock) => lock,
        Err(error) => {
            report.errors.push(error.to_string());
            return report;
        }
    };
    let library_path = root.canonicalize().unwrap_or_else(|_| root.to_owned());
    let mut seen = std::collections::HashSet::new();
    let mut saved = read_items(root).map_or_else(
        |_| already_saved.to_vec(),
        |items| items.into_iter().map(|item| item.path).collect(),
    );
    for source in files {
        let key = source.canonicalize().unwrap_or_else(|_| source.clone());
        if key.starts_with(&library_path) || !seen.insert(key) {
            report.skipped += 1;
            continue;
        }
        let result = save_copy(root, &source, &saved);
        match result {
            Ok(Some(path)) => {
                report.copied += 1;
                saved.push(path);
            }
            Ok(None) => report.skipped += 1,
            Err(err) => {
                let name = source
                    .file_name()
                    .unwrap_or(source.as_os_str())
                    .to_string_lossy();
                report.errors.push(format!("{name}: {err}"));
            }
        }
    }
    drop(lock);
    report
}

fn save_copy(root: &Path, file: &Path, already_saved: &[PathBuf]) -> io::Result<Option<PathBuf>> {
    let name = file.file_name().filter(|_| file.is_file()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            tr!(
                "Select a valid file",
                "Selecciona un archivo válido",
                "Sélectionnez un fichier valide"
            ),
        )
    })?;
    use std::io::{Seek, SeekFrom};
    let mut source = fs::File::open(file)?;
    let length = source.metadata()?.len();
    for existing in already_saved {
        if let Ok(saved) = fs::File::open(existing) {
            source.seek(SeekFrom::Start(0))?;
            if saved.metadata()?.len() == length
                && files_equal(BufReader::new(&source), BufReader::new(saved))?
            {
                return Ok(None);
            }
        }
    }
    source.seek(SeekFrom::Start(0))?;
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_IMPORT.fetch_add(1, Ordering::Relaxed);
    let id = format!("{created:020}-{:010}-{sequence:06}", std::process::id());
    let pending = root.join(format!(".import-{id}"));
    let saved = root.join(format!("media-{id}"));
    fs::create_dir(&pending)?;
    let result = (|| {
        let mut dest = BufWriter::new(fs::File::create(pending.join(name))?);
        let copied = io::copy(&mut source, &mut dest)?;
        if copied != length {
            return Err(io::Error::other(tr!(
                "The file size changed while copying",
                "El tamaño del archivo cambió durante la copia",
                "La taille du fichier a changé pendant la copie"
            )));
        }
        dest.flush()?;
        dest.get_ref().sync_all()?;
        drop(dest);
        fs::rename(&pending, &saved)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(pending);
    }
    result.map(|()| Some(saved.join(name)))
}

fn copy_library(source: &Path, destination: &Path) -> io::Result<()> {
    let parent = destination.parent().unwrap();
    let parent = parent.canonicalize()?;
    let destination = parent.join(destination.file_name().unwrap());
    if source
        .canonicalize()
        .is_ok_and(|root| destination.starts_with(root))
    {
        return Err(io::Error::other(tr!(
            "Choose a location outside the current library",
            "Elige una ubicación fuera de la biblioteca actual",
            "Choisissez un emplacement en dehors de la bibliothèque actuelle"
        )));
    }
    if destination.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            tr!(
                "The destination already contains a Biblioteca PAN folder",
                "Ya existe una carpeta Biblioteca PAN en el destino",
                "La destination contient déjà un dossier Biblioteca PAN"
            ),
        ));
    }
    let staging = tempfile::tempdir_in(&parent)?;
    match fs::read_dir(source) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry?;
                if entry.file_type()?.is_dir()
                    && entry.file_name().to_string_lossy().starts_with("media-")
                {
                    copy_directory(&entry.path(), &staging.path().join(entry.file_name()))?;
                }
            }
        }
        Err(error) => return Err(error),
    }
    if destination.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            tr!(
                "The destination was created during the copy",
                "El destino se creó durante la copia",
                "La destination a été créée pendant la copie"
            ),
        ));
    }
    fs::rename(staging.path(), destination)
}

fn copy_directory(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let output = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_directory(&entry.path(), &output)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &output)?;
        }
    }
    Ok(())
}

fn files_equal(mut left: impl io::BufRead, mut right: impl io::BufRead) -> io::Result<bool> {
    loop {
        let a = left.fill_buf()?;
        let b = right.fill_buf()?;
        let n = a.len().min(b.len());
        if n == 0 {
            return Ok(a.is_empty() && b.is_empty());
        }
        if a[..n] != b[..n] {
            return Ok(false);
        }
        left.consume(n);
        right.consume(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_media_survives_source_removal_and_reopening_library() -> io::Result<()> {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test = env::temp_dir().join(format!("pan-library-test-{}-{id}", std::process::id()));
        let downloads = test.join("Descargas");
        let library = test.join("Biblioteca");
        fs::create_dir_all(&downloads)?;
        let original = downloads.join("Vídeo con espacios.mkv");
        let other_dir = test.join(tr!("Other", "Otros", "Autres"));
        fs::create_dir_all(&other_dir)?;
        let same_name = other_dir.join(original.file_name().unwrap());
        fs::write(&original, b"media original")?;
        fs::write(&same_name, b"otro contenido")?;

        let result = import_files(&library, &[], vec![original.clone(), same_name]);
        assert_eq!(result.copied, 2);
        let items = read_items(&library)?;
        let paths = items
            .iter()
            .map(|item| item.path.clone())
            .collect::<Vec<_>>();
        assert_eq!(import_files(&library, &paths, vec![original]).skipped, 1);
        fs::remove_dir_all(downloads)?;
        let items = read_items(&library)?;
        assert_eq!(items.len(), 2);
        assert!(items
            .iter()
            .any(|item| fs::read(&item.path).unwrap() == b"media original"));
        assert!(items.iter().all(|item| item.kind == Kind::Video));
        fs::remove_dir_all(test)?;
        Ok(())
    }
}
