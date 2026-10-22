//! The `postMessage` bridge to a same-origin parent page (`?host=parent`), for pages that embed
//! WordCraft in an `<iframe>`. Messages are plain objects with a `type`:
//!
//! - `wordcraft:ready` (to the parent): `{ version }`, the app is listening; send a file now.
//! - `wordcraft:open` (from the parent): `{ name, bytes, policy? }`, bytes an `ArrayBuffer` or
//!   `Uint8Array`. It opens like a document picked with File › Open or dropped on the window.
//!   `policy: { overrides: { "<command id>": true | false } }` switches commands for this document
//!   (true turns on a command that is off by default, such as `file.print`, `file.saveAs` and
//!   `ui.externalLinks`; false turns one off; locked commands ignore it). Without a valid policy
//!   every command keeps its default.
//! - `wordcraft:save` (to the parent): `{ name, bytes }` (a transferred `ArrayBuffer`) for every
//!   file WordCraft would otherwise download: Save and Save As (a .docx document) and Export
//!   (such as PDF). The parent stores it and reports failures itself.
//! - `wordcraft:dirty` (to the parent): `{ dirty }` whenever unsaved changes appear or go away.
//! - `wordcraft:failed` (to the parent): `{ error }` when the app couldn't start.
//! - `wordcraft:insert-table` (from the parent): `{ id, rows }`, rows an array of arrays of
//!   strings. It inserts a table with those cell texts at the caret, as one undo step.
//! - `wordcraft:table-inserted` (to the parent): `{ id }` when it did.
//! - `wordcraft:table-failed` (to the parent): `{ id, error }` when it couldn't (bad rows, a
//!   document that refuses the table).
//!
//! Only the parent window on the page's own origin is heard or answered. With a parent on another
//! origin there is no bridge: the app runs as if unframed. In host mode the page starts on an
//! empty document (no Start screen, no sample).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use wordcraft_ui_egui::host_table::{self, MAX_CELL_CHARS, MAX_COLS, MAX_ROWS};
use wordcraft_engine::host_policy::HostPolicy;
use wordcraft_ui_egui::{HostPolicySlot, Inbox, Services, WordApp};

/// The longest user name taken from `?author=`.
const MAX_AUTHOR_CHARS: usize = 200;

/// Most table requests waiting for the next frame; more are dropped.
const MAX_PENDING_TABLES: usize = 16;

/// The longest request id taken from the parent.
const MAX_ID_BYTES: usize = 200;

/// A table the parent asked for: its request id and the rows, or why they couldn't be read.
struct TableRequest {
    id: String,
    rows: Result<Vec<Vec<String>>, String>,
}

#[derive(Clone)]
pub struct Bridge {
    window: web_sys::Window,
    parent: web_sys::Window,
    origin: String,
    /// The unsaved-changes state last reported to the parent.
    reported: Rc<Cell<Option<bool>>>,
    /// Tables from the parent not yet inserted.
    tables: Rc<RefCell<Vec<TableRequest>>>,
}

impl Bridge {
    /// `None` when the page isn't framed by a page on its own origin.
    pub fn new() -> Option<Self> {
        let window = web_sys::window()?;
        let parent = window.parent().ok().flatten()?;
        if js_sys::Object::is(&parent, &window) {
            return None;
        }
        let origin = window.location().origin().ok()?;
        // Reading another origin's location throws, which is `None` too.
        if parent.location().origin().ok()? != origin {
            return None;
        }
        Some(Self { window, parent, origin, reported: Rc::default(), tables: Rc::default() })
    }

    /// Send saved and exported files to the parent instead of downloading them, report the dirty
    /// flag to it, and hear files from it. A file handed over counts as a finished save: the app
    /// marks the document saved when `download` returns `Ok` and no picker ticket was started.
    pub fn connect(&self, services: &mut Services, ctx: egui::Context) {
        // The parent's documents wait apart from the user's own files (picker, drops).
        let host_inbox = Inbox::default();
        let policy = HostPolicySlot::default();
        services.host_inbox = Some(host_inbox.clone());
        services.host_policy = Some(policy.clone());
        self.listen(host_inbox, policy, ctx);
        let bridge = self.clone();
        services.download = Some(Box::new(move |name: &str, bytes: &[u8]| bridge.post_save(name, bytes)));
        // The app tells the dirty flag after each pass; the parent hears it when it changes.
        let bridge = self.clone();
        let previous = services.on_dirty.take();
        services.on_dirty = Some(Box::new(move |dirty| {
            if let Some(previous) = &previous {
                previous(dirty);
            }
            bridge.report_dirty(dirty);
        }));
    }

    /// `?author=`: the host's name for the user signs new comments and tracked changes.
    pub fn set_author(app: &mut WordApp, author: &str) {
        let author: String = author.trim().chars().take(MAX_AUTHOR_CHARS).collect();
        if author.is_empty() {
            return;
        }
        if let Err(e) = app.run("file.setAuthor", serde_json::json!({ "name": author })) {
            log::warn!("?author= ignored: {e}");
        }
    }

    pub fn post_ready(&self) {
        self.send(&[("type", "wordcraft:ready".into()), ("version", env!("CARGO_PKG_VERSION").into())]);
    }

    pub fn post_failed(&self, error: &str) {
        self.send(&[("type", "wordcraft:failed".into()), ("error", error.into())]);
    }

    /// Before each pass: tables from the parent go in at the caret and the parent hears how each went.
    pub fn deliver_tables(&self, app: &mut WordApp) {
        for TableRequest { id, rows } in std::mem::take(&mut *self.tables.borrow_mut()) {
            match rows.and_then(|rows| host_table::insert_table(app, &rows)) {
                Ok(()) => self.send(&[("type", "wordcraft:table-inserted".into()), ("id", id.as_str().into())]),
                Err(error) => self.send(&[("type", "wordcraft:table-failed".into()), ("id", id.as_str().into()), ("error", error.as_str().into())]),
            };
        }
    }

