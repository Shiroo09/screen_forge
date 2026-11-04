//! Network and encoding logic from `ScreenForge/Translate`, `ScreenForge/Search`
//! and `ScreenForge/Upload` — the parts that are pure functions, so they can be
//! pinned against the C# behaviour without a live server.

pub mod http_upload;
pub mod lang_router;
pub mod lens;
pub mod proto;
pub mod reverse_http;
pub mod reverse_search;
pub mod translate;
pub mod upload;

pub use proto::ProtoWriter;
