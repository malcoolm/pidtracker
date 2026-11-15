//! Vyhledání spojení: formulář (odkud, kam, čas, vyloučené druhy dopravy) a seznam spojení.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use chrono_tz::Tz;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use pid_core::realtime::Delays;
use pid_core::ride::RouteKind;
use pid_core::time::TZ;
use pid_core::timetable::query::{DelayKind, DepartureSource, NearbyGroup, TimetableIndex, time_display};
use pid_core::timetable::router::{Access, Journey, JourneyQuery, Leg, TimeMode, labels};

use crate::components::RouteBadge;
use crate::interop::{Fix, GeoError, current_position};
use crate::state::AppState;
use crate::stop_input::StopInput;
use crate::util::{duration_label, now, page_hidden};

/// Kolik spojení chce jedno hledání (`plan` jich může vrátit víc).
const MIN_RESULTS: usize = 3;
/// „Další spojení“: posun za poslední (u příjezdu před první) výsledek.
const MORE_STEP_S: i64 = 60;
const DELAYS_REFRESH_MS: u32 = 60_000;
/// Pauza před výpočtem, aby se stihlo vykreslit „Hledám…“.
const PAINT_MS: u32 = 30;
/// Horší přesnost polohy se nepoužije (chůze k zastávkám by neseděla).
const MAX_ACCURACY_M: f64 = 1000.0;
/// Počet blízkých uzlů v rozbalení „Moje poloha“.
const NEAR_NODES: usize = 8;
/// Formát `<input type="datetime-local">`.
const INPUT_FMT: &str = "%Y-%m-%dT%H:%M";
/// Druhy dopravy, které lze vyloučit, v pořadí zaškrtávátek.
const EXCLUDABLE: [RouteKind; 7] = [
    RouteKind::Metro,
    RouteKind::Tram,
    RouteKind::Bus,
    RouteKind::Trolleybus,
    RouteKind::Train,
    RouteKind::Ferry,
    RouteKind::Funicular,
];

#[derive(Clone, Copy, PartialEq)]
enum Origin {
    /// Z polohy uživatele (všechna stanoviště v dosahu chůze).
    Position,
    /// Ze zastávky (uzlu) vybrané v poli Odkud.
    Stop,
}

/// Text chyby polohy pro tuto obrazovku (zastávka se tu vybírá, ne vyhledává).
fn geo_message(e: GeoError) -> &'static str {
    match e {
        GeoError::Denied => {
            "Přístup k poloze je zakázaný. Povolte ho v Nastavení → Soukromí → Polohové služby, nebo zastávku vyberte ručně."
        }
        GeoError::Unavailable => "Polohu se nepodařilo zjistit. Zastávku vyberte ručně.",
        GeoError::Timeout => "Zjištění polohy trvá příliš dlouho. Zastávku vyberte ručně.",
    }
}

/// Uzly z `access_from`: (uzel, název, nejkratší chůze v s), seřazené podle chůze.
fn near_nodes(ix: &TimetableIndex, access: &[Access]) -> Vec<(u32, String, u32)> {
    let mut best: Vec<(u32, u32)> = Vec::new();
    for a in access {
        let g = ix.tt.stops[a.stop as usize].group;
        match best.iter_mut().find(|(bg, _)| *bg == g) {
            Some((_, w)) => *w = (*w).min(a.walk_s),
            None => best.push((g, a.walk_s)),
        }
    }
    best.sort_by_key(|&(g, w)| (w, g));
    best.into_iter().take(NEAR_NODES).map(|(g, w)| (g, ix.tt.group_name(g).to_string(), w)).collect()
}

/// Místní čas (Praha); v mezeře při přechodu na letní čas o hodinu později.
fn prague(ndt: NaiveDateTime) -> Option<DateTime<Tz>> {
    TZ.from_local_datetime(&ndt).earliest().or_else(|| TZ.from_local_datetime(&(ndt + Duration::hours(1))).earliest())
}

