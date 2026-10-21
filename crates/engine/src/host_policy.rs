//! The host lock: which commands a PPM-hosted session runs.
//!
//! PPM embeds WordCraft in its own page and owns the files: the parent hands the document over and
//! stores what WordCraft saves. A session locked to its host ([`Session::lock_to_host`]) therefore
//! runs only the commands in [`ALLOWED`] (default deny): everything that opens, creates, exports,
//! prints or downloads files, loads templates, macros or fonts from the user's machine, or opens
//! other windows is refused in [`Session::run`], whichever control (ribbon, shortcut, context
//! menu, command search, script) asked. A new command that is in none of the three tables fails
//! `every_command_is_classified`, so nothing becomes available by being forgotten.
//!
//! The lock is one-way: no command clears it. The app turns it on once at startup from a build
//! switch, so a running page can't be talked out of it.

use serde_json::Value;

use crate::{CmdError, Session};

/// Commands a host-locked session runs: editing and reading the document that is open.
#[rustfmt::skip]
const ALLOWED: &[&str] = &[
    "arrange.align", "arrange.bounds", "arrange.bringForward", "arrange.copyTo", "arrange.group", "arrange.moveWithText", "arrange.nudge",
    "arrange.position", "arrange.rotate", "arrange.rotation", "arrange.selectionPane", "arrange.sendBackward", "arrange.ungroup", "arrange.wrap",
    "arrange.wrapText", "canvas.expand", "canvas.fit", "canvas.member", "canvas.scale", "caret.docEnd", "caret.docStart", "caret.down", "caret.end",
    "caret.home", "caret.left", "caret.pageDown", "caret.pageUp", "caret.paraDown", "caret.paraUp", "caret.pastCitation", "caret.right", "caret.set",
    "caret.up", "caret.wordLeft", "caret.wordRight", "chart.dataLabels", "chart.editData", "chart.legend", "chart.title", "chart.type",
    "design.effects", "design.pageBorders", "design.pageColor", "design.paragraphSpacing", "design.setDefault", "design.styleSet", "design.theme",
    "design.themeColors", "design.themeFonts", "design.themes", "design.watermark", "developer.buildingBlock", "developer.checkBox",
    "developer.chooseItem", "developer.comboBox", "developer.control", "developer.datePicker", "developer.designMode", "developer.dropDown",
    "developer.plainText", "developer.properties", "developer.removeControl", "developer.repeatingSection", "developer.richText", "developer.setDate",
    "developer.toggleCheckBox", "document.inspect", "document.layout", "document.paragraph", "document.selection", "document.setText",
    "document.text", "draw.addPen", "draw.convertInk", "draw.erase", "draw.eraser", "draw.highlighter", "draw.inkToShape", "draw.lasso",
    "draw.lassoColor", "draw.lassoDelete", "draw.lassoMove", "draw.pen", "draw.pencil", "draw.removePen", "draw.replay", "draw.select", "draw.stroke",
    "edit.advancedFind", "edit.clearClipboard", "edit.clipboardItems", "edit.clipboardPane", "edit.copy", "edit.copyFormat", "edit.cut",
    "edit.deleteClipboardItem", "edit.find", "edit.findNext", "edit.findPrevious", "edit.findReplace", "edit.formatPainter", "edit.goBack",
    "edit.goto", "edit.insertSpike", "edit.paste", "edit.pasteAllClipboard", "edit.pasteClipboardItem", "edit.pasteFormat", "edit.pasteMerge",
    "edit.pasteSpecial", "edit.pasteText", "edit.redo", "edit.repeat", "edit.replace", "edit.replaceAll", "edit.spike", "edit.undo",
    "edit.undoHistory", "edit.undoTo", "equation.backspace", "equation.convert", "equation.delete", "equation.display", "equation.edit",
    "equation.enter", "equation.exit", "equation.gallery", "equation.get", "equation.inputFormat", "equation.insertStructure",
    "equation.insertSymbol", "equation.justify", "equation.move", "equation.normalText", "equation.number", "equation.set", "equation.structure",
    "equation.structureActions", "equation.type", "fields.lock", "fields.next", "fields.previous", "fields.selected", "fields.setCode",
    "fields.toggleCode", "fields.unlink", "fields.unlock", "file.accessibility", "file.compatibility", "file.info", "file.inspect", "file.options",
    "file.privacy", "file.privacyOptions", "file.properties", "file.save", "file.setAuthor", "font.list", "format.allCaps", "format.bold",
    "format.border", "format.changeCase", "format.charStyle", "format.clear", "format.color", "format.doubleStrikethrough", "format.doubleUnderline",
    "format.emboss", "format.enclose", "format.engrave", "format.font", "format.fontDialog", "format.growFont", "format.growFont1", "format.hidden",
    "format.highlight", "format.italic", "format.outline", "format.phonetic", "format.position", "format.resetChar", "format.scale", "format.set",
    "format.shading", "format.shadow", "format.shrinkFont", "format.shrinkFont1", "format.size", "format.smallCaps", "format.spacing", "format.state",
    "format.strikethrough", "format.subscript", "format.superscript", "format.textEffects", "format.underline", "format.wordUnderline", "hf.next",
    "hf.position", "hf.previous", "insert.autoText", "insert.blankPage", "insert.bookmark", "insert.canvas", "insert.chart", "insert.closeHeader",
    "insert.coverPage", "insert.createFooter", "insert.createHeader", "insert.crossReference", "insert.dateField", "insert.dateTime",
    "insert.deleteBookmark", "insert.docProperty", "insert.dropCap", "insert.editFooter", "insert.editHeader", "insert.equation", "insert.field",
    "insert.footer", "insert.header", "insert.horizontalLine", "insert.icon", "insert.link", "insert.pageBreak", "insert.pageField",
    "insert.pageNumber", "insert.quickParts", "insert.removeEmptyHeaders", "insert.removeFooter", "insert.removeHeader", "insert.removeLink",
    "insert.shape", "insert.signatureLine", "insert.smartArt", "insert.symbol", "insert.table", "insert.textBox", "insert.timeField",
    "insert.wordArt", "layout.break", "layout.columns", "layout.differentFirstPage", "layout.differentOddEven", "layout.documentGrid",
    "layout.hyphenation", "layout.lineNumbers", "layout.linkToPrevious", "layout.manualHyphenation", "layout.margins", "layout.orientation",
    "layout.pageNumberFormat", "layout.pageSetup", "layout.section", "layout.sectionRtl", "layout.size", "layout.textDirection",
    "layout.verticalAlign", "link.targets", "list.define", "list.get", "list.multilevelPreset", "object.altText", "outline.close", "outline.collapse",
    "outline.demote", "outline.demoteToBody", "outline.expand", "outline.firstLineOnly", "outline.level", "outline.moveDown", "outline.moveUp",
    "outline.promote", "outline.promoteToHeading1", "outline.selectSubtree", "outline.showAll", "outline.showFormatting", "outline.showLevel",
    "outline.showLevel1", "outline.showLevel2", "outline.showLevel3", "outline.showLevel4", "outline.showLevel5", "outline.showLevel6",
    "outline.showLevel7", "outline.showLevel8", "outline.showLevel9", "outline.state", "outline.toggle", "para.addSpaceBefore", "para.align",
    "para.alignCenter", "para.alignLeft", "para.alignRight", "para.asianTypography", "para.borders", "para.bullets", "para.defineBullet",
    "para.defineNumber", "para.demote", "para.dialog", "para.distribute", "para.double", "para.hangingIndent", "para.heading1", "para.heading2",
    "para.heading3", "para.indent", "para.indents", "para.justify", "para.keepLines", "para.keepNext", "para.lineSpacing", "para.listLevel",
    "para.ltr", "para.moveDown", "para.moveUp", "para.multilevel", "para.normal", "para.numbering", "para.oneAndHalf", "para.outdent",
    "para.outlineLevel", "para.pageBreakBefore", "para.promote", "para.removeHanging", "para.removeSpaceAfter", "para.reset", "para.restartNumbering",
    "para.rtl", "para.set", "para.setNumberingValue", "para.shading", "para.single", "para.sort", "para.spacing", "para.style", "para.tabs",
    "para.widowControl", "picture.altText", "picture.border", "picture.color", "picture.compress", "picture.corrections", "picture.crop",
    "picture.effects", "picture.iconColor", "picture.removeBackground", "picture.reset", "picture.size", "picture.style", "picture.transparency",
    "readAloud.next", "readAloud.playPause", "readAloud.previous", "readAloud.skipCitations", "readAloud.speed", "readAloud.status", "readAloud.stop",
    "references.addText", "references.bibliography", "references.caption", "references.citation", "references.citationStyle",
    "references.convertNotes", "references.endnote", "references.footnote", "references.index", "references.markCitation", "references.markEntry",
    "references.nextFootnote", "references.noteOptions", "references.notes", "references.removeToc", "references.researcher", "references.sources",
    "references.tableOfAuthorities", "references.tableOfFigures", "references.toc", "references.updateAuthorities", "references.updateFields",
    "references.updateFigures", "references.updateIndex", "references.updateToc", "review.accept", "review.acceptAll", "review.addToDictionary",
    "review.applySuggestion", "review.changes", "review.comments", "review.deleteComment", "review.editComment", "review.editor", "review.hideInk",
    "review.ignoreAll", "review.issues", "review.language", "review.markup", "review.newComment", "review.nextChange", "review.nextComment",
    "review.previousChange", "review.previousComment", "review.proofing", "review.readAloud", "review.reject", "review.rejectAll", "review.reply",
    "review.resolveComment", "review.restrict", "review.showMarkup", "review.spelling", "review.suggestions", "review.thesaurus",
    "review.trackChanges", "review.trackingOptions", "review.wordCount", "select.addObject", "select.all", "select.collapse", "select.column",
    "select.dragUnit", "select.extend", "select.line", "select.objects", "select.paragraph", "select.range", "select.sentence", "select.similar",
    "select.text", "select.word", "shape.alignText", "shape.breakLink", "shape.change", "shape.connect", "shape.effects", "shape.fill", "shape.link",
    "shape.outline", "shape.textDirection", "smartArt.addShape", "smartArt.colors", "smartArt.demote", "smartArt.items", "smartArt.layout",
    "smartArt.promote", "smartArt.reset", "smartArt.textPane", "styles.addToGallery", "styles.apply", "styles.copyFrom", "styles.create",
    "styles.delete", "styles.inspect", "styles.inspector", "styles.inspectorClear", "styles.list", "styles.manage", "styles.modify",
    "styles.organizer", "styles.pane", "styles.rename", "styles.setVisibility", "styles.updateToMatch", "table.altText", "table.autofit",
    "table.borderPainter", "table.borders", "table.cellAlign", "table.cellMargins", "table.columnWidth", "table.deleteCells", "table.deleteColumn",
    "table.deleteRow", "table.deleteStyle", "table.deleteTable", "table.distributeColumns", "table.distributeRows", "table.draw", "table.eraser",
    "table.fitColumn", "table.formula", "table.fromText", "table.insertAt", "table.insertColumnLeft", "table.insertColumnRight",
    "table.insertRowAbove", "table.insertRowBelow", "table.look", "table.merge", "table.modifyStyle", "table.moveDrag", "table.newStyle",
    "table.properties", "table.quick", "table.repeatHeader", "table.resizeDrag", "table.rowHeight", "table.rtl", "table.selectAt", "table.selectCell",
    "table.selectColumn", "table.selectRow", "table.selectTable", "table.shading", "table.sort", "table.split", "table.splitTable", "table.style",
    "table.textDirection", "table.toText", "table.viewGridlines", "text.backTab", "text.backspace", "text.columnBreak", "text.copyright",
    "text.delete", "text.deleteWordBack", "text.deleteWordForward", "text.ellipsis", "text.emDash", "text.insert", "text.lineBreak", "text.nbHyphen",
    "text.nbsp", "text.newParagraph", "text.optionalHyphen", "text.overtype", "text.pageBreak", "text.registered", "text.tab", "text.toggleUnicode",
    "text.trademark", "text.typeCode", "tools.autocorrect", "tools.search", "view.accessibilityPane", "view.altTextPane", "view.commentsPane",
    "view.darkMode", "view.draft", "view.fieldCodes", "view.focus", "view.fullScreen", "view.gridlines", "view.hideWhiteSpace", "view.immersive",
    "view.interfaceTheme", "view.marks", "view.multiplePages", "view.navigationPane", "view.numeral", "view.onePage", "view.outline",
    "view.pageWidth", "view.printLayout", "view.readColumnWidth", "view.readLineSpacing", "view.readMode", "view.readTextSize", "view.ruler",
    "view.sideToSide", "view.split", "view.state", "view.stylesPane", "view.vertical", "view.webLayout", "view.zoom", "view.zoom100", "view.zoomIn",
    "view.zoomOut", "wordArt.style", "wordArt.textFill", "wordArt.textOutline", "wordArt.transform",
];

