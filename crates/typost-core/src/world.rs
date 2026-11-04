//! The Typst [`World`] for a typost project.

use std::any::Any;
use std::collections::HashMap;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use typst::diag::FileResult;
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::downloader::Downloader;
use typst_kit::files::{FileLoader, FileStore, FsRoot, SystemFiles};
use typst_kit::fonts::{self, FontStore};
use typst_kit::packages::SystemPackages;

use crate::plugin::TypstOverlay;

/// Downloads Typst packages through `ureq` (rustls-backed).
///
/// Typst Universe is a plain HTTPS registry; `package.rs` in `typst-kit` does
/// the archive handling, so this only needs to provide the bytes.
#[derive(Debug, Default, Clone, Copy)]
struct HttpDownloader;

impl Downloader for HttpDownloader {
    fn stream(&self, _: &dyn Any, url: &str) -> io::Result<(Option<usize>, Box<dyn Read>)> {
        let response = ureq::AgentBuilder::new()
            .user_agent(concat!("typost/", env!("CARGO_PKG_VERSION")))
            .build()
            .get(url)
            .call()
            .map_err(|err| match err {
                ureq::Error::Status(404, _) => io::Error::new(io::ErrorKind::NotFound, err),
                err => io::Error::other(err),
            })?;

        let content_len = response
            .header("Content-Length")
            .and_then(|value| value.parse().ok());
        Ok((content_len, response.into_reader()))
    }
}

/// Serves project files from disk, with an in-memory overlay taking precedence,
/// and Typst packages from the standard package directories and Typst Universe.
///
/// The overlay carries plugin-contributed Typst modules so they resolve without
/// being written to disk.
pub struct ProjectLoader {
    files: SystemFiles,
    overlay: HashMap<VirtualPath, Bytes>,
}

impl ProjectLoader {
    fn new(root: PathBuf, overlay: HashMap<VirtualPath, Bytes>, packages: SystemPackages) -> Self {
        Self {
            files: SystemFiles::new(FsRoot::new(root), packages),
            overlay,
        }
    }
}

impl FileLoader for ProjectLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        // Overlays belong to the project; never let them shadow a package file.
        if matches!(id.root(), VirtualRoot::Project)
            && let Some(bytes) = self.overlay.get(id.vpath())
        {
            return Ok(bytes.clone());
        }
        self.files.load(id)
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
    ///
    /// Packages (`@preview/...`, `@local/...`) are resolved from Typst's
    /// standard package directories, downloading `@preview` packages on demand.
    pub fn new(root: PathBuf, entry: &str, overlay: TypstOverlay) -> Result<Self> {
        Self::with_packages(root, entry, overlay, SystemPackages::new(HttpDownloader))
    }

    fn with_packages(
        root: PathBuf,
        entry: &str,
        overlay: TypstOverlay,
        packages: SystemPackages,
    ) -> Result<Self> {
        let mut fonts = FontStore::new();
        fonts.extend(fonts::embedded());

        let entry = VirtualPath::new(entry)
            .map_err(|err| anyhow!("invalid entry path `{entry}`: {err:?}"))?;
        let main = RootedPath::new(VirtualRoot::Project, entry).intern();

        let library = Library::builder()
            .with_features([Feature::Html, Feature::Bundle].into_iter().collect())
            .build();

        let loader = ProjectLoader::new(root, overlay.files, packages);
        Ok(Self {
            library: LazyHash::from(library),
            fonts,
            main,
            store: FileStore::new(loader),
            today: Time::system(),
        })
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

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::io::{self, Read};
    use std::path::Path;

    use typst::syntax::package::PackageSpec;
    use typst_kit::packages::{FsPackages, UniversePackages};

    use super::*;

    /// Offline `Downloader`: local packages only.
    #[derive(Debug, Clone, Copy)]
    struct NoopDownloader;

    impl Downloader for NoopDownloader {
        fn stream(&self, _: &dyn Any, _: &str) -> io::Result<(Option<usize>, Box<dyn Read>)> {
            Err(io::Error::new(io::ErrorKind::NotFound, "offline"))
        }
    }

    fn write(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn loads_third_party_packages() {
        let base = std::env::temp_dir().join(format!("typost-packages-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let project = base.join("site");
        let packages = base.join("packages");

        write(
            &project.join("home.typ"),
            "#import \"@test/hello:0.1.0\": greet\n#document(\"index.html\")[#greet()]\n",
        );
        write(
            &packages.join("test/hello/0.1.0/typst.toml"),
            "[package]\nname = \"hello\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
        );
        write(
            &packages.join("test/hello/0.1.0/lib.typ"),
            "#let greet() = [hello from a package]\n",
        );
        write(&packages.join("test/hello/0.1.0/data.txt"), "package data");

        let store = SystemPackages::from_parts(
            Some(FsPackages::new(&packages)),
            None,
            UniversePackages::new(NoopDownloader),
        );

        // A plugin overlay must not shadow a package's files at the same path.
        let mut overlay = TypstOverlay::new();
        overlay.add("lib.typ", "project overlay").unwrap();

        let world = TypostWorld::with_packages(project, "home.typ", overlay, store).unwrap();

        let spec: PackageSpec = "@test/hello:0.1.0".parse().unwrap();
        let package_file = |path: &str| {
            RootedPath::new(
                VirtualRoot::Package(spec.clone()),
                VirtualPath::new(path).unwrap(),
            )
            .intern()
        };

        assert_eq!(
            world.source(package_file("lib.typ")).unwrap().text(),
            "#let greet() = [hello from a package]\n"
        );
        assert_eq!(
            &world.file(package_file("data.txt")).unwrap()[..],
            b"package data"
        );

        // Project overlays still apply to project-rooted files.
        let project_lib =
            RootedPath::new(VirtualRoot::Project, VirtualPath::new("lib.typ").unwrap()).intern();
        assert_eq!(world.source(project_lib).unwrap().text(), "project overlay");

        // The entry can actually import and call the package.
        let warned = typst::compile::<typst_bundle::Bundle>(&world);
        assert!(
            warned.output.is_ok(),
            "package import failed: {:?}",
            warned.output.as_ref().err()
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
