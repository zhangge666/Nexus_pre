//! 本文件编排知识卡片 Memory、派生关联和初始 ReviewState 的一致创建流程。

use crate::{
    ContentFormat, CoreError, CreateCardInput, Embedder, IngestInput, Ingestor, LinkRelation,
    Memory, MemoryKind, MemorySource, MemoryStore, Result,
};

impl MemoryStore {
    /// 创建手动或 AI 生成的知识卡片，并建立可追溯来源与初始复习状态。
    pub fn create_card<E: Embedder + ?Sized>(
        &self,
        input: CreateCardInput,
        embedder: &E,
    ) -> Result<Memory> {
        let front = input.card_front.trim().to_owned();
        let back = input.card_back.trim().to_owned();
        if front.is_empty() || back.is_empty() {
            return Err(CoreError::InvalidInput("卡片正面和背面不能为空".into()));
        }
        if let Some(source_id) = input.source_memory_id
            && self.get(&source_id)?.is_none()
        {
            return Err(CoreError::NotFound(source_id));
        }
        let content = format!("## 正面\n{front}\n\n## 背面\n{back}");
        let memory = Ingestor::new(self, embedder).ingest(IngestInput {
            source: MemorySource::Orbit,
            kind: MemoryKind::Card,
            title: Some(front.chars().take(80).collect()),
            content,
            content_format: ContentFormat::Markdown,
            tags: input.tags,
            captured_at: None,
            device_id: "orbit-intelligence".into(),
            meta: serde_json::json!({
                "card_front": front,
                "card_back": back,
                "deck": input.deck,
                "provider": input.provider,
            }),
        })?;

        let result = (|| {
            if let Some(source_id) = input.source_memory_id {
                self.create_link(
                    memory.id,
                    source_id,
                    LinkRelation::DerivedFrom,
                    input.created_by,
                )?;
            }
            self.create_review_state(memory.id, front, back, input.deck)?;
            Ok(())
        })();
        if let Err(error) = result {
            // 三项写入暂不共享一个事务；失败时补偿删除卡片，避免留下半成品。
            let _ = self.delete(&memory.id);
            return Err(error);
        }
        Ok(memory)
    }
}