/// Commands a host-locked session refuses: they reach files, printing, exports, templates, macros, fonts or other windows.
#[rustfmt::skip]
const BLOCKED: &[&str] = &[
    "file.autosave", "file.close", "file.encrypt", "file.exportPdf", "file.exportPng", "file.new", "file.newFromTemplate", "file.open", "file.print",
    "file.printMarkup", "file.printPdf", "file.recover", "file.saveAs", "file.saveTemplate", "file.share", "font.add", "font.loadSystem",
    "font.remove", "insert.object", "insert.picture", "insert.textFromFile", "picture.change", "tools.attachTemplate", "tools.detachTemplate",
    "tools.linkStyles", "tools.macros", "tools.recordMacro", "tools.templates", "tools.updateStyles", "view.newWindow",
];

/// Commands nobody has decided on yet ("판단 필요"). Refused like [`BLOCKED`] until someone moves them.
#[rustfmt::skip]
const UNDECIDED: &[&str] = &[
    "developer.picture", "file.protect", "file.rename", "file.versions", "insert.spreadsheet", "mailings.addressBlock", "mailings.checkErrors",
    "mailings.editRecipients", "mailings.envelopes", "mailings.filterRecipients", "mailings.findRecipient", "mailings.finish",
    "mailings.greetingLine", "mailings.highlightFields", "mailings.insertField", "mailings.labels", "mailings.matchFields", "mailings.next",
    "mailings.preview", "mailings.previous", "mailings.recipients", "mailings.rules", "mailings.sortRecipients", "mailings.start", "review.combine",
    "review.compare", "tools.customizeKeyboard", "tools.customizeRibbon",
];

