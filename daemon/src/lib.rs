pub mod audio;
pub mod config;
pub mod context_store;
pub mod dbus_server;
pub mod llm;
pub mod ocr;
pub mod profile;
pub mod prompt_assembler;
pub mod rvms;

use std::sync::OnceLock;

pub fn get_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Failed to build HTTP client")
    })
}
