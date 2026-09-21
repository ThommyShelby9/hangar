//! Per-game controller setup, the way Steam Input does it: when a game starts, DS4Windows switches
//! to the profile that suits it (a virtual DualShock 4 for games that support PlayStation
//! controllers, a virtual Xbox 360 pad for the others), and goes back to the user's own profiles
//! when the last game closes. Profiles are loaded "temporarily", so DS4Windows' saved choices never
//! change. Which games support PlayStation controllers comes from PCGamingWiki, looked up once.
//!
//! Hangar only sends DS4Windows commands while DS4Windows is running: it never starts it.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{LazyLock, Mutex};

use regex::Regex;
use rusqlite::{params, Connection};
use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::db;
use crate::library::names;

/// Setting: switch DS4Windows profiles by game (on unless "false").
pub const ENABLED: &str = "pads_auto";
/// PCGamingWiki answers are kept this long before being asked again.
const RECHECK_SECS: i64 = 30 * 24 * 3600;
const USER_AGENT: &str = concat!("Hangar/", env!("CARGO_PKG_VERSION"), " (lanceur de jeux personnel)");

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    /// What games see: "DS4", "X360"… (DS4Windows' OutputContDevice).
    pub output: String,
}

pub struct Ds4Windows {
    exe: PathBuf,
    /// Folder holding Profiles.xml and Profiles\: next to the exe (portable) or in %APPDATA%.
    home: PathBuf,
}

impl Ds4Windows {
    /// The running DS4Windows, if any.
    pub fn running() -> Option<Ds4Windows> {
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let exe = system
            .processes()
            .values()
            .find(|p| p.name().eq_ignore_ascii_case("DS4Windows.exe"))?
            .exe()?
            .to_path_buf();
        let portable = exe.parent()?.to_path_buf();
        let home = if portable.join("Profiles.xml").is_file() {
            portable
        } else {
            PathBuf::from(std::env::var_os("APPDATA")?).join("DS4Windows")
        };
        Some(Ds4Windows { exe, home })
    }

    pub fn profiles(&self) -> Vec<Profile> {
        static OUTPUT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<OutputContDevice>\s*(\w+)\s*<").unwrap());
        let Ok(entries) = std::fs::read_dir(self.home.join("Profiles")) else { return vec![] };
        let mut profiles: Vec<Profile> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml")))
            .filter_map(|p| {
                let text = std::fs::read_to_string(&p).ok()?;
                Some(Profile {
                    name: p.file_stem()?.to_string_lossy().into_owned(),
                    output: OUTPUT.captures(&text).map(|c| c[1].to_string()).unwrap_or_else(|| "X360".into()),
                })
            })
            .collect();
        profiles.sort_by_key(|p| p.name.to_lowercase());
        profiles
    }

    /// The profile each controller slot uses normally (Profiles.xml).
    pub fn slot_profiles(&self) -> Vec<(u8, String)> {
        std::fs::read_to_string(self.home.join("Profiles.xml")).map(|t| slot_profiles(&t)).unwrap_or_default()
    }

    /// Sends a command to the running DS4Windows (it forwards it and exits).
    fn command(&self, command: &str) {
        let mut cmd = Command::new(&self.exe);
        cmd.args(["-command", command]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        match cmd.status() {
            Ok(_) => {}
            Err(e) => eprintln!("Hangar : commande DS4Windows « {command} » impossible : {e}"),
        }
    }
}

fn slot_profiles(xml: &str) -> Vec<(u8, String)> {
    static SLOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<Controller([1-8])>([^<]+)</Controller[1-8]>").unwrap());
    let mut slots: Vec<(u8, String)> = SLOT
        .captures_iter(xml)
        .filter_map(|c| Some((c[1].parse().ok()?, c[2].trim().to_string())))
        .collect();
    slots.sort();
    slots.dedup_by_key(|s| s.0);
    slots
}

/// The profile a game gets: its own choice, or automatically the one whose virtual controller the
/// game understands best. `None`: leave DS4Windows alone.
pub fn choose(choice: Option<&str>, ps_native: Option<bool>, profiles: &[Profile], usual: Option<&str>) -> Option<String> {
    match choice {
        Some("") => None,
        Some(name) => profiles.iter().any(|p| p.name == name).then(|| name.to_string()),
        None => {
            let wanted = if ps_native == Some(true) { "DS4" } else { "X360" };
            let fits = |p: &&Profile| p.output.eq_ignore_ascii_case(wanted);
            // The user's usual profile first, when it already gives the right controller.
            profiles
                .iter()
                .filter(fits)
                .find(|p| Some(p.name.as_str()) == usual)
                .or_else(|| profiles.iter().find(fits))
                .map(|p| p.name.clone())
        }
    }
}

// ── What each game wants ────────────────────────────────────────────────────

pub struct GamePad {
    pub title: String,
    pub choice: Option<String>,
    pub ps_native: Option<bool>,
    pub checked_at: Option<i64>,
}

pub fn game_pad(conn: &Connection, game_id: i64) -> rusqlite::Result<GamePad> {
    conn.query_row(
        "SELECT g.title, p.profile, p.ps_native, p.checked_at FROM games g
         LEFT JOIN game_pads p ON p.game_id = g.id WHERE g.id = ?1",
        [game_id],
        |r| Ok(GamePad { title: r.get(0)?, choice: r.get(1)?, ps_native: r.get(2)?, checked_at: r.get(3)? }),
    )
}

impl GamePad {
    pub fn needs_lookup(&self, now: i64) -> bool {
        self.checked_at.is_none_or(|t| now - t > RECHECK_SECS)
    }
}

pub fn set_choice(conn: &Connection, game_id: i64, profile: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO game_pads (game_id, profile) VALUES (?1, ?2)
         ON CONFLICT (game_id) DO UPDATE SET profile = excluded.profile",
        params![game_id, profile],
    )?;
    Ok(())
}