/// How the lock treats a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Allowed,
    Blocked,
    Undecided,
    /// In no table: refused, and `every_command_is_classified` fails until it is added.
    Unclassified,
}

/// Where `id` stands.
pub fn classify(id: &str) -> Class {
    if ALLOWED.contains(&id) {
        Class::Allowed
    } else if BLOCKED.contains(&id) {
        Class::Blocked
    } else if UNDECIDED.contains(&id) {
        Class::Undecided
    } else {
        Class::Unclassified
    }
}

/// `file.save` params that change what the parent receives: another name or format, or a password.
const SAVE_PARAMS_REFUSED: [&str; 3] = ["path", "password", "format"];

/// `Err` says why a locked session refuses `id` with `params`. Only [`Class::Allowed`] commands
/// pass, and Save only as the plain save the parent stores.
pub fn check(id: &str, params: &Value) -> Result<(), String> {
    if classify(id) != Class::Allowed {
        return Err(format!("{id}: not available in PPM"));
    }
    if id == "file.save"
        && let Some(p) = params.as_object()
        && SAVE_PARAMS_REFUSED.iter().any(|k| p.get(*k).is_some_and(|v| !v.is_null()))
    {
        return Err("file.save: PPM stores the document under its own name".to_string());
    }
    Ok(())
}

