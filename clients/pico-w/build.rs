fn main() {
    for var in &["OUBOT_WIFI_SSID", "OUBOT_WIFI_PASS", "OUBOT_SERVER", "OUBOT_TOKEN"] {
        match std::env::var(var) {
            Err(_) => println!("cargo:warning={var} is not set — build will fail at env!() macro"),
            Ok(v) if v.is_empty() => println!("cargo:warning={var} is empty — build will fail at const assertion"),
            _ => {}
        }
        println!("cargo:rerun-if-env-changed={var}");
    }
}
