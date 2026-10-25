//! Build úlohy: `cargo xtask dist`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Sestaví distribuční složku `dist/` pro GitHub Pages.
    Dist {
        /// Cesta, pod kterou aplikace poběží (GitHub Pages podcesta).
        #[arg(long, default_value = "/pidtracker/")]
        public_url: String,
        /// Lokální GTFS (zip nebo rozbalená složka).
        #[arg(long, conflicts_with_all = ["fetch_gtfs", "no_data"])]
        gtfs: Option<PathBuf>,
        /// Stáhnout aktuální PID GTFS.
        #[arg(long, conflicts_with = "no_data")]
        fetch_gtfs: bool,
        /// Sestavit bez dat jízdního řádu.
        #[arg(long)]
        no_data: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Dist { public_url, gtfs, fetch_gtfs, no_data } => {
            let data = match (gtfs, fetch_gtfs, no_data) {
                (Some(p), _, _) => DataSource::Local(p),
                (None, true, _) => DataSource::Fetch,
                (None, false, true) => DataSource::None,
                (None, false, false) => bail!("zadej --gtfs <cesta>, --fetch-gtfs nebo --no-data"),
            };
            dist(&public_url, data)
        }
    }
}

enum DataSource {
    None,
    Local(PathBuf),
    Fetch,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

pub fn make_build_id(version: &str, git_hash: &str, now: DateTime<Utc>) -> String {
    format!("{version}+{git_hash}.{}", now.format("%Y%m%d%H%M"))
}

pub fn patch_sw(src: &str, build_id: &str, precache: &[String]) -> String {
    let list = serde_json::to_string(precache).expect("seznam řetězců jde vždy serializovat");
    src.replace("__BUILD_ID__", build_id).replace("__PRECACHE__", &list)
}

/// Soubory aplikace, které si service worker uloží při instalaci (bez dat, sw.js a version.json).
/// Cesty jsou relativní k umístění sw.js; "./" je samotná stránka.
pub fn precache_list(dist: &Path) -> Result<Vec<String>> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(&path, base, out)?;
            } else {
                let rel = path.strip_prefix(base)?.to_string_lossy().replace('\\', "/");
                let hidden = rel.split('/').any(|part| part.starts_with('.'));
                if !hidden && rel != "sw.js" && rel != "version.json" && !rel.starts_with("data/") {
                    out.push(rel);
                }
            }
        }
        Ok(())
    }
    let mut out = vec!["./".to_string()];
    walk(dist, dist, &mut out)?;
    out[1..].sort();
    Ok(out)
}

fn git_hash() -> String {
    let out = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(root())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let hash = out(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "nogit".into());
    let dirty = out(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty());
    if dirty { format!("{hash}-dirty") } else { hash }
}

fn dist(public_url: &str, data: DataSource) -> Result<()> {
    let root = root();
    let dist = root.join("dist");
    let now = Utc::now();
    let version = env!("CARGO_PKG_VERSION");
    let build_id = make_build_id(version, &git_hash(), now);
    let built_at = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let status = Command::new("trunk")
        .args(["build", "--release", "--public-url", public_url])
        .current_dir(root.join("crates/app"))
        .env("PIDTRACKER_BUILD_ID", &build_id)
        .env("PIDTRACKER_BUILT_AT", &built_at)
        .status()
        .context("spuštění trunk (nainstaluj: cargo install trunk --locked)")?;
    if !status.success() {
        bail!("trunk build selhal");
    }

    let version_json = serde_json::json!({
        "app_version": version,
        "build_id": build_id,
        "built_at": built_at,
    });
    std::fs::write(dist.join("version.json"), serde_json::to_string_pretty(&version_json)?)?;

    let sw_path = dist.join("sw.js");
    let sw = std::fs::read_to_string(&sw_path).context("dist/sw.js chybí")?;
    let precache = precache_list(&dist)?;
    std::fs::write(&sw_path, patch_sw(&sw, &build_id, &precache))?;

    let source = match data {
        DataSource::None => None,
        DataSource::Local(p) => Some(gtfs_prep::FeedSource::Path(p)),
        DataSource::Fetch => Some(gtfs_prep::FeedSource::Fetch),
    };
    if let Some(source) = source {
        let feed = gtfs_prep::prepare_feed(source, &root.join(".gtfs"))?;
        let timetable = gtfs_prep::build_timetable(&feed, now)?;
        let info = gtfs_prep::write_output(&timetable, &dist.join("data"))?;
        println!(
            "jízdní řád {} ({}–{}): {:.2} MB",
            info.version,
            timetable.feed_start,
            timetable.feed_end,
            info.bytes as f64 / 1024.0 / 1024.0
        );
    }

    println!("dist hotov: {} ({build_id})", dist.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn build_id_format() {
        let id = make_build_id(
            "0.1.0",
            "a1b2c3d",
            chrono::Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
        );
        assert_eq!(id, "0.1.0+a1b2c3d.202610251200");
    }

    #[test]
    fn sw_placeholder_replaced() {
        assert_eq!(
            patch_sw("const V='__BUILD_ID__'; const P=__PRECACHE__;", "X", &["./".into(), "a.js".into()]),
            r#"const V='X'; const P=["./","a.js"];"#
        );
    }

    #[test]
    fn precache_list_covers_app_shell_only() {
        let dir = std::env::temp_dir().join(format!("pt-precache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for f in [
            "index.html",
            "app-1.js",
            "app-1_bg.wasm",
            "style-2.css",
            "manifest.webmanifest",
            "icons/icon-192.png",
            "snippets/app-3/inline0.js",
            "sw.js",
            "version.json",
            "data/manifest.json",
            "data/timetable-x.pidt",
            ".trunk-lock",
        ] {
            let p = dir.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        let list = precache_list(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            list,
            vec![
                "./",
                "app-1.js",
                "app-1_bg.wasm",
                "icons/icon-192.png",
                "index.html",
                "manifest.webmanifest",
                "snippets/app-3/inline0.js",
                "style-2.css",
            ]
        );
    }
}
