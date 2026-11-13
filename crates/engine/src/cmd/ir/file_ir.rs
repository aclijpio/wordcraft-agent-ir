//! File-based Agent IR: export/sync one folder per document.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wordcraft_doc::props::{Align, CharProps};
use wordcraft_doc::{Block, InlineObject, Paragraph, Table};

use super::ids::{ensure_all_block_ids, set_block_id};
use super::styles_catalog::{self, apply_styles_json, export_styles_json, merge_block_overrides};
use super::{MAX_IR_BLOCKS, detect_role, full_ir, is_heading1, parse_align};
use crate::{CmdError, CmdResult, Session, p};

pub(crate) fn ir_path_cmd(s: &mut Session, _: &Value) -> CmdResult {
    let path = resolve_ir_dir(s);
    Ok(json!({ "path": path.to_string_lossy(), "exists": path.is_dir() }))
}

pub(crate) fn export_ir(s: &mut Session, v: &Value) -> CmdResult {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (s, v);
        return Err(CmdError::Failed("doc.exportIr is not available on wasm".into()));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        export_ir_native(s, v)
    }
}

pub(crate) fn sync_ir(s: &mut Session, v: &Value) -> CmdResult {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (s, v);
        return Err(CmdError::Failed("doc.syncIr is not available on wasm".into()));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        sync_ir_native(s, v)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn export_ir_native(s: &mut Session, v: &Value) -> CmdResult {
    let dir = p::str(v, "path").map(PathBuf::from).unwrap_or_else(|| resolve_ir_dir(s));
    let sections_dir = dir.join("sections");
    if sections_dir.is_dir() {
        let _ = std::fs::remove_dir_all(&sections_dir);
    }
    std::fs::create_dir_all(&sections_dir).map_err(|e| CmdError::Failed(e.to_string()))?;
    std::fs::create_dir_all(dir.join("assets")).map_err(|e| CmdError::Failed(e.to_string()))?;

    let ids = ensure_all_block_ids(s)?;
    let styles = export_styles_json(s);
    styles_catalog::save_styles_file(&dir.join("styles.json"), &styles)?;

    let end = s.doc.body.len().min(MAX_IR_BLOCKS);
    let mut section_files: Vec<String> = Vec::new();
    let mut current: Vec<Value> = Vec::new();
    let mut section_idx = 0u32;
    let mut section_slug = "start".to_string();

    let flush = |section_idx: u32,
                 section_slug: &str,
                 current: &mut Vec<Value>,
                 section_files: &mut Vec<String>,
                 dir: &Path|
     -> Result<(), CmdError> {
        if current.is_empty() && !section_files.is_empty() {
            return Ok(());
        }
        let name = format!("{section_idx:02}-{section_slug}.json");
        let path = dir.join("sections").join(&name);
        let text = serde_json::to_string_pretty(current).map_err(|e| CmdError::Failed(e.to_string()))?;
        styles_catalog::write_text(&path, &text)?;
        section_files.push(format!("sections/{name}"));
        current.clear();
        Ok(())
    };

    for i in 0..end {
        let Some(b) = s.doc.body.get(i) else { break };
        let id = ids.get(i).cloned().unwrap_or_else(|| format!("b{i:08x}"));
        let split = should_split_section(s, i, b);
        if split && !current.is_empty() {
            flush(section_idx, &section_slug, &mut current, &mut section_files, &dir)?;
            section_idx = section_idx.saturating_add(1);
            section_slug = slug_for_block(s, i, b);
        } else if current.is_empty() {
            section_slug = slug_for_block(s, i, b);
        }
        current.push(export_block(s, i, &id, b, &dir)?);
    }
    flush(section_idx, &section_slug, &mut current, &mut section_files, &dir)?;

    let manifest = json!({
        "version": 3,
        "documentId": s.document_id(),
        "exportedRev": s.rev(),
        "sectionOrder": section_files,
    });
    styles_catalog::write_text(&dir.join("manifest.json"), &serde_json::to_string_pretty(&manifest).map_err(|e| CmdError::Failed(e.to_string()))?)?;

    Ok(json!({
        "path": dir.to_string_lossy(),
        "sections": section_files.len(),
        "blocks": end,
        "styles": styles["styles"].as_array().map(|a| a.len()).unwrap_or(0),
    }))
}

#[cfg(not(target_arch = "wasm32"))]
fn sync_ir_native(s: &mut Session, v: &Value) -> CmdResult {
    let dir = p::str(v, "path").map(PathBuf::from).unwrap_or_else(|| resolve_ir_dir(s));
    if !dir.is_dir() {
        return Err(CmdError::Failed(format!("IR folder not found: {}", dir.display())));
    }
    let manifest: Value = read_json(&dir.join("manifest.json"))?;
    let section_order = manifest
        .get("sectionOrder")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if section_order.is_empty() {
        return Err(CmdError::Failed("manifest.json has empty sectionOrder".into()));
    }

    let styles_path = dir.join("styles.json");
    if styles_path.is_file() {
        let catalog = styles_catalog::load_styles_file(&styles_path)?;
        apply_styles_json(s, &catalog)?;
    }

    let mut blocks: Vec<(String, Block)> = Vec::new();
    for (si, rel) in section_order.iter().enumerate() {
        let rel = rel.as_str().ok_or_else(|| CmdError::Failed(format!("sectionOrder[{si}] not a string")))?;
        if rel.contains("..") {
            return Err(CmdError::Failed("section path rejected".into()));
        }
        let path = dir.join(rel);
        let arr = read_json(&path)?;
        let arr = arr.as_array().ok_or_else(|| CmdError::Failed(format!("{rel}: expected array")))?;
        for (bi, block) in arr.iter().enumerate() {
            if blocks.len() >= MAX_IR_BLOCKS {
                return Err(CmdError::Params(format!("at most {MAX_IR_BLOCKS} IR blocks")));
            }
            let id = p::str(block, "id").unwrap_or("").to_string();
            if id.is_empty() {
                return Err(CmdError::Params(format!("{rel}[{bi}]: missing id")));
            }
            let built = build_block(s, block, &dir)?;
            blocks.push((id, built));
        }
    }

    s.doc.body.clear();
    for (_, block) in &blocks {
        s.doc.body.push(Arc::new(block.clone()));
    }
    s.doc.ensure_nonempty();
    for (i, (id, _)) in blocks.iter().enumerate() {
        if i >= s.doc.body.len() {
            break;
        }
        let _ = set_block_id(s, i, id);
    }
    s.clamp_selection();
    s.touch();
    let mut ir = full_ir(s);
    if let Some(o) = ir.as_object_mut() {
        o.insert("synced".into(), json!(true));
        o.insert("path".into(), json!(dir.to_string_lossy()));
        o.insert("blocks".into(), json!(blocks.len()));
    }
    Ok(ir)
}

pub(crate) fn resolve_ir_dir(s: &Session) -> PathBuf {
    if let Some(path) = s.path.as_ref() {
        let stem = path.file_stem().and_then(|x| x.to_str()).unwrap_or("document");
        return path.with_file_name(format!("{stem}.wordcraft-ir"));
    }
    let base = wordcraft_control_key::settings_dir().unwrap_or_else(|| std::env::temp_dir().join("wordcraft"));
    base.join("ir").join(format!("{}", s.document_id()))
}

fn read_json(path: &Path) -> Result<Value, CmdError> {
    let bytes = crate::io::read_file(&path.to_string_lossy()).map_err(CmdError::Failed)?;
    let text = String::from_utf8(bytes).map_err(|e| CmdError::Failed(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| CmdError::Failed(format!("{}: {e}", path.display())))
}

fn should_split_section(s: &Session, index: usize, block: &Block) -> bool {
    match block {
        Block::Para(para) => {
            let style = para.props.style.as_deref().unwrap_or("");
            if style == "TOCHeading" || style == "Heading1" {
                return true;
            }
            if is_heading1(s, &para.props) {
                return true;
            }
            para.props.page_break_before == Some(true) && index > 0
        }
        Block::Table(_) => false,
    }
}

fn slug_for_block(_s: &Session, index: usize, block: &Block) -> String {
    match block {
        Block::Para(para) => {
            let style = para.props.style.as_deref().unwrap_or("");
            if style == "TOCHeading" {
                return "toc".into();
            }
            let text = para.plain_text();
            let slug = slugify(&text);
            if !slug.is_empty() {
                return slug;
            }
            if style.starts_with("Heading") {
                return style.to_ascii_lowercase();
            }
            format!("sec{index}")
        }
        Block::Table(_) => format!("table{index}"),
    }
}

fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars().take(40) {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if (c.is_whitespace() || c == '-' || c == '_') && !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.chars().take(32).collect()
}

fn export_block(s: &Session, index: usize, id: &str, block: &Block, dir: &Path) -> Result<Value, CmdError> {
    match block {
        Block::Para(para) => {
            let style = para.props.style.clone().unwrap_or_else(|| "Normal".into());
            let text = para.plain_text();
            let pictures = picture_media(para);
            let picture_only = !pictures.is_empty()
                && pictures.len() == 1
                && text.chars().all(|c| c == '\u{FFFC}' || c.is_whitespace());
            if picture_only {
                let media = &pictures[0];
                let asset = write_asset(s, dir, id, media)?;
                let mut o = json!({
                    "id": id,
                    "type": "picture",
                    "style": style,
                    "asset": asset,
                });
                if let Some(a) = para.props.align {
                    o["align"] = json!(align_name(a));
                }
                return Ok(o);
            }
            let mut o = json!({
                "id": id,
                "type": "paragraph",
                "style": style,
                "text": strip_obj_chars(&text),
            });
            if let Some(role) = detect_role(s, &style, &text, para) {
                o["role"] = json!(role);
            }
            emit_overrides(&mut o, para);
            if !pictures.is_empty() {
                let mut assets = Vec::new();
                for (n, media) in pictures.iter().enumerate() {
                    let aid = if n == 0 { id.to_string() } else { format!("{id}_{n}") };
                    assets.push(write_asset(s, dir, &aid, media)?);
                }
                o["assets"] = json!(assets);
            }
            let _ = index;
            Ok(o)
        }
        Block::Table(t) => {
            let cells: Vec<Vec<String>> = t
                .rows
                .iter()
                .map(|r| {
                    r.cells
                        .iter()
                        .map(|c| c.blocks.iter().filter_map(|b| b.as_para().map(|p| strip_obj_chars(&p.plain_text()))).collect::<Vec<_>>().join("\n"))
                        .collect()
                })
                .collect();
            let mut o = json!({
                "id": id,
                "type": "table",
                "style": t.props.style,
                "rows": t.rows.len(),
                "cols": t.cols(),
                "cells": cells,
            });
            if t.rows.first().is_some_and(|r| r.props.header) {
                o["header"] = json!(true);
            }
            Ok(o)
        }
    }
}

fn emit_overrides(o: &mut Value, para: &Paragraph) {
    if let Some(a) = para.props.align {
        o["align"] = json!(align_name(a));
    }
    if let Some(v) = para.props.indent_first {
        o["indentFirst"] = json!(v);
    }
    if let Some(v) = para.props.indent_left {
        o["indentLeft"] = json!(v);
    }
    if let Some(v) = para.props.indent_right {
        o["indentRight"] = json!(v);
    }
    if let Some(on) = para.props.page_break_before {
        o["pageBreakBefore"] = json!(on);
    }
    if let Some(wordcraft_doc::props::LineSpacing::Multiple(v)) = para.props.line_spacing {
        o["lineSpacing"] = json!(v);
    }
}

fn picture_media(para: &Paragraph) -> Vec<String> {
    para.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Image { media, .. } => Some(media.clone()),
            _ => None,
        })
        .collect()
}

