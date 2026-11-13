use serde_json::json;
use std::sync::Arc;
use wordcraft_doc::props::{Align, CharProps};
use wordcraft_doc::{Block, Document, Paragraph, Path, StoryRef};

use crate::Session;

fn sample() -> Session {
    Session::new(Document::from_text("Hello\nWorld\nFigure 1 - Demo"))
}

#[test]
fn get_ir_is_version_2() {
    let mut s = sample();
    let ir = s.run("doc.getIr", &json!({})).unwrap();
    assert_eq!(ir["version"], 2);
    assert!(ir["blocks"].as_array().unwrap().len() >= 3);
}

#[test]
fn apply_layout_default_sets_calibri_normal() {
    let mut s = sample();
    s.run("doc.applyLayout", &json!({})).unwrap();
    let st = s.doc.styles.get("Normal").unwrap();
    assert_eq!(st.chr.font.as_deref(), Some("Calibri"));
    assert_eq!(st.chr.size, Some(11.0));
}

#[test]
fn apply_layout_accepts_custom_profile() {
    let mut s = sample();
    s.run(
        "doc.applyLayout",
        &json!({"profile":{
            "margins":{"left":85.05,"right":42.55,"top":56.7,"bottom":56.7},
            "normal":{"font":"Times New Roman","size":14.0,"align":"justify","indentFirst":35.45,"lineSpacing":1.5},
            "headings":{"levels":2,"numbering":false,"indentFirst":35.45},
            "normalizeCaptions":true
        }}),
    )
    .unwrap();
    let st = s.doc.styles.get("Normal").unwrap();
    assert_eq!(st.chr.font.as_deref(), Some("Times New Roman"));
    assert_eq!(st.para.indent_first, Some(35.45));
    let cap = s.doc.para(StoryRef::Body, &Path::top(2)).unwrap();
    assert_eq!(cap.props.style.as_deref(), Some("Caption"));
    assert_eq!(cap.props.align, Some(Align::Center));
}

#[test]
fn apply_ir_patch_insert_delete_move_set_cell() {
    let mut s = sample();
    s.run(
        "doc.applyIrPatch",
        &json!({"ops":[
            {"op":"insert","index":1,"block":{"type":"paragraph","text":"Mid","style":"Normal"}},
            {"op":"setStyle","index":0,"style":"Heading1"},
            {"op":"replaceIn","index":2,"find":"World","with":"Earth"}
        ]}),
    )
    .unwrap();
    assert_eq!(s.doc.para(StoryRef::Body, &Path::top(0)).unwrap().props.style.as_deref(), Some("Heading1"));
    assert_eq!(s.doc.para(StoryRef::Body, &Path::top(1)).unwrap().plain_text(), "Mid");
    assert_eq!(s.doc.para(StoryRef::Body, &Path::top(2)).unwrap().plain_text(), "Earth");

    s.run(
        "doc.applyIrPatch",
        &json!({"ops":[
            {"op":"insert","block":{"type":"table","rows":2,"cols":2,"cells":[["a","b"],["c","d"]]}},
            {"op":"setCell","index":4,"row":0,"col":1,"text":"B2"}
        ]}),
    )
    .unwrap();
    let ir = s.run("doc.getIr", &json!({"kinds":["table"]})).unwrap();
    let tables = ir["blocks"].as_array().unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0]["cells"][0][1], "B2");

    s.run("doc.applyIrPatch", &json!({"ops":[{"op":"delete","index":1}]})).unwrap();
    assert!(!s.doc.plain_text(StoryRef::Body).contains("Mid"));
}

#[test]
fn apply_ir_patch_delete_range_and_table_values() {
    let mut s = sample();
    s.run(
        "doc.applyIrPatch",
        &json!({"ops":[
            {"op":"insert","index":1,"block":{"type":"paragraph","text":"A"}},
            {"op":"insert","index":2,"block":{"type":"paragraph","text":"B"}},
            {"op":"insert","index":3,"block":{"type":"paragraph","text":"C"}}
        ]}),
    )
    .unwrap();
    s.run("doc.applyIrPatch", &json!({"ops":[{"op":"delete","from":1,"to":3}]})).unwrap();
    let body = s.doc.plain_text(StoryRef::Body);
    assert!(!body.contains('A') && !body.contains('B'), "{body}");
    assert!(body.contains('C') || body.contains("Hello") || body.contains("World"), "{body}");

    s.run(
        "doc.applyIrPatch",
        &json!({"ops":[{"op":"insert","block":{"type":"table","values":[["H1","H2"],["a","b"]],"header":true}}]}),
    )
    .unwrap();
    let ir = s.run("doc.getIr", &json!({"kinds":["table"]})).unwrap();
    let table = &ir["blocks"].as_array().unwrap()[0];
    assert_eq!(table["cells"][0][0], "H1");
    assert_eq!(table["cells"][1][1], "b");
}

#[test]
fn apply_layout_caption_not_italic_by_default() {
    let mut s = sample();
    s.run(
        "doc.applyLayout",
        &json!({"profile":{"caption":{"font":"Times New Roman","size":14.0,"figureAlign":"center"}}}),
    )
    .unwrap();
    let st = s.doc.styles.get("Caption").unwrap();
    assert_eq!(st.chr.italic, Some(false));
    assert_eq!(st.chr.size, Some(14.0));
}

