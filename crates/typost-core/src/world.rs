//! The Typst [`World`] for a typost project.

use std::collections::HashMap;
use std::path::PathBuf;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::{self, FontStore};

use crate::plugin::TypstOverlay;

/// Serves project files from disk, with an in-memory overlay taking precedence.
///
/// The overlay carries plugin-contributed Typst modules so they resolve without
/// being written to disk.
pub struct ProjectLoader {
    root: PathBuf,
    overlay: HashMap<VirtualPath, Bytes>,
}

impl FileLoader for ProjectLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let vpath = id.vpath();
        if let Some(bytes) = self.overlay.get(vpath) {
            return Ok(bytes.clone());
        }
        let path = vpath.realize(&self.root).map_err(FileError::Realize)?;
        let data = std::fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Bytes::new(data))
    }
}

/// The compilation environment for a typost project.
pub struct TypostWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    store: FileStore<ProjectLoader>,
    today: Time,
}

impl TypostWorld {
    /// Create a world for the given project root and entry file (relative to
    /// the root), with the plugins' Typst overlay applied.
    pub fn new(root: PathBuf, entry: &str, overlay: TypstOverlay) -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(fonts::embedded());

        let entry = VirtualPath::new(entry).expect("entry path must be valid");
        let main = RootedPath::new(VirtualRoot::Project, entry).intern();

        let library = Library::builder()
            .with_features([Feature::Html, Feature::Bundle].into_iter().collect())
            .build();

        let loader = ProjectLoader {
            root,
            overlay: overlay.files,
        };
        Self {
            library: LazyHash::from(library),
            fonts,
            main,
            store: FileStore::new(loader),
            today: Time::system(),
        }
    }
}

impl World for TypostWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.store.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.store.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.today.today(offset)
    }
}