    fn report_dirty(&self, dirty: bool) {
        if self.reported.get() != Some(dirty) && self.send(&[("type", "wordcraft:dirty".into()), ("dirty", dirty.into())]) {
            self.reported.set(Some(dirty));
        }
    }

    fn post_save(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
        let buffer = js_sys::Uint8Array::from(bytes).buffer();
        let message = message(&[("type", "wordcraft:save".into()), ("name", name.as_str().into()), ("bytes", buffer.clone().into())])
            .ok_or_else(|| "couldn't build the message".to_string())?;
        self.parent
            .post_message_with_transfer(&message, &self.origin, &js_sys::Array::of1(&buffer))
            .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))
    }

    fn listen(&self, inbox: Inbox, policy_slot: HostPolicySlot, ctx: egui::Context) {
        let parent = self.parent.clone();
        let origin = self.origin.clone();
        let tables = self.tables.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            if event.origin() != origin || !event.source().is_some_and(|s| js_sys::Object::is(&s, &parent)) {
                return;
            }
            let data = event.data();
            match string(&data, "type").as_deref() {
                Some("wordcraft:open") => {
                    let name = string(&data, "name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Document.docx".to_string());
                    let Some(bytes) = bytes(&data) else { return };
                    // The policy goes in before the file, so the app finds both together (one per file).
                    policy_slot.lock().unwrap_or_else(|e| e.into_inner()).push_back(policy(&data));
                    inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                }
                Some("wordcraft:insert-table") => {
                    // Without a usable id there is nobody to answer.
                    let Some(id) = string(&data, "id").filter(|id| !id.is_empty() && id.len() <= MAX_ID_BYTES) else { return };
                    if tables.borrow().len() < MAX_PENDING_TABLES {
                        tables.borrow_mut().push(TableRequest { id, rows: rows(&data) });
                    }
                }
                _ => return,
            }
            ctx.request_repaint();
        });
        if self.window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref()).is_ok() {
            // The listener lives as long as the page.
            on_message.forget();
        }
    }

    /// Post a message to the parent; false if it couldn't be sent.
    fn send(&self, fields: &[(&str, JsValue)]) -> bool {
        message(fields).is_some_and(|m| self.parent.post_message(&m, &self.origin).is_ok())
    }
}

fn message(fields: &[(&str, JsValue)]) -> Option<js_sys::Object> {
    let m = js_sys::Object::new();
    for (key, value) in fields {
        js_sys::Reflect::set(&m, &JsValue::from_str(key), value).ok()?;
    }
    Some(m)
}

/// The per-command switches of the open message's `policy.overrides` (`{ "<command id>": bool }`),
/// read as [`HostPolicy::from_entries`] does: a missing or malformed policy changes nothing, a
/// value that is not a boolean is skipped, an id that is no command does nothing.
fn policy(data: &JsValue) -> HostPolicy {
    let get = |from: &JsValue, key: &str| js_sys::Reflect::get(from, &JsValue::from_str(key)).ok();
    let Some(overrides) = get(data, "policy").and_then(|p| get(&p, "overrides")).filter(|o| o.is_object() && !js_sys::Array::is_array(o)) else {
        return HostPolicy::default();
    };
    let entries = js_sys::Object::entries(&js_sys::Object::from(overrides));
    HostPolicy::from_entries(entries.iter().filter_map(|e| {
        let pair = js_sys::Array::from(&e);
        Some((pair.get(0).as_string()?, pair.get(1).as_bool()))
    }))
}

fn string(data: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(data, &JsValue::from_str(key)).ok()?.as_string()
}

/// `rows` as an array of arrays of strings, no larger than a table may be (checked before the
/// values are copied out).
fn rows(data: &JsValue) -> Result<Vec<Vec<String>>, String> {
    let bad = || "rows must be an array of arrays of strings".to_string();
    let rows = js_sys::Reflect::get(data, &JsValue::from_str("rows")).map_err(|_| bad())?;
    if !js_sys::Array::is_array(&rows) {
        return Err(bad());
    }
    let rows = js_sys::Array::from(&rows);
    if rows.length() as usize > MAX_ROWS {
        return Err(format!("a table needs 1 to {MAX_ROWS} rows"));
    }
    rows.iter()
        .map(|row| {
            if !js_sys::Array::is_array(&row) {
                return Err(bad());
            }
            let row = js_sys::Array::from(&row);
            if row.length() as usize > MAX_COLS {
                return Err(format!("a row needs 1 to {MAX_COLS} cells"));
            }
            row.iter()
                .map(|cell| {
                    // UTF-16 units are never fewer than characters: a longer string is refused before it is copied.
                    let units = cell.dyn_ref::<js_sys::JsString>().map(|s| s.length() as usize).ok_or_else(bad)?;
                    if units > MAX_CELL_CHARS * 2 {
                        return Err(format!("a cell holds at most {MAX_CELL_CHARS} characters"));
                    }
                    cell.as_string().ok_or_else(bad)
                })
                .collect()
        })
        .collect()
}

/// `bytes` as an `ArrayBuffer` or `Uint8Array`.
fn bytes(data: &JsValue) -> Option<Vec<u8>> {
    let bytes = js_sys::Reflect::get(data, &JsValue::from_str("bytes")).ok()?;
    if let Some(array) = bytes.dyn_ref::<js_sys::Uint8Array>() {
        Some(array.to_vec())
    } else if bytes.is_instance_of::<js_sys::ArrayBuffer>() {
        Some(js_sys::Uint8Array::new(&bytes).to_vec())
    } else {
        None
    }
}
