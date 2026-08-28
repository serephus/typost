//! Proves the `eval → manifest → prepare → render → post` loop and the
//! generic, plugin-defined metadata channel.

use std::sync::{Arc, Mutex};

use typost_core::{BuildOptions, Plugin, RenderOutput, SiteManifest, TypstOverlay, build};

struct Recorder {
    seen: Arc<Mutex<Option<SiteManifest>>>,
}

impl Plugin for Recorder {
    fn name(&self) -> &str {
        "recorder"
    }

    fn prepare(&self, manifest: &mut SiteManifest) -> anyhow::Result<()> {
        *self.seen.lock().unwrap() = Some(manifest.clone());
        Ok(())
    }

    fn post(&self, out: &mut RenderOutput, _: &SiteManifest) -> anyhow::Result<()> {
        out.insert("recorder.txt", b"ran".to_vec());
        Ok(())
    }
}

/// A plugin that contributes a Typst helper whose `metadata` entries travel
/// through the generic channel. Core knows nothing about `test-marker`.
struct Marker;

impl Plugin for Marker {
    fn name(&self) -> &str {
        "marker"
    }

    fn typst(&self, overlay: &mut TypstOverlay) -> anyhow::Result<()> {
        overlay.add(
            "lib/marker.typ",
            r#"
#let mark(value) = {
  metadata((typost: (kind: "test-marker", value: value)))
  html.elem("span", value, attrs: (class: "test-marker"))
}
"#,
        );
        Ok(())
    }
}

#[test]
fn build_pipeline_lifts_manifest_and_runs_plugins() {
    let dir = std::env::temp_dir().join(format!("typost-pipeline-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // A page using the show directive and a plugin-contributed helper.
    std::fs::write(
        dir.join("page.typ"),
        "#import \"lib/typost.typ\": page\n\
         #import \"lib/marker.typ\": mark\n\
         #show: page.with(route: \"index.html\", title: \"Home\", tags: (\"a\", \"b\"))\n\
         = Hi\n\
         #mark(\"hello\")\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("home.typ"),
        "#import \"lib/typost.typ\": site\n\
         #site(title: \"Site\")\n\
         #include \"page.typ\"\n",
    )
    .unwrap();

    let seen = Arc::new(Mutex::new(None));
    let plugins: Vec<Box<dyn Plugin>> =
        vec![Box::new(Marker), Box::new(Recorder { seen: seen.clone() })];

    let options = BuildOptions {
        root: dir.clone(),
        entry: "home.typ".to_owned(),
        out: dir.join("dist"),
    };
    build(&options, &plugins).expect("build should succeed");

    // `build` materialized the embedded stdlib into `lib/` for editors.
    assert_eq!(
        std::fs::read_to_string(dir.join("lib/typost.typ")).unwrap(),
        typost_core::STDLIB_TYP,
    );

    // The manifest is available before render and carries typed front matter.
    let manifest = seen.lock().unwrap().clone().expect("prepare ran");
    assert_eq!(manifest.title.as_deref(), Some("Site"));
    assert_eq!(manifest.pages.len(), 1);
    assert_eq!(manifest.pages[0].route, "index.html");
    let tags = manifest.pages[0].data.get("tags").unwrap();
    assert_eq!(tags.as_array().unwrap().len(), 2);

    // The plugin-defined metadata was collected generically, tagged with its
    // enclosing page route. Core never interpreted `test-marker`.
    let entry = manifest
        .entries
        .iter()
        .find(|entry| entry.kind == "test-marker")
        .expect("generic metadata entry");
    assert_eq!(entry.route.as_deref(), Some("index.html"));
    assert_eq!(
        entry.data.get("value").and_then(|v| v.as_str()),
        Some("hello")
    );

    // `post` saw the exported files and added one.
    assert_eq!(
        std::fs::read(dir.join("dist/recorder.txt")).unwrap(),
        b"ran"
    );

    // The content region marker is present in the rendered page.
    let html = std::fs::read_to_string(dir.join("dist/index.html")).unwrap();
    assert!(html.contains("id=\"typost-content\""));

    let _ = std::fs::remove_dir_all(&dir);
}