pub fn store_lookup(conn: &Connection, game_id: i64, ps_native: Option<bool>) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO game_pads (game_id, ps_native, checked_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (game_id) DO UPDATE SET ps_native = excluded.ps_native, checked_at = excluded.checked_at",
        params![game_id, ps_native, db::now()],
    )?;
    Ok(())
}

pub fn enabled(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(db::get_setting(conn, ENABLED)?.as_deref() != Some("false"))
}

/// Games whose PlayStation support hasn't been looked up (or not for a long time).
pub fn to_look_up(conn: &Connection) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT g.id, g.title FROM games g LEFT JOIN game_pads p ON p.game_id = g.id
         WHERE p.checked_at IS NULL OR p.checked_at < ?1",
    )?;
    let rows = stmt.query_map([db::now() - RECHECK_SECS], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

// ── PCGamingWiki ────────────────────────────────────────────────────────────

/// Names a game may have on PCGamingWiki: its title, another known name (Ludusavi's), the title
/// without a short edition tag ("Ghost of Tsushima DC"), abbreviations spelled out ("PES 2017").
pub fn candidate_titles(title: &str, also: Option<&str>) -> Vec<String> {
    const ABBREVIATIONS: &[(&str, &str)] = &[
        ("PES", "Pro Evolution Soccer"),
        ("GTA", "Grand Theft Auto"),
        ("COD", "Call of Duty"),
        ("RDR", "Red Dead Redemption"),
        ("AC", "Assassin's Creed"),
        ("MGS", "Metal Gear Solid"),
    ];
    let mut names = vec![title.to_string()];
    names.extend(also.filter(|a| !a.is_empty()).map(String::from));
    let words: Vec<&str> = title.split_whitespace().collect();
    if let [rest @ .., last] = words.as_slice() {
        let tag = last.len() <= 4 && last.chars().all(|c| c.is_ascii_uppercase());
        if tag && rest.len() >= 2 {
            names.push(rest.join(" "));
        }
    }
    if let Some((_, long)) = words.first().and_then(|w| ABBREVIATIONS.iter().find(|(a, _)| a.eq_ignore_ascii_case(w))) {
        names.push(std::iter::once(*long).chain(words[1..].iter().copied()).collect::<Vec<_>>().join(" "));
    }
    let spaced: Vec<String> = names.iter().map(|n| n.replace(" - ", " ")).collect();
    names.extend(spaced);
    let mut seen = std::collections::HashSet::new();
    names.retain(|n| seen.insert(n.to_lowercase()));
    names
}

/// Does the game support PlayStation controllers? `Ok(None)`: no PCGamingWiki page found.
pub async fn look_up(names: &[String]) -> Result<Option<bool>, String> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let mut pages = names.to_vec();
    // Then the wiki's own search, trusted only when the page it finds has one of the names.
    for name in names.iter().take(2) {
        let search: serde_json::Value = client
            .get(wiki_url(&[("action", "query"), ("list", "search"), ("srsearch", name), ("srlimit", "3"), ("format", "json"), ("formatversion", "2")]))
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("PCGamingWiki injoignable : {e}"))?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if let Some(results) = search["query"]["search"].as_array() {
            pages.extend(
                results
                    .iter()
                    .filter_map(|r| r["title"].as_str())
                    .filter(|t| names.iter().any(|n| names::similarity(t, n) >= 0.8))
                    .map(String::from),
            );
        }
    }

    for page in pages {
        let answer: serde_json::Value = client
            .get(wiki_url(&[("action", "parse"), ("page", &page), ("prop", "wikitext"), ("redirects", "1"), ("format", "json"), ("formatversion", "2")]))
            .send()
            .await
            .map_err(|e| format!("PCGamingWiki injoignable : {e}"))?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if let Some(text) = answer["parse"]["wikitext"].as_str() {
            return Ok(Some(ps_supported(text)));
        }
    }
    Ok(None)
}

fn wiki_url(params: &[(&str, &str)]) -> String {
    let query: Vec<String> = params.iter().map(|(k, v)| format!("{k}={}", urlencoding::encode(v))).collect();
    format!("https://www.pcgamingwiki.com/w/api.php?{}", query.join("&"))
}

