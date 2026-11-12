//! Generic layout profiles and stylesheet helpers.

use serde_json::{Value, json};
use std::sync::Arc;
use wordcraft_doc::numbering::Level;
use wordcraft_doc::props::{Align, CharProps, LineSpacing, NumRef, ParaProps, Rgb, TextColor};
use wordcraft_doc::section::NumFormat;
use wordcraft_doc::styles::{Style, StyleKind};
use wordcraft_doc::{Path, StoryRef};

use super::{caption_figure_prefix, caption_table_prefix, full_ir, is_heading1, parse_align};
use crate::cmd::page::{margins_fit, with_sect};
use crate::cmd::para::{ensure_heading_style, touch_all};
use crate::{CmdError, CmdResult, Session, p};

pub(crate) const DEFAULT_FIRST_LINE: f32 = 36.0;
const A4_W: f32 = 595.28;
const A4_H: f32 = 841.89;
const DEFAULT_MARGIN: f32 = 72.0;

/// Neutral built-in profile (not thesis-specific).
pub(crate) fn default_profile() -> Value {
    json!({
        "page": {"w": A4_W, "h": A4_H},
        "margins": {"top": DEFAULT_MARGIN, "bottom": DEFAULT_MARGIN, "left": DEFAULT_MARGIN, "right": DEFAULT_MARGIN},
        "normal": {
            "font": "Calibri",
            "size": 11.0,
            "align": "left",
            "lineSpacing": 1.15,
            "indentFirst": 0.0,
            "indentLeft": 0.0,
            "indentRight": 0.0
        },
        "caption": {
            "font": "Calibri",
            "size": 9.0,
            "figureAlign": "center",
            "tableAlign": "left",
            "indentFirst": 0.0,
            "indentLeft": 0.0
        },
        "headings": {
            "levels": 3,
            "font": "Calibri",
            "size": 14.0,
            "bold": true,
            "align": "left",
            "indentFirst": 0.0,
            "numbering": true,
            "pageBreakOnHeading1": true
        },
        "clearStrayPageBreaks": true,
        "normalizeCaptions": true
    })
}

pub(crate) fn apply_layout(s: &mut Session, v: &Value) -> CmdResult {
    let profile = v.get("profile").cloned().unwrap_or_else(default_profile);
    let cleared = apply_layout_inner(s, &profile, p::bool(v, "clearStrayPageBreaks"))?;
    let mut ir = full_ir(s);
    if let Some(o) = ir.as_object_mut() {
        o.insert("clearedPageBreaks".into(), json!(cleared));
        o.insert("applied".into(), json!("applyLayout"));
    }
    Ok(ir)
}

pub(crate) fn apply_body_style(s: &mut Session, v: &Value) -> CmdResult {
    let mut profile = v.get("profile").cloned().unwrap_or_else(default_profile);
    if let Some(o) = profile.as_object_mut() {
        o.entry("clearStrayPageBreaks".to_string()).or_insert(json!(true));
    }
    apply_layout(s, &json!({"profile": profile}))
}

