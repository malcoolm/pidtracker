//! Historie jízd seskupená po dnech, s filtrem osob.

use leptos::prelude::*;
use pid_core::ride::Ride;

use crate::components::{RideRow, day_label};
use crate::state::AppState;

#[component]
pub fn History() -> impl IntoView {
    let state = expect_context::<AppState>();
    // None = všichni; výchozí jsou jízdy přihlášeného uživatele.
    let filter = RwSignal::new(state.me.get_untracked().map(|me| me.to_lowercase()));
    let persons = move || {
        let mut p: Vec<String> = state.rides.with(|rs| rs.iter().map(|r| r.person.to_lowercase()).collect());
        if let Some(me) = state.me.get() {
            p.push(me.to_lowercase());
        }
        p.sort();
        p.dedup();
        // já první
        let me = state.me.get().unwrap_or_default().to_lowercase();
        p.sort_by_key(|x| *x != me);
        p
    };
    let groups = move || {
        let f = filter.get();
        let mut rides: Vec<Ride> = state
            .rides
            .get()
            .into_iter()
            .filter(|r| !r.deleted && f.as_ref().is_none_or(|p| r.person.eq_ignore_ascii_case(p)))
            .collect();
        rides.sort_by_key(|r| std::cmp::Reverse(r.board_time));
        let mut out: Vec<(String, Vec<Ride>)> = Vec::new();
        for r in rides {
            let label = day_label(&r);
            match out.last_mut() {
                Some((l, v)) if *l == label => v.push(r),
                _ => out.push((label, vec![r])),
            }
        }
        out
    };

    view! {
        <section class="history">
            <h2>"Historie"</h2>
            <div class="segmented">
                <For each=persons key=|p| p.clone() let:p>
                    {
                        let label = if state.me.get_untracked().is_some_and(|me| me.eq_ignore_ascii_case(&p)) {
                            "Já".to_string()
                        } else {
                            state.nickname(&p)
                        };
                        let p2 = p.clone();
                        view! {
                            <button class:selected=move || filter.get().as_deref() == Some(p.as_str())
                                on:click=move |_| filter.set(Some(p2.clone()))>{label}</button>
                        }
                    }
                </For>
                <button class:selected=move || filter.get().is_none() on:click=move |_| filter.set(None)>"Vše"</button>
            </div>
            <Show when=move || groups().is_empty()>
                <p class="muted">"Zatím žádné jízdy."</p>
            </Show>
            <For each=groups key=|(label, rides)| (label.clone(), rides.len(), rides.iter().map(|r| r.updated_at.timestamp()).sum::<i64>()) let:group>
                <h3 class="day">{group.0.clone()}</h3>
                <div class="list">
                    {group.1.into_iter().map(|ride| view! { <RideRow ride=ride /> }).collect_view()}
                </div>
            </For>
        </section>
    }
}
