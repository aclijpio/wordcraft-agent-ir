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
