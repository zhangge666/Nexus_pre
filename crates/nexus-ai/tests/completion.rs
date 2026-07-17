//! 本文件验证本地 Completion 回退、Provider 元数据与远程配置护栏。

use nexus_ai::{
    Completion, CompletionContext, CompletionRequest, CompletionTask, CustomCompletion,
    LocalExtractiveCompletion, OpenAiCompletion,
};

/// 构造本地 Completion 测试请求。
fn request(task: CompletionTask) -> CompletionRequest {
    CompletionRequest {
        task,
        system: "只基于上下文回答".into(),
        prompt: "FSRS 的两个参数是什么？".into(),
        context: vec![CompletionContext {
            label: "[1]".into(),
            title: "FSRS 笔记".into(),
            text: "FSRS 使用 stability 和 difficulty 刻画记忆。".into(),
        }],
        max_tokens: 300,
        temperature: 0.1,
    }
}

/// 验证本地问答只重组给定片段并保留引用标签。
#[tokio::test]
async fn answers_from_minimized_local_context() {
    let completion = LocalExtractiveCompletion;
    let response = completion
        .complete(request(CompletionTask::Answer))
        .await
        .expect("本地问答应成功");

    assert_eq!(response.provider, "local");
    assert!(response.text.contains("stability"));
    assert!(response.text.contains("[1]"));
    assert!(!completion.sends_data_remote());
}

/// 验证本地卡片以稳定 JSON 结构返回正反面候选。
#[tokio::test]
async fn generates_local_card_json() {
    let completion = LocalExtractiveCompletion;
    let response = completion
        .complete(request(CompletionTask::Card))
        .await
        .expect("本地卡片生成应成功");
    let cards: serde_json::Value = serde_json::from_str(&response.text).expect("卡片应为 JSON");

    assert_eq!(cards[0]["card_front"], "「FSRS 笔记」的核心要点是什么？");
    assert!(
        cards[0]["card_back"]
            .as_str()
            .unwrap()
            .contains("difficulty")
    );
}

/// 验证云端 Provider 拒绝空 Key 和非安全远程地址，但允许回环自定义模型。
#[test]
fn validates_remote_provider_configuration() {
    assert!(OpenAiCompletion::new("", "gpt-4.1-mini", None).is_err());
    assert!(CustomCompletion::new("http://example.com/v1", "secret", "model").is_err());
    let local = CustomCompletion::new("http://127.0.0.1:11434/v1", "local", "qwen")
        .expect("回环自定义 Provider 应允许 HTTP");
    assert_eq!(local.provider_name(), "custom");
    assert!(local.sends_data_remote());
}
