//! Shared helpers for the integration tests.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document, Object, ObjectId};

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The `hideo` binary with a clean, reproducible environment.
pub fn hideo() -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("hideo"));
    cmd.env_remove("SOURCE_DATE_EPOCH").env("NO_COLOR", "1");
    cmd
}

/// Build `fixture_name` into a fresh temp dir and return it (kept alive by the caller).
pub fn build_fixture(
    fixture_name: &str,
    extra: &[&str],
) -> (tempfile::TempDir, std::process::Output) {
    let out = tempfile::tempdir().unwrap();
    let output = hideo()
        .args(["build", "--input"])
        .arg(fixture(fixture_name))
        .arg("--output")
        .arg(out.path())
        .args(extra)
        .output()
        .unwrap();
    (out, output)
}

pub fn load(path: &Path) -> Document {
    Document::load(path).unwrap_or_else(|e| panic!("cannot load {}: {e}", path.display()))
}

pub fn name_of(obj: &Object) -> Option<String> {
    obj.as_name()
        .ok()
        .map(|n| String::from_utf8_lossy(n).into_owned())
}

/// A flattened structure element.
#[derive(Debug, Clone)]
pub struct StructElem {
    pub role: String,
    pub dict: Dictionary,
    pub depth: usize,
    pub id: ObjectId,
    pub parent_role: Option<String>,
}

fn deref<'a>(doc: &'a Document, obj: &'a Object) -> Option<(Option<ObjectId>, &'a Object)> {
    match obj {
        Object::Reference(id) => doc.get_object(*id).ok().map(|o| (Some(*id), o)),
        other => Some((None, other)),
    }
}

/// All structure elements of the tagged PDF in document order.
pub fn struct_elems(doc: &Document) -> Vec<StructElem> {
    let catalog = doc.catalog().expect("catalog");
    let root_obj = catalog
        .get(b"StructTreeRoot")
        .expect("tagged PDF has a StructTreeRoot");
    let (_, root) = deref(doc, root_obj).unwrap();
    let root = root.as_dict().unwrap();
    let mut out = Vec::new();
    if let Ok(k) = root.get(b"K") {
        walk(doc, k, 0, None, &mut out);
    }
    out
}

fn walk(
    doc: &Document,
    obj: &Object,
    depth: usize,
    parent: Option<&str>,
    out: &mut Vec<StructElem>,
) {
    let Some((id, resolved)) = deref(doc, obj) else {
        return;
    };
    match resolved {
        Object::Array(items) => items.iter().for_each(|i| walk(doc, i, depth, parent, out)),
        Object::Dictionary(d) => {
            let Ok(s) = d.get(b"S") else { return };
            let role = name_of(deref(doc, s).map(|(_, o)| o).unwrap_or(s)).unwrap_or_default();
            out.push(StructElem {
                role: role.clone(),
                dict: d.clone(),
                depth,
                id: id.unwrap_or((0, 0)),
                parent_role: parent.map(str::to_string),
            });
            if let Ok(k) = d.get(b"K") {
                walk(doc, k, depth + 1, Some(&role), out);
            }
        }
        _ => {}
    }
}

pub fn count_roles(elems: &[StructElem]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for e in elems {
        *m.entry(e.role.clone()).or_insert(0) += 1;
    }
    m
}

/// Recursively copy a directory (fixtures are small).
pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

pub fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Text of 1-based page `n`.
pub fn page_text(doc: &Document, n: u32) -> String {
    doc.extract_text(&[n])
        .unwrap_or_else(|e| panic!("cannot extract page {n}: {e}"))
}

pub fn page_count(doc: &Document) -> usize {
    doc.get_pages().len()
}

/// Marked-content summary of a page: how many text-showing operators sit inside an
/// `/Artifact` sequence and how many inside tagged (non-artifact) sequences.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TextOps {
    pub in_artifact: usize,
    pub in_tagged: usize,
    pub untagged: usize,
}

pub fn text_ops(doc: &Document, page: u32) -> TextOps {
    let page_id = *doc.get_pages().get(&page).expect("page exists");
    let data = doc.get_page_content(page_id);
    let content = lopdf::content::Content::decode(&data).expect("decodes");
    let mut stack: Vec<bool> = Vec::new(); // true = artifact
    let mut ops = TextOps::default();
    for op in content.operations {
        match op.operator.as_str() {
            "BMC" | "BDC" => {
                let tag = op.operands.first().and_then(name_of).unwrap_or_default();
                stack.push(tag == "Artifact");
            }
            "EMC" => {
                stack.pop();
            }
            "Tj" | "TJ" | "'" | "\"" => match stack.iter().rev().find(|_| true) {
                None => ops.untagged += 1,
                Some(_) if stack.iter().any(|a| *a) => ops.in_artifact += 1,
                Some(_) => ops.in_tagged += 1,
            },
            _ => {}
        }
    }
    ops
}
