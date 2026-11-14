use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum Language {
    #[default]
    En,
    Es,
    Fr,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::En, Self::Es, Self::Fr];

    pub fn native_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Es => "Español",
            Self::Fr => "Français",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Es => "es",
            Self::Fr => "fr",
        }
    }
}

static LANGUAGE: AtomicU8 = AtomicU8::new(Language::En as u8);

pub fn current() -> Language {
    match LANGUAGE.load(Ordering::Relaxed) {
        1 => Language::Es,
        2 => Language::Fr,
        _ => Language::En,
    }
}

pub fn set(language: Language) {
    LANGUAGE.store(language as u8, Ordering::Relaxed);
}

macro_rules! tr {
    ($en:literal, $es:literal, $fr:literal) => {{
        match $crate::locale::current() {
            $crate::locale::Language::En => $en,
            $crate::locale::Language::Es => $es,
            $crate::locale::Language::Fr => $fr,
        }
    }};
}

macro_rules! tf {
    ($en:literal, $es:literal, $fr:literal $(; $($args:tt)*)?) => {{
        match $crate::locale::current() {
            $crate::locale::Language::En => format!($en $(, $($args)*)?),
            $crate::locale::Language::Es => format!($es $(, $($args)*)?),
            $crate::locale::Language::Fr => format!($fr $(, $($args)*)?),
        }
    }};
}

#[cfg(target_os = "macos")]
#[no_mangle]
extern "C" fn pan_ui_language() -> std::ffi::c_int {
    current() as std::ffi::c_int
}
