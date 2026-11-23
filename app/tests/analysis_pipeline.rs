// Allows reviewing the independent backend before the UI declares these modules.
// Remove this harness when main integrates them; their unit tests then run there.
#[path = "../src/ai_consent.rs"]
mod ai_consent;
#[path = "../src/analysis_backend.rs"]
mod analysis_backend;
#[path = "../src/analysis_text.rs"]
mod analysis_text;
#[path = "../src/reader_epub.rs"]
mod reader_epub;
