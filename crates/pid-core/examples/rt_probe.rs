//! Kontrola realtime feedu: `cargo run -p pid-core --example rt_probe -- vehicle_positions.pb`
use pid_core::realtime::FeedMessage;
use prost::Message;

fn main() {
    let path = std::env::args().nth(1).expect("cesta k .pb");
    let msg = FeedMessage::decode(std::fs::read(path).unwrap().as_slice()).unwrap();
    println!("entit: {}", msg.entity.len());
    let mut shown = std::collections::HashMap::new();
    for e in &msg.entity {
        let Some(vp) = &e.vehicle else { continue };
        let trip = vp.trip.as_ref().and_then(|t| t.trip_id.clone()).unwrap_or_default();
        let prefix = trip.split('_').next().unwrap_or("").to_string();
        let n = shown.entry(prefix.clone()).or_insert(0);
        if *n < 1 && (prefix.starts_with("13") || prefix.starts_with("1"))  && prefix.len() == 4 {
            *n += 1;
            let v = vp.vehicle.as_ref();
            println!("{trip:<28} label={:?} id={:?}", v.and_then(|v| v.label.clone()), v.and_then(|v| v.id.clone()));
        }
    }
}