/// Rozsah výběru času: od začátku prvního do konce posledního dne platnosti dat.
fn feed_bounds(start: NaiveDate, end: NaiveDate) -> (NaiveDateTime, NaiveDateTime) {
    (start.and_time(NaiveTime::MIN), end.and_hms_opt(23, 59, 0).unwrap_or(end.and_time(NaiveTime::MIN)))
}

/// Čas z `datetime-local`, oříznutý na platnost dat. Prázdné / nečitelné = `None` (teď).
fn picked_time(value: &str, ix: &TimetableIndex) -> Option<DateTime<Tz>> {
    let v = value.trim();
    let ndt =
        NaiveDateTime::parse_from_str(v, INPUT_FMT).or_else(|_| NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M:%S")).ok()?;
    let (min, max) = feed_bounds(ix.tt.feed_start, ix.tt.feed_end);
    prague(ndt.clamp(min, max))
}

/// Klíč spojení pro odstranění duplicit: posloupnost spojů.
fn trip_key(j: &Journey) -> Vec<u32> {
    j.legs
        .iter()
        .filter_map(|l| match *l {
            Leg::Ride { trip, .. } => Some(trip),
            Leg::Walk { .. } => None,
        })
        .collect()
}

/// Přidá nová spojení bez duplicit, seřadí podle odchodu; vrací počet přidaných.
fn merge(list: &mut Vec<Journey>, found: Vec<Journey>) -> usize {
    let mut added = 0;
    for j in found {
        let key = trip_key(&j);
        if list.iter().all(|o| trip_key(o) != key) {
            list.push(j);
            added += 1;
        }
    }
    list.sort_by_key(|j| (j.leave, j.arrive, j.transfers));
    added
}

fn transfers_label(n: u8) -> String {
    match n {
        0 => "bez přestupu".into(),
        1 => "1 přestup".into(),
        2..=4 => format!("{n} přestupy"),
        _ => format!("{n} přestupů"),
    }
}

/// Minuty chůze zaokrouhlené nahoru.
fn walk_min(seconds: u32) -> u32 {
    seconds.div_ceil(60)
}

fn hm(t: DateTime<Tz>) -> String {
    t.format("%H:%M").to_string()
}

/// Jedno nalezené spojení. Zatím se na něj neklepe (detail přidá Task 6).
#[component]
fn JourneyCard(j: Journey, fastest: bool, fewest: bool, delays: RwSignal<Delays>) -> impl IntoView {
    let state = expect_context::<AppState>();
    let ix = state.tt.get_untracked().index();
    // Doba podle zobrazených minut, aby „7:44 → 8:21“ bylo vždy „37 min“.
    let minutes = j.arrive.timestamp().div_euclid(60) - j.leave.timestamp().div_euclid(60);
    let mut meta = format!("{} · {}", duration_label(minutes), transfers_label(j.transfers));
    if j.walk_s > 0 {
        meta.push_str(&format!(" · pěšky {} min", walk_min(j.walk_s)));
    }

    let last_ride = j.legs.iter().rposition(|l| matches!(l, Leg::Ride { .. }));
    let mut seen_ride = false;
    let legs = ix.as_ref().map(|ix| {
        j.legs
            .iter()
            .enumerate()
            .filter_map(|(i, l)| match *l {
                // Chůze z polohy na první zastávku.
                Leg::Walk { from: None, seconds, .. } if seconds > 0 => {
                    Some(view! { <span class="jc-walk">{format!("🚶{}", walk_min(seconds))}</span> }.into_any())
                }
                // Pěšky z poslední zastávky do jiného uzlu cíle (přestupy uvnitř cesty jen „›“).
                Leg::Walk { from: Some(_), seconds, .. } if seconds > 0 && last_ride.is_some_and(|r| i > r) => {
                    Some(view! { <span class="jc-walk">{format!("🚶{}", walk_min(seconds))}</span> }.into_any())
                }
                Leg::Walk { .. } => None,
                Leg::Ride { trip, .. } => {
                    let tt = &ix.tt;
                    let pattern = tt.profiles[tt.trips[trip as usize].profile as usize].pattern;
                    let route = &tt.routes[tt.patterns[pattern as usize].route as usize];
                    let sep = std::mem::replace(&mut seen_ride, true).then(|| view! { <span class="jc-sep">"›"</span> });
                    Some(view! { {sep}<RouteBadge name=route.short_name.clone() color=route.color /> }.into_any())
                }
            })
            .collect_view()
    });

    // Odjezd prvního spoje se živým zpožděním.
    let first = j.legs.iter().find_map(|l| match *l {
        Leg::Ride { trip, board_stop, dep, .. } => Some((trip, board_stop, dep)),
        Leg::Walk { .. } => None,
    });
    let departure = ix.as_ref().zip(first).map(|(ix, (trip, stop, dep))| {
        let name = ix.tt.stop_name(stop).to_string();
        let trip_id = ix.tt.trips[trip as usize].gtfs_id.clone();
        let stop_id = ix.tt.stops[stop as usize].gtfs_id.clone();
        let time = move || {
            let delay = delays.with(|d| d.at(&trip_id, &stop_id));
            let t = time_display(dep, delay, now());
            let (class, note) = match t.kind {
                DelayKind::Late => ("late", t.detail.split(" · ").next().map(str::to_string)),
                DelayKind::Early => ("early", t.detail.split(" · ").next().map(str::to_string)),
                DelayKind::OnTime => ("ok", Some("včas".to_string())),
                DelayKind::Unknown => ("", None),
            };
            let main = match t.actual {
                Some(actual) => view! { <s class="sched">{t.scheduled}</s>" "<b class=class>{actual}</b> }.into_any(),
                None => view! { <b>{t.scheduled}</b> }.into_any(),
            };
            view! { {main}{note.map(|n| view! { " "<span class=class>{n}</span> })} }
        };
        view! { <div class="jc-dep">{name}" odj. "{time}</div> }
    });

    view! {
        <div class="journey-card">
            <div class="jc-head">
                <span class="jc-times">{format!("{} → {}", hm(j.leave), hm(j.arrive))}</span>
                {fastest.then(|| view! { <span class="jc-label">"nejrychlejší"</span> })}
                {fewest.then(|| view! { <span class="jc-label">"nejméně přestupů"</span> })}
            </div>
            <div class="jc-meta">{meta}</div>
            <div class="jc-legs">{legs}</div>
            {departure}
        </div>
    }
}

#[component]
pub fn Search() -> impl IntoView {
    let state = expect_context::<AppState>();
    let origin = RwSignal::new(Origin::Position);
    let fix = RwSignal::new(None::<Fix>);
    let geo_msg = RwSignal::new(None::<String>);
    let locating = RwSignal::new(true);
    let show_near = RwSignal::new(false);
    let from_text = RwSignal::new(String::new());
    let from_group = RwSignal::new(None::<u32>);
    let to_text = RwSignal::new(String::new());
    let to_group = RwSignal::new(None::<u32>);
    let mode = RwSignal::new(TimeMode::DepartAt);
    // Prázdné = teď (v okamžiku hledání).
    let time_text = RwSignal::new(String::new());

    let results = RwSignal::new(Vec::<Journey>::new());
    let last_query = StoredValue::new(None::<JourneyQuery>);
    let summary = RwSignal::new(None::<String>);
    let message = RwSignal::new(None::<String>);
    let searching = RwSignal::new(false);
    let searched = RwSignal::new(false);
    let loading_more = RwSignal::new(false);
    let no_more = RwSignal::new(false);
    // Režim posledního hledání (formulář se mezitím může přepnout).
    let shown_mode = RwSignal::new(TimeMode::DepartAt);
    let delays = RwSignal::new(Delays::default());
    let lab = Memo::new(move |_| results.with(|r| labels(r)));

    spawn_local(async move {
        match current_position().await {
            Ok(f) => {
                state.remember_fix(f);
                if f.accuracy_m <= MAX_ACCURACY_M {
                    let _ = fix.try_set(Some(f));
                } else {
                    let _ = geo_msg.try_set(Some(format!(
                        "Poloha je příliš nepřesná (±{:.0} m). Zastávku vyberte ručně.",
                        f.accuracy_m
                    )));
                    let _ = origin.try_set(Origin::Stop);
                }
            }
            Err(e) => {
                let _ = geo_msg.try_set(Some(geo_message(e).into()));
                let _ = origin.try_set(Origin::Stop);
            }
        }
        let _ = locating.try_set(false);
    });

    // Zpoždění: po hledání jednou hned, pak každých 60 s, jen když je stránka vidět.
    let alive = Arc::new(AtomicBool::new(true));
    {
        let alive = alive.clone();
        on_cleanup(move || alive.store(false, Ordering::Relaxed));
    }
    let fetch_now = {
        let alive = alive.clone();
        move || {
            let alive = alive.clone();
            spawn_local(async move {
                let fetched = crate::realtime::fetch_delays().await;
                if alive.load(Ordering::Relaxed) && let Some(d) = fetched {
                    delays.set(d);
                }
            });
        }
    };
    {
        let alive = alive.clone();
        spawn_local(async move {
            loop {
                TimeoutFuture::new(DELAYS_REFRESH_MS).await;
                if !alive.load(Ordering::Relaxed) {
                    break;
                }
                if page_hidden() || results.with_untracked(|r| r.is_empty()) {
                    continue;
                }
                let fetched = crate::realtime::fetch_delays().await;
                if !alive.load(Ordering::Relaxed) {
                    break;
                }
                if let Some(d) = fetched {
                    delays.set(d);
                }
            }
        });
    }
    // Návrat z pozadí: čerstvá zpoždění, jsou-li výsledky.
    {
        let fetch_now = fetch_now.clone();
        Effect::new(move |prev: Option<u32>| {
            let n = state.foreground.get();
            if prev.is_some_and(|p| p != n) && results.with_untracked(|r| !r.is_empty()) {
                fetch_now();
            }
            n
        });
    }

    let near = Signal::derive(move || fix.get().map(|f| (f.lat, f.lon)).or_else(|| state.last_pos()));
    let nodes = Memo::new(move |_| {
        let (Some(ix), Some(f)) = (state.tt.get().index(), fix.get()) else { return Vec::new() };
        near_nodes(&ix, &ix.access_from(f.lat, f.lon))
    });
    // Návrhy pro prázdné pole Odkud: zastávky v okolí polohy.
    let from_candidates = Signal::derive(move || {
        let (Some(ix), Some(f)) = (state.tt.get().index(), fix.get()) else { return Vec::<NearbyGroup>::new() };
        ix.nearby_groups(f.lat, f.lon, f.accuracy_m)
    });

    let excluded = move |k: RouteKind| state.planner_excluded.with(|v| v.contains(&k));
    let set_excluded = move |k: RouteKind, on: bool| {
        let mut v = state.planner_excluded.get_untracked();
        v.retain(|&x| x != k);
        if on {
            v.push(k);
        }
        v.sort();
        v.dedup();
        state.set_planner_excluded(v);
    };

    let bounds = move || {
        state.tt.get().index().map(|ix| {
            let (min, max) = feed_bounds(ix.tt.feed_start, ix.tt.feed_end);
            (min.format(INPUT_FMT).to_string(), max.format(INPUT_FMT).to_string())
        })
    };

    let can_search = move || {
        let from_ok = match origin.get() {
            Origin::Position => fix.get().is_some(),
            Origin::Stop => from_group.get().is_some(),
        };
        state.tt.get().index().is_some() && to_group.get().is_some() && from_ok && !searching.get()
    };

    let search = {
        let alive = alive.clone();
        let fetch_now = fetch_now.clone();
        move |_| {
            let Some(ix) = state.tt.get_untracked().index() else { return };
            let Some(target) = to_group.get_untracked() else { return };
            message.set(None);
            no_more.set(false);
            searched.set(false);
            results.set(Vec::new());
            summary.set(None);
            let (origins, from_name) = match origin.get_untracked() {
                Origin::Position => {
                    let Some(f) = fix.get_untracked() else { return };
                    (ix.access_from(f.lat, f.lon), "📍 Moje poloha".to_string())
                }
                Origin::Stop => {
                    let Some(g) = from_group.get_untracked() else { return };
                    if g == target {
                        message.set(Some("Odkud a kam je stejná zastávka.".into()));
                        return;
                    }
                    (ix.access_group(g), ix.tt.group_name(g).to_string())
                }
            };
            if origins.is_empty() {
                message.set(Some("Do 1,5 km od vás není žádná zastávka. Zastávku vyberte ručně.".into()));
                return;
            }
            let picked = picked_time(&time_text.get_untracked(), &ix);
            let time = picked.unwrap_or_else(now);
            let m = mode.get_untracked();
            let mut excluded = state.planner_excluded.get_untracked();
            excluded.sort();
            excluded.dedup();
            let q = JourneyQuery {
                origins,
                target_group: target,
                mode: m,
                time,
                excluded,
                buffer_s: u32::from(state.planner_buffer_min.get_untracked()) * 60,
            };
            let when = if time.date_naive() == now().date_naive() {
                hm(time)
            } else {
                time.format("%-d. %-m. %H:%M").to_string()
            };
            let what = if m == TimeMode::DepartAt { "odjezd" } else { "příjezd" };
            summary.set(Some(format!("{from_name} → {} · {what} {when}", ix.tt.group_name(target))));
            searching.set(true);
            let alive = alive.clone();
            let fetch_now = fetch_now.clone();
            spawn_local(async move {
                TimeoutFuture::new(PAINT_MS).await;
                if !alive.load(Ordering::Relaxed) {
                    return;
                }
                let found = ix.plan(&q, MIN_RESULTS);
                shown_mode.set(q.mode);
                last_query.set_value(Some(q));
                results.set(found);
                searched.set(true);
                searching.set(false);
                fetch_now();
            });
        }
    };

    let more = Callback::new({
        let alive = alive.clone();
        move |_: ()| {
            let (Some(ix), Some(mut q)) = (state.tt.get_untracked().index(), last_query.get_value()) else { return };
            let next = results.with_untracked(|r| match q.mode {
                TimeMode::DepartAt => r.iter().map(|j| j.leave).max().map(|t| t + Duration::seconds(MORE_STEP_S)),
                TimeMode::ArriveBy => r.iter().map(|j| j.arrive).min().map(|t| t - Duration::seconds(MORE_STEP_S)),
            });
            let Some(next) = next else { return };
            q.time = next;
            loading_more.set(true);
            let alive = alive.clone();
            spawn_local(async move {
                TimeoutFuture::new(PAINT_MS).await;
                if !alive.load(Ordering::Relaxed) {
                    return;
                }
                let found = ix.plan(&q, MIN_RESULTS);
                let mut list = results.get_untracked();
                if merge(&mut list, found) == 0 {
                    no_more.set(true);
                }
                results.set(list);
                loading_more.set(false);
            });
        }
    });
    let more_label = move || match (loading_more.get(), shown_mode.get()) {
        (true, _) => "Hledám…",
        (false, TimeMode::DepartAt) => "Další spojení ↓",
        (false, TimeMode::ArriveBy) => "Dřívější spojení ↑",
    };

    let from_field = move || match origin.get() {
        Origin::Position => view! {
            <button class="search-from" on:click=move |_| show_near.update(|v| *v = !*v)>
                <span>"📍 Moje poloha"</span>
                <span class="muted">{move || if locating.get() { "zjišťuji…" } else if show_near.get() { "▴" } else { "▾" }}</span>
            </button>
            <Show when=move || show_near.get()>
                <div class="chips near-nodes">
                    <For each=move || nodes.get() key=|n| n.0 let:n>
                        {
                            let (g, name, walk) = n;
                            let picked = name.clone();
                            view! {
                                <button class="chip" on:click=move |_| {
                                    from_text.set(picked.clone());
                                    from_group.set(Some(g));
                                    origin.set(Origin::Stop);
                                    show_near.set(false);
                                }>
                                    {name}<small>{format!(" · 🚶{} min", walk_min(walk))}</small>
                                </button>
                            }
                        }
                    </For>
                    <button class="chip" on:click=move |_| {
                        from_text.set(String::new());
                        from_group.set(None);
                        origin.set(Origin::Stop);
                        show_near.set(false);
                    }>"Jiná zastávka…"</button>
                </div>
            </Show>
        }
        .into_any(),
        Origin::Stop => view! {
            <StopInput
                text=from_text
                selected=from_group
                near=near
                candidates=from_candidates
                placeholder="Odkud – zastávka…"
            />
            <Show when=move || fix.get().is_some()>
                <button class="link small search-back" on:click=move |_| origin.set(Origin::Position)>"📍 Použít moji polohu"</button>
            </Show>
        }
        .into_any(),
    };

    view! {
        <section class="search">
            <div class="search-form">
                <div class="search-field">
                    <span class="search-label">"Odkud"</span>
                    {from_field}
                    {move || geo_msg.get().filter(|_| origin.get() == Origin::Stop).map(|m| view! { <p class="warn small">{m}</p> })}
                </div>
                <div class="search-field">
                    <span class="search-label">"Kam"</span>
                    <StopInput text=to_text selected=to_group near=near placeholder="Kam – zastávka…" />
                </div>
                <div class="search-time">
                    <div class="segmented">
                        <button class:selected=move || mode.get() == TimeMode::DepartAt on:click=move |_| mode.set(TimeMode::DepartAt)>"Odjezd"</button>
                        <button class:selected=move || mode.get() == TimeMode::ArriveBy on:click=move |_| mode.set(TimeMode::ArriveBy)>"Příjezd"</button>
                    </div>
                    <button class="chip" class:selected=move || time_text.get().is_empty() on:click=move |_| time_text.set(String::new())>"teď"</button>
                    <input
                        type="datetime-local"
                        aria-label="Datum a čas"
                        min=move || bounds().map(|b| b.0)
                        max=move || bounds().map(|b| b.1)
                        prop:value=move || time_text.get()
                        on:change=move |ev| time_text.set(event_target_value(&ev))
                    />
                </div>
                <div class="search-exclude">
                    <span class="muted small">"Vyloučit:"</span>
                    {EXCLUDABLE.into_iter().map(|k| view! {
                        <label class="check">
                            <input type="checkbox"
                                prop:checked=move || excluded(k)
                                on:change=move |ev| set_excluded(k, event_target_checked(&ev)) />
                            {k.label_cs()}
                        </label>
                    }).collect_view()}
                </div>
                <button class="primary big" disabled=move || !can_search() on:click=search>
                    {move || if searching.get() { "Hledám…" } else { "Hledat" }}
                </button>
            </div>

            {move || message.get().map(|m| view! { <p class="warn">{m}</p> })}
            {move || summary.get().map(|s| view! { <p class="search-summary">{s}</p> })}
            <Show when=move || searched.get() && results.with(|r| r.is_empty())>
                <p class="muted">"Spojení nenalezeno. Zkuste jiný čas nebo povolte víc druhů dopravy."</p>
            </Show>
            <div class="list">
                {move || {
                    let l = lab.get();
                    results
                        .get()
                        .into_iter()
                        .enumerate()
                        .map(|(i, j)| view! {
                            <JourneyCard j=j fastest=l.fastest == Some(i) fewest=l.fewest == Some(i) delays=delays />
                        })
                        .collect_view()
                }}
            </div>
            <Show when=move || searched.get() && results.with(|r| !r.is_empty())>
                <Show
                    when=move || !no_more.get()
                    fallback=|| view! { <p class="muted small">"Další spojení nenalezena."</p> }
                >
                    <button class="link search-more" disabled=move || loading_more.get() on:click=move |_| more.run(())>
                        {more_label}
                    </button>
                </Show>
            </Show>
        </section>
    }
}
