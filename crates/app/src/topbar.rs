//! Horní proužek, boční menu a stavový štítek synchronizace.

use leptos::prelude::*;

use pid_core::alerts::{AlertPhase, ServiceAlert};

use crate::components::{RouteBadge, SyncBadge};
use crate::state::{AppState, Screen, SyncStatus};

/// Krátký stav synchronizace v proužku; klepnutí otevře menu.
#[component]
pub fn StatusChip() -> impl IntoView {
    let state = expect_context::<AppState>();
    let text = move || match state.sync.get() {
        SyncStatus::Idle { last: Some(t) } => format!("✓ {t}"),
        SyncStatus::Idle { last: None } => "–".to_string(),
        SyncStatus::Running => "⟳".to_string(),
        SyncStatus::Pending(n) => format!("čeká {n}"),
        SyncStatus::NeedsSignIn(_) | SyncStatus::Error(_) => "⚠".to_string(),
    };
    view! {
        <button class="status-chip" aria-label="Stav synchronizace" on:click=move |_| state.menu_open.set(true)>
            {text}
        </button>
    }
}

/// Boční menu přes ztmavení.
#[component]
pub fn Menu() -> impl IntoView {
    let state = expect_context::<AppState>();
    let item = move |label: &'static str, target: Screen| {
        let t = target.clone();
        let active = move || {
            let s = state.screen.get();
            std::mem::discriminant(&s) == std::mem::discriminant(&t)
                || (matches!(t, Screen::Home) && matches!(s, Screen::Board { .. } | Screen::Alight))
                || (matches!(t, Screen::History) && matches!(s, Screen::Edit(_)))
        };
        let is_alerts = matches!(target, Screen::Alerts { .. });
        view! {
            <button
                class="menu-item"
                class:active=active
                on:click=move |_| {
                    state.screen.set(target.clone());
                    state.menu_open.set(false);
                }
            >
                {label}
                <Show when=move || is_alerts && (state.alert_count.get() > 0)>
                    <span class="menu-count">{move || state.alert_count.get()}</span>
                </Show>
            </button>
        }
    };
    view! {
        <Show when=move || state.menu_open.get()>
            <div class="menu-backdrop" on:click=move |_| state.menu_open.set(false)></div>
            <nav class="menu" aria-label="Menu">
                <div class="menu-head">
                    <div class="menu-user">"Přihlášen: " {move || state.me.get().map(|e| state.nickname(&e)).unwrap_or_default()}</div>
                    <SyncBadge />
                </div>
                {item("Jízda", Screen::Home)}
                {item("Historie", Screen::History)}
                {item("Statistiky", Screen::Stats)}
                {item("Mimořádnosti", Screen::Alerts { open: None })}
                {item("Nastavení", Screen::Settings)}
            </nav>
        </Show>
    }
}

#[component]
pub fn TopBar() -> impl IntoView {
    let state = expect_context::<AppState>();
    view! {
        <header class="topbar">
            <button
                class="topbar-btn"
                aria-label="Menu"
                on:click=move |_| state.menu_open.update(|o| *o = !*o)
            >
                {move || if state.menu_open.get() { "✕" } else { "☰" }}
                <Show when=move || !state.menu_open.get() && (state.alert_count.get() > 0)>
                    <span class="dot"></span>
                </Show>
            </button>
            <span class="topbar-title">"PID Tracker"</span>
            <StatusChip />
        </header>
        <AlertBanner />
        <Menu />
    }
}

/// Pruh nové mimořádnosti pod proužkem. Klepnutí otevře seznam, ✕ jen zavře.
#[component]
pub fn AlertBanner() -> impl IntoView {
    let state = expect_context::<AppState>();
    let content = move || {
        let ids = state.alert_banner.get();
        let found: Vec<ServiceAlert> =
            state.alerts.with(|all| ids.iter().filter_map(|id| all.iter().find(|a| &a.id == id).cloned()).collect());
        let first = found.first()?.clone();
        let first_id = first.id.clone();
        let label = if found.len() == 1 {
            let planned = (first.phase(crate::alerts::now_s()) == AlertPhase::Planned)
                .then(|| first.start.map(|s| format!(" · od {}", crate::alerts::when_label(s))))
                .flatten()
                .unwrap_or_default();
            let badges = first
                .route_ids
                .iter()
                .take(4)
                .map(|r| {
                    let (name, color) = crate::alerts::route_badge_info(state, r);
                    view! { <RouteBadge name=name color=color /> }
                })
                .collect_view();
            let more = first.route_ids.len().saturating_sub(4);
            view! {
                "⚠ Nová mimořádnost"{planned}" · "{badges}{(more > 0).then(|| format!(" +{more}"))}
                <span class="alert-banner-text">" · "{first.header.clone()}</span>
            }
            .into_any()
        } else {
            view! { {format!("⚠ {} nové mimořádnosti", found.len())} }.into_any()
        };
        Some(view! {
            <div class="alert-banner" role="status">
                <button
                    class="alert-banner-main"
                    on:click=move |_| {
                        state.alert_banner.set(Vec::new());
                        state.screen.set(Screen::Alerts { open: Some(first_id.clone()) });
                    }
                >
                    {label}
                </button>
                <button class="alert-banner-close" aria-label="Zavřít" on:click=move |_| state.alert_banner.set(Vec::new())>"✕"</button>
            </div>
        })
    };
    view! { {content} }
}
