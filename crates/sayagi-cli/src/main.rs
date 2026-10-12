//! `sayagi` — CLI metaframework Sayagi Rust.
//!
//! ```text
//! sayagi new <dir>            scaffold proyek PWA baru
//! sayagi dev                  dev server + hot reload (cargo leptos watch)
//! sayagi build [--release]    build produksi: binary server + bundle Wasm
//! sayagi routes               tampilkan rute hasil pemindaian src/pages
//! sayagi pwa                  regenerate manifest, service worker, dan ikon
//! ```

mod templates;

use std::path::{Path, PathBuf};
use std::process::Command;

type CliResult = Result<(), String>;

const HELP: &str = "\
sayagi — CLI metaframework Sayagi Rust

USAGE:
    sayagi <COMMAND> [OPTIONS]

COMMANDS:
    new <dir>        Scaffold proyek PWA baru di <dir>
    dev              Jalankan dev server dengan hot reload
    build            Build produksi (binary Axum + bundle Wasm)
    routes           Tampilkan rute yang dipetakan dari src/pages
    pwa              Regenerate manifest.webmanifest, sw.js, dan ikon
    help             Tampilkan bantuan ini

OPTIONS:
    --name <crate>   (new) Nama crate; default diambil dari nama folder
    --dir <path>     (dev/build/routes/pwa) Root proyek; default direktori saat ini
    --release        (build) Build dalam mode release
    --force          (new) Timpa berkas yang sudah ada
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("new") => cmd_new(&args[1..]),
        Some("dev") => cmd_leptos(&args[1..], LeptosMode::Watch),
        Some("build") => cmd_leptos(&args[1..], LeptosMode::Build),
        Some("routes") => cmd_routes(&args[1..]),
        Some("pwa") => cmd_pwa(&args[1..]),
        Some("help") | Some("-h") | Some("--help") | None => {
            println!("{HELP}");
            Ok(())
        }
        Some(other) => Err(format!("perintah tidak dikenal: {other}\n\n{HELP}")),
    };

    if let Err(error) = result {
        eprintln!("sayagi: {error}");
        std::process::exit(1);
    }
}

fn cmd_new(args: &[String]) -> CliResult {
    let mut target: Option<PathBuf> = None;
    let mut name: Option<String> = None;
    let mut force = false;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--name" => {
                name = Some(next_value(args, &mut index, "--name")?);
            }
            "--force" => force = true,
            flag if flag.starts_with("--") => return Err(format!("opsi tidak dikenal: {flag}")),
            value => target = Some(PathBuf::from(value)),
        }
        index += 1;
    }

    let target = target.ok_or("`sayagi new` butuh argumen <dir>")?;
    let project_name = name.unwrap_or_else(|| {
        target
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "sayagi-app".to_string())
    });
    let project_name = normalize_crate_name(&project_name);
    let crate_name = project_name.replace('-', "_");
    let project_title = project_name.replace('-', " ");
    let cli_dir = templates::workspace_root().join("crates/sayagi-cli");
    let sayagi_root = templates::workspace_root().to_string_lossy().to_string();

    if target.exists() && !force {
        let is_empty = std::fs::read_dir(&target)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !is_empty {
            return Err(format!(
                "{} sudah ada dan tidak kosong (gunakan --force untuk menimpa)",
                target.display()
            ));
        }
    }

    let vars = [
        ("project_name", project_name.as_str()),
        ("crate_name", crate_name.as_str()),
        ("project_title", project_title.as_str()),
        ("sayagi_root", sayagi_root.as_str()),
        ("site_addr", "127.0.0.1:3000"),
    ];

    let written = templates::scaffold(&target, &vars)?;

    println!("Proyek `{project_name}` dibuat di {}", target.display());
    for path in &written {
        println!("  + {}", path.display());
    }
    println!(
        "\nLangkah berikutnya:\n  cd {}\n  cargo run --manifest-path {}/Cargo.toml --bin sayagi -- dev\n",
        target.display(),
        cli_dir.display()
    );
    Ok(())
}

fn cmd_leptos(args: &[String], mode: LeptosMode) -> CliResult {
    let mut dir = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut release = false;
    let mut passthrough = Vec::new();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dir" => dir = PathBuf::from(next_value(args, &mut index, "--dir")?),
            "--release" => release = true,
            other => passthrough.push(other.to_string()),
        }
        index += 1;
    }

    ensure_cargo_leptos()?;

    let mut command = Command::new("cargo");
    command.arg("leptos");
    command.arg(mode.command());
    if mode == LeptosMode::Build && release {
        command.arg("--release");
    }
    command.args(&passthrough).current_dir(&dir);

    println!("sayagi: cargo leptos {} ({})", mode.command(), dir.display());
    let status = command.status().map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("cargo leptos {} gagal ({status})", mode.command()));
    }
    Ok(())
}

fn cmd_routes(args: &[String]) -> CliResult {
    let dir = project_dir(args)?;
    let routes = sayagi_build::scan_routes(&dir).map_err(|error| error.to_string())?;

    let path_width = routes
        .iter()
        .map(|route| route.url.len())
        .max()
        .unwrap_or(4)
        .max(4);

    println!("{:<path_width$}  {:<8}  {}", "PATH", "KIND", "FILE");
    for route in &routes {
        println!(
            "{:<path_width$}  {:<8}  {}",
            route.url,
            route.kind.label(),
            route.rel_file
        );
    }
    println!("\n{} rute dipetakan dari src/pages", routes.len());
    Ok(())
}

fn cmd_pwa(args: &[String]) -> CliResult {
    let dir = project_dir(args)?;
    let report = sayagi_build::generate_assets(&dir).map_err(|error| error.to_string())?;
    for path in &report.files {
        println!("  + {}", path.display());
    }
    println!("\n{} berkas PWA digenerate", report.files.len());
    Ok(())
}

fn project_dir(args: &[String]) -> Result<PathBuf, String> {
    let mut dir = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dir" => dir = PathBuf::from(next_value(args, &mut index, "--dir")?),
            flag => return Err(format!("opsi tidak dikenal: {flag}")),
        }
        index += 1;
    }
    Ok(dir)
}

fn next_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} butuh sebuah nilai"))
}

fn ensure_cargo_leptos() -> CliResult {
    let available = Command::new("cargo")
        .args(["leptos", "--version"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if available {
        Ok(())
    } else {
        Err("cargo-leptos tidak ditemukan. Pasang dengan:\n  cargo install cargo-leptos --locked".to_string())
    }
}

fn normalize_crate_name(value: &str) -> String {
    let name = Path::new(value)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| value.to_string());

    let mut normalized = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        } else if !normalized.ends_with('-') {
            normalized.push('-');
        }
    }
    normalized.trim_matches('-').to_string()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LeptosMode {
    Watch,
    Build,
}

impl LeptosMode {
    fn command(self) -> &'static str {
        match self {
            LeptosMode::Watch => "watch",
            LeptosMode::Build => "build",
        }
    }
}
