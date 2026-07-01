use std::path::Path;

use crate::tool::documents;

pub(crate) const ALL_DOC_TYPES: &[&str] = &[
    "dsgn", "plan", "pdsg", "ddtl", "revw", "task", "ctrt", "rprt", "anls", "reqs", "precepts",
];

/// Read a document's content by its full ID (e.g. "dsgn_005").
pub(crate) async fn read_doc_by_id(working_dir: &Path, doc_id: &str) -> Option<String> {
    let prefix = doc_id.split('_').next().unwrap_or("");
    let dir = documents::type_to_dir(prefix);
    let rel_path = if dir.is_empty() {
        format!(".shuji/{}.md", doc_id)
    } else {
        format!(".shuji/{}/{}.md", dir, doc_id)
    };
    let full = crate::tool::resolve_scoped_path(working_dir, &rel_path)
        .await
        .ok()?;
    tokio::fs::read_to_string(&full).await.ok()
}

/// Find a document by its numeric ID (from refs), searching across all types.
pub(crate) async fn find_by_numeric_id(working_dir: &Path, num: u64) -> Option<(String, String)> {
    for prefix in ALL_DOC_TYPES {
        let doc_id = format!("{}_{:03}", prefix, num);
        if let Some(content) = read_doc_by_id(working_dir, &doc_id).await {
            return Some((doc_id, content));
        }
    }
    None
}
