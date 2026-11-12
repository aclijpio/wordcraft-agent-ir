//! Agent templates: built-in ids + optional path via file.newFromTemplate.

use serde_json::{Value, json};

use super::full_ir;
use crate::{CmdError, CmdResult, Session, p};

pub(crate) fn new_from_template(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(path) = p::str(v, "path") {
        s.run("file.newFromTemplate", &json!({"path": path, "replaceBlank": p::bool(v, "replaceBlank").unwrap_or(false)}))?;
        return Ok(full_ir(s));
    }
    let id = p::str(v, "id").or_else(|| p::str(v, "template")).unwrap_or("blank");
    match id {
        "blank" | "sample" | "letter" | "resume" | "report" => {
            s.run("file.new", &json!({"template": id}))?;
        }
        "agent-blank" => {
            s.run("file.new", &json!({"template": "blank"}))?;
        }
        "agent-report" => {
            s.run("file.new", &json!({"template": "report"}))?;
        }
        "agent-letter" => {
            s.run("file.new", &json!({"template": "letter"}))?;
        }
        other => {
            // Resolve packaged template relative to CARGO_MANIFEST_DIR / packaging when present.
            let packaged = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packaging/agent-ir/templates")
                .join(format!("{other}.docx"));
            if packaged.is_file() {
                s.run(
                    "file.newFromTemplate",
                    &json!({"path": packaged.to_string_lossy(), "replaceBlank": p::bool(v, "replaceBlank").unwrap_or(false)}),
                )?;
            } else {
                return Err(CmdError::Params(format!(
                    "unknown template `{other}` (use blank|sample|letter|resume|report|agent-* or path)"
                )));
            }
        }
    }
    Ok(full_ir(s))
}
