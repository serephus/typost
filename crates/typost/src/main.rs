//! The typost command-line interface.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use typost_core::{BuildOptions, Plugin, Site};
use typost_plugin_encrypt::Encrypt;
use typost_plugin_feed::Config as FeedConfig;
use typost_plugin_sitemap::Config as SitemapConfig;
use typost_plugin_spoiler::Spoiler;
use typost_plugin_taxonomies::Taxonomies;

/// `typost.toml`.
#[derive(Debug, Deserialize)]
struct Config {
    /// The entry file, relative to the project root.
    entry: String,
    /// The output directory, relative to the project root.
    out: PathBuf,
    /// The site's base URL, shared by plugins that need absolute URLs.
    #[serde(default)]
    base_url: Option<String>,
    /// Optional sitemap plugin configuration.
    #[serde(default)]
    sitemap: Option<SitemapConfig>,
    /// Optional Atom feed plugin configuration.
    #[serde(default)]
    feed: Option<FeedConfig>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("init") => init_cmd(),
        Some("build") => build_cmd(),
        Some("help" | "--help" | "-h") | None => {
            print_help();
            Ok(())
        }
        Some(command) => bail!("unknown command `{command}` (try `typost help`)"),
    }
}

fn init_cmd() -> Result<()> {
    let root = std::env::current_dir().context("failed to determine current directory")?;

    let config_path = root.join("typost.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, "entry = \"home.typ\"\nout = \"dist\"\n")
            .with_context(|| format!("failed to write `{}`", config_path.display()))?;
        println!("created `{}`", config_path.display());
    }

    // Materialize the same Typst helpers a build would, so editors resolve them
    // before the first build.
    let config = read_config(&root)?;
    typost_core::materialize_typst(&root, &plugins(&config)?)?;
    println!(
        "materialized the stdlib and plugin Typst into `{}`",
        root.join("lib").display()
    );
    Ok(())
}

/// Read and parse `<root>/typost.toml`.
fn read_config(root: &Path) -> Result<Config> {
    let path = root.join("typost.toml");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read `{}`", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse `{}`", path.display()))
}

/// The URL path of `base_url` (e.g. `/typost` for
/// `https://user.github.io/typost`).
///
/// Root-absolute links in the output are prefixed with it, so the same site
/// can be served from a project subpath. Empty means the site root.
fn url_base(base_url: Option<&str>) -> String {
    let Some(url) = base_url else {
        return String::new();
    };
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let path = after_scheme.find('/').map_or("", |at| &after_scheme[at..]);
    path.trim_end_matches('/').to_owned()
}

fn build_cmd() -> Result<()> {
    let root = std::env::current_dir().context("failed to determine current directory")?;
    let config = read_config(&root)?;

    let options = BuildOptions {
        root: root.clone(),
        entry: config.entry.clone(),
        out: root.join(&config.out),
        base: url_base(config.base_url.as_deref()),
    };

    Site::new(options).plugins(plugins(&config)?).build()?;

    println!(
        "built `{}` -> `{}`",
        config.entry,
        root.join(&config.out).display()
    );
    Ok(())
}

/// The compile-time plugin list. Register plugins here, in the order they
/// should run. Plugin options come from `typost.toml`.
fn plugins(config: &Config) -> Result<Vec<Box<dyn Plugin>>> {
    let mut plugins: Vec<Box<dyn Plugin>> = Vec::new();

    // Pages opt in by declaring a `password` in their front matter.
    plugins.push(Box::new(Encrypt::new()));

    // Contributes the `typost.taxonomies` module; sites declare their
    // taxonomies (tags, categories, difficulty, ...) in Typst.
    plugins.push(Box::new(Taxonomies::new()));

    // Contributes `lib/typost/spoiler.typ` (`#spoiler[...]`) and its assets.
    plugins.push(Box::new(Spoiler::new()));

    if let Some(sitemap) = &config.sitemap {
        plugins.push(Box::new(sitemap.build(config.base_url.as_deref())?));
    }

    if let Some(feed) = &config.feed {
        plugins.push(Box::new(feed.build(config.base_url.as_deref())?));
    }

    Ok(plugins)
}

fn print_help() {
    println!(
        "typost — a Typst-powered static site generator

USAGE:
    typost <COMMAND>

COMMANDS:
    init     Create typost.toml (if missing) and materialize the Typst helpers
    build    Build the site described by typost.toml
    help     Print this help

The project root is the current directory. It must contain typost.toml:
    entry = \"home.typ\"
    out = \"dist\"
    base_url = \"https://example.com\"   # shared by plugins

Plugin config example:
    [sitemap]
    [feed]
    limit = 20"
    );
}

#[cfg(test)]
mod tests {
    use super::url_base;

    #[test]
    fn extracts_the_url_path() {
        assert_eq!(
            url_base(Some("https://serephus.github.io/typost")),
            "/typost"
        );
        assert_eq!(
            url_base(Some("https://serephus.github.io/typost/")),
            "/typost"
        );
        assert_eq!(url_base(Some("https://example.com")), "");
        assert_eq!(url_base(Some("https://example.com/")), "");
        assert_eq!(url_base(None), "");
    }
}
