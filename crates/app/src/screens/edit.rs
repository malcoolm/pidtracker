//! Detail jízdy: úprava (jen vlastní jízdy), ukončení, smazání.

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone};
use leptos::prelude::*;
use pid_core::ride::{Ride, RideStatus, RouteKind, on_vehicle_changed};
use pid_core::time::TZ;
use pid_core::timetable::query::DepartureSource;

use crate::components::{RouteBadge, ride_color};
use crate::state::{AppState, Screen};
use crate::stop_input::StopInput;
use crate::util::{duration_label, local, now_fixed};

const INPUT_FMT: &str = "%Y-%m-%dT%H:%M";

fn to_input(t: &DateTime<FixedOffset>) -> String {
    local(t).format(INPUT_FMT).to_string()
}

fn from_input(s: &str) -> Option<DateTime<FixedOffset>> {
    let naive = NaiveDateTime::parse_from_str(s, INPUT_FMT).ok()?;
    TZ.from_local_datetime(&naive).earliest().map(|t| t.fixed_offset())
}

#[component]
pub fn Edit(id: String) -> impl IntoView {
    let state = expect_context::<AppState>();
    let Some(ride) = state.ride(&id) else {
        return view! { <p>"Jízda nenalezena."</p> }.into_any();
    };
    let editable = state.me.get_untracked().is_some_and(|me| ride.can_edit(&me));
    if !editable {
        return view! { <ReadOnly ride=ride /> }.into_any();
    }

    let board_stop = RwSignal::new(ride.board_stop_name.clone());
    let alight_stop = RwSignal::new(ride.alight_stop_name.clone().unwrap_or_default());
    let line = RwSignal::new(ride.route_short_name.clone());
    let vehicle = RwSignal::new(ride.vehicle.clone());
    let plate = RwSignal::new(ride.license_plate.clone());
    let operator = RwSignal::new(ride.operator.clone());
    let kind = RwSignal::new(ride.route_type);
    let board_time = RwSignal::new(to_input(&ride.board_time));
    let alight_time = RwSignal::new(ride.alight_time.as_ref().map(to_input).unwrap_or_default());
    let error = RwSignal::new(None::<String>);
    let confirm_delete = RwSignal::new(false);
    let board_sel = RwSignal::new(None::<u32>);
    let alight_sel = RwSignal::new(None::<u32>);
    let board_pos = ride.board_lat.zip(ride.board_lon);
    let alight_pos = ride.alight_lat.zip(ride.alight_lon);
    let board_near = Signal::derive(move || board_pos.or_else(|| state.last_pos()));
    let alight_near = Signal::derive(move || alight_pos.or(board_pos).or_else(|| state.last_pos()));
    let route_suggestions = move || {
        let q = line.get();
        state.tt.get().index().map(|ix| ix.search_routes(&q, 6)).unwrap_or_default()
    };

    let original = ride.clone();
    let save = move |_| {
        let mut r = original.clone();
        let Some(bt) = from_input(&board_time.get_untracked()) else {
            error.set(Some("Neplatný čas nástupu.".into()));
            return;
        };
        let at_text = alight_time.get_untracked();
        let at = if at_text.trim().is_empty() {
            None
        } else {
            match from_input(&at_text) {
                Some(t) => Some(t),
                None => {
                    error.set(Some("Neplatný čas výstupu.".into()));
                    return;
                }
            }
        };
        if at.is_some_and(|a| a <= bt) {
            error.set(Some("Výstup musí být po nástupu.".into()));
            return;
        }
        let ix = state.tt.get_untracked().index();
        let stop_id_for = |name: &str| -> String {
            ix.as_ref()
                .and_then(|ix| {
                    ix.search_groups(name, 5)
                        .into_iter()
                        .find(|g| g.name == name)
                        .map(|g| ix.tt.stops[ix.group_first_stop(g.group) as usize].gtfs_id.clone())
                })
                .unwrap_or_default()
        };
        let group_stop = |g: u32| {
            ix.as_ref().map(|ix| (ix.tt.stops[ix.group_first_stop(g) as usize].gtfs_id.clone(), ix.tt.group_name(g).to_string()))
        };
        // Vybraná zastávka z návrhů, jinak ruční text (stop_id jen při přesné shodě názvu).
        let bs = board_stop.get_untracked().trim().to_string();
        if let Some((id, name)) = board_sel.get_untracked().and_then(group_stop) {
            r.board_stop_id = id;
            r.board_stop_name = name;
        } else if bs != r.board_stop_name {
            r.board_stop_id = stop_id_for(&bs);
            r.board_stop_name = bs;
        }
        let als = alight_stop.get_untracked().trim().to_string();
        if let Some((id, name)) = alight_sel.get_untracked().and_then(group_stop) {
            r.alight_stop_id = Some(id);
            r.alight_stop_name = Some(name);
        } else if Some(&als) != r.alight_stop_name.as_ref() {
            r.alight_stop_id = (!als.is_empty()).then(|| stop_id_for(&als));
            r.alight_stop_name = (!als.is_empty()).then_some(als);
        }
        let ln = line.get_untracked().trim().to_string();
        if ln != r.route_short_name {
            let found = ix.as_ref().and_then(|ix| ix.search_routes(&ln, 5).into_iter().find(|x| x.1 == ln));
            match found {
                Some((rid, _, rtype)) => {
                    r.route_id = rid;
                    r.route_type = Some(rtype);
                }
                None => {
                    r.route_id = String::new();
                    r.route_type = kind.get_untracked();
                }
            }
            r.route_short_name = ln;
            r.trip_id = String::new();
            r.pattern_id = None;
            r.headsign = String::new();
        } else if r.route_id.is_empty() {
            r.route_type = kind.get_untracked();
        }
        r.board_time = bt;
        let new_vehicle = vehicle.get_untracked().trim().to_string();
        let new_plate = plate.get_untracked().trim().to_string();
        let vehicle_changed = new_vehicle != r.vehicle;
        let plate_touched = new_plate != r.license_plate;
        r.vehicle = new_vehicle;
        r.license_plate = new_plate;
        // Jiný vůz → SPZ podle nového vozu, pokud ji uživatel v téže úpravě sám nepřepsal.
        if vehicle_changed && !plate_touched {
            state.rides.with_untracked(|all| on_vehicle_changed(&mut r, all));
        }
        r.operator = operator.get_untracked().trim().to_string();
        r.alight_time = at;
        if at.is_some() {
            r.status = RideStatus::Done;
            if state.active_ride.get_untracked().as_deref() == Some(r.id.as_str()) {
                state.set_active_ride(None);
            }
        }
        r.updated_at = now_fixed();
        state.save_ride(r);
        state.show_toast("Uloženo");
        state.screen.set(Screen::History);
    };

    let id_del = id.clone();
    let delete = move |_| {
        if !confirm_delete.get() {
            confirm_delete.set(true);
            return;
        }
        if let Some(mut r) = state.ride(&id_del) {
            r.deleted = true;
            r.updated_at = now_fixed();
            if state.active_ride.get_untracked().as_deref() == Some(r.id.as_str()) {
                state.set_active_ride(None);
            }
            state.save_ride(r);
            state.show_toast("Jízda smazána");
        }
        state.screen.set(Screen::History);
    };

    let in_progress = ride.status == RideStatus::InProgress;
    view! {
        <section class="edit">
            <h2>"Úprava jízdy"</h2>
            <datalist id="line-suggestions">
                <For each=route_suggestions key=|r| r.0.clone() let:r>
                    <option value=r.1.clone()>{RouteKind::from_gtfs(r.2).label_cs()}</option>
                </For>
            </datalist>
            <label>"Linka"
                <input type="text" list="line-suggestions" prop:value=move || line.get()
                    on:input=move |ev| line.set(event_target_value(&ev)) />
            </label>
            <Show when=move || route_suggestions().iter().all(|r| r.1 != line.get().trim())>
                <label>"Typ dopravy"
                    <select on:change=move |ev| kind.set(event_target_value(&ev).parse().ok())>
                        <option value="" selected=move || kind.get().is_none()>"—"</option>
                        {RouteKind::ALL.into_iter().filter_map(|k| k.to_gtfs().map(|t| (k, t))).map(|(k, t)| view! {
                            <option value=t.to_string() selected=move || kind.get() == Some(t)>{k.label_cs()}</option>
                        }).collect_view()}
                    </select>
                </label>
            </Show>
            <label>"Číslo vozu"
                <input type="text" inputmode="numeric" prop:value=move || vehicle.get()
                    on:input=move |ev| vehicle.set(event_target_value(&ev)) />
            </label>
            <label>"SPZ"
                <input type="text" autocapitalize="characters" placeholder="např. 1AB 2345" prop:value=move || plate.get()
                    on:input=move |ev| plate.set(event_target_value(&ev)) />
            </label>
            <label>"Dopravce"
                <input type="text" prop:value=move || operator.get()
                    on:input=move |ev| operator.set(event_target_value(&ev)) />
            </label>
            <label>"Nástup"</label>
            <StopInput text=board_stop selected=board_sel near=board_near placeholder="Nástupní zastávka" />
            <label>"Čas nástupu"
                <input type="datetime-local" prop:value=move || board_time.get()
                    on:input=move |ev| board_time.set(event_target_value(&ev)) />
            </label>
            <label>"Výstup"</label>
            <StopInput text=alight_stop selected=alight_sel near=alight_near placeholder="Výstupní zastávka" />
            <label>"Čas výstupu"
                <input type="datetime-local" prop:value=move || alight_time.get()
                    on:input=move |ev| alight_time.set(event_target_value(&ev)) />
            </label>
            <Show when=move || in_progress>
                <button class="secondary" on:click=move |_| alight_time.set(to_input(&now_fixed()))>"Ukončit teď"</button>
            </Show>
            {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
            <div class="actions">
                <button class="primary big" on:click=save>"Uložit"</button>
                <button class="link" on:click=move |_| state.screen.set(Screen::History)>"Zpět"</button>
                <button class="link danger" on:click=delete>
                    {move || if confirm_delete.get() { "Opravdu smazat? Klepněte znovu" } else { "Smazat jízdu" }}
                </button>
            </div>
        </section>
    }
    .into_any()
}

#[component]
fn ReadOnly(ride: Ride) -> impl IntoView {
    let state = expect_context::<AppState>();
    let color = ride_color(&state, &ride);
    let fmt = |t: &DateTime<FixedOffset>| local(t).format("%-d. %-m. %Y %H:%M").to_string();
    view! {
        <section class="edit">
            <h2>"Jízda – " {state.nickname(&ride.person)}</h2>
            <p><RouteBadge name=ride.route_short_name.clone() color=color /> " " {ride.headsign.clone()}</p>
            <p>"Nástup: " <strong>{ride.board_stop_name.clone()}</strong> ", " {fmt(&ride.board_time)}</p>
            <p>"Výstup: " <strong>{ride.alight_stop_name.clone().unwrap_or_else(|| "—".into())}</strong>
                {ride.alight_time.as_ref().map(|t| format!(", {}", fmt(t)))}</p>
            {ride.duration_minutes().filter(|m| *m >= 0).map(|m| view! { <p>"Doba: " {duration_label(m)}</p> })}
            {(!ride.vehicle.is_empty()).then(|| view! { <p>"Vůz: " {ride.vehicle.clone()}</p> })}
            {(!ride.license_plate.is_empty()).then(|| view! { <p>"SPZ: " {ride.license_plate.clone()}</p> })}
            {(!ride.operator.is_empty()).then(|| view! { <p>"Dopravce: " {ride.operator.clone()}</p> })}
            <p class="muted">"Jízdy jiných osob nelze upravovat."</p>
            <button class="link" on:click=move |_| state.screen.set(Screen::History)>"Zpět"</button>
        </section>
    }
}
