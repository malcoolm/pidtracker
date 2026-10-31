//! Jednotný výběr zastávky: našeptávač bez diakritiky, řazený podle blízkosti, s ručním zadáním.

use leptos::prelude::*;
use pid_core::timetable::query::{DepartureSource, NearbyGroup};

use crate::state::AppState;

const SUGGESTIONS: usize = 8;

/// Pole pro zastávku. Text bez výběru = ručně zadaný název.
#[component]
pub fn StopInput(
    /// Co je v poli; bez výběru je to ručně zadaný název.
    text: RwSignal<String>,
    /// Vybraná skupina zastávek, `None` = ruční zadání nebo nic.
    selected: RwSignal<Option<u32>>,
    /// Poloha pro řazení návrhů podle blízkosti (lat, lon).
    near: Signal<Option<(f64, f64)>>,
    /// Návrhy pro prázdné pole (např. zastávky v okolí aktuální polohy).
    #[prop(optional)]
    candidates: Option<Signal<Vec<NearbyGroup>>>,
    #[prop(optional, into)] placeholder: Option<String>,
    #[prop(optional)] on_pick: Option<Callback<u32>>,
) -> impl IntoView {
    let state = expect_context::<AppState>();
    let has_tt = move || state.tt.get().index().is_some();
    let placeholder = placeholder.unwrap_or_else(|| "Hledat zastávku…".into());
    // Po výběru je v poli název zastávky – návrhy pak ukazují okolí, ne jen tuto jednu shodu.
    let query = move || if selected.get().is_some() { String::new() } else { text.get() };
    let shown = move || {
        let Some(ix) = state.tt.get().index() else { return Vec::new() };
        let q = query();
        if q.trim().is_empty() {
            if let Some(c) = candidates.map(|c| c.get()).filter(|c| !c.is_empty()) {
                return c;
            }
        }
        ix.search_groups_near(&q, SUGGESTIONS, near.get())
    };
    view! {
        <div class="picker">
            <input
                type="search"
                autocomplete="off"
                placeholder=placeholder
                prop:value=move || text.get()
                on:input=move |ev| {
                    text.set(event_target_value(&ev));
                    selected.set(None);
                }
            />
            <Show when=has_tt>
                <div class="chips">
                    <For each=shown key=|g| g.group let:g>
                        {
                            let id = g.group;
                            let name = g.name.clone();
                            view! {
                                <button
                                    class="chip"
                                    class:selected=move || selected.get() == Some(id)
                                    on:click=move |_| {
                                        text.set(name.clone());
                                        selected.set(Some(id));
                                        if let Some(cb) = on_pick { cb.run(id); }
                                    }
                                >
                                    {g.name.clone()}
                                    {(g.distance_m > 0.0).then(|| view! { <small>{format!(" · {:.0} m", g.distance_m)}</small> })}
                                </button>
                            }
                        }
                    </For>
                </div>
            </Show>
        </div>
    }
}
