//! Prints the PPM command list (`commands.json`) for `ppm/build-web.sh`:
//! `cargo run -p wordcraft-ui-egui --example commands_json > dist/web/commands.json`.

fn main() {
    match serde_json::to_string_pretty(&wordcraft_ui_egui::host_lock::commands_json()) {
        Ok(text) => println!("{text}"),
        Err(e) => {
            eprintln!("commands.json: {e}");
            std::process::exit(1);
        }
    }
}
