//! `Sayagi.toml` — the single source of truth for app metadata, routing and PWA behaviour.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{escape, Error, GENERATED_HEADER, Result};

/// The whole `Sayagi.toml` file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// App identity: used by the manifest, the HTML shell and the icons.
    pub app: AppConfig,
    /// Where pages live and how they map to URLs.
    #[serde(default)]
    pub routes: RoutesConfig,
    /// Service worker and manifest behaviour.
    #[serde(default)]
    pub pwa: PwaConfig,
    /// Embedded SQLite settings baked into the server binary.
    #[serde(default)]
    pub database: DatabaseConfig,
}

impl Config {
    /// Reads and validates `Sayagi.toml`.
    pub fn load(manifest_dir: &Path) -> Result<Self> {
        let path = manifest_dir.join("Sayagi.toml");
        let raw = std::fs::read_to_string(&path).map_err(|error| {
            Error::new(format!("cannot read {}: {error}", path.display()))
        })?;
        toml::from_str(&raw)
            .map_err(|error| Error::new(format!("invalid {}: {error}", path.display())))
    }

    /// Absolute path of the pages directory.
    pub fn routes_dir(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.routes.dir)
    }

    /// Absolute path of the static assets directory.
    pub fn assets_dir(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.app.assets_dir)
    }
}

/// `[app]`
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// Full application name.
    pub name: String,
    /// Name shown under the home screen icon.
    pub short_name: String,
    /// Manifest description.
    #[serde(default)]
    pub description: String,
    /// BCP-47 language tag.
    #[serde(default = "default_lang")]
    pub lang: String,
    /// `theme_color` of the manifest and the `<meta name="theme-color">` tag.
    #[serde(default = "default_theme_color")]
    pub theme_color: String,
    /// `background_color` of the manifest.
    #[serde(default = "default_background_color")]
    pub background_color: String,
    /// Manifest `start_url`.
    #[serde(default = "default_start_url")]
    pub start_url: String,
    /// Manifest `scope`.
    #[serde(default = "default_scope")]
    pub scope: String,
    /// Manifest `display` mode.
    #[serde(default = "default_display")]
    pub display: String,
    /// Manifest `orientation`.
    #[serde(default = "default_orientation")]
    pub orientation: Option<String>,
    /// Address the Axum server binds to (`cargo leptos serve`).
    #[serde(default = "default_site_addr")]
    pub site_addr: String,
    /// Animate route changes with the View Transition API; browsers without the
    /// API simply render the new view without animation.
    #[serde(default = "default_true")]
    pub page_transitions: bool,
    /// Directory whose contents are copied to the site root.
    #[serde(default = "default_assets_dir")]
    pub assets_dir: PathBuf,
}

/// `[routes]`
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutesConfig {
    /// Directory scanned for file-based routes, relative to the app root.
    #[serde(default = "default_pages_dir")]
    pub dir: PathBuf,
}

impl Default for RoutesConfig {
    fn default() -> Self {
        Self { dir: default_pages_dir() }
    }
}

/// `[pwa]`
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PwaConfig {
    /// Emit `sw.js` and register it from the Wasm client.
    #[serde(default = "default_true")]
    pub service_worker: bool,
    /// Route rendered when a navigation request fails while offline.
    #[serde(default = "default_offline_url")]
    pub offline_fallback: Option<String>,
    /// Icon sizes (square, in pixels) generated as `icon-<size>.png` and `icon-<size>-maskable.png`.
    #[serde(default = "default_icons")]
    pub icons: Vec<u32>,
    /// URLs precached during service worker installation.
    #[serde(default = "default_precache")]
    pub precache: Vec<String>,
    /// URL prefixes fetched network-first (API/server functions).
    #[serde(default = "default_network_first")]
    pub network_first: Vec<String>,
    /// URL prefixes served stale-while-revalidate (Wasm bundle, icons, assets).
    #[serde(default = "default_stale_while_revalidate")]
    pub stale_while_revalidate: Vec<String>,
    /// Upper bound of entries kept in the Workbox runtime cache.
    #[serde(default = "default_runtime_max_entries")]
    pub runtime_max_entries: u32,
    /// Age (in seconds) after which a runtime cache entry is evicted.
    #[serde(default = "default_runtime_max_age_seconds")]
    pub runtime_max_age_seconds: u64,
}

impl Default for PwaConfig {
    fn default() -> Self {
        Self {
            service_worker: true,
            offline_fallback: default_offline_url(),
            icons: default_icons(),
            precache: default_precache(),
            network_first: default_network_first(),
            stale_while_revalidate: default_stale_while_revalidate(),
            runtime_max_entries: default_runtime_max_entries(),
            runtime_max_age_seconds: default_runtime_max_age_seconds(),
        }
    }
}

