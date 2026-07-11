//! 本文件组织 Nexus 共享核心对外暴露的数据模型、存储、写入与检索能力。

pub mod crypto;
pub mod embed;
pub mod error;
pub mod events;
pub mod ingest;
pub mod media;
pub mod model;
mod repository;
pub mod search;
pub mod store;

pub use crypto::{CryptoError, EncryptedMediaRef, MasterKey, MediaVault};
pub use embed::{Embedder, HashEmbedder};
pub use error::{CoreError, Result};
pub use events::{CoreEvent, EventSubscription};
pub use ingest::Ingestor;
pub use media::MediaService;
pub use model::{
    Block, ContentFormat, IngestInput, ListQuery, MediaKind, MediaMetadata, MediaRecord, Memory,
    MemoryFilters, MemoryKind, MemoryPage, MemoryPatch, MemorySource, SearchHit, SearchMode,
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
