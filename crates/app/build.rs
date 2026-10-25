fn main() {
    for key in ["PIDTRACKER_BUILD_ID", "PIDTRACKER_BUILT_AT", "PIDTRACKER_GOOGLE_CLIENT_ID"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let build_id = std::env::var("PIDTRACKER_BUILD_ID").unwrap_or_else(|_| "dev".into());
    let built_at = std::env::var("PIDTRACKER_BUILT_AT").unwrap_or_else(|_| "dev".into());
    println!("cargo:rustc-env=PIDTRACKER_BUILD_ID={build_id}");
    println!("cargo:rustc-env=PIDTRACKER_BUILT_AT={built_at}");
}
