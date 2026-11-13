//! Stable IR block ids anchored as hidden bookmarks `_WcIr_<id>`.

use std::collections::HashSet;

use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Block, InlineObject, Path, StoryRef};

use crate::{CmdError, Session};

pub(crate) const IR_BOOKMARK_PREFIX: &str = "_WcIr_";

pub(crate) fn bookmark_name(id: &str) -> String {
    format!("{IR_BOOKMARK_PREFIX}{id}")
}

pub(crate) fn parse_ir_id(name: &str) -> Option<&str> {
    name.strip_prefix(IR_BOOKMARK_PREFIX).filter(|id| !id.is_empty() && id.chars().count() <= 34)
}

pub(crate) fn block_id_at(s: &Session, index: usize) -> Option<String> {
    let path = anchor_path(&s.doc, index)?;
    let para = s.doc.para(StoryRef::Body, &path)?;
    for (_, o) in para.objects_with_offsets() {
        if let InlineObject::BookmarkStart { name } = o
            && let Some(id) = parse_ir_id(name)
        {
            return Some(id.to_string());
        }
    }
    None
}

pub(crate) fn find_index_by_id(s: &Session, id: &str) -> Option<usize> {
    let want = bookmark_name(id);
    for (name, pos) in s.doc.bookmarks() {
        if name == want {
            return pos.path.0.first().copied().map(|i| i as usize);
        }
    }
    None
}

pub(crate) fn ensure_all_block_ids(s: &mut Session) -> Result<Vec<String>, CmdError> {
    let n = s.doc.body.len();
    let mut used: HashSet<String> = HashSet::new();
    for i in 0..n {
        if let Some(id) = block_id_at(s, i) {
            used.insert(id);
        }
    }
    let mut ids = Vec::with_capacity(n);
    for i in 0..n {
        if let Some(id) = block_id_at(s, i) {
            ids.push(id);
            continue;
        }
        let id = alloc_id(&used, i, s.document_id(), s.rev());
        used.insert(id.clone());
        set_block_id(s, i, &id)?;
        ids.push(id);
    }
    Ok(ids)
}

pub(crate) fn set_block_id(s: &mut Session, index: usize, id: &str) -> Result<(), CmdError> {
    if id.is_empty() || id.chars().count() > 34 {
        return Err(CmdError::Params("bad IR block id".into()));
    }
    let name = bookmark_name(id);
    if !crate::cmd::insert::bookmark_name_ok(&name) {
        return Err(CmdError::Params("IR bookmark name rejected".into()));
    }
    clear_ir_bookmarks_on_block(s, index)?;
    let path = anchor_path(&s.doc, index).ok_or_else(|| CmdError::Params(format!("block {index} has no anchor paragraph")))?;
    let para = s.doc.para_mut(StoryRef::Body, &path)?;
    let mark = CharProps::default();
    para.insert_object(0, InlineObject::BookmarkEnd { name: name.clone() }, &mark).map_err(|e| CmdError::Failed(e.to_string()))?;
    para.insert_object(0, InlineObject::BookmarkStart { name }, &mark).map_err(|e| CmdError::Failed(e.to_string()))?;
    para.touch();
    Ok(())
}

pub(crate) fn clear_ir_bookmarks_on_block(s: &mut Session, index: usize) -> Result<(), CmdError> {
    let Some(path) = anchor_path(&s.doc, index) else {
        return Ok(());
    };
    let Some(para) = s.doc.para(StoryRef::Body, &path) else {
        return Ok(());
    };
    let mark_len = wordcraft_doc::para::OBJ.len_utf8();
    let mut offs = Vec::new();
    for off in para.object_offsets() {
        match para.object_at(off) {
            Some(InlineObject::BookmarkStart { name }) | Some(InlineObject::BookmarkEnd { name }) if parse_ir_id(name).is_some() => {
                offs.push(off);
            }
            _ => {}
        }
    }
    if offs.is_empty() {
        return Ok(());
    }
    let para = s.doc.para_mut(StoryRef::Body, &path)?;
    for off in offs.into_iter().rev() {
        para.delete(off, off.saturating_add(mark_len)).map_err(|e| CmdError::Failed(e.to_string()))?;
        for at in [&mut s.sel.anchor, &mut s.sel.focus] {
            if at.story == StoryRef::Body && at.path == path && at.off > off {
                at.off = at.off.saturating_sub(mark_len);
            }
        }
    }
    para.touch();
    Ok(())
}

fn anchor_path(doc: &wordcraft_doc::Document, index: usize) -> Option<Path> {
    match doc.body.get(index).map(|b| &**b)? {
        Block::Para(_) => Some(Path::top(index)),
        Block::Table(t) => {
            let _ = t.rows.first()?.cells.first()?.blocks.first()?;
            Some(Path(vec![index as u32, 0, 0, 0]))
        }
    }
}

fn alloc_id(used: &HashSet<String>, index: usize, document_id: u64, rev: u64) -> String {
    let mut n = document_id.wrapping_mul(1_000_003).wrapping_add(rev.wrapping_mul(97)).wrapping_add(index as u64).wrapping_add(1);
    for _ in 0..10_000 {
        let id = format!("b{n:08x}");
        if !used.contains(&id) {
            return id;
        }
        n = n.wrapping_add(0x9E37_79B9);
    }
    format!("b{index:08x}")
}

pub(crate) fn resolve_op_index(s: &Session, op: &serde_json::Value, i: usize) -> Result<usize, CmdError> {
    if let Some(id) = crate::p::str(op, "id") {
        return find_index_by_id(s, id).ok_or_else(|| CmdError::Params(format!("ops[{i}]: unknown id `{id}`")));
    }
    let index = crate::p::u64(op, "index").ok_or_else(|| CmdError::Params(format!("ops[{i}]: missing `index` or `id`")))? as usize;
    if index >= s.doc.body.len() {
        return Err(CmdError::Params(format!("ops[{i}]: index {index} out of range")));
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::Document;

    #[test]
    fn bookmark_round_trip() {
        let mut s = Session::new(Document::from_text("A\nB"));
        let ids = ensure_all_block_ids(&mut s).unwrap();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        assert_eq!(find_index_by_id(&s, &ids[1]), Some(1));
        assert_eq!(block_id_at(&s, 0).as_deref(), Some(ids[0].as_str()));
    }
}
