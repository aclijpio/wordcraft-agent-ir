//! Figures, captions, TOC, fields, cross-refs, table style by index.

use serde_json::{Value, json};
use wordcraft_doc::{Block, Path, Pos, StoryRef};

use super::full_ir;
use crate::{CmdError, CmdResult, Selection, Session, p};

pub(crate) fn insert_figure(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(index) = p::u64(v, "index") {
        let index = index as usize;
        if index >= s.doc.body.len() {
            return Err(CmdError::Params(format!("index {index} out of range")));
        }
        let path = Path::top(index);
        let off = s.doc.para(StoryRef::Body, &path).map(|para| para.len()).unwrap_or(0);
        s.sel = Selection::caret(Pos { story: StoryRef::Body, path, off });
    }
    let mut pic = json!({});
    if let Some(path) = p::str(v, "path") {
        pic["path"] = json!(path);
    }
    if let Some(data) = p::str(v, "data") {
        pic["data"] = json!(data);
    }
    if pic.get("path").is_none() && pic.get("data").is_none() {
        return Err(CmdError::Params("insertFigure needs `path` or `data`".into()));
    }
    if let Some(w) = v.get("width") {
        pic["width"] = w.clone();
    }
    if let Some(alt) = v.get("alt") {
        pic["alt"] = alt.clone();
    }
    s.run("insert.picture", &pic)?;
    if let Some(align) = p::str(v, "align") {
        let _ = s.run("para.align", &json!({"align": align}));
    } else {
        let _ = s.run("para.align", &json!({"align": "center"}));
    }
    if let Some(caption) = p::str(v, "caption") {
        let mut cap = json!({
            "text": caption,
            "position": p::str(v, "position").unwrap_or("below"),
        });
        if let Some(label) = p::str(v, "captionLabel") {
            cap["label"] = json!(label);
        }
        if let Some(format) = p::str(v, "format") {
            cap["format"] = json!(format);
        }
        if let Some(sep) = p::str(v, "separator") {
            cap["separator"] = json!(sep);
        }
        s.run("references.caption", &cap)?;
    }
    Ok(full_ir(s))
}

pub(crate) fn insert_caption(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(index) = p::u64(v, "index") {
        let index = index as usize;
        if index >= s.doc.body.len() {
            return Err(CmdError::Params(format!("index {index} out of range")));
        }
        let path = Path::top(index);
        let off = s.doc.para(StoryRef::Body, &path).map(|para| para.len()).unwrap_or(0);
        s.sel = Selection::caret(Pos { story: StoryRef::Body, path, off });
    }
    let mut params = v.clone();
    if let Some(o) = params.as_object_mut() {
        o.remove("index");
    }
    s.run("references.caption", &params)?;
    Ok(full_ir(s))
}

pub(crate) fn insert_toc(s: &mut Session, v: &Value) -> CmdResult {
    s.run("references.toc", v)?;
    Ok(full_ir(s))
}

pub(crate) fn update_toc(s: &mut Session, v: &Value) -> CmdResult {
    s.run("references.updateToc", v)?;
    Ok(full_ir(s))
}

pub(crate) fn remove_toc(s: &mut Session, v: &Value) -> CmdResult {
    s.run("references.removeToc", v)?;
    Ok(full_ir(s))
}

pub(crate) fn update_fields(s: &mut Session, v: &Value) -> CmdResult {
    s.run("references.updateFields", v)?;
    Ok(full_ir(s))
}

pub(crate) fn insert_cross_ref(s: &mut Session, v: &Value) -> CmdResult {
    s.run("insert.crossReference", v)?;
    Ok(full_ir(s))
}

pub(crate) fn apply_table_style(s: &mut Session, v: &Value) -> CmdResult {
    let index = p::u64(v, "index").ok_or_else(|| CmdError::Params("missing `index`".into()))? as usize;
    let style = p::str(v, "style").ok_or_else(|| CmdError::Params("missing `style`".into()))?;
    if index >= s.doc.body.len() {
        return Err(CmdError::Params(format!("index {index} out of range")));
    }
    let Some(block) = s.doc.body.get_mut(index) else {
        return Err(CmdError::Params(format!("index {index} out of range")));
    };
    let block = std::sync::Arc::make_mut(block);
    let Block::Table(t) = block else {
        return Err(CmdError::Params(format!("block {index} is not a table")));
    };
    t.props.style = Some(style.to_string());
    Ok(full_ir(s))
}
