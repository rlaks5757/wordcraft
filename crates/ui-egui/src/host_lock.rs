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
use wordcraft_engine::host_policy::HostPolicy;

/// Interface commands a host-locked app runs.
#[rustfmt::skip]
const ALLOWED_UI: &[&str] = &[
    "ui.alignmentGuides", "ui.animations", "ui.backstage", "ui.caretAnimation", "ui.changePicture", "ui.click", "ui.clickText", "ui.closeDialog", "ui.collapseRibbon", "ui.contextMenu",
    "ui.crop", "ui.dark", "ui.dialog", "ui.drag", "ui.focus", "ui.inspect", "ui.interfaceSize", "ui.key", "ui.keytips", "ui.language", "ui.linkTextBox", "ui.move",
    "ui.openRecipientList", "ui.parity", "ui.press", "ui.quickAccessToolbar", "ui.release", "ui.render", "ui.resize", "ui.saveChanges", "ui.screenshot", "ui.tab", "ui.text", "ui.theme",
    "ui.unlistedCommand", "ui.wheel",
];

/// Interface commands a host-locked app refuses: they open documents, print, recover files, or reach outside.
#[rustfmt::skip]
const BLOCKED_UI: &[&str] = &[
    "ui.discord", "ui.fileBrowser", "ui.makeDefaultApp", "ui.openFileDialog", "ui.print", "ui.printSettings", "ui.printSubmit", "ui.recoveryDiscard",
    "ui.startScreen",
];

/// Blocked interface commands the policy's `print` group switches on.
const PRINT_UI: &[&str] = &["ui.print", "ui.printSettings", "ui.printSubmit"];

/// Blocked interface commands the policy's `external_links` group switches on.
const LINK_UI: &[&str] = &["ui.discord"];

/// Reference-manager commands (`ui.zotero.*`): the desktop app talks to a program on the user's machine.
const BLOCKED_UI_PREFIXES: &[&str] = &["ui.zotero."];

/// Dialogs behind blocked commands: templates and the Organizer (macros, styles from files), the
/// password and rename dialogs (they change what the parent receives). `ui.dialog` with one of
/// these names opens nothing.
pub const BLOCKED_DIALOGS: &[&str] = &["templates", "organizer", "encryptPassword", "renameDocument"];

/// Whether a host-locked app shows Backstage page `page`: Info, Save and Options always; Print
/// and Save As / Export only with the policy groups that allow them.
pub fn backstage_page_allowed(page: &str, policy: HostPolicy) -> bool {
    match page {
        "info" | "save" | "options" => true,
        "print" => policy.print,
        "saveAs" | "export" => policy.export,
        _ => false,
    }
}

/// Options panes a host-locked app hides (they set AutoSave, recent files and agent control).
pub const HIDDEN_OPTIONS_PANES: &[&str] = &["save", "agents"];

/// How the lock sees a `ui.*` id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Allowed,
    Blocked,
    Unclassified,
}

pub fn classify(id: &str) -> Class {
    if ALLOWED_UI.contains(&id) {
        Class::Allowed
    } else if BLOCKED_UI.contains(&id) || BLOCKED_UI_PREFIXES.iter().any(|p| id.starts_with(p)) {
        Class::Blocked
    } else {
        Class::Unclassified
    }
}

/// `Err` says why a locked app refuses the `ui.*` command `id` with `params`.
pub fn check_ui(id: &str, params: &Value, policy: HostPolicy) -> Result<(), String> {
    let switched_on = (policy.print && PRINT_UI.contains(&id)) || (policy.external_links && LINK_UI.contains(&id));
    if classify(id) != Class::Allowed && !(classify(id) == Class::Blocked && switched_on) {
        return Err(format!("{id}: not available in PPM"));
    }
    let name = params.get("name").and_then(Value::as_str);
    if id == "ui.dialog" && name.is_some_and(|n| BLOCKED_DIALOGS.contains(&n)) {
        return Err(format!("the {} dialog is not available in PPM", name.unwrap_or_default()));
    }
    if id == "ui.backstage"
        && let Some(page) = params.get("page").and_then(Value::as_str)
        && !backstage_page_allowed(page, policy)
    {
        return Err(format!("the {page} page is not available in PPM"));
    }
    Ok(())
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
        let loose: Vec<&String> = found.iter().filter(|id| classify(id) == Class::Unclassified).collect();
        assert!(loose.is_empty(), "classify these in host_lock.rs (ALLOWED_UI / BLOCKED_UI): {loose:?}");
        for id in ALLOWED_UI.iter().chain(BLOCKED_UI) {
            assert!(found.contains(*id), "{id} is listed but no longer exists");
        }
        for id in ALLOWED_UI {
            assert!(!BLOCKED_UI.contains(id), "{id} is in both tables");
        }
    }

    #[test]
    fn blocked_ui_commands_and_dialogs_are_refused() {
        let off = HostPolicy::default();
        for id in ["ui.openFileDialog", "ui.print", "ui.discord", "ui.startScreen", "ui.zotero.refresh", "ui.somethingNew"] {
            assert!(check_ui(id, &json!({}), off).is_err(), "{id}");
        }
        assert!(check_ui("ui.dialog", &json!({"name": "templates"}), off).is_err());
        assert!(check_ui("ui.dialog", &json!({"name": "font"}), off).is_ok());
        assert!(check_ui("ui.dialog", &json!({"name": "compare"}), off).is_ok(), "compare puts a file into the document");
        assert!(check_ui("ui.changePicture", &json!({}), off).is_ok());
        assert!(check_ui("ui.backstage", &json!({"page": "open"}), off).is_err());
        assert!(check_ui("ui.backstage", &json!({"page": "print"}), off).is_err());
        assert!(check_ui("ui.backstage", &json!({"page": "info"}), off).is_ok());
        assert!(check_ui("ui.backstage", &json!({"value": false}), off).is_ok());
    }

    /// The policy groups switch on their own interface commands and Backstage pages only.
    #[test]
    fn policy_groups_switch_on_their_own_interface_commands() {
        let print = HostPolicy::from_groups(&["print"]);
        let links = HostPolicy::from_groups(&["external_links"]);
        let export = HostPolicy::from_groups(&["export"]);
        for id in PRINT_UI {
            assert_eq!(classify(id), Class::Blocked, "{id}");
            assert!(check_ui(id, &json!({}), print).is_ok() && check_ui(id, &json!({}), links).is_err(), "{id}");
        }
        assert!(check_ui("ui.discord", &json!({}), links).is_ok() && check_ui("ui.discord", &json!({}), print).is_err());
        assert!(check_ui("ui.openFileDialog", &json!({}), HostPolicy::from_groups(&["print", "export", "external_links"])).is_err(), "opening stays blocked");
        assert!(check_ui("ui.backstage", &json!({"page": "print"}), print).is_ok());
        assert!(check_ui("ui.backstage", &json!({"page": "saveAs"}), export).is_ok());
        assert!(check_ui("ui.backstage", &json!({"page": "open"}), export).is_err());
    }
}
