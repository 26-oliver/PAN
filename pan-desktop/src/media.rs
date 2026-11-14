use std::{ffi::OsStr, path::Path};
use url::Url;

#[derive(Clone)]
pub struct Media {
    pub title: String,
    pub tag: String,
    pub url: Url,
}

impl Media {
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir()
                .map_err(|_| {
                    tr!(
                        "Folder unavailable.",
                        "Carpeta no disponible.",
                        "Dossier indisponible."
                    )
                })?
                .join(path)
        };
        if !path.is_file() {
            return Err(
                tf!("Could not open {}.", "No se pudo abrir {}.", "Impossible d’ouvrir {}."; path.display()),
            );
        }
        let url = Url::from_file_path(&path).map_err(|_| {
            tr!(
                "Invalid file path.",
                "Ruta de archivo inválida.",
                "Chemin de fichier non valide."
            )
        })?;
        Ok(Self {
            title: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            tag: path
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or(tr!("File", "Archivo", "Fichier"))
                .to_uppercase(),
            url,
        })
    }

    pub fn from_address(address: &str) -> Result<Self, String> {
        let url = Url::parse(address.trim()).map_err(|_| {
            tr!(
                "Enter a valid URL.",
                "Introduce una URL válida.",
                "Saisissez une URL valide."
            )
        })?;
        if url.scheme() == "file" {
            return Self::from_path(
                &url.to_file_path()
                    .map_err(|_| tr!("Invalid path.", "Ruta inválida.", "Chemin non valide."))?,
            );
        }
        if !matches!(
            url.scheme(),
            "http" | "https" | "rtsp" | "rtmp" | "mms" | "ftp" | "sftp" | "smb" | "udp" | "rtp"
        ) {
            return Err(tr!(
                "PAN does not support that protocol.",
                "Ese protocolo no está disponible en PAN.",
                "PAN ne prend pas en charge ce protocole."
            )
            .into());
        }
        let title = url
            .host_str()
            .unwrap_or(tr!("Stream", "Emisión", "Flux"))
            .to_owned();
        let tag = url.scheme().to_uppercase();
        Ok(Self { title, tag, url })
    }
}
