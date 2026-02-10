use std::sync::OnceLock;

/// Embedded error mascot image (roxy_error_small.png)
const ROXY_ERROR_IMAGE: &[u8] = include_bytes!("../../assets/roxy_error_small.png");

/// Embedded 404 mascot image (roxy_404_small.png)
const ROXY_404_IMAGE: &[u8] = include_bytes!("../../assets/roxy_404_small.png");

static ROXY_ERROR_DATA_URI: OnceLock<String> = OnceLock::new();
static ROXY_404_DATA_URI: OnceLock<String> = OnceLock::new();

fn png_data_uri(bytes: &[u8]) -> String {
    use base64::{Engine as _, engine::general_purpose};
    let b64 = general_purpose::STANDARD.encode(bytes);
    format!("data:image/png;base64,{b64}")
}

pub fn roxy_error_data_uri() -> &'static str {
    ROXY_ERROR_DATA_URI
        .get_or_init(|| png_data_uri(ROXY_ERROR_IMAGE))
        .as_str()
}

pub fn roxy_404_data_uri() -> &'static str {
    ROXY_404_DATA_URI
        .get_or_init(|| png_data_uri(ROXY_404_IMAGE))
        .as_str()
}
