//! The Typst `World`: all inputs live in memory (plan 7.8).
//!
//! Only the four embedded font families exist (no system fonts), so output cannot vary with the
//! machine. Typst types are confined to this file and `compile.rs` (plan risk table).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::LazyLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::{self, FontStore};

static LIBRARY: LazyLock<LazyHash<Library>> = LazyLock::new(|| LazyHash::new(Library::default()));

static FONTS: LazyLock<FontStore> = LazyLock::new(|| {
    let mut store = FontStore::new();
    store.extend(fonts::embedded());
    store
});

/// All virtual files of one document: generated `.typ` pages, JPEGs, SVGs.
struct MemLoader {
    files: HashMap<FileId, Bytes>,
}

impl FileLoader for MemLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        self.files
            .get(&id)
            .cloned()
            .ok_or_else(|| FileError::NotFound(PathBuf::from(id.vpath().get_without_slash())))
    }
}

/// Unicode coverage of each embedded font, for character coverage checks.
static EMBEDDED_COVERAGE: LazyLock<Vec<typst::text::FontInfo>> =
    LazyLock::new(|| fonts::embedded().map(|(_font, info)| info).collect());

/// Characters of `text` that none of the embedded fonts can display. Typst falls back across
/// all fonts, so the union of their coverage is what counts. Whitespace, controls and
/// zero-width characters are never reported.
pub fn uncovered_chars(text: &str) -> Vec<char> {
    let mut seen = Vec::new();
    for c in text.chars() {
        let ignorable = c.is_whitespace()
            || c.is_control()
            || matches!(c, '\u{200B}'..='\u{200D}' | '\u{FEFF}' | '\u{00AD}');
        if ignorable || seen.contains(&c) {
            continue;
        }
        if !EMBEDDED_COVERAGE
            .iter()
            .any(|info| info.coverage.contains(c as u32))
        {
            seen.push(c);
        }
    }
    seen
}

pub struct HideoWorld {
    main: FileId,
    files: FileStore<MemLoader>,
    today: Datetime,
}

/// The id of a virtual project file. One id per distinct file (ids are interned and limited
/// to 65 535 per process).
pub fn file_id(path: &str) -> Result<FileId, String> {
    let vpath = VirtualPath::new(path).map_err(|e| format!("bad virtual path `{path}`: {e:?}"))?;
    Ok(FileId::new(RootedPath::new(VirtualRoot::Project, vpath)))
}

impl HideoWorld {
    /// `files` maps virtual paths (`main.typ`, `pages/001.typ`, `images/x.jpg`) to contents.
    /// `main.typ` must be present.
    pub fn new(files: &BTreeMap<String, Vec<u8>>, today: Datetime) -> Result<Self, String> {
        let mut map = HashMap::with_capacity(files.len());
        for (path, bytes) in files {
            map.insert(file_id(path)?, Bytes::new(bytes.clone()));
        }
        let main = file_id("main.typ")?;
        if !map.contains_key(&main) {
            return Err("main.typ is missing".to_string());
        }
        Ok(Self {
            main,
            files: FileStore::new(MemLoader { files: map }),
            today,
        })
    }

    /// Convenience for tests: a world with one `main.typ`.
    pub fn from_source(source: &str, today: Datetime) -> Self {
        let mut files = BTreeMap::new();
        files.insert("main.typ".to_string(), source.as_bytes().to_vec());
        Self::new(&files, today).expect("main.typ present")
    }
}

impl World for HideoWorld {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        FONTS.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        FONTS.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        Some(self.today)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_paths_use_forward_slashes() {
        let id = file_id("pages/001.typ").unwrap();
        assert_eq!(id.vpath().get_without_slash(), "pages/001.typ");
    }

    #[test]
    fn uncovered_chars_finds_scripts_the_fonts_lack() {
        assert!(uncovered_chars("Hello äöå ÄÖÅ é € — … ‘quotes’ \u{200B}\t\n").is_empty());
        assert_eq!(uncovered_chars("a日本語b日"), vec!['日', '本', '語']);
        assert!(uncovered_chars("").is_empty());
    }

    #[test]
    fn test_missing_main_is_an_error() {
        let files = BTreeMap::new();
        assert!(HideoWorld::new(&files, Datetime::from_ymd(2000, 1, 1).unwrap()).is_err());
    }
}
