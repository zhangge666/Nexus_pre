//! 本文件实现 FTS5 关键词检索、向量余弦检索及 RRF 融合排序。

use std::collections::{HashMap, HashSet};

use rusqlite::params;
use uuid::Uuid;

use crate::{
    CoreError, Embedder, MemoryStore, Result, SearchHit, SearchMode, SearchQuery, store::parse_uuid,
};

impl MemoryStore {
    /// 根据请求模式执行关键词、语义或混合检索。
    pub fn search<E: Embedder + ?Sized>(
        &self,
        query: &SearchQuery,
        embedder: &E,
    ) -> Result<Vec<SearchHit>> {
        self.ensure_embedding_profile(embedder)?;
        if query.text.trim().is_empty() {
            return Err(CoreError::InvalidInput("检索文本不能为空".into()));
        }
        if query.limit == 0 {
            return Ok(Vec::new());
        }

        let candidate_limit = query.limit.saturating_mul(8).clamp(40, 200);
        let keyword = match query.mode {
            SearchMode::Semantic => Vec::new(),
            SearchMode::Keyword | SearchMode::Hybrid => {
                self.keyword_search(&query.text, candidate_limit)?
            }
        };
        let semantic = match query.mode {
            SearchMode::Keyword => Vec::new(),
            SearchMode::Semantic | SearchMode::Hybrid => {
                self.semantic_search(&query.text, candidate_limit, embedder)?
            }
        };

        let hits = match query.mode {
            SearchMode::Keyword => keyword,
            SearchMode::Semantic => semantic,
            SearchMode::Hybrid => reciprocal_rank_fusion(&keyword, &semantic),
        };
        let mut filtered = Vec::with_capacity(hits.len());
        for hit in hits {
            if self.matches_filters(&hit.memory_id, &query.filters)? {
                filtered.push(hit);
            }
            if filtered.len() == query.limit {
                break;
            }
        }
        Ok(filtered)
    }

    /// 使用 FTS5 BM25 取得关键词候选，并转换为越大越优的分数。
    fn keyword_search(&self, text: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let fts_query = text
            .split_whitespace()
            .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" OR ");
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT memory_id, block_id, text, bm25(blocks_fts) FROM blocks_fts WHERE blocks_fts MATCH ?1 ORDER BY bm25(blocks_fts) LIMIT ?2",
        )?;
        let rows = statement.query_map(params![fts_query, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f32>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (memory_id, block_id, snippet, rank) = row?;
            Ok(SearchHit {
                memory_id: parse_uuid(&memory_id)?,
                block_id: parse_uuid(&block_id)?,
                score: 1.0 / (1.0 + rank.abs()),
                snippet,
            })
        })
        .collect()
    }

    /// 读取同库向量并使用余弦相似度得到语义候选。
    fn semantic_search<E: Embedder + ?Sized>(
        &self,
        text: &str,
        limit: usize,
        embedder: &E,
    ) -> Result<Vec<SearchHit>> {
        let query_vector = embedder.embed(text)?;
        let encoded_query = serde_json::to_string(&query_vector)?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT b.memory_id, v.block_id, v.distance, b.text FROM block_vectors_vec v JOIN blocks b ON b.id = v.block_id WHERE v.embedding MATCH ?1 AND k = ?2 ORDER BY v.distance",
        )?;
        let rows = statement.query_map(params![encoded_query, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f32>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let hits = rows
            .map(|row| {
                let (memory_id, block_id, distance, snippet) = row?;
                Ok(SearchHit {
                    memory_id: parse_uuid(&memory_id)?,
                    block_id: parse_uuid(&block_id)?,
                    score: 1.0 / (1.0 + distance),
                    snippet,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(hits)
    }
}

/// 使用常量 60 的 RRF 合并两路块级结果，并优先保留关键词命中块。
fn reciprocal_rank_fusion(keyword: &[SearchHit], semantic: &[SearchHit]) -> Vec<SearchHit> {
    let mut fused: HashMap<Uuid, SearchHit> = HashMap::new();
    for route in [keyword, semantic] {
        let mut seen_memories = HashSet::new();
        for (rank, hit) in route.iter().enumerate() {
            // 同一路径只让一条 Memory 贡献一次，避免块数较多的长文获得不合理加权。
            if !seen_memories.insert(hit.memory_id) {
                continue;
            }
            let contribution = 1.0 / (60.0 + rank as f32 + 1.0);
            fused
                .entry(hit.memory_id)
                .and_modify(|entry| entry.score += contribution)
                .or_insert_with(|| SearchHit {
                    score: contribution,
                    ..hit.clone()
                });
        }
    }
    let mut hits = fused.into_values().collect::<Vec<_>>();
    hits.sort_by(|left, right| right.score.total_cmp(&left.score));
    hits
}