/// `[database]`
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    /// sqlx connection string. Overridable at runtime with `SAYAGI_DATABASE_URL`.
    #[serde(default = "default_database_url")]
    pub url: String,
    /// Connection pool size.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Seed demo rows on first boot.
    #[serde(default = "default_true")]
    pub seed: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_database_url(),
            max_connections: default_max_connections(),
            seed: true,
        }
    }
}

pub(crate) fn codegen(config: &Config) -> String {
    let app = &config.app;
    let pwa = &config.pwa;
    let database = &config.database;
    let smallest_icon = pwa.icons.iter().min().copied().unwrap_or(192);

    let mut out = String::new();
    out.push_str(GENERATED_HEADER);
    out.push('\n');
    out.push_str("// Constants baked from `Sayagi.toml` at build time.\n\n");
    out.push_str(&format!("pub const APP_NAME: &str = \"{}\";\n", escape(&app.name)));
    out.push_str(&format!(
        "pub const APP_SHORT_NAME: &str = \"{}\";\n",
        escape(&app.short_name)
    ));
    out.push_str(&format!(
        "pub const APP_DESCRIPTION: &str = \"{}\";\n",
        escape(&app.description)
    ));
    out.push_str(&format!("pub const LANG: &str = \"{}\";\n", escape(&app.lang)));
    out.push_str(&format!(
        "pub const THEME_COLOR: &str = \"{}\";\n",
        escape(&app.theme_color)
    ));
    out.push_str(&format!(
        "pub const BACKGROUND_COLOR: &str = \"{}\";\n",
        escape(&app.background_color)
    ));
    out.push_str(&format!(
        "pub const SITE_ADDR: &str = \"{}\";\n",
        escape(&app.site_addr)
    ));
    out.push_str("pub const MANIFEST_PATH: &str = \"/manifest.webmanifest\";\n");
    out.push_str("pub const SERVICE_WORKER_PATH: &str = \"/sw.js\";\n");
    out.push_str("pub const PAGE_TRANSITIONS: bool = ");
    out.push_str(if app.page_transitions { "true" } else { "false" });
    out.push_str(";\n");
    out.push_str("pub const SERVICE_WORKER_ENABLED: bool = ");
    out.push_str(if pwa.service_worker { "true" } else { "false" });
    out.push_str(";\n");
    out.push_str(&format!(
        "pub const PRIMARY_ICON: &str = \"/icons/icon-{smallest_icon}.png\";\n"
    ));
    out.push_str(&format!(
        "pub const DATABASE_URL: &str = \"{}\";\n",
        escape(&database.url)
    ));
    out.push_str(&format!(
        "pub const DATABASE_MAX_CONNECTIONS: u32 = {};\n",
        database.max_connections
    ));
    out.push_str(&format!("pub const DATABASE_SEED: bool = {};\n", database.seed));
    out
}

fn default_true() -> bool {
    true
}

fn default_max_connections() -> u32 {
    5
}

fn default_database_url() -> String {
    "sqlite://sayagi.db".to_string()
}

fn default_lang() -> String {
    "en".to_string()
}

fn default_theme_color() -> String {
    "#0f172a".to_string()
}

fn default_background_color() -> String {
    "#0f172a".to_string()
}

fn default_start_url() -> String {
    "/".to_string()
}

fn default_scope() -> String {
    "/".to_string()
}

fn default_display() -> String {
    "standalone".to_string()
}

fn default_orientation() -> Option<String> {
    Some("portrait".to_string())
}

fn default_site_addr() -> String {
    "127.0.0.1:3000".to_string()
}

fn default_assets_dir() -> PathBuf {
    PathBuf::from("public")
}

fn default_pages_dir() -> PathBuf {
    PathBuf::from("src/pages")
}

fn default_offline_url() -> Option<String> {
    Some("/offline".to_string())
}

fn default_icons() -> Vec<u32> {
    vec![192, 512]
}

fn default_precache() -> Vec<String> {
    vec![
        "/".to_string(),
        "/offline".to_string(),
        "/manifest.webmanifest".to_string(),
    ]
}

fn default_runtime_max_entries() -> u32 {
    128
}

fn default_runtime_max_age_seconds() -> u64 {
    60 * 60 * 24 * 30
}

fn default_network_first() -> Vec<String> {
    vec!["/api/".to_string()]
}

fn default_stale_while_revalidate() -> Vec<String> {
    vec!["/pkg/".to_string(), "/icons/".to_string(), "/assets/".to_string()]
}
