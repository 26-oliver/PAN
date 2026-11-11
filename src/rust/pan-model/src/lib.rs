//! Application state for PAN's Rust desktop interface.
//! Playback requests are transient. Library changes require explicit calls.

use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaLocation {
    LocalFile(PathBuf),
    DirectUrl(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackItem {
    pub location: MediaLocation,
    pub title: String,
}

impl PlaybackItem {
    pub fn local(path: PathBuf) -> Self {
        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        Self {
            location: MediaLocation::LocalFile(path),
            title,
        }
    }

    pub fn url(url: String) -> Self {
        Self {
            title: url.clone(),
            location: MediaLocation::DirectUrl(url),
        }
    }
}

#[derive(Default)]
pub struct PlaybackQueue {
    items: Vec<PlaybackItem>,
    current: Option<usize>,
}

impl PlaybackQueue {
    pub fn open_now(&mut self, item: PlaybackItem) {
        self.items.clear();
        self.items.push(item);
        self.current = Some(0);
    }

    pub fn enqueue(&mut self, item: PlaybackItem) {
        self.items.push(item);
        if self.current.is_none() {
            self.current = Some(0);
        }
    }

    pub fn current(&self) -> Option<&PlaybackItem> {
        self.current.and_then(|index| self.items.get(index))
    }

    pub fn items(&self) -> &[PlaybackItem] {
        &self.items
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LibrarySource {
    LocalFolder(PathBuf),
    LocalFile(PathBuf),
    Provider {
        provider: String,
        source_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Playlist {
    pub name: String,
    pub items: Vec<MediaLocation>,
}

#[derive(Default)]
pub struct Library {
    sources: Vec<LibrarySource>,
    playlists: BTreeMap<String, Playlist>,
}

impl Library {
    pub fn sources(&self) -> &[LibrarySource] {
        &self.sources
    }

    pub fn add_source(&mut self, source: LibrarySource) -> bool {
        if self.sources.contains(&source) {
            return false;
        }
        self.sources.push(source);
        true
    }

    pub fn remove_source(&mut self, source: &LibrarySource) -> bool {
        let Some(index) = self.sources.iter().position(|entry| entry == source) else {
            return false;
        };
        self.sources.remove(index);
        true
    }

    pub fn create_playlist(&mut self, name: String) -> bool {
        if name.trim().is_empty() || self.playlists.contains_key(&name) {
            return false;
        }
        self.playlists.insert(
            name.clone(),
            Playlist {
                name,
                items: Vec::new(),
            },
        );
        true
    }

    pub fn playlist(&self, name: &str) -> Option<&Playlist> {
        self.playlists.get(name)
    }

    pub fn add_to_playlist(&mut self, name: &str, location: MediaLocation) -> bool {
        let Some(playlist) = self.playlists.get_mut(name) else {
            return false;
        };
        playlist.items.push(location);
        true
    }
}

#[derive(Default)]
pub struct PanState {
    pub playback: PlaybackQueue,
    pub library: Library,
}

impl PanState {
    pub fn open_file(&mut self, path: PathBuf) {
        self.playback.open_now(PlaybackItem::local(path));
    }

    pub fn open_url(&mut self, url: String) {
        self.playback.open_now(PlaybackItem::url(url));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_a_download_does_not_add_it_to_the_library() {
        let mut pan = PanState::default();
        let download = PathBuf::from("/downloads/movie.mkv");
        pan.open_file(download.clone());

        assert_eq!(
            pan.playback.current().map(|item| &item.location),
            Some(&MediaLocation::LocalFile(download))
        );
        assert!(pan.library.sources().is_empty());
    }

    #[test]
    fn opening_a_url_does_not_add_a_source() {
        let mut pan = PanState::default();
        pan.open_url("https://example.org/video.mp4".to_owned());

        assert!(pan.playback.current().is_some());
        assert!(pan.library.sources().is_empty());
    }

    #[test]
    fn library_source_is_explicit_and_unique() {
        let mut pan = PanState::default();
        let source = LibrarySource::LocalFolder(PathBuf::from("/media/music"));
        assert!(pan.library.add_source(source.clone()));
        assert!(!pan.library.add_source(source.clone()));
        assert_eq!(pan.library.sources(), &[source.clone()]);
        assert!(pan.library.remove_source(&source));
        assert!(pan.library.sources().is_empty());
    }

    #[test]
    fn playlist_is_independent_of_library_sources() {
        let mut pan = PanState::default();
        assert!(pan.library.create_playlist("Favorites".to_owned()));
        assert!(pan.library.add_to_playlist(
            "Favorites",
            MediaLocation::DirectUrl("https://example.org/song.mp3".to_owned())
        ));
        assert_eq!(pan.library.playlist("Favorites").unwrap().items.len(), 1);
        assert!(pan.library.sources().is_empty());
    }
}