fn apply_layout_inner(s: &mut Session, profile: &Value, clear_override: Option<bool>) -> Result<u32, CmdError> {
    let page = profile.get("page").cloned().unwrap_or(json!({}));
    let margins = profile.get("margins").cloned().unwrap_or(json!({}));
    let w = p::f32(&page, "w").unwrap_or(A4_W).clamp(72.0, 1584.0);
    let h = p::f32(&page, "h").unwrap_or(A4_H).clamp(72.0, 1584.0);
    let t = p::f32(&margins, "top").unwrap_or(DEFAULT_MARGIN).clamp(0.0, 1584.0);
    let b = p::f32(&margins, "bottom").unwrap_or(DEFAULT_MARGIN).clamp(0.0, 1584.0);
    let l = p::f32(&margins, "left").unwrap_or(DEFAULT_MARGIN).clamp(0.0, 1584.0);
    let r = p::f32(&margins, "right").unwrap_or(DEFAULT_MARGIN).clamp(0.0, 1584.0);
    if !margins_fit((w, h), (t, b, l, r), 0.0) {
        return Err(CmdError::Params("margins leave no room for text".into()));
    }
    with_sect(s, |x| {
        if x.landscape {
            x.page_w = h;
            x.page_h = w;
        } else {
            x.page_w = w;
            x.page_h = h;
        }
        x.margin_top = t;
        x.margin_bottom = b;
        x.margin_left = l;
        x.margin_right = r;
    })?;

    let normal = profile.get("normal").cloned().unwrap_or(json!({}));
    ensure_style_id(s, "Normal")?;
    overlay_normal(s, &normal)?;

    let caption = profile.get("caption").cloned().unwrap_or(json!({}));
    ensure_caption_from_profile(s, &caption)?;

    let headings = profile.get("headings").cloned().unwrap_or(json!({}));
    let levels = p::u64(&headings, "levels").unwrap_or(3).clamp(1, 9) as u8;
    ensure_heading_from_profile(s, &headings, levels)?;

    let clear = clear_override.unwrap_or_else(|| p::bool(profile, "clearStrayPageBreaks").unwrap_or(true));
    let cleared = if clear { clear_stray_inner(s)? } else { 0 };
    if p::bool(profile, "normalizeCaptions").unwrap_or(true) {
        normalize_captions(s, &caption)?;
    }
    touch_all(s);
    Ok(cleared)
}

fn overlay_normal(s: &mut Session, normal: &Value) -> Result<(), CmdError> {
    let st = Arc::make_mut(&mut s.doc.styles).get_mut("Normal").ok_or_else(|| CmdError::Params("no Normal style".into()))?;
    st.chr.overlay(&chr_from_value(normal, "Calibri", 11.0, false));
    st.para.overlay(&para_from_value(normal, Align::Left, 0.0, 1.15)?);
    Ok(())
}

fn ensure_caption_from_profile(s: &mut Session, cap: &Value) -> Result<(), CmdError> {
    ensure_style_id(s, "Caption")?;
    let st = Arc::make_mut(&mut s.doc.styles).get_mut("Caption").ok_or_else(|| CmdError::Params("no Caption style".into()))?;
    st.chr.overlay(&chr_from_value(cap, "Calibri", 9.0, false));
    let align = parse_align(p::str(cap, "figureAlign").or_else(|| p::str(cap, "align")).unwrap_or("center"))?;
    let indent_first = p::f32(cap, "indentFirst").unwrap_or(0.0);
    st.para.overlay(&ParaProps {
        align: Some(align),
        indent_left: Some(p::f32(cap, "indentLeft").unwrap_or(0.0)),
        indent_right: Some(p::f32(cap, "indentRight").unwrap_or(0.0)),
        indent_first: Some(indent_first),
        line_spacing: Some(LineSpacing::Multiple(p::f32(cap, "lineSpacing").unwrap_or(1.0))),
        page_break_before: Some(false),
        space_before: Some(0.0),
        space_after: Some(p::f32(cap, "spaceAfter").unwrap_or(6.0)),
        ..ParaProps::default()
    });
    Ok(())
}

fn ensure_heading_from_profile(s: &mut Session, headings: &Value, levels: u8) -> Result<(), CmdError> {
    let font = p::str(headings, "font").unwrap_or("Calibri");
    let size = p::f32(headings, "size").unwrap_or(14.0);
    let bold = p::bool(headings, "bold").unwrap_or(true);
    let align = parse_align(p::str(headings, "align").unwrap_or("left"))?;
    let indent_first = p::f32(headings, "indentFirst").unwrap_or(0.0);
    let line_spacing = p::f32(headings, "lineSpacing").unwrap_or(1.15);
    let page_break_h1 = p::bool(headings, "pageBreakOnHeading1").unwrap_or(true);
    let numbering = p::bool(headings, "numbering").unwrap_or(true);
    let chr = CharProps {
        font: Some(font.into()),
        size: Some(size.clamp(1.0, 1638.0)),
        bold: Some(bold),
        color: Some(TextColor::Rgb(Rgb::BLACK)),
        ..CharProps::default()
    };
    for n in 1..=levels {
        let id = ensure_heading_style(s, n);
        let st = Arc::make_mut(&mut s.doc.styles).get_mut(&id).ok_or_else(|| CmdError::Params(format!("no style `{id}`")))?;
        st.chr.overlay(&chr);
        st.para.overlay(&ParaProps {
            align: Some(align),
            indent_left: Some(0.0),
            indent_right: Some(0.0),
            indent_first: Some(indent_first),
            line_spacing: Some(LineSpacing::Multiple(line_spacing)),
            page_break_before: Some(page_break_h1 && n == 1),
            space_before: Some(0.0),
            space_after: Some(0.0),
            ..ParaProps::default()
        });
    }
    if numbering {
        link_heading_numbering(s, indent_first)?;
    }
    for i in 0..s.doc.body.len() {
        let path = Path::top(i);
        let Some(para) = s.doc.para(StoryRef::Body, &path) else { continue };
        let style = para.props.style.clone().unwrap_or_default();
        let h1 = is_heading1(s, &para.props);
        let heading = h1 || style.starts_with("Heading");
        if heading {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.align = Some(align);
            para.props.indent_left = None;
            para.props.indent_right = None;
            para.props.indent_first = None;
            para.props.cancel_char_indents(true, true, true);
            if page_break_h1 {
                para.props.page_break_before = Some(h1);
            }
            para.touch();
        }
    }
    Ok(())
}

