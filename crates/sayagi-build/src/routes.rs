//! File-based routing: `src/pages/**/*.rs` becomes the route table.
//!
//! | File                          | URL                 |
//! | ----------------------------- | ------------------- |
//! | `src/pages/index.rs`          | `/`                 |
//! | `src/pages/dashboard.rs`      | `/dashboard`        |
//! | `src/pages/containers/index.rs` | `/containers`     |
//! | `src/pages/containers/[id].rs`  | `/containers/:id` |
//! | `src/pages/files/[...path].rs`  | `/files/*path`    |
//!
//! Files whose name starts with `_` and `mod.rs` are ignored, so partials can live
//! next to pages. Every page module must export `#[component] pub fn Page() -> impl IntoView`.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use crate::config::Config;
use crate::{escape, Error, GENERATED_HEADER, Result};

/// Which flavour of route a segment list describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    /// Only literal segments, e.g. `/dashboard`.
    Static,
    /// Contains at least one `[param]` segment, e.g. `/containers/:id`.
    Dynamic,
    /// Ends with a `[...rest]` catch-all, e.g. `/files/*path`.
    Wildcard,
}

impl RouteKind {
    /// Variant name as used in generated code.
    pub fn variant_name(self) -> &'static str {
        match self {
            RouteKind::Static => "Static",
            RouteKind::Dynamic => "Dynamic",
            RouteKind::Wildcard => "Wildcard",
        }
    }

    /// Lowercase label, used by the CLI.
    pub fn label(self) -> &'static str {
        match self {
            RouteKind::Static => "static",
            RouteKind::Dynamic => "dynamic",
            RouteKind::Wildcard => "wildcard",
        }
    }

    fn rank(self) -> u8 {
        match self {
            RouteKind::Static => 0,
            RouteKind::Dynamic => 1,
            RouteKind::Wildcard => 2,
        }
    }
}

/// A single URL segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Literal text.
    Static(String),
    /// `[name]` — matches one path segment.
    Param(String),
    /// `[...name]` — matches the rest of the path.
    Wildcard(String),
}

impl Segment {
    fn to_router_syntax(&self) -> String {
        match self {
            Segment::Static(text) => text.clone(),
            Segment::Param(name) => format!(":{name}"),
            Segment::Wildcard(name) => format!("*{name}"),
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Segment::Static(_) => 0,
            Segment::Param(_) => 1,
            Segment::Wildcard(_) => 2,
        }
    }
}

/// One page file and the URL it serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// leptos_router path pattern, e.g. `/containers/:id`.
    pub url: String,
    /// Absolute path of the page source file.
    pub file: PathBuf,
    /// Path relative to the app root, e.g. `src/pages/containers/[id].rs`.
    pub rel_file: String,
    /// Module name used in the generated router tree.
    pub module: String,
    /// Parsed URL segments.
    pub segments: Vec<Segment>,
    /// Static, dynamic or wildcard.
    pub kind: RouteKind,
}

impl Route {
    /// Whether the route expects parameters.
    pub fn is_dynamic(&self) -> bool {
        self.kind != RouteKind::Static
    }
}

pub(crate) fn scan(manifest_dir: &Path, config: &Config) -> Result<Vec<Route>> {
    let dir = config.routes_dir(manifest_dir);
    if !dir.is_dir() {
        return Err(Error::new(format!(
            "pages directory {} does not exist (configured as [routes] dir)",
            dir.display()
        )));
    }

    let mut sources = Vec::new();
    collect(&dir, &mut sources)?;
    sources.sort();

    if sources.is_empty() {
        return Err(Error::new(format!(
            "no page files found under {}: create at least `index.rs`",
            dir.display()
        )));
    }

    let mut used = HashSet::new();
    let mut routes = Vec::with_capacity(sources.len());
    for file in sources {
        let relative = file
            .strip_prefix(&dir)
            .map_err(|_| Error::new("page file escaped the pages directory"))?
            .to_path_buf();
        let (url, segments) = to_url(&relative);
        let kind = kind_of(&segments);
        routes.push(Route {
            url,
            rel_file: relative_to(manifest_dir, &file),
            module: unique_module(&relative, &mut used),
            segments,
            kind,
            file,
        });
    }

    // Static paths first, then dynamic, then wildcards — `/containers/new` must win
    // over `/containers/:id` no matter what the file system returns first.
    routes.sort_by(|left, right| ordering(left).cmp(&ordering(right)).then(left.url.cmp(&right.url)));
    Ok(routes)
}

