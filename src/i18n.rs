
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

static LANG: AtomicU8 = AtomicU8::new(0);

const EN: u8 = 0;
const ID: u8 = 1;
const FR: u8 = 2;
const MS: u8 = 3;

pub fn set(choice: &str) {
    let lang = match choice {
        "en" => EN,
        "id" => ID,
        "fr" => FR,
        "ms" => MS,
        _ => system_language(),
    };
    LANG.store(lang, Ordering::Relaxed);
}

pub fn is_id() -> bool {
    LANG.load(Ordering::Relaxed) == ID
}

pub fn code() -> &'static str {
    match LANG.load(Ordering::Relaxed) {
        ID => "id",
        FR => "fr",
        MS => "ms",
        _ => "en",
    }
}

pub fn name() -> &'static str {
    match LANG.load(Ordering::Relaxed) {
        ID => "Indonesian",
        FR => "French",
        MS => "Malay",
        _ => "English",
    }
}

#[cfg(windows)]
fn system_language() -> u8 {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
        fn GetUserDefaultLCID() -> u32;
    }
    let pick = |l: u32| match l & 0x3ff {
        0x21 => Some(ID),
        0x0c => Some(FR),
        0x3e => Some(MS),
        _ => None,
    };
    unsafe { pick(GetUserDefaultUILanguage() as u32).or_else(|| pick(GetUserDefaultLCID())).unwrap_or(EN) }
}

#[cfg(not(windows))]
fn system_language() -> u8 {
    let v = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().filter_map(|k| std::env::var(k).ok()).find(|v| !v.is_empty()).unwrap_or_default();
    match v.get(..2) {
        Some("id") => ID,
        Some("fr") => FR,
        Some("ms") => MS,
        _ => EN,
    }
}

type Table = HashMap<&'static str, &'static str>;

fn table_id() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| crate::i18n_id::ID.iter().copied().collect())
}

static PACK_FR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fr.tsv.z"));
static PACK_MS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ms.tsv.z"));

fn table_packed(cell: &'static OnceLock<Table>, packed: &'static [u8]) -> &'static Table {
    cell.get_or_init(|| {
        let bytes = miniz_oxide::inflate::decompress_to_vec(packed).unwrap_or_default();
        let text: &'static str = Box::leak(String::from_utf8_lossy(&bytes).into_owned().into_boxed_str());
        parse_tsv(text)
    })
}

fn parse_tsv(text: &'static str) -> Table {
    fn unescape(s: &'static str) -> &'static str {
        if !s.contains('\\') {
            return s;
        }
        let mut out = String::with_capacity(s.len());
        let mut it = s.chars();
        while let Some(ch) = it.next() {
            if ch == '\\' {
                match it.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(o) => out.push(o),
                    None => {}
                }
            } else {
                out.push(ch);
            }
        }
        Box::leak(out.into_boxed_str())
    }
    text.lines().filter_map(|l| l.split_once('\t')).filter(|(_, t)| !t.is_empty()).map(|(en, t)| (unescape(en), unescape(t))).collect()
}

fn table() -> Option<&'static Table> {
    static FR_T: OnceLock<Table> = OnceLock::new();
    static MS_T: OnceLock<Table> = OnceLock::new();
    match LANG.load(Ordering::Relaxed) {
        ID => Some(table_id()),
        FR => Some(table_packed(&FR_T, PACK_FR)),
        MS => Some(table_packed(&MS_T, PACK_MS)),
        _ => None,
    }
}

pub fn tr(en: &'static str) -> &'static str {
    match table() {
        Some(t) => t.get(en).copied().unwrap_or(en),
        None => en,
    }
}

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
    fn packed_tables_parse_and_unescape() {
        let t = super::parse_tsv("Open folder\tOuvrir le dossier\nTwo\\nlines\tDeux\\nlignes\nEmpty\t\n");
        assert_eq!(t.get("Open folder"), Some(&"Ouvrir le dossier"));
        assert_eq!(t.get("Two\nlines"), Some(&"Deux\nlignes"));
        assert!(!t.contains_key("Empty"));
    }

    #[test]
    fn every_language_pack_translates_its_strings() {
        static FR: std::sync::OnceLock<super::Table> = std::sync::OnceLock::new();
        static MS: std::sync::OnceLock<super::Table> = std::sync::OnceLock::new();
        for (code, t) in [("fr", super::table_packed(&FR, super::PACK_FR)), ("ms", super::table_packed(&MS, super::PACK_MS))] {
            assert!(t.len() > 1000, "{code}: only {} strings", t.len());
            assert!(t.get("Settings").is_some_and(|s| *s != "Settings"), "{code}: Settings is not translated");
        }
    }

    #[test]
    fn fills() {
        let n = "app.py";
        assert_eq!(fill("Saved {0}", &[("0", &n)]), "Saved app.py");
        assert_eq!(fill("{b} lalu {a} {{x}}", &[("a", &1), ("b", &2)]), "2 lalu 1 {x}");
        assert_eq!(fill("no {slot}", &[]), "no {slot}");
    }
}