/// The page's input table: "|playstation controllers = true". Anything else (false, limited,
/// hackable, missing) means the Xbox pad is the safer choice.
fn ps_supported(wikitext: &str) -> bool {
    static FIELD: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?im)^\|\s*playstation controllers\s*=\s*(\w*)").unwrap());
    FIELD.captures(wikitext).is_some_and(|c| c[1].eq_ignore_ascii_case("true"))
}

// ── Switching while games run ───────────────────────────────────────────────

/// Slots switched for the running game, with the profile each goes back to.
struct Applied {
    game_id: i64,
    slots: Vec<(u8, String)>,
}

static APPLIED: Mutex<Option<Applied>> = Mutex::new(None);

/// A game starts: DS4Windows switches every controller slot to the game's profile.
pub fn game_started(conn: &Mutex<Connection>, game_id: i64) {
    if APPLIED.lock().is_ok_and(|a| a.as_ref().is_some_and(|a| a.game_id == game_id)) {
        return;
    }
    let Ok(Some((choice, ps_native))) = conn.lock().map(|c| {
        enabled(&c)
            .ok()
            .filter(|on| *on)
            .and_then(|_| game_pad(&c, game_id).ok())
            .map(|g| (g.choice, g.ps_native))
    }) else {
        return;
    };
    let Some(ds4w) = Ds4Windows::running() else { return };
    let usual = ds4w.slot_profiles();
    let profiles = ds4w.profiles();
    let Some(profile) = choose(choice.as_deref(), ps_native, &profiles, usual.first().map(|s| s.1.as_str())) else {
        return;
    };

    let mut switched = Vec::new();
    for (slot, normal) in usual.into_iter().filter(|(slot, _)| *slot <= 4) {
        if normal != profile {
            ds4w.command(&format!("LoadTempProfile.{slot}.{profile}"));
            switched.push((slot, normal));
        }
    }
    if let Ok(mut applied) = APPLIED.lock() {
        *applied = Some(Applied { game_id, slots: switched });
    }
}

/// The last game closed: every slot gets its usual profile back.
pub fn games_stopped() {
    let Some(applied) = APPLIED.lock().ok().and_then(|mut a| a.take()) else { return };
    if applied.slots.is_empty() {
        return;
    }
    if let Some(ds4w) = Ds4Windows::running() {
        for (slot, normal) in applied.slots {
            ds4w.command(&format!("LoadProfile.{slot}.{normal}"));
        }
    }
}

/// For the game page: the profile the game would get right now.
pub fn current_choice(choice: Option<&str>, ps_native: Option<bool>) -> (bool, Vec<Profile>, Option<String>) {
    match Ds4Windows::running() {
        Some(ds4w) => {
            let profiles = ds4w.profiles();
            let usual = ds4w.slot_profiles();
            let picked = choose(choice, ps_native, &profiles, usual.first().map(|s| s.1.as_str()));
            (true, profiles, picked)
        }
        None => (false, vec![], None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profiles() -> Vec<Profile> {
        vec![
            Profile { name: "Default".into(), output: "DS4".into() },
            Profile { name: "xbox".into(), output: "X360".into() },
        ]
    }

    #[test]
    fn automatic_choice_follows_playstation_support() {
        let p = profiles();
        assert_eq!(choose(None, Some(true), &p, Some("Default")).as_deref(), Some("Default"));
        assert_eq!(choose(None, Some(false), &p, Some("Default")).as_deref(), Some("xbox"));
        assert_eq!(choose(None, None, &p, Some("Default")).as_deref(), Some("xbox"), "inconnu : manette Xbox");
        assert_eq!(choose(Some(""), Some(true), &p, None), None, "ne rien changer");
        assert_eq!(choose(Some("xbox"), Some(true), &p, None).as_deref(), Some("xbox"));
        assert_eq!(choose(Some("supprimé"), None, &p, None), None);
    }

    #[test]
    fn candidate_names() {
        assert_eq!(
            candidate_titles("Ghost of Tsushima DC", Some("Ghost of Tsushima Director's Cut")),
            ["Ghost of Tsushima DC", "Ghost of Tsushima Director's Cut", "Ghost of Tsushima"]
        );
        assert!(candidate_titles("PES 2017", None).contains(&"Pro Evolution Soccer 2017".to_string()));
        assert!(candidate_titles("God of War - Ragnarok", None).contains(&"God of War Ragnarok".to_string()));
        assert_eq!(candidate_titles("Limbo", Some("")), ["Limbo"]);
    }

    #[test]
    fn reads_slots_and_wiki_fields() {
        let xml = "<Profile><Controller1>Default</Controller1><Controller2>xbox</Controller2></Profile>";
        assert_eq!(slot_profiles(xml), vec![(1, "Default".into()), (2, "xbox".into())]);
        assert!(ps_supported("{{Input\n|controller support = true\n|playstation controllers   = true\n"));
        assert!(!ps_supported("{{Input\n|playstation controllers = false\n"));
        assert!(!ps_supported("mod can be used to force PlayStation button prompts"));
    }
}
