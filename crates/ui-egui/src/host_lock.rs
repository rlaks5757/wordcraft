//! The UI half of the PPM host lock (the engine's half is `wordcraft_engine::host_policy`).
//!
//! A host-locked app ([`wordcraft_engine::Session::lock_to_host`]) refuses the engine commands
//! outside the engine's allowlist, and the interface commands (`ui.*`) outside [`ALLOWED_UI`],
//! before anything runs: from the ribbon, a shortcut, a context menu, command search or a script.
//! The platform functions behind them (file pickers, print, fonts from the user's machine,
//! links) check the lock themselves too, because several are reached without a command.
//!
//! The tables are default deny. A `ui.*` id that appears in the source but in neither table fails
//! `every_ui_command_is_classified`.

use serde_json::Value;
use wordcraft_engine::host_policy::{Grade, HostPolicy};

/// Interface commands a host-locked app runs.
#[rustfmt::skip]
const ALLOWED_UI: &[&str] = &[
    "ui.alignmentGuides", "ui.animations", "ui.backstage", "ui.caretAnimation", "ui.changePicture", "ui.click", "ui.clickText", "ui.closeDialog", "ui.collapseRibbon", "ui.contextMenu",
    "ui.crop", "ui.dark", "ui.dialog", "ui.drag", "ui.focus", "ui.inspect", "ui.interfaceSize", "ui.key", "ui.keytips", "ui.linkTextBox", "ui.move",
    "ui.openRecipientList", "ui.parity", "ui.press", "ui.quickAccessToolbar", "ui.release", "ui.render", "ui.resize", "ui.saveChanges", "ui.screenshot", "ui.tab", "ui.text", "ui.theme",
    "ui.unlistedCommand", "ui.wheel",
];

/// Interface commands no policy turns on: they open documents, recover files, or reach outside
/// (`ui.language`: the PPM interface is always Korean).
#[rustfmt::skip]
const LOCKED_UI: &[&str] = &[
    "ui.fileBrowser", "ui.language", "ui.makeDefaultApp", "ui.openFileDialog", "ui.recoveryDiscard", "ui.startScreen",
];

/// Interface commands that are off until the policy turns them on: print, and the Discord link.
#[rustfmt::skip]
const OFF_UI: &[&str] = &["ui.discord", "ui.print", "ui.printSettings", "ui.printSubmit"];

/// The switch for every external link (help, Discord, contributors, links in the document). It is
/// not a command in the source, only an id the parent can switch in its policy.
pub const EXTERNAL_LINKS: &str = "ui.externalLinks";

/// The id whose switch an interface command follows: printing follows `file.print`, the Discord
/// link follows [`EXTERNAL_LINKS`], anything else its own id.
fn policy_id(id: &str) -> &str {
    match id {
        "ui.print" | "ui.printSettings" | "ui.printSubmit" => "file.print",
        "ui.discord" => EXTERNAL_LINKS,
        _ => id,
    }
}

/// Reference-manager commands (`ui.zotero.*`): the desktop app talks to a program on the user's machine.
const BLOCKED_UI_PREFIXES: &[&str] = &["ui.zotero."];

/// Dialogs behind locked commands: the rename dialog (the parent names the document), the
/// password dialog and the Restrict Editing pane. `ui.dialog` with one of these names opens nothing.
pub const BLOCKED_DIALOGS: &[&str] = &["encryptPassword", "renameDocument", "restrictEditing"];

/// Whether a host-locked app shows Backstage page `page`: Save and Options always (Info never: it
/// has the file properties and password protection); Print and
/// Save As / Export only when the policy has the command behind them on.
pub fn backstage_page_allowed(page: &str, policy: &HostPolicy) -> bool {
    match page {
        "save" | "options" => true,
        "print" => policy.command_on("file.print"),
        "saveAs" => policy.command_on("file.saveAs"),
        "export" => policy.command_on("file.exportPdf") || policy.command_on("file.exportPng"),
        _ => false,
    }
}

/// Whether the policy lets links open (help, Discord, contributors, links in the document).
pub fn links_on(policy: &HostPolicy) -> bool {
    policy.allows(EXTERNAL_LINKS, Grade::Off)
}

/// Options panes a host-locked app hides (they set AutoSave, recent files and agent control).
pub const HIDDEN_OPTIONS_PANES: &[&str] = &["save", "agents"];

/// The grade of a `ui.*` id; `None` for one in no table.
pub fn grade_ui(id: &str) -> Option<Grade> {
    if ALLOWED_UI.contains(&id) {
        Some(Grade::On)
    } else if OFF_UI.contains(&id) || id == EXTERNAL_LINKS {
        Some(Grade::Off)
    } else if LOCKED_UI.contains(&id) || BLOCKED_UI_PREFIXES.iter().any(|p| id.starts_with(p)) {
        Some(Grade::Locked)
    } else {
        None
    }
}

