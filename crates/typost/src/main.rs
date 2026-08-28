//! The typost command-line interface.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use typost_core::{BuildOptions, Site};
use typost_plugin_encrypt::Encrypt;
use typost_plugin_feed::Feed;
use typost_plugin_sitemap::Sitemap;
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

impl Config {
    /// Resolve a plugin's base URL: its own override, then the top-level one.
    fn base_url<'a>(&'a self, override_url: Option<&'a str>) -> Option<&'a str> {
        override_url.or(self.base_url.as_deref())
    }
}

/// `[sitemap]` in `typost.toml`.
#[derive(Debug, Deserialize)]
struct SitemapConfig {
    /// Overrides the top-level `base_url`.
    #[serde(default)]
    base_url: Option<String>,
}

/// `[feed]` in `typost.toml`.
#[derive(Debug, Deserialize)]
struct FeedConfig {
    /// Overrides the top-level `base_url`.
    #[serde(default)]
    base_url: Option<String>,
    /// The feed's output path (default `atom.xml`).
    #[serde(default)]
    path: Option<String>,
    /// The maximum number of entries (default 20).
    #[serde(default)]
    limit: Option<usize>,
    /// The front-matter `section` to include (default `post`).
    #[serde(default)]
    section: Option<String>,
    /// The feed title (defaults to the site title).
    #[serde(default)]
    title: Option<String>,
    /// The feed subtitle.
    #[serde(default)]
    description: Option<String>,
    /// The feed author name.
    #[serde(default)]
    author: Option<String>,
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

    typost_core::materialize_stdlib(&root)?;
    println!("materialized stdlib into `{}`", root.join("lib").display());
    Ok(())
}

fn build_cmd() -> Result<()> {
    let root = std::env::current_dir().context("failed to determine current directory")?;
    let config_path = root.join("typost.toml");
    let text = std::fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read `{}`", config_path.display()))?;
    let config: Config = toml::from_str(&text)
        .with_context(|| format!("failed to parse `{}`", config_path.display()))?;

    let options = BuildOptions {
        root: root.clone(),
        entry: config.entry.clone(),
        out: root.join(&config.out),
    };

    register_plugins(Site::new(options), &config)?.build()?;

    println!(
        "built `{}` -> `{}`",
        config.entry,
        root.join(&config.out).display()
    );
    Ok(())
}

/// The compile-time plugin list. Register plugins here, in the order they
/// should run. Plugin options come from `typost.toml`.
fn register_plugins(site: Site, config: &Config) -> Result<Site> {
    let mut site = site;

    // Pages opt in by declaring a `password` in their front matter.
    site = site.plugin(Encrypt::new());

    // Contributes the `typost.taxonomies` module; sites declare their
    // taxonomies (tags, categories, difficulty, ...) in Typst.
    site = site.plugin(Taxonomies::new());

    if let Some(sitemap) = &config.sitemap {
        let base = config.base_url(sitemap.base_url.as_deref()).context(
            "`[sitemap]` needs a base URL (set `base_url` top-level or in `[sitemap]`)",
        )?;
        site = site.plugin(Sitemap::new(base));
    }

    if let Some(feed) = &config.feed {
        let base = config.base_url(feed.base_url.as_deref()).context(
            "`[feed]` needs a base URL (set `base_url` top-level or in `[feed]`)",
        )?;
        let mut plugin = Feed::new(base);
        if let Some(path) = &feed.path {
            plugin = plugin.path(path);
        }
        if let Some(limit) = feed.limit {
            plugin = plugin.limit(limit);
        }
        if let Some(section) = &feed.section {
            plugin = plugin.section(section);
        }
        if let Some(title) = &feed.title {
            plugin = plugin.title(title);
        }
        if let Some(description) = &feed.description {
            plugin = plugin.description(description);
        }
        if let Some(author) = &feed.author {
            plugin = plugin.author(author);
        }
        site = site.plugin(plugin);
    }

    Ok(site)
}

fn print_help() {
    println!(
        "typost — a Typst-powered static site generator

USAGE:
    typost <COMMAND>

COMMANDS:
    init     Create typost.toml (if missing) and materialize the stdlib
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
