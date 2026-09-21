//! Turning messy folder / file names into searchable game titles.

use regex::Regex;
use std::sync::LazyLock;

/// Release groups, repackers and distribution tags that are never part of a title.
const NOISE_WORDS: &[&str] = &[
    "fitgirl", "dodi", "repack", "repacks", "elamigos", "kaos", "xatab", "codex", "cpy", "plaza",
    "skidrow", "reloaded", "empress", "tenoke", "rune", "flt", "razor1911", "goldberg", "ali213",
    "gog", "steamrip", "portable", "multi", "truefrench", "french", "vf", "vff", "x64", "win64",
    "minversion", "miniversion", "crack", "cracked", "update", "incl", "dlc", "dlcs",
];

static BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]*\]|\([^)]*\)|\{[^}]*\}").unwrap());
static GROUP_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-[A-Za-z0-9]+$").unwrap());
static VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(v\d+[\w.]*|\d+(\.\d+){1,}[a-z]?|build\d*|multi\d+)$").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
/// A version glued to a scene name with dots or underscores: "Game_v1.2.3".
static INLINE_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[._\s-]v?\d+(\.\d+)+[a-z]?\b").unwrap());

/// "Grand.Theft.Auto.V-CODEX" → "Grand Theft Auto V"; "Limbo [FitGirl Repack]" → "Limbo".
pub fn clean_title(raw: &str) -> String {
    let mut s = BRACKETS.replace_all(raw, " ").into_owned();

    // Scene-style names use dots/underscores instead of spaces and end with "-GROUP".
    if !s.trim().contains(' ') {
        if let Some(m) = GROUP_SUFFIX.find(&s) {
            let group = &s[m.start() + 1..];
            if is_noise(group) || group.chars().all(|c| !c.is_lowercase()) {
                s.truncate(m.start());
            }
        }
        s = INLINE_VERSION.replace_all(&s, " ").replace(['.', '_'], " ");
    } else {
        s = s.replace('_', " ");
    }

    let words: Vec<&str> = SPACES
        .split(s.trim())
        .filter(|w| !is_noise(w) && !VERSION.is_match(w))
        .collect();
    let title = words.join(" ");
    let title = title.trim_matches(|c: char| c == '-' || c == '+' || c.is_whitespace());
    if title.is_empty() {
        raw.trim().to_string()
    } else {
        title.to_string()
    }
}

fn is_noise(word: &str) -> bool {
    let w = word.to_lowercase();
    NOISE_WORDS.contains(&w.as_str())
}

/// Lowercase letters and digits only, accents removed: "God of War - Ragnarök" → "godofwarragnarok".
pub fn compact(s: &str) -> String {
    fold(s).chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Lowercase without accents, roman sequel numbers as digits: "Red Dead Redemption II" → "red dead redemption 2".
fn fold(s: &str) -> String {
    let lower = fold_accents(s);
    lower
        .split(' ')
        .map(|word| match word {
            "ii" => "2",
            "iii" => "3",
            "iv" => "4",
            "vi" => "6",
            "vii" => "7",
            "viii" => "8",
            "ix" => "9",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn fold_accents(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            other => other,
        })
        .collect()
}

/// Sort key: lowercase, leading article removed.
pub fn sort_title(title: &str) -> String {
    let lower = title.trim().to_lowercase();
    for article in ["the ", "a ", "an ", "le ", "la ", "les ", "l'"] {
        if let Some(rest) = lower.strip_prefix(article) {
            return rest.to_string();
        }
    }
    lower
}

/// 0.0–1.0 resemblance between two names, tolerant to spacing ("PES2017" vs "PES 2017").
pub fn similarity(a: &str, b: &str) -> f32 {
    let (ca, cb) = (compact(a), compact(b));
    if ca.is_empty() || cb.is_empty() {
        return 0.0;
    }
    if ca == cb {
        return 1.0;
    }
    let (short, long) = if ca.len() <= cb.len() { (&ca, &cb) } else { (&cb, &ca) };
    if long.contains(short.as_str()) {
        return 0.5 + 0.5 * short.len() as f32 / long.len() as f32;
    }
    let words = |s: &str| -> Vec<String> {
        fold(s).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
    };
    let (wa, wb) = (words(a), words(b));
    let common = wa.iter().filter(|w| wb.contains(w)).count();
    let total = wa.len().max(wb.len());
    if total == 0 {
        0.0
    } else {
        0.45 * common as f32 / total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_real_folder_names() {
        let cases = [
            ("Assassin's Creed Black Flag Resynced [FitGirl Repack]", "Assassin's Creed Black Flag Resynced"),
            ("Grand Theft Auto V Enhanced [DODI Repack]", "Grand Theft Auto V Enhanced"),
            ("PES 2017 NextSeasonPatchV3 (MiniVersion)", "PES 2017 NextSeasonPatchV3"),
            ("God of War - Ragnarok", "God of War - Ragnarok"),
            ("Ghost of Tsushima DC", "Ghost of Tsushima DC"),
            ("Grand.Theft.Auto.V-CODEX", "Grand Theft Auto V"),
            ("Some_Game_v1.2.3-FLT", "Some Game"),
            ("Hollow Knight v1.5.78 MULTi9", "Hollow Knight"),
            ("Limbo", "Limbo"),
        ];
        for (raw, expected) in cases {
            assert_eq!(clean_title(raw), expected, "pour « {raw} »");
        }
    }

    #[test]
    fn similarity_ranks_the_right_exe() {
        let folder = "Red Dead Redemption 2";
        assert!(similarity(folder, "Red Dead Redemption 2") > similarity(folder, "Launcher"));
        assert!(similarity("PES 2017 NextSeasonPatchV3", "PES2017") > 0.5);
        assert_eq!(similarity("God of War - Ragnarok", "God of War Ragnarök"), 1.0);
        assert_eq!(similarity("Red Dead Redemption 2", "Red Dead Redemption II"), 1.0);
        assert!(similarity("Red Dead Redemption 2", "Red Dead Redemption") < 1.0);
        assert_eq!(similarity("", "x"), 0.0);
    }

    #[test]
    fn sort_title_skips_articles() {
        assert_eq!(sort_title("The Witcher 3"), "witcher 3");
        assert_eq!(sort_title("Limbo"), "limbo");
    }
}
