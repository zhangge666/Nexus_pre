//! 本文件定义可替换的文本嵌入接口和用于最小闭环的确定性哈希嵌入器。

use rusqlite::params;

use crate::{CoreError, MemoryStore, Result};
use nexus_ai::{Embedder, EmbeddingError};

/// 使用字符三元组生成确定性向量，供 ONNX 模型接入前验证完整数据链路。
#[derive(Debug, Clone)]
pub struct HashEmbedder {
    dimension: usize,
}

impl HashEmbedder {
    /// 创建指定维度的哈希嵌入器，维度至少为 8。
    #[must_use]
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension: dimension.max(8),
        }
    }
}

impl Default for HashEmbedder {
    /// 创建与 BGE-small 向量空间维度一致的 384 维回退嵌入器。
    fn default() -> Self {
        Self::new(384)
    }
}

impl Embedder for HashEmbedder {
    /// 返回哈希向量空间维度。
    fn dimension(&self) -> usize {
        self.dimension
    }

    /// 返回 M0 离线回退向量空间标识。
    fn model_id(&self) -> &str {
        "hash-384-m0"
    }

    /// 将规范化文本的字符窗口散列到固定维度并执行 L2 归一化。
    fn embed(&self, text: &str) -> std::result::Result<Vec<f32>, EmbeddingError> {
        let normalized = text.to_lowercase();
        let chars = normalized.chars().collect::<Vec<_>>();
        let mut vector = vec![0.0_f32; self.dimension];

        // 同时记录单字符和三字符窗口，让中文短查询与英文词组都能保留局部语义信号。
        for window_size in [1, 3] {
            for window in chars.windows(window_size) {
                let mut hash = 2_166_136_261_u64;
                for character in window {
                    hash ^= u64::from(*character as u32);
                    hash = hash.wrapping_mul(16_777_619);
                }
                vector[hash as usize % self.dimension] += 1.0;
            }
        }

        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm > 0.0 {
            vector.iter_mut().for_each(|value| *value /= norm);
        }
        Ok(vector)
    }
}

/// 表示数据库当前记录的嵌入空间。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingProfile {
    /// 模型稳定标识。
    pub model: String,
    /// 向量维度。
    pub dimensions: usize,
    /// 配置版本。
    pub version: u32,
}

impl MemoryStore {
    /// 返回数据库当前嵌入空间配置。
    pub fn embedding_profile(&self) -> Result<EmbeddingProfile> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT model, dimensions, version FROM embedding_config WHERE singleton=1",
                [],
                |row| {
                    Ok(EmbeddingProfile {
                        model: row.get(0)?,
                        dimensions: row.get(1)?,
                        version: row.get(2)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    /// 验证 Provider 与数据库向量空间一致。
    pub fn ensure_embedding_profile<E: Embedder + ?Sized>(&self, embedder: &E) -> Result<()> {
        let profile = self.embedding_profile()?;
        if profile.model != embedder.model_id() || profile.dimensions != embedder.dimension() {
            return Err(CoreError::EmbeddingSpaceMismatch {
                stored_model: profile.model,
                stored_dimensions: profile.dimensions,
                requested_model: embedder.model_id().into(),
                requested_dimensions: embedder.dimension(),
            });
        }
        Ok(())
    }

    /// 使用新 Provider 重新生成全部块向量，并原子切换嵌入空间版本。
    pub fn reembed_all<E: Embedder + ?Sized>(&self, embedder: &E) -> Result<usize> {
        if embedder.dimension() != 384 {
            return Err(CoreError::InvalidInput(
                "当前 sqlite-vec 索引要求 384 维向量".into(),
            ));
        }
        let blocks = {
            let connection = self.connection()?;
            let mut statement =
                connection.prepare("SELECT id, memory_id, text FROM blocks ORDER BY id")?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let vectors = blocks
            .iter()
            .map(|(_, _, text)| embedder.embed(text))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM block_vectors_vec", [])?;
        transaction.execute("DELETE FROM block_vectors", [])?;
        for ((block_id, memory_id, _), vector) in blocks.iter().zip(&vectors) {
            let encoded = serde_json::to_string(vector)?;
            transaction.execute(
                "INSERT INTO block_vectors (block_id, memory_id, embedding) VALUES (?1, ?2, ?3)",
                params![block_id, memory_id, encoded],
            )?;
            transaction.execute(
                "INSERT INTO block_vectors_vec (block_id, embedding) VALUES (?1, ?2)",
                params![block_id, serde_json::to_string(vector)?],
            )?;
        }
        transaction.execute(
            "UPDATE embedding_config SET model=?1, dimensions=?2, version=version+1 WHERE singleton=1",
            params![embedder.model_id(), embedder.dimension()],
        )?;
        transaction.commit()?;
        Ok(blocks.len())
    }
}

/// 计算两个单位向量的余弦相似度，不同维度只比较公共部分。
#[must_use]
pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}
