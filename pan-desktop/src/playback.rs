use crate::{
    catalog::{self, Playback},
    media::Media,
    vlc::{Player, Snapshot, Status},
    Pan,
};
use std::time::{Duration, Instant, SystemTime};

pub struct Active {
    pub key: Option<String>,
    pub settings: Playback,
    pub started: Instant,
    pub last_save: Instant,
    pub seek_pending: Option<i64>,
    pub settings_pending: bool,
    pub subtitle_pending: Option<i32>,
    pub external_pending: Option<String>,
    pub seeks_started: Option<Instant>,
    pub seen_playing: bool,
    pub ended: bool,
}

impl Pan {
    pub fn begin_playback(&mut self, item: &Media, resume: bool) {
        let key = item.url.to_file_path().ok().map(|path| {
            self.library
                .id_for(&path)
                .map_or_else(|| item.url.to_string(), |id| format!("library:{id}"))
        });
        let existing = key
            .as_ref()
            .and_then(|id| self.catalog.data.history.get(id));
        let mut settings = existing.cloned().unwrap_or_else(|| Playback {
            volume: self.volume,
            muted: self.muted,
            speed: self.speed,
            ..Default::default()
        });
        let seek_pending = (resume && settings.time >= 2_000).then_some(settings.time);
        if !resume {
            settings.time = 0;
        }
        self.volume = settings.volume.clamp(0, 100);
        self.muted = settings.muted;
        self.speed = settings.speed.clamp(0.5, 2.0);

        let prefs = &self.catalog.data.settings;
        let config = (prefs.language.clone(), prefs.subtitle_size);
        if self.engine_preferences != config {
            self.player = None;
            match Player::new(&config.0, config.1) {
                Ok(player) => {
                    self.player = Some(player);
                    self.engine_error = None;
                }
                Err(error) => self.engine_error = Some(error),
            }
            self.engine_preferences = config;
        }

        self.active = self.player.as_ref().and_then(|player| {
            self.error = player.open(&item.url).err();
            let now = Instant::now();
            self.error.is_none().then(|| Active {
                key,
                seek_pending,
                subtitle_pending: settings.subtitle_id,
                external_pending: settings.subtitle.clone(),
                settings,
                started: now,
                last_save: now,
                settings_pending: true,
                seen_playing: false,
                ended: false,
                seeks_started: None,
            })
        });
    }

    pub fn progress(&mut self, state: &Snapshot) {
        let (Some(active), Some(player)) = (&mut self.active, &self.player) else {
            return;
        };
        if matches!(state.status, Status::Playing | Status::Paused) {
            if !active.seen_playing {
                active.seen_playing = true;
                active.settings.last_played = catalog::unix_seconds(SystemTime::now());
            }
            if active.settings_pending {
                player.audio(self.volume, self.muted);
                player.rate(self.speed);
                player.delay(false, active.settings.audio_delay.clamp(-60_000, 60_000));
                player.delay(true, active.settings.subtitle_delay.clamp(-60_000, 60_000));
                active.settings_pending = false;
            }
            if let Some(address) = active.external_pending.take() {
                if let Ok(url) = url::Url::parse(&address) {
                    if !player.subtitle(&url) {
                        self.error =
                            Some("El archivo de subtítulos guardado ya no está disponible.".into());
                    } else {
                        active.subtitle_pending = None;
                    }
                }
            }
            if let Some(id) = active.subtitle_pending {
                if player.tracks(true).1.iter().any(|(track, _)| *track == id) {
                    player.select_track(true, id);
                    active.subtitle_pending = None;
                } else if active.started.elapsed() >= Duration::from_secs(5) {
                    active.subtitle_pending = None;
                }
            }
            if let Some(time) = active.seek_pending {
                if active.seeks_started.is_some() && state.time.abs_diff(time) < 2_000 {
                    active.seek_pending = None;
                } else if active.seeks_started.is_none() && state.seekable && state.duration > time
                {
                    player.seek(time);
                    active.seeks_started = Some(Instant::now());
                } else if active
                    .seeks_started
                    .is_some_and(|t| t.elapsed() > Duration::from_secs(2))
                    || active.started.elapsed() > Duration::from_secs(10)
                {
                    active.seek_pending = None;
                }
            }
        }
        if active.last_save.elapsed() > Duration::from_secs(4) {
            self.save_position(state, false);
        }
    }

    pub fn finish_position(&mut self) {
        if let Some(state) = self.player.as_ref().map(Player::state) {
            self.save_position(&state, false);
        }
    }

    pub fn save_position(&mut self, state: &Snapshot, ended: bool) {
        let Some(active) = &mut self.active else {
            return;
        };
        if !active.seen_playing || active.ended {
            return;
        }
        active.last_save = Instant::now();
        active.ended = ended;
        active.settings.volume = self.volume;
        active.settings.muted = self.muted;
        active.settings.speed = self.speed;
        if ended {
            active.settings.time = 0;
        } else if active.seek_pending.is_none()
            && state.seekable
            && matches!(state.status, Status::Playing | Status::Paused)
        {
            active.settings.time = state.time;
        }
        if let Some(key) = &active.key {
            self.catalog
                .data
                .history
                .insert(key.clone(), active.settings.clone());
            self.save_catalog();
        }
    }
}
