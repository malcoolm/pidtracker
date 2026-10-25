//! Slučování lokálních jízd s tabulkou a plán zápisu.

use std::collections::{HashMap, HashSet};

use crate::ride::Ride;
use crate::sheet::{LAST_COL, RIDES_SHEET};

#[derive(Debug, Default, PartialEq)]
pub struct MergeReport {
    pub inserted: u32,
    pub updated: u32,
}

/// Sloučí jízdy z tabulky do lokálních. Jízdy čekající na odeslání (`pending`) se nepřepisují,
/// jen převezmou smazání. Smazání je trvalé; jinak vyhrává novější `updated_at`.
pub fn merge_remote(
    local: &mut HashMap<String, Ride>,
    remote: &[(u32, Ride)],
    pending: &HashSet<String>,
) -> MergeReport {
    let mut newest: HashMap<&str, &Ride> = HashMap::new();
    for (_, r) in remote {
        newest
            .entry(r.id.as_str())
            .and_modify(|cur| {
                if r.updated_at > cur.updated_at {
                    *cur = r;
                }
            })
            .or_insert(r);
    }

    let mut report = MergeReport::default();
    for (id, remote) in newest {
        match local.get_mut(id) {
            None => {
                local.insert(id.to_string(), remote.clone());
                report.inserted += 1;
            }
            Some(existing) if pending.contains(id) => {
                if remote.deleted && !existing.deleted {
                    existing.deleted = true;
                    report.updated += 1;
                }
            }
            Some(existing) => {
                let was_deleted = existing.deleted;
                if remote.updated_at > existing.updated_at {
                    *existing = remote.clone();
                    existing.deleted |= was_deleted;
                    report.updated += 1;
                } else if remote.deleted && !existing.deleted {
                    existing.deleted = true;
                    report.updated += 1;
                }
            }
        }
    }
    report
}

#[derive(Debug, Clone, PartialEq)]
pub enum SheetWrite {
    Update { row: u32, ride: Ride },
    Append { ride: Ride },
}

/// Pro každou jízdu k odeslání: přepsat její řádek (podle čerstvě načtené tabulky), nebo připojit.
/// `row_ids` = id → řádek z čerstvě načtené tabulky (`ParsedRows::row_ids`, včetně nečitelných řádků).
pub fn plan_push(row_ids: &HashMap<String, u32>, to_push: &[Ride]) -> Vec<SheetWrite> {
    to_push
        .iter()
        .map(|ride| match row_ids.get(&ride.id) {
            Some(&row) => SheetWrite::Update { row, ride: ride.clone() },
            None => SheetWrite::Append { ride: ride.clone() },
        })
        .collect()
}

pub fn update_range(row: u32) -> String {
    format!("{RIDES_SHEET}!A{row}:{LAST_COL}{row}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ride::tests::sample_ride;
    use chrono::Duration;

    fn ride(id: &str) -> Ride {
        let mut r = sample_ride();
        r.id = id.into();
        r
    }

    fn newer(mut r: Ride, minutes: i64) -> Ride {
        r.updated_at += Duration::minutes(minutes);
        r
    }

    fn local_of(rides: &[Ride]) -> HashMap<String, Ride> {
        rides.iter().map(|r| (r.id.clone(), r.clone())).collect()
    }

    #[test]
    fn inserts_unknown_remote() {
        let mut local = HashMap::new();
        let rep = merge_remote(&mut local, &[(2, ride("a"))], &HashSet::new());
        assert_eq!(rep.inserted, 1);
        assert!(local.contains_key("a"));
    }

    #[test]
    fn newer_remote_replaces_when_not_pending() {
        let mut local = local_of(&[ride("a")]);
        let mut remote = newer(ride("a"), 5);
        remote.route_short_name = "9".into();
        let rep = merge_remote(&mut local, &[(2, remote)], &HashSet::new());
        assert_eq!(rep.updated, 1);
        assert_eq!(local["a"].route_short_name, "9");
    }

    #[test]
    fn older_remote_is_ignored() {
        let mut local = local_of(&[newer(ride("a"), 5)]);
        let rep = merge_remote(&mut local, &[(2, ride("a"))], &HashSet::new());
        assert_eq!(rep.updated, 0);
        assert_eq!(local["a"].updated_at, newer(ride("a"), 5).updated_at);
    }

    #[test]
    fn pending_local_is_kept() {
        let mut local = local_of(&[ride("a")]);
        let mut remote = newer(ride("a"), 5);
        remote.route_short_name = "9".into();
        let pending = HashSet::from(["a".to_string()]);
        merge_remote(&mut local, &[(2, remote)], &pending);
        assert_eq!(local["a"].route_short_name, "22");
    }

    #[test]
    fn delete_is_sticky() {
        let mut deleted = ride("a");
        deleted.deleted = true;
        let mut local = local_of(&[deleted]);
        merge_remote(&mut local, &[(2, newer(ride("a"), 5))], &HashSet::new());
        assert!(local["a"].deleted);

        let mut local = local_of(&[ride("b")]);
        let mut remote = ride("b");
        remote.deleted = true;
        let pending = HashSet::from(["b".to_string()]);
        merge_remote(&mut local, &[(2, remote)], &pending);
        assert!(local["b"].deleted);
    }

    #[test]
    fn duplicate_remote_rows_newest_wins() {
        let mut a2 = newer(ride("a"), 5);
        a2.route_short_name = "9".into();
        let mut local = HashMap::new();
        merge_remote(&mut local, &[(2, a2), (3, ride("a"))], &HashSet::new());
        assert_eq!(local["a"].route_short_name, "9");
    }

    fn ids(pairs: &[(&str, u32)]) -> HashMap<String, u32> {
        pairs.iter().map(|(id, row)| (id.to_string(), *row)).collect()
    }

    #[test]
    fn push_updates_existing_and_appends_new() {
        let w = plan_push(&ids(&[("a", 7)]), &[ride("a"), ride("b")]);
        assert_eq!(w.len(), 2);
        assert!(matches!(&w[0], SheetWrite::Update { row: 7, .. }));
        assert!(matches!(&w[1], SheetWrite::Append { ride } if ride.id == "b"));
    }

    #[test]
    fn push_uses_fresh_row_numbers() {
        let w = plan_push(&ids(&[("x", 3), ("a", 12)]), &[ride("a")]);
        assert!(matches!(&w[0], SheetWrite::Update { row: 12, .. }));
    }

    #[test]
    fn push_overwrites_unparseable_row_with_same_id() {
        let w = plan_push(&ids(&[("zz", 3)]), &[ride("zz")]);
        assert!(matches!(&w[0], SheetWrite::Update { row: 3, .. }));
    }

    #[test]
    fn update_range_format() {
        assert_eq!(update_range(5), "rides!A5:X5");
    }

}
