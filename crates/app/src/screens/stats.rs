//! Statistiky: období (týden/měsíc/rok/celkem), osoby, počty, čas, typy dopravy, linky.

use leptos::prelude::*;
use pid_core::stats::{Period, PeriodKind, PersonStats, StatsQuery, compute};

use crate::state::AppState;
use crate::util::{css_color, duration_label, kind_color, now};

#[derive(Clone, PartialEq)]
enum Who {
    Me,
    Other(String),
    Both,
}

#[component]
pub fn Stats() -> impl IntoView {
    let state = expect_context::<AppState>();
    let period = RwSignal::new(Period::containing(PeriodKind::Month, now().date_naive()));
    let who = RwSignal::new(Who::Both);

    let me = move || state.me.get().unwrap_or_default().to_lowercase();
    let others = move || {
        let me = me();
        let mut p: Vec<String> = state.rides.with(|rs| {
            rs.iter().map(|r| r.person.to_lowercase()).filter(|p| *p != me).collect()
        });
        p.sort();
        p.dedup();
        p
    };
    let result = Memo::new(move |_| {
        let persons = match who.get() {
            Who::Me => vec![me()],
            Who::Other(p) => vec![p],
            Who::Both => {
                let mut v = vec![me()];
                v.extend(others());
                v
            }
        };
        state.rides.with(|rs| compute(rs, &StatsQuery { period: period.get(), persons }))
    });
    let set_kind = move |k: PeriodKind| period.set(Period::containing(k, now().date_naive()));

    view! {
        <section class="stats">
            <h2>"Statistiky"</h2>
            <div class="segmented">
                {[(PeriodKind::Week, "Týden"), (PeriodKind::Month, "Měsíc"), (PeriodKind::Year, "Rok"), (PeriodKind::All, "Celkem")]
                    .into_iter()
                    .map(|(k, label)| view! {
                        <button class:selected=move || period.get().kind() == k on:click=move |_| set_kind(k)>{label}</button>
                    })
                    .collect_view()}
            </div>
            <Show when=move || period.get() != Period::All>
                <div class="period-nav">
                    <button class="icon" on:click=move |_| period.update(|p| *p = p.prev())>"‹"</button>
                    <span>{move || period.get().label_cs()}</span>
                    <button class="icon" on:click=move |_| period.update(|p| *p = p.next())>"›"</button>
                </div>
            </Show>
            <div class="segmented">
                <button class:selected=move || who.get() == Who::Me on:click=move |_| who.set(Who::Me)>"Já"</button>
                <For each=others key=|p| p.clone() let:p>
                    {
                        let p2 = p.clone();
                        let label = state.nickname(&p);
                        view! {
                            <button class:selected=move || who.get() == Who::Other(p.clone())
                                on:click=move |_| who.set(Who::Other(p2.clone()))>{label}</button>
                        }
                    }
                </For>
                <button class:selected=move || who.get() == Who::Both on:click=move |_| who.set(Who::Both)>"Oba"</button>
            </div>
            <div class="stats-grid">
                <For each=move || result.get() key=|p| (p.person.clone(), p.rides, p.minutes, p.series.len()) let:ps>
                    <PersonCard stats=ps />
                </For>
            </div>
        </section>
    }
}

#[component]
fn PersonCard(stats: PersonStats) -> impl IntoView {
    let state = expect_context::<AppState>();
    let is_me = state.me.get_untracked().is_some_and(|m| m.eq_ignore_ascii_case(&stats.person));
    let name = if is_me { "Já".to_string() } else { state.nickname(&stats.person) };
    let max_kind = stats.by_kind.iter().map(|k| k.1).max().unwrap_or(1).max(1);
    view! {
        <article class="card">
            <h3>{name}</h3>
            <div class="kpis">
                <div><strong>{stats.rides}</strong><span>"jízd"</span></div>
                <div><strong>{duration_label(stats.minutes)}</strong><span>"v MHD"</span></div>
            </div>
            <Show when=move || stats.rides == 0>
                <p class="muted">"V tomto období žádné jízdy."</p>
            </Show>
            <div class="bars">
                {stats.by_kind.iter().map(|(kind, n)| {
                    let w = *n as f64 / max_kind as f64 * 100.0;
                    let color = css_color(kind_color(kind.to_gtfs()));
                    view! {
                        <div class="bar-row">
                            <span class="bar-label">{kind.label_cs()}</span>
                            <span class="bar"><span style=format!("width:{w:.0}%;background:{color}")></span></span>
                            <span class="bar-value">{*n}</span>
                        </div>
                    }
                }).collect_view()}
            </div>
            <Show when={let empty = stats.top_routes.is_empty(); move || !empty}>
                <h4>"Nejčastější linky"</h4>
                <ol class="top">
                    {stats.top_routes.iter().map(|(r, n)| view! { <li><strong>{r.clone()}</strong> {format!(" × {n}")}</li> }).collect_view()}
                </ol>
            </Show>
            <SeriesChart series=stats.series.clone() />
        </article>
    }
}

/// Jednoduchý sloupcový graf v SVG.
#[component]
fn SeriesChart(series: Vec<(String, u32)>) -> impl IntoView {
    let max = series.iter().map(|s| s.1).max().unwrap_or(0);
    if series.is_empty() || max == 0 {
        return ().into_any();
    }
    let n = series.len() as f64;
    let (w, h, label_h) = (300.0, 90.0, 14.0);
    let slot = w / n;
    let bar_w = (slot * 0.7).max(1.0);
    let every = (series.len() / 8).max(1);
    view! {
        <svg class="chart" viewBox=format!("0 0 {w} {}", h + label_h) role="img" aria-label="Počet jízd v čase">
            {series.iter().enumerate().map(|(i, (label, v))| {
                let bh = (*v as f64 / max as f64) * (h - 12.0);
                let x = i as f64 * slot + (slot - bar_w) / 2.0;
                let show_label = i % every == 0;
                view! {
                    <g>
                        <rect x=x y=h - bh width=bar_w height=bh rx="1.5" class="chart-bar"><title>{format!("{label}: {v}")}</title></rect>
                        {(*v > 0 && n <= 12.0).then(|| view! { <text x=x + bar_w / 2.0 y=h - bh - 2.0 class="chart-value" text-anchor="middle">{*v}</text> })}
                        {show_label.then(|| view! { <text x=x + bar_w / 2.0 y=h + label_h - 2.0 class="chart-label" text-anchor="middle">{label.clone()}</text> })}
                    </g>
                }
            }).collect_view()}
        </svg>
    }
    .into_any()
}
