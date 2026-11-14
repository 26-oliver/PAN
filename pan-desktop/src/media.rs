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
                .map_err(|_| "Carpeta no disponible.")?
                .join(path)
        };
        if !path.is_file() {
            return Err(format!("No se pudo abrir {}.", path.display()));
        }
        let url = Url::from_file_path(&path).map_err(|_| "Ruta de archivo inválida.")?;
        Ok(Self {
            title: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            tag: path
                .extension()
                .and_then(OsStr::to_str)
                .unwrap_or("Archivo")
                .to_uppercase(),
            url,
        })
    }

    pub fn from_address(address: &str) -> Result<Self, String> {
        let url = Url::parse(address.trim()).map_err(|_| "Introduce una URL válida.")?;
        if url.scheme() == "file" {
            return Self::from_path(&url.to_file_path().map_err(|_| "Ruta inválida.")?);
        }
        if !matches!(
            url.scheme(),
            "http" | "https" | "rtsp" | "rtmp" | "mms" | "ftp" | "sftp" | "smb" | "udp" | "rtp"
        ) {
            return Err("Ese protocolo no está disponible en PAN.".into());
        }
        let title = url.host_str().unwrap_or("Emisión").to_owned();
        let tag = url.scheme().to_uppercase();
        Ok(Self { title, tag, url })
    }
}
