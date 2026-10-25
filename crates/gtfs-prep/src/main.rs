use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use gtfs_prep::{FeedSource, build_timetable, prepare_feed, write_output};

/// Převede PID GTFS na kompaktní jízdní řád pro PID Tracker.
#[derive(Parser)]
struct Args {
    /// Stáhnout aktuální GTFS z data.pid.cz.
    #[arg(long, conflicts_with = "gtfs", required_unless_present = "gtfs")]
    fetch: bool,
    /// Lokální GTFS (zip nebo rozbalená složka).
    #[arg(long)]
    gtfs: Option<PathBuf>,
    /// Výstupní složka (vznikne manifest.json + timetable-<verze>.pidt).
    #[arg(long)]
    out: PathBuf,
    /// Pracovní složka pro stažení a rozbalení.
    #[arg(long, default_value = ".gtfs")]
    work_dir: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let started = Instant::now();
    let source = match args.gtfs {
        Some(p) => FeedSource::Path(p),
        None => FeedSource::Fetch,
    };
    let dir = prepare_feed(source, &args.work_dir)?;
    let t = build_timetable(&dir, chrono::Utc::now())?;
    let info = write_output(&t, &args.out)?;
    println!(
        "verze {} | platnost {}–{} | zastávek {} | skupin {} | linek {} | patternů {} | profilů {} | spojů {}",
        t.version,
        t.feed_start,
        t.feed_end,
        t.stops.len(),
        t.groups.len(),
        t.routes.len(),
        t.patterns.len(),
        t.profiles.len(),
        t.trips.len()
    );
    println!(
        "{} → {:.2} MB za {:.1} s",
        args.out.join(&info.file).display(),
        info.bytes as f64 / 1024.0 / 1024.0,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
