use crate::catalog::atomic_write;
use eframe::egui::ColorImage;
use serde::Deserialize;
use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
    time::Duration,
};
use url::Url;

#[derive(Clone, Deserialize)]
pub struct Match {
    pub title: String,
    pub key: String,
    pub description: Option<String>,
    thumbnail: Option<Thumb>,
}

#[derive(Clone, Deserialize)]
struct Thumb {
    url: String,
}

impl Match {
    pub fn source(&self, language: &str) -> String {
        let mut url = Url::parse(&format!("https://{language}.wikipedia.org/")).unwrap();
        url.path_segments_mut()
            .unwrap()
            .push("wiki")
            .push(&self.key);
        url.into()
    }

    pub fn has_image(&self) -> bool {
        self.thumbnail.is_some()
    }

    pub fn download(&self) -> Result<Vec<u8>, String> {
        let raw = &self.thumbnail.as_ref().ok_or("Resultado sin imagen")?.url;
        let address = if raw.starts_with("//") {
            format!("https:{raw}")
        } else {
            raw.clone()
        };
        let url = Url::parse(&address).map_err(|e| e.to_string())?;
        if !matches!(
            url.host_str(),
            Some("upload.wikimedia.org" | "thumb.wikimedia.org")
        ) || url.scheme() != "https"
        {
            return Err("Dirección de imagen no compatible".into());
        }
        fetch(&url, MAX_IMAGE)
    }
}

const MAX_IMAGE: usize = 12 * 1024 * 1024;

fn fetch(url: &Url, limit: usize) -> Result<Vec<u8>, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent("PAN/0.2.0 (desktop media library; https://www.wikimedia.org/)")
        .max_redirects(0)
        .build()
        .new_agent();
    let response = agent.get(url.as_str()).call().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Wikipedia respondió con HTTP {}",
            response.status().as_u16()
        ));
    }
    let mut body = response.into_body();
    body.with_config()
        .limit(limit as u64)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

pub fn search(query: &str, language: &str) -> Result<Vec<Match>, String> {
    #[derive(Deserialize)]
    struct Response {
        pages: Vec<Match>,
    }
    let mut address = Url::parse(&format!(
        "https://{language}.wikipedia.org/w/rest.php/v1/search/page"
    ))
    .map_err(|e| e.to_string())?;
    address
        .query_pairs_mut()
        .append_pair("q", query)
        .append_pair("limit", "8");
    let bytes = fetch(&address, 512 * 1024)?;
    serde_json::from_slice::<Response>(&bytes)
        .map(|response| response.pages)
        .map_err(|e| e.to_string())
}

pub fn local_file(path: &Path) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(MAX_IMAGE as u64 + 1).read_to_end(&mut data))
        .map_err(|e| e.to_string())?;
    if data.len() > MAX_IMAGE {
        return Err("La imagen supera 12 MB".into());
    }
    Ok(data)
}

fn decode(data: &[u8]) -> Result<image::DynamicImage, String> {
    let mut reader = image::ImageReader::new(io::Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(80 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|e| e.to_string())
}

pub fn save(root: &Path, data: &[u8]) -> Result<(), String> {
    let thumbnail = decode(data)?.thumbnail(512, 512).to_rgba8();
    let mut output = io::Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let path = cover_path(root);
    atomic_write(path.parent().unwrap(), &path, |file| {
        file.write_all(output.get_ref())
    })
    .map_err(|e| e.to_string())
}

pub fn load(path: &Path) -> Option<ColorImage> {
    let image = decode(&local_file(path).ok()?).ok()?.into_rgba8();
    Some(ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

pub fn preview(data: &[u8]) -> Result<ColorImage, String> {
    let image = decode(data)?.thumbnail(512, 512).into_rgba8();
    Ok(ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

pub fn cover_path(root: &Path) -> std::path::PathBuf {
    root.join(".pan").join("cover.png")
}