#[test]
fn insert_toc_does_not_replace_a_range_selection() {
    let mut d = Document::new();
    d.body.clear();
    d.body.push(Arc::new(Block::Para(Paragraph::with_text("Keep me", CharProps::default()))));
    d.body.push(Arc::new(Block::Para(Paragraph::with_text("Chapter", CharProps::default()).styled("Heading1"))));
    let mut s = Session::new(d);
    s.run("doc.ensureHeading", &json!({"levels": 1, "numbering": false})).unwrap();
    // Select the whole first paragraph — Word insert would delete it; agent TOC must keep it.
    s.sel = crate::Selection {
        anchor: wordcraft_doc::Pos::body(0, 0),
        focus: wordcraft_doc::Pos::body(0, "Keep me".len()),
    };
    s.run("doc.insertToc", &json!({"levels": 1, "title": "Contents"})).unwrap();
    assert!(s.doc.plain_text(StoryRef::Body).contains("Keep me"), "{}", s.doc.plain_text(StoryRef::Body));
}

#[test]
fn ensure_styles_upserts() {
    let mut s = sample();
    s.run(
        "doc.ensureStyles",
        &json!({"styles":[{"id":"Quote","para":{"align":"center","indentLeft":36.0},"chr":{"italic":true,"size":12.0}}]}),
    )
    .unwrap();
    let st = s.doc.styles.get("Quote").unwrap();
    assert_eq!(st.para.align, Some(Align::Center));
    assert_eq!(st.chr.italic, Some(true));
}

#[test]
fn no_apply_vkr_layout_command() {
    let mut s = sample();
    assert!(s.run("doc.applyVkrLayout", &json!({})).is_err());
}

#[test]
fn insert_toc_and_update_fields_smoke() {
    let mut d = Document::new();
    d.body.clear();
    d.body.push(Arc::new(Block::Para(Paragraph::with_text("Intro", CharProps::default()).styled("Heading1"))));
    d.body.push(Arc::new(Block::Para(Paragraph::with_text("Body", CharProps::default()))));
    let mut s = Session::new(d);
    s.run("doc.ensureHeading", &json!({"levels": 2, "numbering": false})).unwrap();
    s.sel = crate::Selection::caret(s.doc.start_of(StoryRef::Body));
    s.run("doc.insertToc", &json!({"levels": 2, "title": "Contents"})).unwrap();
    s.run("doc.updateFields", &json!({"all": true})).unwrap();
    let ir = s.run("doc.getIr", &json!({})).unwrap();
    assert!(ir["blocks"].as_array().unwrap().len() >= 2);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn export_sync_ir_folder_round_trip() {
    let dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target"))
        .join(format!("wc-ir-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = sample();
    s.run(
        "doc.ensureStyles",
        &json!({"styles":[{"id":"Caption","para":{"align":"center","indentFirst":0.0},"chr":{"italic":false,"size":14.0}}]}),
    )
    .unwrap();
    let exported = s.run("doc.exportIr", &json!({"path": dir.to_string_lossy()})).unwrap();
    assert!(exported["blocks"].as_u64().unwrap() >= 3);
    let styles: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("styles.json")).unwrap()).unwrap();
    assert!(styles["styles"].as_array().unwrap().iter().any(|st| st["id"] == "Normal"));
    assert!(styles["styles"].as_array().unwrap().iter().any(|st| st["id"] == "Caption"));

    let mut styles = styles;
    for st in styles["styles"].as_array_mut().unwrap() {
        if st["id"] == "Caption" {
            st["chr"]["italic"] = json!(false);
            st["chr"]["size"] = json!(14.0);
        }
        if st["id"] == "Heading2" || st["id"] == "Normal" {
            st["para"]["spaceAfter"] = json!(8.0);
        }
    }
    std::fs::write(dir.join("styles.json"), serde_json::to_string_pretty(&styles).unwrap()).unwrap();

    let ir = s.run("doc.getIr", &json!({})).unwrap();
    let id0 = ir["blocks"][0]["id"].as_str().unwrap().to_string();
    s.run(
        "doc.applyIrPatch",
        &json!({"ops":[{"op":"update","id":id0,"set":{"text":"Patched by id"}}]}),
    )
    .unwrap();
    assert!(s.doc.plain_text(StoryRef::Body).contains("Patched by id"));

    s.run("doc.exportIr", &json!({"path": dir.to_string_lossy()})).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    let rel = manifest["sectionOrder"][0].as_str().unwrap();
    let section = dir.join(rel);
    let mut blocks: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&section).unwrap()).unwrap();
    if let Some(b0) = blocks.as_array_mut().and_then(|a| a.get_mut(0)) {
        b0["text"] = json!("From disk");
        b0["style"] = json!("Normal");
        b0["align"] = json!("center");
    }
    std::fs::write(&section, serde_json::to_string_pretty(&blocks).unwrap()).unwrap();

    s.run("doc.syncIr", &json!({"path": dir.to_string_lossy()})).unwrap();
    assert!(s.doc.plain_text(StoryRef::Body).contains("From disk"));
    let path = Path::top(0);
    let para = s.doc.para(StoryRef::Body, &path).unwrap();
    assert_eq!(para.props.align, Some(Align::Center));
    let cap = s.doc.styles.get("Caption").unwrap();
    assert_eq!(cap.chr.italic, Some(false));
    assert_eq!(cap.chr.size, Some(14.0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn ir_path_reports_folder() {
    let mut s = sample();
    let v = s.run("doc.irPath", &json!({})).unwrap();
    assert!(v["path"].as_str().unwrap().contains("ir"));
}
