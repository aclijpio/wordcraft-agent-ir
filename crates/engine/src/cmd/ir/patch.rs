//! Structural IR patch ops.

use serde_json::{Value, json};
use wordcraft_doc::props::{CharProps, LineSpacing, NumRef};
use wordcraft_doc::{Block, Paragraph, Path, StoryRef, Table};

use super::ids::{block_id_at, ensure_all_block_ids, resolve_op_index, set_block_id};
use super::{find_nth, full_ir, max_patch_ops, parse_align};
use crate::{CmdError, CmdResult, Session, p};

pub(crate) fn apply_ir_patch(s: &mut Session, v: &Value) -> CmdResult {
    let ops = v.get("ops").and_then(Value::as_array).ok_or_else(|| CmdError::Params("missing `ops` array".into()))?;
    if ops.len() > max_patch_ops() {
        return Err(CmdError::Params(format!("at most {} ops", max_patch_ops())));
    }
    if ops.iter().any(|op| op.get("id").and_then(Value::as_str).is_some_and(|id| !id.is_empty())) {
        let _ = ensure_all_block_ids(s)?;
    }
    let mut changed = 0u32;
    for (i, op) in ops.iter().enumerate() {
        let kind = p::str(op, "op").ok_or_else(|| CmdError::Params(format!("ops[{i}]: missing `op`")))?;
        match kind {
            "update" => {
                let index = resolve_op_index(s, op, i)?;
                let set = op.get("set").cloned().unwrap_or(json!({}));
                apply_update(s, index, &set)?;
                changed = changed.saturating_add(1);
            }
            "replace" => {
                let index = resolve_op_index(s, op, i)?;
                let block = op.get("block").ok_or_else(|| CmdError::Params(format!("ops[{i}]: replace needs `block`")))?;
                apply_replace(s, index, block)?;
                changed = changed.saturating_add(1);
            }
            "replaceIn" => {
                let index = resolve_op_index(s, op, i)?;
                let find = p::str(op, "find").ok_or_else(|| CmdError::Params(format!("ops[{i}]: replaceIn needs `find`")))?;
                let with = p::str(op, "with").ok_or_else(|| CmdError::Params(format!("ops[{i}]: replaceIn needs `with`")))?;
                let occurrence = p::u64(op, "occurrence").unwrap_or(1).max(1) as usize;
                replace_in_paragraph(s, index, find, with, occurrence)?;
                changed = changed.saturating_add(1);
            }
            "insert" => {
                let at = apply_insert(s, op, i)?;
                if let Some(id) = p::str(op, "id").or_else(|| op.get("block").and_then(|b| p::str(b, "id"))) {
                    let _ = set_block_id(s, at, id);
                } else {
                    let _ = ensure_all_block_ids(s)?;
                }
                changed = changed.saturating_add(1);
            }
            "delete" => {
                if op.get("from").is_some() || op.get("to").is_some() {
                    let from = p::u64(op, "from").ok_or_else(|| CmdError::Params(format!("ops[{i}]: range delete needs `from`")))? as usize;
                    let to = p::u64(op, "to").ok_or_else(|| CmdError::Params(format!("ops[{i}]: range delete needs `to`")))? as usize;
                    if to < from {
                        return Err(CmdError::Params(format!("ops[{i}]: `to` must be >= `from`")));
                    }
                    if from >= s.doc.body.len() {
                        return Err(CmdError::Params(format!("ops[{i}]: from {from} out of range")));
                    }
                    let to = to.min(s.doc.body.len());
                    let n = to.saturating_sub(from);
                    for _ in 0..n {
                        s.doc.remove_block(StoryRef::Body, &Path::top(from)).map_err(|e| CmdError::Failed(e.to_string()))?;
                        changed = changed.saturating_add(1);
                    }
                } else {
                    let index = resolve_op_index(s, op, i)?;
                    s.doc.remove_block(StoryRef::Body, &Path::top(index)).map_err(|e| CmdError::Failed(e.to_string()))?;
                    changed = changed.saturating_add(1);
                }
            }
            "move" => {
                let from = if op.get("id").is_some() {
                    resolve_op_index(s, op, i)?
                } else {
                    p::u64(op, "from").or_else(|| p::u64(op, "index")).ok_or_else(|| CmdError::Params(format!("ops[{i}]: move needs `from`")))?
                        as usize
                };
                let to = p::u64(op, "to").ok_or_else(|| CmdError::Params(format!("ops[{i}]: move needs `to`")))? as usize;
                move_block(s, from, to)?;
                changed = changed.saturating_add(1);
            }
            "setStyle" => {
                let index = resolve_op_index(s, op, i)?;
                let style = p::str(op, "style").or_else(|| op.get("set").and_then(|x| p::str(x, "style")))
                    .ok_or_else(|| CmdError::Params(format!("ops[{i}]: setStyle needs `style`")))?;
                let path = Path::top(index);
                let para = s.doc.para_mut(StoryRef::Body, &path).map_err(|_| CmdError::Params(format!("block {index} is not a paragraph")))?;
                para.props.style = Some(style.to_string());
                para.props.outline_level = None;
                para.touch();
                changed = changed.saturating_add(1);
            }
            "setCell" => {
                set_cell(s, op, i)?;
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
    let keep_id = block_id_at(s, index);
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
        if let Some(id) = keep_id.as_ref() {
            set_block_id(s, index, id)?;
        }
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
    if let Some(level) = p::u64(set, "listLevel")
        && let Some(n) = para.props.numbering.filter(|n| n.num != 0)
    {
        para.props.numbering = Some(NumRef { num: n.num, level: (level as u8).min(8) });
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
    // Optional run mark formatting applied to whole paragraph text.
    let mut mark_patch = CharProps::default();
    let mut any_chr = false;
    if let Some(font) = p::str(set, "font") {
        mark_patch.font = Some(font.into());
        any_chr = true;
    }
    if let Some(size) = p::f32(set, "size") {
        mark_patch.size = Some(size.clamp(1.0, 1638.0));
        any_chr = true;
    }
    if let Some(bold) = p::bool(set, "bold") {
        mark_patch.bold = Some(bold);
        any_chr = true;
    }
    if let Some(italic) = p::bool(set, "italic") {
        mark_patch.italic = Some(italic);
        any_chr = true;
    }
    if any_chr {
        let len = para.len();
        if len > 0 {
            let patch = mark_patch.clone();
            para.format(0, len, &|c| c.overlay(&patch)).map_err(|e| CmdError::Failed(e.to_string()))?;
        }
        para.mark.overlay(&mark_patch);
    }
    para.touch();
    Ok(())
}

fn plain_to_raw(para: &Paragraph, plain_off: usize) -> Option<usize> {
    let mut plain = 0usize;
    let mut raw = 0usize;
    let mut oi = 0usize;
    for c in para.text.chars() {
        if plain == plain_off {
            return Some(raw);
        }
        let raw_len = c.len_utf8();
        if c == wordcraft_doc::para::OBJ {
            let chunk_len = para.objects.get(oi).map(|o| o.plain_text().len()).unwrap_or(0);
            oi = oi.saturating_add(1);
            if plain.saturating_add(chunk_len) > plain_off {
                return Some(raw);
            }
            plain = plain.saturating_add(chunk_len);
            raw = raw.saturating_add(raw_len);
        } else {
            if plain.saturating_add(raw_len) > plain_off {
                return None;
            }
            plain = plain.saturating_add(raw_len);
            raw = raw.saturating_add(raw_len);
        }
    }
    (plain == plain_off).then_some(raw)
}

fn apply_replace(s: &mut Session, index: usize, block: &Value) -> Result<(), CmdError> {
    let ty = p::str(block, "type").unwrap_or("paragraph");
    let keep_id = p::str(block, "id").map(str::to_string).or_else(|| block_id_at(s, index));
    match ty {
        "paragraph" => {
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
            apply_update(s, index, &set)?;
        }
        "table" => {
            let new_block = table_block_from_ir(block, "replace")?;
            s.doc.remove_block(StoryRef::Body, &Path::top(index)).map_err(|e| CmdError::Failed(e.to_string()))?;
            s.doc
                .insert_block(StoryRef::Body, &Path::top(index), new_block)
                .map_err(|e| CmdError::Failed(e.to_string()))?;
            if let Some(id) = keep_id.as_deref() {
                set_block_id(s, index, id)?;
            }
        }
        other => return Err(CmdError::Params(format!("replace does not support type `{other}`"))),
    }
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
    let plain_start = find_nth(&text, find, occurrence)
        .ok_or_else(|| CmdError::Params(format!("occurrence {occurrence} of `find` not in paragraph {index}")))?;
    let plain_end = plain_start.saturating_add(find.len());
    if !text.is_char_boundary(plain_start) || !text.is_char_boundary(plain_end) {
        return Err(CmdError::Params("replace range is not on a char boundary".into()));
    }
    let start = plain_to_raw(para, plain_start).ok_or_else(|| CmdError::Params("replace start is not on a char boundary".into()))?;
    let end = plain_to_raw(para, plain_end).ok_or_else(|| CmdError::Params("replace end is not on a char boundary".into()))?;
    let mark = para.props_at(start).clone();
    let para = s.doc.para_mut(StoryRef::Body, &path)?;
    para.delete(start, end).map_err(|e| CmdError::Failed(e.to_string()))?;
    if !with.is_empty() {
        para.insert_text(start, with, &mark).map_err(|e| CmdError::Failed(e.to_string()))?;
    }
    para.touch();
    Ok(())
}

fn apply_insert(s: &mut Session, op: &Value, i: usize) -> Result<usize, CmdError> {
    let at = match op.get("index") {
        None => s.doc.body.len(),
        Some(Value::Null) => s.doc.body.len(),
        Some(x) if x.as_i64() == Some(-1) => s.doc.body.len(),
        Some(_) => p::u64(op, "index").unwrap_or(0) as usize,
    };
    let at = at.min(s.doc.body.len());
    let block = op.get("block").cloned().unwrap_or_else(|| json!({"type": "paragraph", "text": ""}));
    let ty = p::str(&block, "type").unwrap_or("paragraph");
    let new_block = match ty {
        "paragraph" => {
            let text = p::str(&block, "text").unwrap_or("");
            let style = p::str(&block, "style").unwrap_or("Normal");
            let mut para = Paragraph::with_text(text, CharProps::default());
            para.props.style = Some(style.to_string());
            if let Some(a) = p::str(&block, "align") {
                para.props.align = Some(parse_align(a)?);
            }
            if let Some(f) = p::f32(&block, "indentFirst") {
                para.props.indent_first = Some(f);
            }
            if let Some(on) = p::bool(&block, "pageBreakBefore") {
                para.props.page_break_before = Some(on);
            }
            Block::Para(para)
        }
        "table" => table_block_from_ir(&block, &format!("ops[{i}]"))?,
        other => return Err(CmdError::Params(format!("ops[{i}]: unsupported insert type `{other}`"))),
    };
    s.doc.insert_block(StoryRef::Body, &Path::top(at), new_block).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok(at)
}

fn table_block_from_ir(block: &Value, ctx: &str) -> Result<Block, CmdError> {
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
    if rows == 0 || cols == 0 {
        return Err(CmdError::Params(format!("{ctx}: table needs rows and cols")));
    }
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

fn move_block(s: &mut Session, from: usize, to: usize) -> Result<(), CmdError> {
    if from >= s.doc.body.len() {
        return Err(CmdError::Params(format!("move from {from} out of range")));
    }
    let to = to.min(s.doc.body.len().saturating_sub(1));
    if from == to {
        return Ok(());
    }
    let block = s.doc.remove_block(StoryRef::Body, &Path::top(from)).map_err(|e| CmdError::Failed(e.to_string()))?;
    let insert_at = to.min(s.doc.body.len());
    s.doc.insert_block(StoryRef::Body, &Path::top(insert_at), block).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok(())
}

fn set_cell(s: &mut Session, op: &Value, i: usize) -> Result<(), CmdError> {
    let index = resolve_op_index(s, op, i)?;
    let row = p::u64(op, "row").ok_or_else(|| CmdError::Params(format!("ops[{i}]: setCell needs `row`")))? as usize;
    let col = p::u64(op, "col").ok_or_else(|| CmdError::Params(format!("ops[{i}]: setCell needs `col`")))? as usize;
    let Some(Block::Table(_)) = s.doc.body.get(index).map(|b| &**b) else {
        return Err(CmdError::Params(format!("block {index} is not a table")));
    };
    let path = Path(vec![index as u32, row as u32, col as u32, 0]);
    if s.doc.para(StoryRef::Body, &path).is_none() {
        return Err(CmdError::Params(format!("ops[{i}]: cell ({row},{col}) missing")));
    }
    if let Some(set) = op.get("set") {
        let text = p::str(set, "text");
        if let Some(text) = text {
            let mark = s
                .doc
                .para(StoryRef::Body, &path)
                .ok_or_else(|| CmdError::Params(format!("ops[{i}]: cell ({row},{col}) missing")))?
                .mark
                .clone();
            let para = s.doc.para_mut(StoryRef::Body, &path)?;
            let len = para.len();
            if len > 0 {
                para.delete(0, len).map_err(|e| CmdError::Failed(e.to_string()))?;
            }
            if !text.is_empty() {
                para.insert_text(0, text, &mark).map_err(|e| CmdError::Failed(e.to_string()))?;
            }
            para.touch();
        }
        let para = s.doc.para_mut(StoryRef::Body, &path)?;
        if let Some(a) = p::str(set, "align") {
            para.props.align = Some(parse_align(a)?);
        }
        para.touch();
    } else if let Some(text) = p::str(op, "text") {
        let mark = s
            .doc
            .para(StoryRef::Body, &path)
            .ok_or_else(|| CmdError::Params(format!("ops[{i}]: cell ({row},{col}) missing")))?
            .mark
            .clone();
        let para = s.doc.para_mut(StoryRef::Body, &path)?;
        let len = para.len();
        if len > 0 {
            para.delete(0, len).map_err(|e| CmdError::Failed(e.to_string()))?;
        }
        if !text.is_empty() {
            para.insert_text(0, text, &mark).map_err(|e| CmdError::Failed(e.to_string()))?;
        }
        para.touch();
    } else {
        return Err(CmdError::Params(format!("ops[{i}]: setCell needs `text` or `set`")));
    }
    Ok(())
}