pub(crate) fn codegen(routes: &[Route]) -> String {
    let mut out = String::new();
    out.push_str(GENERATED_HEADER);
    out.push('\n');
    out.push_str("// Route table generated from the `src/pages` directory.\n\n");
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use leptos_router::components::{Route, Routes};\n");
    out.push_str("#[allow(unused_imports)]\nuse leptos_router::path;\n\n");

    for route in routes {
        let _ = writeln!(
            out,
            "#[path = \"{}\"]\npub mod {};\n",
            escape(&route.file.to_string_lossy()),
            route.module
        );
    }

    out.push_str("/// Every route discovered at build time.\n");
    out.push_str("pub const ROUTES: &[sayagi::RouteInfo] = &[\n");
    for route in routes {
        let _ = writeln!(
            out,
            "    sayagi::RouteInfo {{ path: \"{}\", file: \"{}\", module: \"{}\", kind: sayagi::RouteKind::{} }},",
            escape(&route.url),
            escape(&route.rel_file),
            route.module,
            route.kind.variant_name()
        );
    }
    out.push_str("];\n\n");

    out.push_str("/// Rendered when no page file matches the current URL.\n");
    out.push_str("#[component]\npub fn NotFound() -> impl IntoView {\n    view! {\n");
    out.push_str("        <section class=\"sy-page sy-empty\">\n");
    out.push_str("            <p class=\"sy-empty__code\">\"404\"</p>\n");
    out.push_str("            <h1 class=\"sy-empty__title\">\"Halaman tidak ditemukan\"</h1>\n");
    out.push_str("            <p class=\"sy-empty__hint\">\"Rute ini belum punya file di `src/pages`.\"</p>\n");
    out.push_str("            <a class=\"sy-btn\" href=\"/\">\"Kembali ke beranda\"</a>\n");
    out.push_str("        </section>\n    }\n}\n\n");

    out.push_str("/// The Leptos router tree generated from the file system.\n");
    out.push_str("#[component]\npub fn FileRoutes() -> impl IntoView {\n    view! {\n");
    out.push_str(
        "        <Routes fallback=NotFound transition=crate::config::PAGE_TRANSITIONS>\n",
    );
    for route in routes {
        // `path!("/")` menghasilkan tuple kosong, sedangkan router Leptos memakai
        // satu segmen statis kosong untuk root — jadi root ditulis eksplisit.
        let path_expression = if route.url == "/" {
            "leptos_router::StaticSegment(\"\")".to_string()
        } else {
            format!("path!(\"{}\")", escape(&route.url))
        };
        let _ = writeln!(
            out,
            "            <Route path={path_expression} view={}::Page/>",
            route.module
        );
    }
    out.push_str("        </Routes>\n    }\n}\n");
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        if stem == "mod" || stem.starts_with('_') {
            continue;
        }
        out.push(path);
    }
    Ok(())
}

fn to_url(relative: &Path) -> (String, Vec<Segment>) {
    let mut components = Vec::new();
    for component in relative.components() {
        if let Component::Normal(name) = component {
            components.push(name.to_string_lossy().to_string());
        }
    }

    let file_name = components.pop().unwrap_or_default();
    let stem = Path::new(&file_name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut segments = Vec::new();
    for directory in &components {
        push_segments(&mut segments, directory);
    }
    if stem != "index" {
        push_segments(&mut segments, &stem);
    }

    let joined = segments
        .iter()
        .map(Segment::to_router_syntax)
        .collect::<Vec<_>>()
        .join("/");
    let url = if joined.is_empty() {
        "/".to_string()
    } else {
        format!("/{joined}")
    };
    (url, segments)
}

/// Turns `containers` into `Static`, `[id]` into `Param`, `[...rest]` into `Wildcard`
/// and `user-[id]` into a static + param pair.
fn push_segments(out: &mut Vec<Segment>, raw: &str) {
    let mut chars = raw.chars();
    let mut literal = String::new();

    while let Some(ch) = chars.next() {
        if ch != '[' {
            literal.push(ch);
            continue;
        }

        let mut inner = String::new();
        let mut closed = false;
        for candidate in chars.by_ref() {
            if candidate == ']' {
                closed = true;
                break;
            }
            inner.push(candidate);
        }

        if !closed {
            literal.push('[');
            literal.push_str(&inner);
            continue;
        }

        if !literal.is_empty() {
            out.push(Segment::Static(std::mem::take(&mut literal)));
        }
        match inner.strip_prefix("...") {
            Some(name) if !name.is_empty() => out.push(Segment::Wildcard(name.to_string())),
            Some(_) => {}
            None if !inner.is_empty() => out.push(Segment::Param(inner)),
            None => {}
        }
    }

    if !literal.is_empty() {
        out.push(Segment::Static(literal));
    }
}

fn kind_of(segments: &[Segment]) -> RouteKind {
    if segments.iter().any(|segment| matches!(segment, Segment::Wildcard(_))) {
        RouteKind::Wildcard
    } else if segments.iter().any(|segment| matches!(segment, Segment::Param(_))) {
        RouteKind::Dynamic
    } else {
        RouteKind::Static
    }
}

fn ordering(route: &Route) -> Vec<(u8, String)> {
    let mut key: Vec<(u8, String)> = route
        .segments
        .iter()
        .map(|segment| (segment.rank(), segment.to_router_syntax()))
        .collect();
    key.push((route.kind.rank(), route.url.clone()));
    key
}

fn relative_to(manifest_dir: &Path, file: &Path) -> String {
    file.strip_prefix(manifest_dir)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

fn unique_module(relative: &Path, used: &mut HashSet<String>) -> String {
    let mut base = String::from("page");
    for component in relative.components() {
        if let Component::Normal(name) = component {
            let stem = Path::new(name)
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_default();
            base.push('_');
            base.push_str(&sanitize(&stem));
        }
    }

    if used.insert(base.clone()) {
        return base;
    }

    let mut counter = 2;
    loop {
        let candidate = format!("{base}_{counter}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        counter += 1;
    }
}

fn sanitize(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_underscore = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_underscore = false;
        } else if !last_underscore {
            out.push('_');
            last_underscore = true;
        }
    }
    out.trim_matches('_').to_string()
}
