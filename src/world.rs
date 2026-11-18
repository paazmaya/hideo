use std::collections::HashMap;
use std::sync::LazyLock;
use std::path::PathBuf;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, Source, VirtualPath, RootedPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::{self, FontStore};

static LIBRARY: LazyLock<LazyHash<Library>> =
    LazyLock::new(|| LazyHash::new(Library::default()));

static FONTS: LazyLock<FontStore> = LazyLock::new(|| {
    let mut store = FontStore::new();
    store.extend(fonts::embedded());
    store
});

struct MemLoader {
    files: HashMap<FileId, Bytes>,
}

impl FileLoader for MemLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        self.files.get(&id).cloned().ok_or_else(|| {
            FileError::NotFound(PathBuf::from(id.vpath().get_without_slash()))
        })
    }
}

pub struct HideoWorld {
    main: FileId,
    files: FileStore<MemLoader>,
    today: Datetime,
}

impl HideoWorld {
    pub fn new(main_source: String, files: HashMap<String, String>, today: Datetime) -> Self {
        let mut file_map = HashMap::new();

        let main_id = FileId::new(RootedPath::new(VirtualRoot::Project, VirtualPath::new("main.typ").unwrap()));
        file_map.insert(main_id, Bytes::new(main_source.into_bytes()));

        for (path, content) in files {
            let virtual_path = VirtualPath::new(&path).unwrap();
            let id = FileId::new(RootedPath::new(VirtualRoot::Project, virtual_path));
            file_map.insert(id, Bytes::new(content.into_bytes()));
        }

        let loader = MemLoader { files: file_map };
        let files = FileStore::new(loader);

        Self {
            main: main_id,
            files,
            today,
        }
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