//! Agent document IR: slim inspect + high-level apply commands.

use serde_json::{Value, json};
use std::sync::Arc;
use wordcraft_doc::numbering::Level;
use wordcraft_doc::props::{Align, CharProps, LineSpacing, NumRef, ParaProps, Rgb, TextColor};
use wordcraft_doc::section::NumFormat;
use wordcraft_doc::{Block, Path, StoryRef};

use super::page::{margins_fit, with_sect};
use super::para::{ensure_heading_style, touch_all};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

const MAX_IR_BLOCKS: usize = 2000;
const MAX_PATCH_OPS: usize = 500;
const FIRST_LINE_PT: f32 = 35.45;
const MARGIN_LEFT: f32 = 85.05;
const MARGIN_RIGHT: f32 = 42.55;
const MARGIN_TOP: f32 = 56.7;
const MARGIN_BOTTOM: f32 = 56.7;
const A4_W: f32 = 595.28;
const A4_H: f32 = 841.89;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("doc.getIr", "Document IR", "Agents", get_ir)
            .params(r#"{"from"?: n, "to"?: n, "fields"?: [string], "pageBreaksOnly"?: bool}"#)
            .pure(),
        CommandSpec::new("doc.applyIrPatch", "Apply IR Patch", "Agents", apply_ir_patch)
            .params(
                r#"{"ops":[{"op":"update"|"replace"|"replaceIn","index":n,"set"?:{…},"block"?:{…},"find"?:string,"with"?:string,"occurrence"?:n}]}"#,
            ),
        CommandSpec::new("doc.applyBodyStyle", "Apply Body Style", "Agents", apply_body_style)
            .params(r#"{}"#),
        CommandSpec::new("doc.ensureHeading", "Ensure Heading Styles", "Agents", ensure_heading)
            .params(r#"{"levels"?: 1-9 (default 3)}"#),
        CommandSpec::new("doc.clearStrayPageBreaks", "Clear Stray Page Breaks", "Agents", clear_stray_page_breaks)
            .params(r#"{}"#),
        CommandSpec::new("doc.applyVkrLayout", "Apply VKR Layout", "Agents", apply_vkr_layout)
            .params(r#"{"levels"?: 1-9 (default 3)}"#),
    ]
}

struct IrFilter {
    from: usize,
    to: usize,
    page_breaks_only: bool,
    fields: Option<Vec<String>>,
}

fn parse_filter(v: &Value) -> IrFilter {
    let from = p::u64(v, "from").unwrap_or(0) as usize;
    let to = p::u64(v, "to").map(|x| x as usize).unwrap_or(usize::MAX);
    let page_breaks_only = p::bool(v, "pageBreaksOnly").unwrap_or(false);
    let fields = v.get("fields").and_then(Value::as_array).map(|a| {
        a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>()
    });
    IrFilter { from, to, page_breaks_only, fields }
}

fn get_ir(s: &mut Session, v: &Value) -> CmdResult {
    Ok(build_ir(s, &parse_filter(v)))
}

fn build_ir(s: &mut Session, filter: &IrFilter) -> Value {
    let mut blocks = Vec::new();
    let end = s.doc.body.len().min(MAX_IR_BLOCKS).saturating_sub(1).min(filter.to);
    let start = filter.from.min(end.saturating_add(1));
    for i in start..=end {
        let Some(b) = s.doc.body.get(i) else { break };
        match &**b {
            Block::Para(p) => {
                let style = p.props.style.clone().unwrap_or_else(|| "Normal".into());
                let resolved = s.doc.styles.resolve_para(&p.props);
                let page_break_before = p.props.page_break_before.unwrap_or(resolved.page_break_before);
                let is_heading = style.starts_with("Heading");
                if filter.page_breaks_only && !page_break_before && !is_heading {
                    continue;
                }
                let text = p.plain_text();
                let mut o = json!({
                    "index": i,
                    "type": "paragraph",
                    "style": style,
                    "text": text,
                    "align": match resolved.align {
                        Align::Center => "center",
                        Align::Right => "right",
                        Align::Justify => "justify",
                        Align::Distribute => "distribute",
                        Align::Left => "left",
                    },
                    "indentFirst": resolved.indent_first,
                    "indentLeft": resolved.indent_left,
                    "indentRight": resolved.indent_right,
                    "pageBreakBefore": page_break_before,
                });
                if is_figure_caption(&text) {
                    o["role"] = json!("figureCaption");
                } else if is_table_caption(&text) {
                    o["role"] = json!("tableCaption");
                } else if is_heading1(s, &p.props) || style.starts_with("Heading") {
                    o["role"] = json!("heading");
                }
                match p.props.line_spacing.unwrap_or(resolved.line_spacing) {
                    LineSpacing::Multiple(v) => o["lineSpacing"] = json!(v),
                    other => o["lineSpacing"] = serde_json::to_value(other).unwrap_or(Value::Null),
                }
                if let Some(n) = p.props.numbering.or(resolved.numbering).filter(|n| n.num != 0) {
                    o["numbering"] = json!({"num": n.num, "level": n.level});
                    o["listLevel"] = json!(n.level);
                }
                if let Some(fields) = &filter.fields {
                    o = keep_fields(o, fields);
                }
                blocks.push(o);
            }
            Block::Table(t) => {
                if filter.page_breaks_only {
                    continue;
                }
                let cells: Vec<Vec<String>> = t
                    .rows
                    .iter()
                    .map(|r| {
                        r.cells
                            .iter()
                            .map(|c| c.blocks.iter().filter_map(|b| b.as_para().map(|p| p.plain_text())).collect::<Vec<_>>().join("\n"))
                            .collect()
                    })
                    .collect();
                let mut o = json!({
                    "index": i,
                    "type": "table",
                    "rows": t.rows.len(),
                    "cols": t.cols(),
                    "style": t.props.style,
                    "cells": cells
                });
                if let Some(fields) = &filter.fields {
                    o = keep_fields(o, fields);
                }
                blocks.push(o);
            }
        }
    }
    let pages = s.layout().pages.len();
    let sections: Vec<Value> = s
        .doc
        .sections()
        .iter()
        .map(|(end, sp)| {
            json!({
                "endBlock": end,
                "margins": {
                    "top": sp.margin_top,
                    "bottom": sp.margin_bottom,
                    "left": sp.margin_left,
                    "right": sp.margin_right
                },
                "page": {"w": sp.page_w, "h": sp.page_h}
            })
        })
        .collect();
    json!({
        "version": 1,
        "pages": pages,
        "blocks": blocks,
        "sections": sections,
    })
}

fn keep_fields(mut o: Value, fields: &[String]) -> Value {
    let Some(map) = o.as_object_mut() else { return o };
    let keep: std::collections::HashSet<&str> =
        fields.iter().map(String::as_str).chain(["index", "type"]).collect();
    map.retain(|k, _| keep.contains(k.as_str()));
    o
}

fn full_ir(s: &mut Session) -> Value {
    build_ir(s, &IrFilter { from: 0, to: usize::MAX, page_breaks_only: false, fields: None })
}

fn apply_ir_patch(s: &mut Session, v: &Value) -> CmdResult {
    let ops = v.get("ops").and_then(Value::as_array).ok_or_else(|| CmdError::Params("missing `ops` array".into()))?;
    if ops.len() > MAX_PATCH_OPS {
        return Err(CmdError::Params(format!("at most {MAX_PATCH_OPS} ops")));
    }
    let mut changed = 0u32;
    for (i, op) in ops.iter().enumerate() {
        let kind = p::str(op, "op").ok_or_else(|| CmdError::Params(format!("ops[{i}]: missing `op`")))?;
        let index = p::u64(op, "index").ok_or_else(|| CmdError::Params(format!("ops[{i}]: missing `index`")))? as usize;
        if index >= s.doc.body.len() {
            return Err(CmdError::Params(format!("ops[{i}]: index {index} out of range")));
        }
        match kind {
            "update" => {
                let set = op.get("set").cloned().unwrap_or(json!({}));
                apply_update(s, index, &set)?;
                changed = changed.saturating_add(1);
            }
            "replace" => {
                let block = op.get("block").ok_or_else(|| CmdError::Params(format!("ops[{i}]: replace needs `block`")))?;
                apply_replace(s, index, block)?;
                changed = changed.saturating_add(1);
            }
            "replaceIn" => {
                let find = p::str(op, "find").ok_or_else(|| CmdError::Params(format!("ops[{i}]: replaceIn needs `find`")))?;
                let with = p::str(op, "with").ok_or_else(|| CmdError::Params(format!("ops[{i}]: replaceIn needs `with`")))?;
                let occurrence = p::u64(op, "occurrence").unwrap_or(1).max(1) as usize;
                replace_in_paragraph(s, index, find, with, occurrence)?;
                changed = changed.saturating_add(1);
            }
            other => return Err(CmdError::Params(format!("ops[{i}]: unknown op `{other}`"))),
        }
    }
    let mut ir = full_ir(s);
    if let Some(o) = ir.as_object_mut() {
        o.insert("changed".into(), json!(changed));
    }
    Ok(ir)
}

fn apply_update(s: &mut Session, index: usize, set: &Value) -> Result<(), CmdError> {
    let path = Path::top(index);
    if s.doc.para(StoryRef::Body, &path).is_none() {
        return Err(CmdError::Params(format!("block {index} is not a paragraph")));
    }
    if let Some(text) = p::str(set, "text") {
        let para = s.doc.para(StoryRef::Body, &path).ok_or_else(|| CmdError::Params(format!("block {index} is not a paragraph")))?;
        let mark = para.mark.clone();
        let props = para.props.clone();
        let section = para.section.clone();
        let para = s.doc.para_mut(StoryRef::Body, &path)?;
        let len = para.len();
        if len > 0 {
            para.delete(0, len).map_err(|e| CmdError::Failed(e.to_string()))?;
        }
        if !text.is_empty() {
            para.insert_text(0, text, &mark).map_err(|e| CmdError::Failed(e.to_string()))?;
        }
        para.props = props;
        para.section = section;
        para.touch();
    }
    if let Some(ri) = set.get("replaceIn") {
        let find = p::str(ri, "find").ok_or_else(|| CmdError::Params("set.replaceIn.find required".into()))?;
        let with = p::str(ri, "with").ok_or_else(|| CmdError::Params("set.replaceIn.with required".into()))?;
        let occurrence = p::u64(ri, "occurrence").unwrap_or(1).max(1) as usize;
        replace_in_paragraph(s, index, find, with, occurrence)?;
    }
    let para = s.doc.para_mut(StoryRef::Body, &path)?;
    if let Some(style) = p::str(set, "style") {
        para.props.style = Some(style.to_string());
        para.props.outline_level = None;
    }
    if let Some(a) = p::str(set, "align") {
        para.props.align = Some(parse_align(a)?);
    }
    if let Some(f) = p::f32(set, "indentFirst") {
        para.props.indent_first = Some(f.clamp(-1584.0, 1584.0));
        para.props.cancel_char_indents(false, false, true);
    }
    if let Some(f) = p::f32(set, "indentLeft") {
        para.props.indent_left = Some(f.clamp(0.0, 1584.0));
        para.props.cancel_char_indents(true, false, false);
    }
    if let Some(f) = p::f32(set, "indentRight") {
        para.props.indent_right = Some(f.clamp(0.0, 1584.0));
        para.props.cancel_char_indents(false, true, false);
    }
    if let Some(v) = p::f32(set, "lineSpacing") {
        para.props.line_spacing = Some(LineSpacing::Multiple(v.clamp(0.0, 132.0)));
    } else if let Some(ls) = set.get("lineSpacing").filter(|x| x.is_object()) {
        para.props.line_spacing = Some(serde_json::from_value(ls.clone()).map_err(|e| CmdError::Params(e.to_string()))?);
    }
    if let Some(on) = p::bool(set, "pageBreakBefore") {
        para.props.page_break_before = Some(on);
    }
    if let Some(level) = p::u64(set, "listLevel") {
        if let Some(n) = para.props.numbering.filter(|n| n.num != 0) {
            para.props.numbering = Some(NumRef { num: n.num, level: (level as u8).min(8) });
        }
    }
    if let Some(num) = set.get("numbering") {
        if num.is_null() {
            para.props.numbering = Some(NumRef { num: 0, level: 0 });
        } else {
            let n = p::u64(num, "num").ok_or_else(|| CmdError::Params("numbering.num required".into()))? as u32;
            let level = p::u64(num, "level").unwrap_or(0).min(8) as u8;
            para.props.numbering = Some(NumRef { num: n, level });
        }
    }
    para.touch();
    Ok(())
}

fn replace_in_paragraph(s: &mut Session, index: usize, find: &str, with: &str, occurrence: usize) -> Result<(), CmdError> {
    if find.is_empty() {
        return Err(CmdError::Params("`find` must not be empty".into()));
    }
    if with.contains('\u{FFFC}') {
        return Err(CmdError::Params("`with` must not contain object placeholders".into()));
    }
    let path = Path::top(index);
    let Some(para) = s.doc.para(StoryRef::Body, &path) else {
        return Err(CmdError::Params(format!("block {index} is not a paragraph")));
    };
    let text = para.plain_text();
    let start = find_nth(&text, find, occurrence).ok_or_else(|| {
        CmdError::Params(format!("occurrence {occurrence} of `find` not in paragraph {index}"))
    })?;
    let end = start.saturating_add(find.len());
    if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return Err(CmdError::Params("replace range is not on a char boundary".into()));
    }
    let mark = para.props_at(start).clone();
    let para = s.doc.para_mut(StoryRef::Body, &path)?;
    para.delete(start, end).map_err(|e| CmdError::Failed(e.to_string()))?;
    if !with.is_empty() {
        para.insert_text(start, with, &mark).map_err(|e| CmdError::Failed(e.to_string()))?;
    }
    para.touch();
    Ok(())
}

fn find_nth(haystack: &str, needle: &str, occurrence: usize) -> Option<usize> {
    let mut from = 0usize;
    let mut seen = 0usize;
    while from <= haystack.len() {
        let Some(rel) = haystack.get(from..)?.find(needle) else {
            return None;
        };
        let at = from.saturating_add(rel);
        seen = seen.saturating_add(1);
        if seen == occurrence {
            return Some(at);
        }
        from = at.saturating_add(needle.len().max(1));
    }
    None
}

fn is_figure_caption(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("Рисунок ") || t.starts_with("Рис. ")
}

fn is_table_caption(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("Таблица ") || t.starts_with("Табл. ")
}

fn apply_replace(s: &mut Session, index: usize, block: &Value) -> Result<(), CmdError> {
    let ty = p::str(block, "type").unwrap_or("paragraph");
    if ty != "paragraph" {
        return Err(CmdError::Params("v1 replace only supports paragraph blocks".into()));
    }
    let path = Path::top(index);
    if s.doc.para(StoryRef::Body, &path).is_none() {
        return Err(CmdError::Params(format!("block {index} is not a paragraph")));
    }
    let text = p::str(block, "text").unwrap_or("");
    let style = p::str(block, "style").unwrap_or("Normal");
    let mut set = block.clone();
    if let Some(o) = set.as_object_mut() {
        o.insert("text".into(), json!(text));
        o.insert("style".into(), json!(style));
    }
    apply_update(s, index, &set)
}

fn parse_align(a: &str) -> Result<Align, CmdError> {
    Ok(match a {
        "left" => Align::Left,
        "center" => Align::Center,
        "right" => Align::Right,
        "justify" | "both" => Align::Justify,
        "distribute" => Align::Distribute,
        x => return Err(CmdError::Params(format!("unknown alignment `{x}`"))),
    })
}

fn apply_body_style(s: &mut Session, _: &Value) -> CmdResult {
    let cleared = apply_body_style_inner(s)?;
    let mut ir = full_ir(s);
    if let Some(o) = ir.as_object_mut() {
        o.insert("clearedPageBreaks".into(), json!(cleared));
    }
    Ok(ir)
}

fn apply_body_style_inner(s: &mut Session) -> Result<u32, CmdError> {
    let (t, b, l, r) = (MARGIN_TOP, MARGIN_BOTTOM, MARGIN_LEFT, MARGIN_RIGHT);
    if !margins_fit((A4_W, A4_H), (t, b, l, r), 0.0) {
        return Err(CmdError::Params("margins leave no room for text".into()));
    }
    with_sect(s, |x| {
        let landscape = x.landscape;
        if landscape {
            x.page_w = A4_H;
            x.page_h = A4_W;
        } else {
            x.page_w = A4_W;
            x.page_h = A4_H;
        }
        x.margin_top = t;
        x.margin_bottom = b;
        x.margin_left = l;
        x.margin_right = r;
    })?;

    let id = "Normal".to_string();
    if s.doc.styles.get(&id).is_none() {
        if let Some(st) = wordcraft_doc::styles::StyleSheet::builtin().get(&id).cloned() {
            Arc::make_mut(&mut s.doc.styles).upsert(st);
        }
    }
    let st = Arc::make_mut(&mut s.doc.styles).get_mut(&id).ok_or_else(|| CmdError::Params("no Normal style".into()))?;
    st.chr.overlay(&CharProps {
        font: Some("Times New Roman".into()),
        size: Some(14.0),
        bold: Some(false),
        color: Some(TextColor::Rgb(Rgb::BLACK)),
        ..CharProps::default()
    });
    st.para.overlay(&ParaProps {
        align: Some(Align::Justify),
        indent_left: Some(0.0),
        indent_right: Some(0.0),
        indent_first: Some(FIRST_LINE_PT),
        line_spacing: Some(LineSpacing::Multiple(1.5)),
        page_break_before: Some(false),
        space_before: Some(0.0),
        space_after: Some(0.0),
        ..ParaProps::default()
    });
    ensure_caption_style(s)?;
    touch_all(s);
    normalize_special_paras(s)?;
    clear_stray_inner(s)
}

fn ensure_caption_style(s: &mut Session) -> Result<(), CmdError> {
    let id = "Caption".to_string();
    if s.doc.styles.get(&id).is_none() {
        if let Some(st) = wordcraft_doc::styles::StyleSheet::builtin().get(&id).cloned() {
            Arc::make_mut(&mut s.doc.styles).upsert(st);
        }
    }
    let st = Arc::make_mut(&mut s.doc.styles).get_mut(&id).ok_or_else(|| CmdError::Params("no Caption style".into()))?;
    st.chr.overlay(&CharProps {
        font: Some("Times New Roman".into()),
        size: Some(14.0),
        italic: Some(false),
        bold: Some(false),
        color: Some(TextColor::Rgb(Rgb::BLACK)),
        ..CharProps::default()
    });
    st.para.overlay(&ParaProps {
        align: Some(Align::Center),
        indent_left: Some(0.0),
        indent_right: Some(0.0),
        indent_first: Some(0.0),
        line_spacing: Some(LineSpacing::Multiple(1.5)),
        space_before: Some(0.0),
        space_after: Some(0.0),
        page_break_before: Some(false),
        ..ParaProps::default()
    });
    Ok(())
}

fn normalize_special_paras(s: &mut Session) -> Result<(), CmdError> {
    for i in 0..s.doc.body.len() {
        let path = Path::top(i);
        let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
        let text = p.plain_text();
        let style = p.props.style.clone().unwrap_or_else(|| "Normal".into());
        let heading = style.starts_with("Heading") || is_heading1(s, &p.props);
        if is_figure_caption(&text) {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.style = Some("Caption".into());
            para.props.align = Some(Align::Center);
            para.props.indent_left = Some(0.0);
            para.props.indent_right = Some(0.0);
            para.props.indent_first = Some(0.0);
            para.props.numbering = Some(NumRef { num: 0, level: 0 });
            para.props.cancel_char_indents(true, true, true);
            para.touch();
        } else if is_table_caption(&text) {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.style = Some("Caption".into());
            para.props.align = Some(Align::Left);
            para.props.indent_left = Some(0.0);
            para.props.indent_right = Some(0.0);
            para.props.indent_first = Some(0.0);
            para.props.numbering = Some(NumRef { num: 0, level: 0 });
            para.props.cancel_char_indents(true, true, true);
            para.touch();
        } else if heading {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.align = Some(Align::Left);
            para.props.indent_left = None;
            para.props.indent_right = None;
            para.props.indent_first = None;
            para.props.cancel_char_indents(true, true, true);
            para.touch();
        } else if style == "Normal" {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            if para.props.indent_left.is_some_and(|v| v > 0.0) {
                para.props.indent_left = Some(0.0);
            }
            para.props.cancel_char_indents(true, false, false);
            para.touch();
        }
    }
    Ok(())
}

fn ensure_heading(s: &mut Session, v: &Value) -> CmdResult {
    ensure_heading_inner(s, p::u64(v, "levels").unwrap_or(3).clamp(1, 9) as u8)?;
    Ok(full_ir(s))
}

fn ensure_heading_inner(s: &mut Session, levels: u8) -> Result<(), CmdError> {
    let chr = CharProps {
        font: Some("Times New Roman".into()),
        size: Some(14.0),
        bold: Some(true),
        color: Some(TextColor::Rgb(Rgb::BLACK)),
        ..CharProps::default()
    };
    let para_base = ParaProps {
        align: Some(Align::Left),
        indent_left: Some(0.0),
        indent_right: Some(0.0),
        indent_first: Some(FIRST_LINE_PT),
        line_spacing: Some(LineSpacing::Multiple(1.5)),
        space_before: Some(0.0),
        space_after: Some(0.0),
        ..ParaProps::default()
    };
    for n in 1..=levels {
        let id = ensure_heading_style(s, n);
        let st = Arc::make_mut(&mut s.doc.styles).get_mut(&id).ok_or_else(|| CmdError::Params(format!("no style `{id}`")))?;
        st.chr.overlay(&chr);
        let mut pp = para_base.clone();
        pp.page_break_before = Some(n == 1);
        st.para.overlay(&pp);
    }
    touch_all(s);
    link_heading_numbering(s)?;

    for i in 0..s.doc.body.len() {
        let path = Path::top(i);
        let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
        let style = p.props.style.clone().unwrap_or_default();
        let h1 = is_heading1(s, &p.props);
        let heading = h1 || style.starts_with("Heading");
        if heading {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.align = Some(Align::Left);
            para.props.indent_left = None;
            para.props.indent_right = None;
            para.props.indent_first = None;
            para.props.cancel_char_indents(true, true, true);
            let want_break = h1;
            if para.props.page_break_before != Some(want_break) {
                para.props.page_break_before = Some(want_break);
            }
            para.touch();
        } else if p.props.page_break_before == Some(true) {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.page_break_before = Some(false);
            para.touch();
        }
    }
    Ok(())
}

fn link_heading_numbering(s: &mut Session) -> Result<(), CmdError> {
    let levels: Vec<Level> = (0..crate::cmd::lists::LEVELS)
        .map(|i| {
            let n = i + 1;
            let text = (1..=n).map(|k| format!("%{k}")).collect::<Vec<_>>().join(".");
            Level {
                format: NumFormat::Decimal,
                text,
                indent: FIRST_LINE_PT,
                hanging: 0.0,
                suffix: wordcraft_doc::numbering::LevelSuffix::Space,
                style: Some(format!("Heading{n}")),
                ..Level::default()
            }
        })
        .collect();
    let num = Arc::make_mut(&mut s.doc.numbering).try_add_levels(None, levels).map_err(|e| CmdError::Failed(e.to_string()))?;
    let mut linked = Vec::new();
    for n in 1..=crate::cmd::lists::LEVELS as u8 {
        let id = ensure_heading_style(s, n);
        if let Some(st) = Arc::make_mut(&mut s.doc.styles).get_mut(&id) {
            st.para.numbering = Some(NumRef { num, level: n - 1 });
            linked.push(id);
        }
    }
    for i in 0..s.doc.body.len() {
        let path = Path::top(i);
        let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
        if p.props.style.as_ref().is_some_and(|st| linked.contains(st)) {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.numbering = None;
            para.touch();
        }
    }
    Ok(())
}

fn clear_stray_page_breaks(s: &mut Session, _: &Value) -> CmdResult {
    let cleared = clear_stray_inner(s)?;
    let mut ir = full_ir(s);
    if let Some(o) = ir.as_object_mut() {
        o.insert("cleared".into(), json!(cleared));
    }
    Ok(ir)
}

fn clear_stray_inner(s: &mut Session) -> Result<u32, CmdError> {
    let mut cleared = 0u32;
    for i in 0..s.doc.body.len() {
        let path = Path::top(i);
        let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
        if is_heading1(s, &p.props) {
            continue;
        }
        if p.props.page_break_before == Some(true) {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.page_break_before = Some(false);
            para.touch();
            cleared = cleared.saturating_add(1);
        }
    }
    Ok(cleared)
}

fn apply_vkr_layout(s: &mut Session, v: &Value) -> CmdResult {
    let levels = p::u64(v, "levels").unwrap_or(3).clamp(1, 9) as u8;
    let cleared = clear_stray_inner(s)?;
    let cleared_body = apply_body_style_inner(s)?;
    ensure_heading_inner(s, levels)?;
    normalize_special_paras(s)?;
    let mut ir = full_ir(s);
    if let Some(o) = ir.as_object_mut() {
        o.insert("cleared".into(), json!(cleared.saturating_add(cleared_body)));
        o.insert(
            "applied".into(),
            json!(["clearStrayPageBreaks", "applyBodyStyle", "ensureHeading", "normalizeCaptions"]),
        );
    }
    Ok(ir)
}

fn is_heading1(s: &Session, props: &ParaProps) -> bool {
    if props.style.as_deref() == Some("Heading1") {
        return true;
    }
    s.doc.styles.resolve_para(props).outline_level == Some(0)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_doc::{Document, Paragraph};

    use super::*;
    use crate::Session;

    fn doc_with_stray_breaks() -> Session {
        let mut d = Document::new();
        d.body.clear();
        for (i, t) in ["Intro", "Body one", "Body two", "More"].iter().enumerate() {
            let mut p = Paragraph::with_text(t, CharProps::default());
            if i > 0 {
                p.props.page_break_before = Some(true);
            }
            d.body.push(Arc::new(Block::Para(p)));
        }
        Session::new(d)
    }

    #[test]
    fn get_ir_reports_page_break_before() {
        let mut s = doc_with_stray_breaks();
        let ir = s.run("doc.getIr", &json!({})).unwrap();
        let blocks = ir["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0]["pageBreakBefore"], false);
        assert_eq!(blocks[1]["pageBreakBefore"], true);
        assert!(ir["pages"].as_u64().unwrap() >= 4);
    }

    #[test]
    fn clear_stray_page_breaks_collapses_pages() {
        let mut s = doc_with_stray_breaks();
        let before = s.layout().pages.len();
        let r = s.run("doc.clearStrayPageBreaks", &json!({})).unwrap();
        assert_eq!(r["cleared"], 3);
        assert!(s.layout().pages.len() < before);
        assert_eq!(s.layout().pages.len(), 1);
    }

    #[test]
    fn apply_body_style_sets_normal_and_margins() {
        let mut s = doc_with_stray_breaks();
        let r = s.run("doc.applyBodyStyle", &json!({})).unwrap();
        assert!(r["clearedPageBreaks"].as_u64().unwrap() >= 3);
        let st = s.doc.styles.get("Normal").unwrap();
        assert_eq!(st.chr.font.as_deref(), Some("Times New Roman"));
        assert_eq!(st.chr.size, Some(14.0));
        assert_eq!(st.para.align, Some(Align::Justify));
        assert_eq!(st.para.indent_first, Some(FIRST_LINE_PT));
        assert_eq!(st.para.line_spacing, Some(LineSpacing::Multiple(1.5)));
        let sect = s.doc.sections()[0].1;
        assert!((sect.margin_left - MARGIN_LEFT).abs() < 0.01);
        assert!((sect.page_w - A4_W).abs() < 0.01);
    }

    #[test]
    fn ensure_heading_links_multilevel_and_page_break_on_h1() {
        let mut d = Document::new();
        d.body.clear();
        d.body.push(Arc::new(Block::Para(Paragraph::with_text("Chapter", CharProps::default()).styled("Heading1"))));
        d.body.push(Arc::new(Block::Para(Paragraph::with_text("Section", CharProps::default()).styled("Heading2"))));
        d.body.push(Arc::new(Block::Para(Paragraph::with_text("Body", CharProps::default()))));
        let mut s = Session::new(d);
        s.run("doc.ensureHeading", &json!({})).unwrap();
        let h1 = s.doc.styles.get("Heading1").unwrap();
        assert_eq!(h1.chr.bold, Some(true));
        assert_eq!(h1.para.page_break_before, Some(true));
        let num = h1.para.numbering.unwrap();
        let l0 = s.doc.numbering.level(num.num, 0).unwrap();
        let l1 = s.doc.numbering.level(num.num, 1).unwrap();
        assert_eq!(l0.text, "%1");
        assert_eq!(l1.text, "%1.%2");
        let body = s.doc.para(StoryRef::Body, &Path::top(2)).unwrap();
        assert_ne!(body.props.page_break_before, Some(true));
        let h1p = s.doc.para(StoryRef::Body, &Path::top(0)).unwrap();
        assert_eq!(h1p.props.page_break_before, Some(true));
    }

    #[test]
    fn apply_ir_patch_updates_paragraph() {
        let mut s = Session::new(Document::from_text("Hello\nWorld"));
        let r = s
            .run(
                "doc.applyIrPatch",
                &json!({"ops":[
                    {"op":"update","index":0,"set":{"style":"Heading1","pageBreakBefore":true,"text":"Title"}},
                    {"op":"update","index":1,"set":{"indentFirst":35.45,"align":"justify"}}
                ]}),
            )
            .unwrap();
        assert_eq!(r["changed"], 2);
        assert_eq!(r["blocks"][0]["text"], "Title");
        assert_eq!(r["blocks"][0]["style"], "Heading1");
        assert_eq!(r["blocks"][0]["pageBreakBefore"], true);
        let p1 = s.doc.para(StoryRef::Body, &Path::top(1)).unwrap();
        assert_eq!(p1.props.indent_first, Some(35.45));
        assert_eq!(p1.props.align, Some(Align::Justify));
    }

    #[test]
    fn apply_ir_patch_rejects_bad_index() {
        let mut s = Session::new(Document::from_text("Hi"));
        assert!(s.run("doc.applyIrPatch", &json!({"ops":[{"op":"update","index":9,"set":{}}]})).is_err());
    }

    #[test]
    fn get_ir_filters_range_fields_and_page_breaks() {
        let mut s = doc_with_stray_breaks();
        let only = s.run("doc.getIr", &json!({"pageBreaksOnly": true})).unwrap();
        assert_eq!(only["blocks"].as_array().unwrap().len(), 3);
        let range = s.run("doc.getIr", &json!({"from": 1, "to": 2, "fields": ["style", "pageBreakBefore"]})).unwrap();
        let blocks = range["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].get("text").is_none());
        assert_eq!(blocks[0]["index"], 1);
        assert_eq!(blocks[0]["style"], "Normal");
        assert_eq!(blocks[0]["pageBreakBefore"], true);
    }

    #[test]
    fn apply_vkr_layout_one_shot() {
        let mut d = Document::new();
        d.body.clear();
        d.body.push(Arc::new(Block::Para(Paragraph::with_text("Chapter", CharProps::default()).styled("Heading1"))));
        let mut body = Paragraph::with_text("Body", CharProps::default());
        body.props.page_break_before = Some(true);
        d.body.push(Arc::new(Block::Para(body)));
        d.body.push(Arc::new(Block::Para(Paragraph::with_text("Section", CharProps::default()).styled("Heading2"))));
        let mut s = Session::new(d);
        let r = s.run("doc.applyVkrLayout", &json!({})).unwrap();
        assert!(r["cleared"].as_u64().unwrap() >= 1);
        assert_eq!(s.doc.styles.get("Normal").unwrap().chr.font.as_deref(), Some("Times New Roman"));
        assert_eq!(s.doc.styles.get("Heading1").unwrap().chr.bold, Some(true));
        assert_eq!(s.doc.para(StoryRef::Body, &Path::top(1)).unwrap().props.page_break_before, Some(false));
        assert_eq!(s.layout().pages.len(), 1);
    }

    #[test]
    fn apply_vkr_layout_normalizes_captions_and_heading_indents() {
        let mut d = Document::new();
        d.body.clear();
        let mut h2 = Paragraph::with_text("Поток", CharProps::default()).styled("Heading2");
        h2.props.align = Some(Align::Justify);
        h2.props.indent_left = Some(72.0);
        d.body.push(Arc::new(Block::Para(h2)));
        let mut cap = Paragraph::with_text("Рисунок 1.1 - Логическая архитектура веб-сервиса управления заявками", CharProps::default());
        cap.props.align = Some(Align::Justify);
        cap.props.indent_first = Some(FIRST_LINE_PT);
        cap.props.indent_left = Some(40.0);
        d.body.push(Arc::new(Block::Para(cap)));
        let mut body = Paragraph::with_text("После рисунка текст", CharProps::default());
        body.props.indent_left = Some(40.0);
        d.body.push(Arc::new(Block::Para(body)));
        let mut s = Session::new(d);
        s.run("doc.applyVkrLayout", &json!({})).unwrap();
        let cap = s.doc.para(StoryRef::Body, &Path::top(1)).unwrap();
        assert_eq!(cap.props.style.as_deref(), Some("Caption"));
        assert_eq!(cap.props.align, Some(Align::Center));
        assert_eq!(cap.props.indent_first, Some(0.0));
        assert_eq!(cap.props.indent_left, Some(0.0));
        let h2 = s.doc.para(StoryRef::Body, &Path::top(0)).unwrap();
        assert_eq!(h2.props.align, Some(Align::Left));
        assert!(h2.props.indent_left.is_none());
        let body = s.doc.para(StoryRef::Body, &Path::top(2)).unwrap();
        assert_eq!(body.props.indent_left, Some(0.0));
        let num = s.doc.styles.get("Heading2").unwrap().para.numbering.unwrap();
        let lvl = s.doc.numbering.level(num.num, 1).unwrap();
        assert!((lvl.indent - FIRST_LINE_PT).abs() < 0.01);
        assert!((lvl.hanging - 0.0).abs() < 0.01);
    }

    #[test]
    fn apply_ir_patch_replace_in_keeps_surrounding_text() {
        let mut s = Session::new(Document::from_text("Сервис управления заявками в контуре"));
        let r = s
            .run(
                "doc.applyIrPatch",
                &json!({"ops":[{"op":"replaceIn","index":0,"find":"заявками","with":"заказами","occurrence":1}]}),
            )
            .unwrap();
        assert_eq!(r["changed"], 1);
        assert_eq!(r["blocks"][0]["text"], "Сервис управления заказами в контуре");
    }
}
