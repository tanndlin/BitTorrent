mod parse;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn parse_torrent(buffer: Vec<u8>) -> parse::Torrent {
    // Read the file contents into the buffer
    crate::parse::parse_metainfo(&buffer)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .invoke_handler(tauri::generate_handler![parse_torrent])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
