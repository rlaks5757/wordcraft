//! A table the host page asks for (`wordcraft:insert-table`): the rows' texts go in at the caret
//! as one `insert.table`, so one undo step removes it and the document counts as changed.

use crate::WordApp;

/// Most rows, columns and characters per cell a request may carry.
pub const MAX_ROWS: usize = 50;
pub const MAX_COLS: usize = 20;
pub const MAX_CELL_CHARS: usize = 1000;

/// The refusal text for a caret or selection in a table; the host page matches on it.
pub const IN_TABLE: &str = "the caret is inside a table";

/// `rows` as cell texts for `insert.table`, or why the request is refused. Line breaks and tabs
/// in a cell become spaces: a cell holds one line of text.
pub fn table_data(rows: &[Vec<String>]) -> Result<serde_json::Value, String> {
    if rows.is_empty() || rows.len() > MAX_ROWS {
        return Err(format!("a table needs 1 to {MAX_ROWS} rows"));
    }
    for row in rows {
        if row.is_empty() || row.len() > MAX_COLS {
            return Err(format!("a row needs 1 to {MAX_COLS} cells"));
        }
        if row.iter().any(|c| c.chars().count() > MAX_CELL_CHARS) {
            return Err(format!("a cell holds at most {MAX_CELL_CHARS} characters"));
        }
    }
    let clean = |c: &String| c.chars().map(|ch| if ch.is_control() { ' ' } else { ch }).collect::<String>();
    Ok(serde_json::json!(rows.iter().map(|r| r.iter().map(clean).collect::<Vec<_>>()).collect::<Vec<_>>()))
}

/// Insert the table at the caret. An `Err` leaves the document as it was.
pub fn insert_table(app: &mut WordApp, rows: &[Vec<String>]) -> Result<(), String> {
    let data = table_data(rows)?;
    // A table inside a cell would merge into the one around it; the page asks the user to move out.
    let sel = &app.session.sel;
    if sel.anchor.path.cell().is_some() || sel.focus.path.cell().is_some() {
        return Err(IN_TABLE.into());
    }
    app.run("insert.table", serde_json::json!({ "data": data })).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::StoryRef;

    fn rows(r: &[&[&str]]) -> Vec<Vec<String>> {
        r.iter().map(|row| row.iter().map(|c| c.to_string()).collect()).collect()
    }

    fn app() -> WordApp {
        WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("before")), Default::default())
    }

    #[test]
    fn a_loop_table_goes_in_as_one_undoable_change() {
        let mut a = app();
        assert!(!a.session.dirty);
        insert_table(&mut a, &rows(&[&["Name", "Qty"], &["{{loop items}}{{name}}", "{{qty}}{{endloop}}"]])).unwrap();
        assert!(a.session.dirty);
        let t = a.session.doc.body.iter().find_map(|b| b.as_table()).expect("a table");
        assert_eq!((t.rows.len(), t.cols()), (2, 2));
        a.run("edit.undo", serde_json::json!({})).unwrap();
        assert!(a.session.doc.body.iter().all(|b| b.as_table().is_none()));
        assert_eq!(a.session.doc.plain_text(StoryRef::Body), "before");
    }

    #[test]
    fn bad_requests_are_refused_without_touching_the_document() {
        let mut a = app();
        let too_wide = vec![vec!["x".to_string(); MAX_COLS + 1]];
        let too_tall = vec![vec!["x".to_string()]; MAX_ROWS + 1];
        let long_cell = vec![vec!["x".repeat(MAX_CELL_CHARS + 1)]];
        for bad in [vec![], vec![vec![]], too_wide, too_tall, long_cell] {
            assert!(insert_table(&mut a, &bad).is_err());
        }
        assert!(!a.session.dirty);
        assert!(a.session.doc.body.iter().all(|b| b.as_table().is_none()));
    }

    #[test]
    fn a_caret_or_selection_in_a_table_is_refused_and_leaves_the_table_alone() {
        let mut a = app();
        insert_table(&mut a, &rows(&[&["a", "b"]])).unwrap();
        a.run("edit.undo", serde_json::json!({})).unwrap();
        insert_table(&mut a, &rows(&[&["a", "b"]])).unwrap();
        // The engine leaves the caret in the first cell of a table it inserted.
        assert!(a.session.sel.focus.path.cell().is_some());
        let before = a.session.doc.plain_text(StoryRef::Body);
        let err = insert_table(&mut a, &rows(&[&["Name", "Qty"], &["{{loop items}}{{name}}", "{{qty}}{{endloop}}"]]));
        assert!(err.is_err(), "a table does not go into a table cell");
        assert_eq!(a.session.doc.plain_text(StoryRef::Body), before);
        assert_eq!(a.session.doc.body.iter().filter(|b| b.as_table().is_some()).count(), 1);
        // A selection that only ends in a cell is refused too.
        let mut sel = a.session.sel.clone();
        sel.anchor = wordcraft_doc::Pos { story: StoryRef::Body, path: wordcraft_doc::Path(vec![0]), off: 0 };
        a.session.sel = sel;
        assert!(insert_table(&mut a, &rows(&[&["x", "y"]])).is_err());
    }

    #[test]
    fn line_breaks_in_a_cell_become_spaces() {
        let data = table_data(&rows(&[&["a\nb\tc"]])).unwrap();
        assert_eq!(data[0][0], "a b c");
    }
}