impl Session {
    /// Lock this session to its host (see the module docs). There is no way back.
    pub fn lock_to_host(&mut self) {
        self.host_lock = true;
    }

    pub fn is_host_locked(&self) -> bool {
        self.host_lock
    }

    /// Let the next `file.open` of exactly this document name (with bytes) through the lock: the
    /// parent page's document. The app arms it when the parent sends a file, because the user's
    /// own Open stays blocked. It is used up by that open (or [`Self::disarm_host_open`]).
    pub fn arm_host_open(&mut self, name: &str) {
        self.host_open = Some(name.to_string());
    }

    pub fn disarm_host_open(&mut self) {
        self.host_open = None;
    }

    /// `Err` when the lock refuses `id` with `params`; always `Ok` for an unlocked session.
    pub fn host_check(&self, id: &str, params: &Value) -> Result<(), String> {
        if !self.host_lock {
            return Ok(());
        }
        let armed = id == "file.open"
            && self.host_open.as_deref().is_some_and(|n| params.get("path").and_then(Value::as_str) == Some(n))
            && params.get("data").is_some_and(Value::is_string);
        if armed { Ok(()) } else { check(id, params) }
    }

    /// [`Self::host_check`] as the error [`Session::run`] returns; uses up an armed open.
    pub(crate) fn host_gate(&mut self, id: &str, params: &Value) -> Result<(), CmdError> {
        self.host_check(id, params).map_err(CmdError::Disabled)?;
        if id == "file.open" && self.host_lock {
            self.host_open = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::Session;

    fn locked() -> Session {
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.lock_to_host();
        s
    }

    /// Drift: every registered command is in exactly one table, and every table entry is real.
    #[test]
    fn every_command_is_classified() {
        let registry = crate::cmd::registry();
        let ids: Vec<&str> = registry.all().iter().map(|c| c.id).collect();
        let unclassified: Vec<&&str> = ids.iter().filter(|id| classify(id) == Class::Unclassified).collect();
        assert!(unclassified.is_empty(), "classify these in host_policy.rs (ALLOWED / BLOCKED / UNDECIDED): {unclassified:?}");
        for (name, table) in [("ALLOWED", ALLOWED), ("BLOCKED", BLOCKED), ("UNDECIDED", UNDECIDED)] {
            let gone: Vec<&&str> = table.iter().filter(|id| !ids.contains(id)).collect();
            assert!(gone.is_empty(), "{name} lists commands that no longer exist: {gone:?}");
        }
        let mut seen = std::collections::BTreeSet::new();
        for id in ALLOWED.iter().chain(BLOCKED).chain(UNDECIDED) {
            assert!(seen.insert(*id), "{id} is in two tables or twice in one");
        }
    }

    /// Everything that reaches the user's files, printers, templates, macros or fonts is refused.
    #[test]
    fn file_and_platform_commands_are_refused() {
        let mut s = locked();
        for id in [
            "file.new", "file.open", "file.saveAs", "file.exportPdf", "file.exportPng", "file.print", "file.printPdf", "file.newFromTemplate", "file.saveTemplate",
            "file.recover", "file.autosave", "file.close", "file.encrypt", "file.share", "insert.picture", "insert.textFromFile", "insert.object",
            "picture.change", "tools.attachTemplate", "tools.macros", "tools.recordMacro", "tools.templates", "font.add", "font.loadSystem", "view.newWindow",
        ] {
            let before = s.doc.plain_text(wordcraft_doc::StoryRef::Body);
            let r = s.run(id, &json!({"path": "/tmp/x.docx", "data": "AAAA", "text": "x"}));
            assert!(matches!(r, Err(CmdError::Disabled(_))), "{id} must be refused, got {r:?}");
            assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), before, "{id} changed the document");
        }
    }