fn write_asset(s: &Session, dir: &Path, id: &str, media: &str) -> Result<String, CmdError> {
    let Some(bytes) = s.doc.media.get(media) else {
        return Err(CmdError::Failed(format!("missing media `{media}`")));
    };
    let ext = media.rsplit('.').next().unwrap_or("bin");
    let rel = format!("assets/{id}.{ext}");
    let path = dir.join(&rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CmdError::Failed(e.to_string()))?;
    }
    std::fs::write(&path, bytes.as_ref()).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok(rel)
}

fn strip_obj_chars(text: &str) -> String {
    text.chars().filter(|&c| c != '\u{FFFC}').collect()
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

fn build_block(s: &mut Session, block: &Value, dir: &Path) -> Result<Block, CmdError> {
    let ty = p::str(block, "type").unwrap_or("paragraph");
    match ty {
        "paragraph" => {
            let text = p::str(block, "text").unwrap_or("");
            let style = p::str(block, "style").unwrap_or("Normal");
            let mut para = Paragraph::with_text(text, CharProps::default());
            para.props.style = Some(style.to_string());
            merge_block_overrides(&mut para, block)?;
            if let Some(assets) = block.get("assets").and_then(Value::as_array) {
                for asset in assets {
                    if let Some(rel) = asset.as_str() {
                        append_picture(&mut para, s, dir, rel, None)?;
                    }
                }
            }
            Ok(Block::Para(para))
        }
        "picture" => {
            let style = p::str(block, "style").unwrap_or("Normal");
            let asset = p::str(block, "asset").ok_or_else(|| CmdError::Params("picture needs `asset`".into()))?;
            let mut para = Paragraph::with_text("", CharProps::default());
            para.props.style = Some(style.to_string());
            if let Some(a) = p::str(block, "align") {
                para.props.align = Some(parse_align(a)?);
            } else {
                para.props.align = Some(Align::Center);
            }
            merge_block_overrides(&mut para, block)?;
            let width = p::f32(block, "width");
            append_picture(&mut para, s, dir, asset, width)?;
            Ok(Block::Para(para))
        }
        "table" => {
            let grid = block
                .get("cells")
                .or_else(|| block.get("values"))
                .and_then(Value::as_array)
                .map(|rows| {
                    rows.iter()
                        .filter_map(|row| row.as_array())
                        .map(|cols| {
                            cols.iter()
                                .map(|c| match c {
                                    Value::Null => String::new(),
                                    Value::String(s) => s.clone(),
                                    other => other.to_string().trim_matches('"').to_string(),
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>()
                });
            let (rows, cols) = if let Some(ref g) = grid {
                let rows = g.len().clamp(1, 1000);
                let cols = g.iter().map(|r| r.len()).max().unwrap_or(1).clamp(1, 63);
                (rows, cols)
            } else {
                (
                    p::u64(block, "rows").unwrap_or(2).clamp(1, 100) as usize,
                    p::u64(block, "cols").unwrap_or(2).clamp(1, 63) as usize,
                )
            };
            let width = p::f32(block, "width").unwrap_or(468.0).clamp(36.0, 1584.0);
            let mut t = Table::new(rows, cols, width);
            if let Some(style) = p::str(block, "style") {
                t.props.style = Some(style.to_string());
            }
            if let Some(g) = grid {
                for (r, row) in g.iter().enumerate().take(rows) {
                    for (c, text) in row.iter().enumerate().take(cols) {
                        if let Some(cell_mut) = t.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) {
                            *cell_mut = wordcraft_doc::table::Cell::with_text(text);
                        }
                    }
                }
            }
            if p::bool(block, "header").unwrap_or(false)
                && let Some(first) = t.rows.get_mut(0)
            {
                first.props.header = true;
            }
            Ok(Block::Table(t))
        }
        other => Err(CmdError::Params(format!("unsupported IR block type `{other}`"))),
    }
}

fn append_picture(para: &mut Paragraph, s: &mut Session, dir: &Path, rel: &str, width: Option<f32>) -> Result<(), CmdError> {
    if rel.contains("..") {
        return Err(CmdError::Failed("asset path rejected".into()));
    }
    let path = dir.join(rel);
    let bytes = crate::io::read_file(&path.to_string_lossy()).map_err(CmdError::Failed)?;
    if bytes.len() > 200 << 20 {
        return Err(CmdError::Failed("image is larger than 200 MB".into()));
    }
    let (natural_w, natural_h, ext) = match wordcraft_geom::picture::probe(&bytes) {
        Ok(pic) => (pic.width as f32 * 0.75, pic.height as f32 * 0.75, crate::cmd::insert::image_ext(&bytes)),
        Err(_) => match wordcraft_render::metafile_size(&bytes) {
            Some(size) => size,
            None => return Err(CmdError::Failed("not a supported image".into())),
        },
    };
    let key = s.doc.add_media(bytes, ext);
    let max_w = s.doc.sections().first().map(|(_, sp)| sp.text_width()).unwrap_or(468.0);
    let (mut w, mut h) = (natural_w, natural_h);
    if let Some(want) = width {
        let k = want.clamp(4.0, 1584.0) / w.max(1.0);
        w *= k;
        h *= k;
    }
    if w > max_w {
        h *= max_w / w;
        w = max_w;
    }
    let obj = InlineObject::Image {
        media: key,
        w,
        h,
        alt: String::new(),
        float: Default::default(),
        crop: [0.0; 4],
        fx: Default::default(),
        ole: None,
    };
    let at = para.len();
    para.insert_object(at, obj, &CharProps::default()).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok(())
}