/// `Err` says why a locked app refuses the `ui.*` command `id` with `params`.
pub fn check_ui(id: &str, params: &Value, policy: &HostPolicy) -> Result<(), String> {
    if !grade_ui(id).is_some_and(|g| policy.allows(policy_id(id), g)) {
        return Err(wordcraft_engine::host_policy::NOT_AVAILABLE.to_string());
    }
    let name = params.get("name").and_then(Value::as_str);
    if id == "ui.dialog" && name.is_some_and(|n| BLOCKED_DIALOGS.contains(&n)) {
        return Err(wordcraft_engine::host_policy::NOT_AVAILABLE.to_string());
    }
    if id == "ui.backstage"
        && let Some(page) = params.get("page").and_then(Value::as_str)
        && !backstage_page_allowed(page, policy)
    {
        return Err(wordcraft_engine::host_policy::NOT_AVAILABLE.to_string());
    }
    Ok(())
}

/// Korean name for a UI text; `None` where the catalog has none.
fn ko(text: &str) -> Option<String> {
    let lang = crate::i18n::Lang::from_code("ko")?;
    // A label with no letters ("100%") reads the same in every language.
    if !text.chars().any(char::is_alphabetic) {
        return Some(text.to_string());
    }
    crate::i18n::has(lang, text).then(|| crate::i18n::tr(lang, text).to_string())
}

