//! 本文件验证本地 Completion 回退、Provider 元数据与远程配置护栏。

use futures_util::StreamExt;
use nexus_ai::{
    Completion, CompletionContext, CompletionRequest, CompletionTask, CustomCompletion,
    LocalExtractiveCompletion, OllamaCompletion, OpenAiCompletion,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
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

/// 启动一次性回环 HTTP 响应服务，用于验证 Provider 对真实分片流的解析而不访问网络。
async fn stream_server(content_type: &str, chunks: &[&str]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("测试流服务应绑定回环端口");
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let content_type = content_type.to_owned();
    let chunks = chunks
        .iter()
        .map(|chunk| (*chunk).to_owned())
        .collect::<Vec<_>>();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("测试流服务应接收请求");
        // 先完整接收小型测试请求，避免服务端提前关闭连接导致客户端收到 TCP 重置。
        let mut request = [0_u8; 8_192];
        let _ = socket
            .read(&mut request)
            .await
            .expect("测试流服务应读取请求");
        let headers =
            format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n");
        socket
            .write_all(headers.as_bytes())
            .await
            .expect("测试流服务应写入响应头");
        for chunk in chunks {
            socket
                .write_all(chunk.as_bytes())
                .await
                .expect("测试流服务应写入响应分片");
        }
    });
    endpoint
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

/// 验证所有 Provider 都可通过统一流式 trait 安全回退，避免非流式本地模式阻塞 SSE 链路。
#[tokio::test]
async fn falls_back_to_one_increment_for_extractive_streaming() {
    let completion = LocalExtractiveCompletion;
    let deltas = completion
        .stream(request(CompletionTask::Answer))
        .collect::<Vec<_>>()
        .await;

    assert_eq!(deltas.len(), 1);
    let delta = deltas
        .into_iter()
        .next()
        .expect("应返回一个流式增量")
        .expect("流式回退应成功");
    assert_eq!(delta.provider, "local");
    assert!(delta.text.contains("[1]"));
}

/// 验证 OpenAI-compatible SSE 在网络分片到达时会逐段输出文本，而不是等待完整回答。
#[tokio::test]
async fn streams_openai_compatible_sse_deltas() {
    let endpoint = stream_server(
        "text/event-stream",
        &[
            "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n",
            "data: [DONE]\n\n",
        ],
    )
    .await;
    let completion =
        CustomCompletion::new(endpoint, "", "qwen").expect("回环测试端点应可配置为自定义 Provider");
    let deltas = completion
        .stream(request(CompletionTask::Answer))
        .collect::<Vec<_>>()
        .await;

    assert_eq!(
        deltas
            .iter()
            .map(|delta| delta.as_ref().unwrap().text.as_str())
            .collect::<Vec<_>>(),
        ["你", "好"]
    );
}

/// 验证 Ollama NDJSON 流能够逐行解析，结尾的完成标记不应产生额外文本。
#[tokio::test]
async fn streams_ollama_ndjson_deltas() {
    let endpoint = stream_server(
        "application/x-ndjson",
        &[
            "{\"message\":{\"content\":\"你\"},\"done\":false}\n",
            "{\"message\":{\"content\":\"好\"},\"done\":false}\n",
            "{\"message\":{\"content\":\"\"},\"done\":true}\n",
        ],
    )
    .await;
    let completion = OllamaCompletion::new("qwen3:8b", Some(endpoint))
        .expect("回环测试端点应可配置为 Ollama Provider");
    let deltas = completion
        .stream(request(CompletionTask::Answer))
        .collect::<Vec<_>>()
        .await;

    assert_eq!(
        deltas
            .iter()
            .map(|delta| delta.as_ref().unwrap().text.as_str())
            .collect::<Vec<_>>(),
        ["你", "好"]
    );
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
    let local = CustomCompletion::new("http://127.0.0.1:11434/v1", "", "qwen")
        .expect("回环自定义 Provider 应允许 HTTP");
    assert_eq!(local.provider_name(), "custom");
    assert!(!local.sends_data_remote());
    assert!(CustomCompletion::new("http://127.0.0.1.example.com/v1", "", "qwen").is_err());
    assert!(OllamaCompletion::new("qwen3:8b", None).is_ok());
    assert!(OllamaCompletion::new("qwen3:8b", Some("https://example.com".into())).is_err());
    assert!(
        OllamaCompletion::new("qwen3:8b", Some("http://localhost.example.com".into())).is_err()
    );
}
