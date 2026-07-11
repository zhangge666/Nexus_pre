//! 本文件组织 Nexus 共享核心对外暴露的数据模型、存储、写入与检索能力。

pub mod embed;
pub mod error;
pub mod ingest;
pub mod model;
pub mod search;
pub mod store;

pub use embed::{Embedder, HashEmbedder};
pub use error::{CoreError, Result};
pub use ingest::Ingestor;
pub use model::{
    Block, ContentFormat, IngestInput, Memory, MemoryKind, MemorySource, SearchHit, SearchMode,
    SearchQuery,
};
pub use store::MemoryStore;

/// 返回当前核心骨架已经声明的领域模块。
#[must_use]
pub const fn modules() -> &'static [&'static str] {
    &[
        "model", "store", "ingest", "search", "embed", "crypto", "sync", "events",
    ]
}