    /// Mail merge, compare and the other undecided commands are refused until someone decides.
    #[test]
    fn undecided_commands_are_refused() {
        let mut s = locked();
        for id in UNDECIDED {
            assert!(matches!(s.run(id, &json!({})), Err(CmdError::Disabled(_))), "{id}");
        }
    }

    /// Editing and saving still work, and an unlocked session is untouched.
    #[test]
    fn editing_and_plain_save_still_work() {
        let mut s = locked();
        s.run("text.insert", &json!({"text": "hello"})).unwrap();
        assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), "hello");
        s.run("format.bold", &json!({})).unwrap();
        assert!(check("file.save", &json!({})).is_ok());
        assert!(check("file.save", &json!({"path": null})).is_ok());
        assert!(check("file.save", &json!({"path": "other.pdf"})).is_err(), "Save keeps the parent's name");
        assert!(check("file.save", &json!({"password": "x"})).is_err());
        assert!(check("file.save", &json!({"format": "flatopc"})).is_err());
        let mut open = Session::new(wordcraft_doc::Document::new());
        assert!(open.run("file.new", &json!({})).is_ok(), "no lock, no refusal");
    }

    /// The parent's own document opens through the lock; any other open does not.
    #[test]
    fn host_open_passes_only_when_armed_for_that_name() {
        let data = json!({"path": "doc.txt", "data": "aGVsbG8="});
        let mut s = locked();
        assert!(s.run("file.open", &data).is_err(), "not armed");
        s.arm_host_open("doc.txt");
        assert!(s.run("file.open", &json!({"path": "other.txt", "data": "aGVsbG8="})).is_err(), "another name");
        assert!(s.run("file.open", &json!({"path": "doc.txt"})).is_err(), "no bytes");
        s.run("file.open", &data).unwrap();
        assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), "hello");
        assert!(s.run("file.open", &data).is_err(), "used up by the first open");
    }

    /// A command the lock allows never runs a refused one underneath (it would half-work).
    #[test]
    fn allowed_commands_do_not_trip_the_lock_on_hostile_params() {
        let mut s = locked();
        s.run("text.insert", &json!({"text": "seed text"})).unwrap();
        for id in ALLOWED {
            let r = s.run(id, &json!({"path": "/nonexistent/x", "data": "!!", "text": "t", "value": true}));
            if let Err(CmdError::Disabled(why)) = &r {
                assert!(!why.contains("not available in PPM"), "{id} ran a command the lock refuses: {why}");
            }
        }
    }
}
