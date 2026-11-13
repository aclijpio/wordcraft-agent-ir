//! Agent document IR: slim inspect + high-level apply (generic, not thesis-specific).

mod file_ir;
mod ids;
mod layout;
mod media;
mod patch;
mod styles_catalog;
mod templates;

use serde_json::{Value, json};
use wordcraft_doc::props::{Align, LineSpacing};
use wordcraft_doc::{Block, InlineObject};

use crate::{CmdResult, CommandSpec, Session, p};

#[cfg(test)]
mod tests;

const MAX_IR_BLOCKS: usize = 2000;
const MAX_PATCH_OPS: usize = 500;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("doc.getIr", "Document IR", "Agents", get_ir)
            .params(r#"{"from"?: n, "to"?: n, "fields"?: [string], "pageBreaksOnly"?: bool, "kinds"?: ["paragraph"|"table"|"picture"]}"#)
            .pure(),
        CommandSpec::new("doc.applyIrPatch", "Apply IR Patch", "Agents", patch::apply_ir_patch).params(
            r#"{"ops":[{"op":"update"|"replace"|"replaceIn"|"insert"|"delete"|"move"|"setStyle"|"setCell","index"|"id"?:…}]}"#,
        ),
        CommandSpec::new("doc.applyLayout", "Apply Layout Profile", "Agents", layout::apply_layout)
            .params(r#"{"profile"?: object, "clearStrayPageBreaks"?: bool}"#),
        CommandSpec::new("doc.ensureStyles", "Ensure Styles", "Agents", layout::ensure_styles)
            .params(r#"{"styles":[{"id":string,"basedOn"?:string,"name"?:string,"para"?:object,"chr"?:object}]}"#),
        CommandSpec::new("doc.applyBodyStyle", "Apply Body Style", "Agents", layout::apply_body_style)
            .params(r#"{"profile"?: object}"#),
        CommandSpec::new("doc.ensureHeading", "Ensure Heading Styles", "Agents", layout::ensure_heading)
            .params(r#"{"levels"?: 1-9, "numbering"?: bool, "indentFirst"?: pt, "font"?: string, "size"?: pt}"#),
        CommandSpec::new("doc.clearStrayPageBreaks", "Clear Stray Page Breaks", "Agents", layout::clear_stray_page_breaks)
            .params(r#"{}"#),
        CommandSpec::new("doc.insertFigure", "Insert Figure", "Agents", media::insert_figure).params(
            r#"{"path"|"data":…, "width"?:pt, "alt"?:string, "caption"?:string, "captionLabel"?:string, "position"?: "below"|"above", "align"?:string, "index"?:n}"#,
        ),
        CommandSpec::new("doc.insertCaption", "Insert Caption", "Agents", media::insert_caption)
            .params(r#"{"label"?:string, "text"?:string, "position"?: "below"|"above", "format"?:string, "separator"?:string, "index"?:n}"#),
        CommandSpec::new("doc.insertToc", "Insert TOC", "Agents", media::insert_toc)
            .params(r#"{"levels"?:n, "title"?:string, "pageNumbers"?:bool, "rightAlign"?:bool, "tabLeader"?:string, "hyperlinks"?:bool}"#),
        CommandSpec::new("doc.updateToc", "Update TOC", "Agents", media::update_toc).params(r#"{}"#),
        CommandSpec::new("doc.removeToc", "Remove TOC", "Agents", media::remove_toc).params(r#"{}"#),
        CommandSpec::new("doc.updateFields", "Update Fields", "Agents", media::update_fields).params(r#"{"all"?: bool}"#),
        CommandSpec::new("doc.insertCrossRef", "Insert Cross-Reference", "Agents", media::insert_cross_ref)
            .params(r#"{"to":string, "target"?:string, "show"?:string}"#),
        CommandSpec::new("doc.applyTableStyle", "Apply Table Style by Index", "Agents", media::apply_table_style)
            .params(r#"{"index":n, "style":string}"#),
        CommandSpec::new("doc.newFromTemplate", "New From Agent Template", "Agents", templates::new_from_template)
            .params(r#"{"id"|"path"|"template": string}"#),
        CommandSpec::new("doc.exportIr", "Export IR Folder", "Agents", file_ir::export_ir)
            .params(r#"{"path"?: string}"#),
        CommandSpec::new("doc.syncIr", "Sync IR Folder", "Agents", file_ir::sync_ir)
            .params(r#"{"path"?: string}"#),
        CommandSpec::new("doc.irPath", "IR Folder Path", "Agents", file_ir::ir_path_cmd)
            .params(r#"{}"#)
            .pure(),
    ]
}

struct IrFilter {
    from: usize,
    to: usize,
    page_breaks_only: bool,
    fields: Option<Vec<String>>,
    kinds: Option<Vec<String>>,
}

fn parse_filter(v: &Value) -> IrFilter {
    let from = p::u64(v, "from").unwrap_or(0) as usize;
    let to = p::u64(v, "to").map(|x| x as usize).unwrap_or(usize::MAX);
    let page_breaks_only = p::bool(v, "pageBreaksOnly").unwrap_or(false);
    let fields = v.get("fields").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>());
    let kinds = v.get("kinds").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>());
    IrFilter { from, to, page_breaks_only, fields, kinds }
}

fn kind_ok(filter: &IrFilter, kind: &str) -> bool {
    filter.kinds.as_ref().is_none_or(|k| k.iter().any(|x| x == kind))
}

fn get_ir(s: &mut Session, v: &Value) -> CmdResult {
    Ok(build_ir(s, &parse_filter(v)))
}

pub(crate) fn full_ir(s: &mut Session) -> Value {
    build_ir(s, &IrFilter { from: 0, to: usize::MAX, page_breaks_only: false, fields: None, kinds: None })
}

fn build_ir(s: &mut Session, filter: &IrFilter) -> Value {
    let mut blocks = Vec::new();
    let end = s.doc.body.len().min(MAX_IR_BLOCKS).saturating_sub(1).min(filter.to);
    let start = filter.from.min(end.saturating_add(1));
    for i in start..=end {
        let Some(b) = s.doc.body.get(i) else { break };
        match &**b {
            Block::Para(para) => {
                let style = para.props.style.clone().unwrap_or_else(|| "Normal".into());
                let resolved = s.doc.styles.resolve_para(&para.props);
                let page_break_before = para.props.page_break_before.unwrap_or(resolved.page_break_before);
                let is_heading = style.starts_with("Heading");
                let pictures = picture_summaries(para);
                let has_picture = !pictures.is_empty();
                if filter.page_breaks_only && !page_break_before && !is_heading {
                    continue;
                }
                let want_para = kind_ok(filter, "paragraph");
                let want_pic = kind_ok(filter, "picture");
                if !want_para && !(want_pic && has_picture) {
                    continue;
                }
                if want_pic && !want_para && !has_picture {
                    continue;
                }
                let text = para.plain_text();
                let picture_only = has_picture && pictures.len() == 1 && text.chars().all(|c| c == '\u{FFFC}' || c.is_whitespace());
                let mut o = json!({
                    "index": i,
                    "type": if picture_only { "picture" } else { "paragraph" },
                    "style": style,
                    "text": text,
                    "align": align_name(resolved.align),
                    "indentFirst": resolved.indent_first,
                    "indentLeft": resolved.indent_left,
                    "indentRight": resolved.indent_right,
                    "pageBreakBefore": page_break_before,
                });
                if let Some(id) = ids::block_id_at(s, i) {
                    o["id"] = json!(id);
                }
                if let Some(role) = detect_role(s, &style, &text, para) {
                    o["role"] = json!(role);
                }
                if has_picture {
                    o["pictures"] = json!(pictures);
                }
                match para.props.line_spacing.unwrap_or(resolved.line_spacing) {
                    LineSpacing::Multiple(v) => o["lineSpacing"] = json!(v),
                    other => o["lineSpacing"] = serde_json::to_value(other).unwrap_or(Value::Null),
                }
                if let Some(n) = para.props.numbering.or(resolved.numbering).filter(|n| n.num != 0) {
                    o["numbering"] = json!({"num": n.num, "level": n.level});
                    o["listLevel"] = json!(n.level);
                }
                if let Some(fields) = &filter.fields {
                    o = keep_fields(o, fields);
                }
                blocks.push(o);
            }
            Block::Table(t) => {
                if filter.page_breaks_only || !kind_ok(filter, "table") {
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
                if let Some(id) = ids::block_id_at(s, i) {
                    o["id"] = json!(id);
                }
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
        "version": 2,
        "pages": pages,
        "blocks": blocks,
        "sections": sections,
    })
}

fn picture_summaries(para: &wordcraft_doc::Paragraph) -> Vec<Value> {
    para.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Image { media, w, h, alt, .. } => Some(json!({
                "media": media,
                "width": w,
                "height": h,
                "alt": alt,
            })),
            InlineObject::Graphic { w, h, alt, .. } => Some(json!({
                "width": w,
                "height": h,
                "alt": alt,
            })),
            _ => None,
        })
        .collect()
}

fn detect_role(s: &Session, style: &str, text: &str, para: &wordcraft_doc::Paragraph) -> Option<&'static str> {
    if style == "TOCHeading" || style.starts_with("TOC") {
        return Some("toc");
    }
    if para.objects.iter().any(|o| match o {
        InlineObject::Field { instr, .. } => instr.to_ascii_uppercase().contains("TOC"),
        _ => false,
    }) {
        return Some("toc");
    }
    if is_heading1(s, &para.props) || style.starts_with("Heading") {
        return Some("heading");
    }
    if style == "Caption" {
        let t = text.trim_start();
        if caption_figure_prefix(t) {
            return Some("figureCaption");
        }
        if caption_table_prefix(t) {
            return Some("tableCaption");
        }
        return Some("caption");
    }
    let t = text.trim_start();
    if caption_figure_prefix(t) {
        return Some("figureCaption");
    }
    if caption_table_prefix(t) {
        return Some("tableCaption");
    }
    None
}

pub(crate) fn caption_figure_prefix(t: &str) -> bool {
    const P: &[&str] = &["Figure ", "Fig. ", "Рис. ", "Рисунок "];
    P.iter().any(|p| t.starts_with(p))
}

pub(crate) fn caption_table_prefix(t: &str) -> bool {
    const P: &[&str] = &["Table ", "Tab. ", "Табл. ", "Таблица "];
    P.iter().any(|p| t.starts_with(p))
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

fn keep_fields(mut o: Value, fields: &[String]) -> Value {
    let Some(map) = o.as_object_mut() else { return o };
    let keep: std::collections::HashSet<&str> = fields.iter().map(String::as_str).chain(["index", "type", "id"]).collect();
    map.retain(|k, _| keep.contains(k.as_str()));
    o
}

pub(crate) fn parse_align(a: &str) -> Result<Align, crate::CmdError> {
    Ok(match a {
        "left" => Align::Left,
        "center" => Align::Center,
        "right" => Align::Right,
        "justify" | "both" => Align::Justify,
        "distribute" => Align::Distribute,
        x => return Err(crate::CmdError::Params(format!("unknown alignment `{x}`"))),
    })
}

pub(crate) fn is_heading1(s: &Session, props: &wordcraft_doc::props::ParaProps) -> bool {
    if props.style.as_deref() == Some("Heading1") {
        return true;
    }
    s.doc.styles.resolve_para(props).outline_level == Some(0)
}

pub(crate) fn find_nth(haystack: &str, needle: &str, occurrence: usize) -> Option<usize> {
    let mut from = 0usize;
    let mut seen = 0usize;
    while from <= haystack.len() {
        let rel = haystack.get(from..)?.find(needle)?;
        let at = from.saturating_add(rel);
        seen = seen.saturating_add(1);
        if seen == occurrence {
            return Some(at);
        }
        from = at.saturating_add(needle.len().max(1));
    }
    None
}

pub(crate) fn max_patch_ops() -> usize {
    MAX_PATCH_OPS
}
