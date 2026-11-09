//! Interface language: English (the source text) or Indonesian.
//!
//! `tr("Open Folder")` returns the text in the chosen language; English is the
//! key, so a missing translation shows English instead of a blank. `trf!` does
//! the same for text with values in it: `trf!("Saved {0}"; 0 => name)`. A
//! translation may move a `{name}` anywhere in its sentence.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

static LANG: AtomicU8 = AtomicU8::new(0);

/// "en", "id", or "auto" (Indonesian when Windows is set to Indonesian).
pub fn set(choice: &str) {
    let id = match choice {
        "id" => true,
        "en" => false,
        _ => system_is_indonesian(),
    };
    LANG.store(id as u8, Ordering::Relaxed);
}

pub fn is_id() -> bool {
    LANG.load(Ordering::Relaxed) == 1
}

#[cfg(windows)]
fn system_is_indonesian() -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
        fn GetUserDefaultLCID() -> u32;
    }
    // LANG_INDONESIAN = 0x21 (primary language id: low 10 bits). The display
    // language or the regional format both count: many Indonesian PCs run an
    // English Windows with Indonesian formats.
    unsafe { GetUserDefaultUILanguage() & 0x3ff == 0x21 || GetUserDefaultLCID() & 0x3ff == 0x21 }
}

#[cfg(not(windows))]
fn system_is_indonesian() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().any(|k| std::env::var(k).is_ok_and(|v| v.starts_with("id")))
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| crate::i18n_id::ID.iter().copied().collect())
}

/// The text in the interface language.
pub fn tr(en: &'static str) -> &'static str {
    if !is_id() {
        return en;
    }
    table().get(en).copied().unwrap_or(en)
}

/// Put values into `{name}` slots (`{{` and `}}` are literal braces).
pub fn fill(tpl: &str, vals: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut out = String::with_capacity(tpl.len() + 16);
    let mut rest = tpl;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let r = &rest[i..];
        if r.starts_with("{{") || r.starts_with("}}") {
            out.push_str(&r[..1]);
            rest = &r[2..];
            continue;
        }
        if r.starts_with('{') {
            if let Some(end) = r.find('}') {
                let name = &r[1..end];
                match vals.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(&v.to_string()),
                    None => out.push_str(&r[..=end]),
                }
                rest = &r[end + 1..];
                continue;
            }
        }
        out.push_str(&r[..1]);
        rest = &r[1..];
    }
    out.push_str(rest);
    out
}

/// `trf!("Saved {0}"; 0 => name)`: `tr` plus values.
#[macro_export]
macro_rules! trf {
    ($lit:literal $(; $($k:tt => $v:expr),* $(,)?)?) => {
        $crate::i18n::fill($crate::i18n::tr($lit), &[$($((stringify!($k), &$v as &dyn std::fmt::Display)),*)?])
    };
}

#[cfg(test)]
mod tests {
    use super::fill;

    #[test]
    fn fills() {
        let n = "app.py";
        assert_eq!(fill("Saved {0}", &[("0", &n)]), "Saved app.py");
        assert_eq!(fill("{b} lalu {a} {{x}}", &[("a", &1), ("b", &2)]), "2 lalu 1 {x}");
        assert_eq!(fill("no {slot}", &[]), "no {slot}");
    }
}
