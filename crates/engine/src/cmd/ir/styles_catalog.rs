//! Agent styles catalog (`styles.json`) ↔ document StyleSheet.

use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;
use wordcraft_doc::props::{Align, CharProps, LineSpacing, ParaProps};
use wordcraft_doc::styles::StyleKind;

use super::parse_align;
use crate::{CmdError, Session, p};

const STANDARD: &[&str] = &[
    "Normal",
    "Heading1",
    "Heading2",
    "Heading3",
    "Heading4",
    "Heading5",
    "Heading6",
    "Caption",
    "TableCaption",
    "TOCHeading",
    "TOC1",
    "TOC2",
    "TOC3",
];

pub(crate) fn export_styles_json(s: &Session) -> Value {
    let mut ids: BTreeSet<String> = STANDARD.iter().map(|x| (*x).to_string()).collect();
    for b in &s.doc.body {
        match &**b {
            wordcraft_doc::Block::Para(para) => {
                if let Some(st) = para.props.style.as_ref() {
                    ids.insert(st.clone());
                }
            }
            wordcraft_doc::Block::Table(t) => {
                if let Some(st) = t.props.style.as_ref() {
                    ids.insert(st.clone());
                }
            }
        }
    }
    let mut styles = Vec::new();
    for id in ids {
        let Some(st) = s.doc.styles.get(&id) else { continue };
        if st.kind != StyleKind::Paragraph && st.kind != StyleKind::Table {
            continue;
        }
        styles.push(style_to_catalog(st));
    }
    json!({ "version": 3, "styles": styles })
}

pub(crate) fn apply_styles_json(s: &mut Session, catalog: &Value) -> Result<(), CmdError> {
    let styles = catalog.get("styles").cloned().unwrap_or(json!([]));
    let _ = super::layout::ensure_styles(s, &json!({ "styles": styles }))?;
    Ok(())
}

pub(crate) fn load_styles_file(path: &Path) -> Result<Value, CmdError> {
    let bytes = crate::io::read_file(&path.to_string_lossy()).map_err(CmdError::Failed)?;
    let text = String::from_utf8(bytes).map_err(|e| CmdError::Failed(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| CmdError::Failed(format!("styles.json: {e}")))
}

pub(crate) fn save_styles_file(path: &Path, catalog: &Value) -> Result<(), CmdError> {
    let text = serde_json::to_string_pretty(catalog).map_err(|e| CmdError::Failed(e.to_string()))?;
    write_text(path, &text)
}

pub(crate) fn write_text(path: &Path, text: &str) -> Result<(), CmdError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CmdError::Failed(e.to_string()))?;
    }
    std::fs::write(path, text.as_bytes()).map_err(|e| CmdError::Failed(e.to_string()))
}

fn style_to_catalog(st: &wordcraft_doc::styles::Style) -> Value {
    let mut o = json!({
        "id": st.id,
        "name": st.name,
        "basedOn": st.based_on,
        "para": para_to_catalog(&st.para),
        "chr": chr_to_catalog(&st.chr),
    });
    if st.kind == StyleKind::Table {
        o["kind"] = json!("table");
    }
    o
}

fn para_to_catalog(p: &ParaProps) -> Value {
    let mut o = json!({});
    if let Some(a) = p.align {
        o["align"] = json!(align_name(a));
    }
    if let Some(v) = p.indent_first {
        o["indentFirst"] = json!(v);
    }
    if let Some(v) = p.indent_left {
        o["indentLeft"] = json!(v);
    }
    if let Some(v) = p.indent_right {
        o["indentRight"] = json!(v);
    }
    if let Some(v) = p.space_before {
        o["spaceBefore"] = json!(v);
    }
    if let Some(v) = p.space_after {
        o["spaceAfter"] = json!(v);
    }
    if let Some(LineSpacing::Multiple(v)) = p.line_spacing {
        o["lineSpacing"] = json!(v);
    } else if let Some(ls) = p.line_spacing {
        o["lineSpacing"] = serde_json::to_value(ls).unwrap_or(Value::Null);
    }
    if let Some(v) = p.page_break_before {
        o["pageBreakBefore"] = json!(v);
    }
    o
}

fn chr_to_catalog(c: &CharProps) -> Value {
    let mut o = json!({});
    if let Some(f) = &c.font {
        o["font"] = json!(f);
    }
    if let Some(v) = c.size {
        o["size"] = json!(v);
    }
    if let Some(v) = c.bold {
        o["bold"] = json!(v);
    }
    if let Some(v) = c.italic {
        o["italic"] = json!(v);
    }
    o
}

fn align_name(a: Align) -> &'static str {
    match a {
        Align::Center => "center",
        Align::Right => "right",
        Align::Justify => "justify",
        Align::Distribute => "distribute",
        Align::Left => "left",
    }
}

pub(crate) fn merge_block_overrides(para: &mut wordcraft_doc::Paragraph, block: &Value) -> Result<(), CmdError> {
    if let Some(a) = p::str(block, "align") {
        para.props.align = Some(parse_align(a)?);
    }
    if let Some(f) = p::f32(block, "indentFirst") {
        para.props.indent_first = Some(f.clamp(-1584.0, 1584.0));
    }
    if let Some(f) = p::f32(block, "indentLeft") {
        para.props.indent_left = Some(f.clamp(0.0, 1584.0));
    }
    if let Some(f) = p::f32(block, "indentRight") {
        para.props.indent_right = Some(f.clamp(0.0, 1584.0));
    }
    if let Some(v) = p::f32(block, "lineSpacing") {
        para.props.line_spacing = Some(LineSpacing::Multiple(v.clamp(0.0, 132.0)));
    }
    if let Some(on) = p::bool(block, "pageBreakBefore") {
        para.props.page_break_before = Some(on);
    }
    if let Some(v) = p::f32(block, "spaceBefore") {
        para.props.space_before = Some(v);
    }
    if let Some(v) = p::f32(block, "spaceAfter") {
        para.props.space_after = Some(v);
    }
    Ok(())
}
