//! The "Auto-PWA Generator": manifest, service worker and icons.
//!
//! Generated from `Sayagi.toml` on every build, so the PWA surface can never
//! drift away from the app configuration.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::{fingerprint, png, write_file, write_file_bytes, Config, Error, Result};

/// Writes `manifest.webmanifest`.
pub(crate) fn write_manifest(assets_dir: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    let app = &config.app;
    let mut manifest = Map::new();

    manifest.insert("name".to_string(), Value::from(app.name.clone()));
    manifest.insert("short_name".to_string(), Value::from(app.short_name.clone()));
    manifest.insert(
        "description".to_string(),
        Value::from(app.description.clone()),
    );
    manifest.insert("lang".to_string(), Value::from(app.lang.clone()));
    manifest.insert("start_url".to_string(), Value::from(app.start_url.clone()));
    manifest.insert("scope".to_string(), Value::from(app.scope.clone()));
    manifest.insert("display".to_string(), Value::from(app.display.clone()));
    manifest.insert(
        "theme_color".to_string(),
        Value::from(app.theme_color.clone()),
    );
    manifest.insert(
        "background_color".to_string(),
        Value::from(app.background_color.clone()),
    );
    if let Some(orientation) = &app.orientation {
        manifest.insert("orientation".to_string(), Value::from(orientation.clone()));
    }

    let mut icons = Vec::with_capacity(config.pwa.icons.len() * 2);
    for size in &config.pwa.icons {
        validate_icon_size(*size)?;
        for (suffix, purpose) in [("", "any"), ("-maskable", "maskable")] {
            let mut icon = Map::new();
            icon.insert(
                "src".to_string(),
                Value::from(format!("/icons/icon-{size}{suffix}.png")),
            );
            icon.insert("sizes".to_string(), Value::from(format!("{size}x{size}")));
            icon.insert("type".to_string(), Value::from("image/png"));
            icon.insert("purpose".to_string(), Value::from(purpose));
            icons.push(Value::Object(icon));
        }
    }
    manifest.insert("icons".to_string(), Value::Array(icons));

    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(manifest))?
    );
    Ok(vec![write_file(
        &assets_dir.join("manifest.webmanifest"),
        &text,
    )?])
}

/// Writes `sw.js`, or removes a previously generated one when disabled.
pub(crate) fn write_service_worker(assets_dir: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    let path = assets_dir.join("sw.js");

    if !config.pwa.service_worker {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        return Ok(Vec::new());
    }

    let pwa = &config.pwa;
    let precache = serde_json::to_string(&pwa.precache)?;
    let offline = match &pwa.offline_fallback {
        Some(url) => serde_json::to_string(url)?,
        None => "null".to_string(),
    };
    let version = format!(
        "sayagi-{}",
        fingerprint(&[
            config.app.name.as_str(),
            config.app.short_name.as_str(),
            &pwa.precache.join(","),
            &pwa.network_first.join(","),
            &pwa.stale_while_revalidate.join(","),
            pwa.offline_fallback.as_deref().unwrap_or("")
        ])
    );

    let script = include_str!("templates/sw.js")
        .replace("__VERSION__", &version)
        .replace("__PRECACHE__", &precache)
        .replace("__OFFLINE_URL__", &offline)
        .replace("__NETWORK_FIRST__", &regex_list(&pwa.network_first))
        .replace(
            "__STALE_WHILE_REVALIDATE__",
            &regex_list(&pwa.stale_while_revalidate),
        );

    write_file_bytes(&path, script.as_bytes())?;
    Ok(vec![path])
}

/// Writes `icons/icon-<size>.png` and `icons/icon-<size>-maskable.png`.
pub(crate) fn write_icons(assets_dir: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    let theme = png::parse_hex_color(&config.app.theme_color)?;
    let directory = assets_dir.join("icons");
    std::fs::create_dir_all(&directory)?;

    let mut written = Vec::with_capacity(config.pwa.icons.len() * 2);
    for size in &config.pwa.icons {
        validate_icon_size(*size)?;
        for (suffix, maskable) in [("", false), ("-maskable", true)] {
            let path = directory.join(format!("icon-{size}{suffix}.png"));
            write_file_bytes(&path, &png::icon_png(*size, theme, maskable))?;
            written.push(path);
        }
    }
    Ok(written)
}

fn validate_icon_size(size: u32) -> Result<()> {
    if (16..=2048).contains(&size) {
        Ok(())
    } else {
        Err(Error::new(format!(
            "[pwa] icons contains {size}: sizes must be between 16 and 2048"
        )))
    }
}

fn regex_list(prefixes: &[String]) -> String {
    let items = prefixes
        .iter()
        .map(|prefix| format!("/{}/", escape_regex(prefix)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

fn escape_regex(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(
            ch,
            '/' | '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}
