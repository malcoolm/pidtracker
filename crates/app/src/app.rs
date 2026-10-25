//! Kořenová komponenta: stav, start služeb, přepínání obrazovek.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::{BottomNav, SyncBadge, Toast};
use crate::config::{APP_VERSION, BUILD_ID};
use crate::screens::{Alight, Board, Edit, History, Home, Onboarding, Settings, Stats};
use crate::state::{AppState, Screen, TtState};
use crate::{storage, sync, timetable_loader, update};

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();
    provide_context(state);

    spawn_local(async move {
        match storage::all_rides().await {
            Ok(rides) => state.rides.set(rides),
            Err(e) => state.show_toast(format!("Lokální data nejdou načíst: {}", e.0)),
        }
        sync::install_triggers(state);
    });
    spawn_local(async move {
        let tt = match timetable_loader::load().await {
            Ok(ix) => TtState::Ready(ix),
            Err(e) => TtState::Missing(e.message()),
        };
        state.tt.set(tt);
    });
    update::install(state);

    let needs_setup = move || state.me.get().is_none();

    view! {
        <Show when=move || !needs_setup() fallback=|| view! { <main class="app"><Onboarding /></main> }>
            <main class="app">
                <SyncBadge />
                {move || match state.screen.get() {
                    Screen::Home => view! { <Home /> }.into_any(),
                    Screen::Board { group } => view! { <Board group=group /> }.into_any(),
                    Screen::Alight => view! { <Alight /> }.into_any(),
                    Screen::History => view! { <History /> }.into_any(),
                    Screen::Edit(id) => view! { <Edit id=id /> }.into_any(),
                    Screen::Stats => view! { <Stats /> }.into_any(),
                    Screen::Settings => view! { <Settings /> }.into_any(),
                }}
                <footer class="version">{format!("v{APP_VERSION} ({BUILD_ID})")}</footer>
            </main>
            <BottomNav />
        </Show>
        <Toast />
    }
}