/// The command list the admin screen reads (`commands.json`, the format all four apps share):
/// every engine command with its English and Korean name, menu place and grade, plus
/// [`EXTERNAL_LINKS`]. Built from the same tables the lock enforces, so the file can't disagree
/// with them.
pub fn commands_json() -> Value {
    let registry = wordcraft_engine::cmd::registry();
    let mut list: Vec<Value> = registry
        .all()
        .iter()
        .map(|c| {
            let grade = wordcraft_engine::host_policy::grade(c.id).map_or("locked", Grade::as_str);
            let place: Vec<&str> = c.location.split(" \u{203a} ").filter(|p| !p.is_empty()).collect();
            serde_json::json!({"id": c.id, "label": c.label, "labelKo": ko(c.label), "location": place, "grades": {"builder": grade}})
        })
        .collect();
    list.push(serde_json::json!({
        "id": EXTERNAL_LINKS, "label": "External links", "labelKo": "외부 링크", "location": ["Help"], "grades": {"builder": "off"},
    }));
    serde_json::json!({"app": "word", "schema": 1, "commands": list})
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Every `"ui.…"` id written anywhere in this crate's sources is in one table.
    #[test]
    fn every_ui_command_is_classified() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = std::collections::BTreeSet::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") || path.file_name().is_some_and(|n| n == "host_lock.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (i, _) in text.match_indices("\"ui.") {
                let rest = &text[i + 1..];
                let end = rest.find('"').unwrap_or(rest.len());
                let id = &rest[..end];
                // `"ui."` alone is the prefix test in `host_refusal`, not a command.
                if id != "ui." && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
                    found.insert(id.to_string());
                }
            }
        }
        assert!(found.len() > 40, "the scan found only {found:?}");
        let loose: Vec<&String> = found.iter().filter(|id| grade_ui(id).is_none()).collect();
        assert!(loose.is_empty(), "classify these in host_lock.rs (ALLOWED_UI / OFF_UI / LOCKED_UI): {loose:?}");
        for id in ALLOWED_UI.iter().chain(OFF_UI).chain(LOCKED_UI) {
            assert!(found.contains(*id), "{id} is listed but no longer exists");
        }
        for id in ALLOWED_UI {
            assert!(!OFF_UI.contains(id) && !LOCKED_UI.contains(id), "{id} is in two tables");
        }
    }

    fn overrides(pairs: &[(&str, bool)]) -> HostPolicy {
        HostPolicy::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)))
    }

    #[test]
    fn blocked_ui_commands_and_dialogs_are_refused() {
        let off = &HostPolicy::default();
        for id in ["ui.openFileDialog", "ui.print", "ui.discord", "ui.startScreen", "ui.zotero.refresh", "ui.somethingNew"] {
            assert!(check_ui(id, &json!({}), off).is_err(), "{id}");
        }
        assert!(check_ui("ui.dialog", &json!({"name": "renameDocument"}), off).is_err());
        // A refusal is worded in Korean, with the part the callers look for.
        for why in [
            check_ui("ui.print", &json!({}), off),
            check_ui("ui.dialog", &json!({"name": "renameDocument"}), off),
            check_ui("ui.backstage", &json!({"page": "open"}), off),
        ] {
            let why = why.unwrap_err();
            assert!(why.contains(wordcraft_engine::host_policy::NOT_AVAILABLE), "{why}");
        }
        assert!(check_ui("ui.dialog", &json!({"name": "templates"}), off).is_ok(), "styles go into the document");
        assert!(check_ui("ui.dialog", &json!({"name": "font"}), off).is_ok());
        assert!(check_ui("ui.dialog", &json!({"name": "compare"}), off).is_ok(), "compare puts a file into the document");
        assert!(check_ui("ui.changePicture", &json!({}), off).is_ok());
        assert!(check_ui("ui.backstage", &json!({"page": "open"}), off).is_err());
        assert!(check_ui("ui.backstage", &json!({"page": "print"}), off).is_err());
        assert!(check_ui("ui.backstage", &json!({"page": "info"}), off).is_err());
        assert!(check_ui("ui.backstage", &json!({"page": "options"}), off).is_ok());
        assert!(check_ui("ui.backstage", &json!({"value": false}), off).is_ok());
    }

    /// Print, links and the Backstage pages follow the engine commands and the link switch.
    #[test]
    fn overrides_switch_their_own_interface_commands() {
        let print = overrides(&[("file.print", true)]);
        let links = overrides(&[(EXTERNAL_LINKS, true)]);
        let export = overrides(&[("file.saveAs", true), ("file.exportPdf", true)]);
        for id in ["ui.print", "ui.printSettings", "ui.printSubmit"] {
            assert!(check_ui(id, &json!({}), &print).is_ok() && check_ui(id, &json!({}), &links).is_err(), "{id}");
        }
        assert!(check_ui("ui.discord", &json!({}), &links).is_ok() && check_ui("ui.discord", &json!({}), &print).is_err());
        assert!(links_on(&links) && !links_on(&print) && !links_on(&HostPolicy::default()));
        let everything =
            overrides(&[("file.print", true), ("file.saveAs", true), (EXTERNAL_LINKS, true), ("ui.openFileDialog", true), ("ui.startScreen", true)]);
        assert!(check_ui("ui.openFileDialog", &json!({}), &everything).is_err(), "opening stays locked");
        assert!(check_ui("ui.startScreen", &json!({}), &everything).is_err());
        assert!(check_ui("ui.backstage", &json!({"page": "print"}), &print).is_ok());
        assert!(check_ui("ui.backstage", &json!({"page": "saveAs"}), &export).is_ok());
        assert!(check_ui("ui.backstage", &json!({"page": "export"}), &export).is_ok());
        assert!(check_ui("ui.backstage", &json!({"page": "open"}), &export).is_err());
    }

    /// Labels that are a key or a product name, written the same in Korean.
    const SAME_IN_KOREAN: &[&str] = &["text.backTab", "caret.pageUp", "caret.pageDown", "insert.smartArt", "insert.wordArt"];

    /// Otherwise a Korean name has Hangul in it; a label with no letters at all ("100%") reads the same.
    fn has_hangul_or_no_letters(s: &str) -> bool {
        s.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c)) || !s.chars().any(char::is_alphabetic)
    }

    /// The admin screen shows the Korean name: every command has one.
    #[test]
    fn every_command_has_a_korean_label() {
        let file = commands_json();
        let missing: Vec<&str> = file["commands"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| !c["labelKo"].as_str().is_some_and(has_hangul_or_no_letters))
            .filter(|c| !c["id"].as_str().is_some_and(|id| SAME_IN_KOREAN.contains(&id)))
            .filter_map(|c| c["id"].as_str())
            .collect();
        assert!(missing.is_empty(), "no Korean label: {missing:?}");
    }

    /// commands.json lists exactly the engine's commands with the grade the lock enforces.
    #[test]
    fn commands_json_matches_the_tables() {
        let file = commands_json();
        let list = file["commands"].as_array().unwrap();
        let registry = wordcraft_engine::cmd::registry();
        assert_eq!(list.len(), registry.all().len() + 1, "every command once, plus the link switch");
        let mut ids = std::collections::BTreeSet::new();
        for c in list {
            let id = c["id"].as_str().unwrap();
            assert!(ids.insert(id), "{id} twice");
            assert!(c["label"].as_str().is_some_and(|t| !t.is_empty()), "{id} has no label");
            assert!(c["labelKo"].is_null() || c["labelKo"].is_string(), "{id}");
            assert!(c["location"].as_array().is_some_and(|l| l.iter().all(|p| p.is_string())), "{id} location is a list of names");
            assert_eq!(c["grades"].as_object().map(|g| g.len()), Some(1), "{id} has the builder grade only");
            if id == EXTERNAL_LINKS {
                assert_eq!(c["grades"]["builder"], "off");
                continue;
            }
            let grade = wordcraft_engine::host_policy::grade(id).expect("classified");
            assert_eq!(c["grades"]["builder"], grade.as_str(), "{id}");
        }
        for id in ["file.open", "file.print", "format.bold"] {
            let c = list.iter().find(|c| c["id"] == id).unwrap();
            assert_eq!(c["grades"]["builder"], wordcraft_engine::host_policy::grade(id).unwrap().as_str());
        }
        let bold = list.iter().find(|c| c["id"] == "format.bold").unwrap();
        assert_eq!(bold["labelKo"], "굵게");
        assert_eq!(bold["location"], serde_json::json!(["Home", "Font"]));
        assert_eq!(file["app"], "word");
        assert_eq!(file["schema"], 1);
    }
}