fn link_heading_numbering(s: &mut Session, indent_first: f32) -> Result<(), CmdError> {
    let indent = if indent_first > 0.0 { indent_first } else { DEFAULT_FIRST_LINE };
    let levels: Vec<Level> = (0..crate::cmd::lists::LEVELS)
        .map(|i| {
            let n = i + 1;
            let text = (1..=n).map(|k| format!("%{k}")).collect::<Vec<_>>().join(".");
            Level {
                format: NumFormat::Decimal,
                text,
                indent,
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
        let Some(para) = s.doc.para(StoryRef::Body, &path) else { continue };
        if para.props.style.as_ref().is_some_and(|st| linked.contains(st)) {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.numbering = None;
            para.touch();
        }
    }
    Ok(())
}

fn normalize_captions(s: &mut Session, cap: &Value) -> Result<(), CmdError> {
    let fig_align = parse_align(p::str(cap, "figureAlign").unwrap_or("center"))?;
    let tab_align = parse_align(p::str(cap, "tableAlign").unwrap_or("left"))?;
    for i in 0..s.doc.body.len() {
        let path = Path::top(i);
        let Some(para) = s.doc.para(StoryRef::Body, &path) else { continue };
        let style = para.props.style.clone().unwrap_or_else(|| "Normal".into());
        let text = para.plain_text();
        let is_cap_style = style == "Caption";
        let fig = caption_figure_prefix(text.trim_start());
        let tab = caption_table_prefix(text.trim_start());
        if !(is_cap_style || fig || tab) {
            continue;
        }
        let para = s.doc.para_mut(StoryRef::Body, &path)?;
        para.props.style = Some("Caption".into());
        para.props.align = Some(if tab { tab_align } else { fig_align });
        para.props.indent_left = Some(0.0);
        para.props.indent_right = Some(0.0);
        para.props.indent_first = Some(0.0);
        para.props.numbering = Some(NumRef { num: 0, level: 0 });
        para.props.cancel_char_indents(true, true, true);
        para.touch();
    }
    Ok(())
}

pub(crate) fn ensure_styles(s: &mut Session, v: &Value) -> CmdResult {
    let styles = v.get("styles").and_then(Value::as_array).ok_or_else(|| CmdError::Params("missing `styles` array".into()))?;
    for (i, st) in styles.iter().enumerate() {
        let id = p::str(st, "id").ok_or_else(|| CmdError::Params(format!("styles[{i}]: missing id")))?;
        if id.is_empty() || id.len() > 255 {
            return Err(CmdError::Params(format!("styles[{i}]: bad id")));
        }
        ensure_style_id(s, id)?;
        let sheet = Arc::make_mut(&mut s.doc.styles);
        let entry = sheet.get_mut(id).ok_or_else(|| CmdError::Params(format!("no style `{id}`")))?;
        if let Some(name) = p::str(st, "name") {
            entry.name = name.to_string();
        }
        if let Some(based) = p::str(st, "basedOn") {
            entry.based_on = Some(based.to_string());
        }
        if let Some(chr) = st.get("chr") {
            entry.chr.overlay(&chr_from_value(chr, "Calibri", 11.0, false));
        }
        if let Some(para) = st.get("para") {
            entry.para.overlay(&para_from_value(para, Align::Left, 0.0, 1.15)?);
        }
    }
    touch_all(s);
    Ok(full_ir(s))
}

pub(crate) fn ensure_heading(s: &mut Session, v: &Value) -> CmdResult {
    let mut headings = json!({
        "levels": p::u64(v, "levels").unwrap_or(3),
        "numbering": p::bool(v, "numbering").unwrap_or(true),
        "indentFirst": p::f32(v, "indentFirst").unwrap_or(0.0),
        "font": p::str(v, "font").unwrap_or("Calibri"),
        "size": p::f32(v, "size").unwrap_or(14.0),
        "pageBreakOnHeading1": true
    });
    if let Some(o) = headings.as_object_mut() {
        if let Some(b) = v.get("bold") {
            o.insert("bold".into(), b.clone());
        }
        if let Some(a) = v.get("align") {
            o.insert("align".into(), a.clone());
        }
    }
    let levels = p::u64(&headings, "levels").unwrap_or(3).clamp(1, 9) as u8;
    ensure_heading_from_profile(s, &headings, levels)?;
    touch_all(s);
    Ok(full_ir(s))
}

pub(crate) fn clear_stray_page_breaks(s: &mut Session, _: &Value) -> CmdResult {
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
        let Some(para) = s.doc.para(StoryRef::Body, &path) else { continue };
        if is_heading1(s, &para.props) {
            continue;
        }
        if para.props.page_break_before == Some(true) {
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            para.props.page_break_before = Some(false);
            para.touch();
            cleared = cleared.saturating_add(1);
        }
    }
    Ok(cleared)
}

fn ensure_style_id(s: &mut Session, id: &str) -> Result<(), CmdError> {
    if s.doc.styles.get(id).is_some() {
        return Ok(());
    }
    if let Some(st) = wordcraft_doc::styles::StyleSheet::builtin().get(id).cloned() {
        Arc::make_mut(&mut s.doc.styles).upsert(st);
        return Ok(());
    }
    Arc::make_mut(&mut s.doc.styles).upsert(Style {
        id: id.into(),
        name: id.into(),
        kind: StyleKind::Paragraph,
        based_on: Some("Normal".into()),
        ..Style::default()
    });
    Ok(())
}

fn chr_from_value(v: &Value, font: &str, size: f32, bold: bool) -> CharProps {
    CharProps {
        font: Some(p::str(v, "font").unwrap_or(font).into()),
        size: Some(p::f32(v, "size").unwrap_or(size).clamp(1.0, 1638.0)),
        bold: Some(p::bool(v, "bold").unwrap_or(bold)),
        italic: p::bool(v, "italic"),
        color: Some(TextColor::Rgb(Rgb::BLACK)),
        ..CharProps::default()
    }
}

fn para_from_value(v: &Value, default_align: Align, default_first: f32, default_ls: f32) -> Result<ParaProps, CmdError> {
    let align = match p::str(v, "align") {
        Some(a) => parse_align(a)?,
        None => default_align,
    };
    Ok(ParaProps {
        align: Some(align),
        indent_left: Some(p::f32(v, "indentLeft").unwrap_or(0.0).clamp(0.0, 1584.0)),
        indent_right: Some(p::f32(v, "indentRight").unwrap_or(0.0).clamp(0.0, 1584.0)),
        indent_first: Some(p::f32(v, "indentFirst").unwrap_or(default_first).clamp(-1584.0, 1584.0)),
        line_spacing: Some(LineSpacing::Multiple(p::f32(v, "lineSpacing").unwrap_or(default_ls).clamp(0.0, 132.0))),
        page_break_before: p::bool(v, "pageBreakBefore").or(Some(false)),
        space_before: Some(p::f32(v, "spaceBefore").unwrap_or(0.0)),
        space_after: Some(p::f32(v, "spaceAfter").unwrap_or(0.0)),
        ..ParaProps::default()
    })
}
