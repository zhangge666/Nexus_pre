//! 本文件实现 Muse 多窗口快捷唤起，以及可选 Orbit 服务的发现、授权登记与灵感同步命令。

use std::{path::PathBuf, sync::Mutex};

use nexus_protocol::{ServiceDiscovery, discover_local_service, shared_nexus_data_dir};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::{Manager, State};

#[cfg(desktop)]
use tauri::WindowEvent;
#[cfg(desktop)]
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, ShortcutState};

#[cfg(desktop)]
const TOOL_WINDOW_LABELS: [&str; 4] = ["idea", "task", "meeting", "clipboard"];

/// 保存 Muse 当前进程获授的可选 Orbit capability token。
#[derive(Clone)]
struct MuseConnection {
    endpoint: String,
    token: String,
}

/// 保存协议客户端、共享发现目录和当前连接。
struct MuseState {
    client: reqwest::Client,
    data_dir: PathBuf,
    connection: Mutex<Option<MuseConnection>>,
}

/// 表示前端展示的服务连接状态和最小诊断信息。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionStatus {
    state: &'static str,
    endpoint: Option<String>,
    message: Option<String>,
}

/// 表示 Muse 登记成功后服务端签发的最小授权。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistrationResponse {
    token: String,
    source: String,
    scopes: Vec<String>,
}

/// 表示记忆创建成功后的协议响应。
#[derive(Debug, Serialize, Deserialize)]
struct CreatedMemory {
    id: String,
    created_at: i64,
}

impl MuseState {
    /// 发现 Orbit 持有的本地服务并登记 `source=muse` 的单一写入授权。
    async fn connect(&self) -> Result<ConnectionStatus, String> {
        let discovery = discover_local_service(&self.data_dir)
            .await
            .map_err(|error| format!("未发现 Orbit 本地服务：{error}"))?;
        self.register(&discovery).await
    }

    /// 使用发现记录中的本地登记凭据换取来源受限令牌。
    async fn register(&self, discovery: &ServiceDiscovery) -> Result<ConnectionStatus, String> {
        let response = self
            .client
            .post(format!("{}/v1/connections", discovery.endpoint))
            .bearer_auth(&discovery.token)
            .json(&serde_json::json!({
                "app_id": "com.nexus.muse",
                "name": "Muse",
                "source": "muse",
                "scopes": ["memory:write"]
            }))
            .send()
            .await
            .map_err(|error| format!("无法连接 Orbit 本地服务：{error}"))?;
        let registration: RegistrationResponse = parse_response(response).await?;
        // 即使服务端契约意外扩权，Muse 也拒绝接收超出当前同步边界的授权。
        if registration.source != "muse" || registration.scopes != ["memory:write"] {
            return Err("Orbit 返回的 Muse 授权范围不符合当前最小同步约束".into());
        }
        *self
            .connection
            .lock()
            .map_err(|_| "Muse 连接状态不可用".to_owned())? = Some(MuseConnection {
            endpoint: discovery.endpoint.clone(),
            token: registration.token,
        });
        Ok(self.status())
    }

    /// 返回当前进程保存的连接状态，不把敏感令牌暴露给 WebView。
    fn status(&self) -> ConnectionStatus {
        match self.connection.lock() {
            Ok(connection) => match connection.as_ref() {
                Some(connection) => ConnectionStatus {
                    state: "connected",
                    endpoint: Some(connection.endpoint.clone()),
                    message: None,
                },
                None => ConnectionStatus {
                    state: "disconnected",
                    endpoint: None,
                    message: Some("Orbit 未连接；Muse 仍可在本地独立使用。".into()),
                },
            },
            Err(_) => ConnectionStatus {
                state: "disconnected",
                endpoint: None,
                message: Some("Muse 连接状态不可用。".into()),
            },
        }
    }

    /// 清除失效连接，确保前端进入可重新登记的恢复路径。
    fn clear_connection(&self) {
        if let Ok(mut saved) = self.connection.lock() {
            *saved = None;
        }
    }

    /// 以固定 `source=muse`、`kind=idea` 写入文字，并保留失败后的前端草稿。
    async fn submit(&self, content: String) -> Result<CreatedMemory, String> {
        let content = content.trim().to_owned();
        if content.is_empty() {
            return Err("请输入要保存的灵感".into());
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Muse 连接状态不可用".to_owned())?
            .clone()
            .ok_or_else(|| "尚未连接 Orbit，请重试连接".to_owned())?;
        let response = self
            .client
            .post(format!("{}/v1/memories", connection.endpoint))
            .bearer_auth(&connection.token)
            .json(&serde_json::json!({
                "source": "muse",
                "kind": "idea",
                "content": content,
                "content_format": "plain",
                "tags": [],
                "device_id": "muse-desktop",
                "meta": {"capture_method": "text", "processed": false, "draft": false}
            }))
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                self.clear_connection();
                return Err(format!("Orbit 本地服务不可用，请重新连接后重试：{error}"));
            }
        };
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            self.clear_connection();
            return Err("Muse 授权已失效，请重新连接后重试".into());
        }
        parse_response(response).await
    }
}

/// 解析协议成功响应，并把服务端错误正文转换为可操作提示。
async fn parse_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, String> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| value["error"].as_str().map(str::to_owned))
            .unwrap_or(body);
        return Err(format!("本地记忆服务返回 {status}：{message}"));
    }
    response
        .json()
        .await
        .map_err(|error| format!("本地记忆服务响应无效：{error}"))
}

/// 通过 Tauri IPC 触发本地服务发现与 Muse 最小授权登记。
#[tauri::command]
async fn connect_service(state: State<'_, MuseState>) -> Result<ConnectionStatus, String> {
    state.connect().await
}

/// 通过 Tauri IPC 返回当前连接状态。
#[tauri::command]
fn get_connection_status(state: State<'_, MuseState>) -> ConnectionStatus {
    state.status()
}

/// 通过 Tauri IPC 把单一文字输入写入 Memory Protocol。
#[tauri::command]
async fn submit_idea(
    content: String,
    state: State<'_, MuseState>,
) -> Result<CreatedMemory, String> {
    state.submit(content).await
}

/// 显示并聚焦指定窗口，让已创建的快捷工具窗可以重复使用。
#[cfg(desktop)]
fn reveal_window(app: &tauri::AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        if let Err(error) = window.show() {
            eprintln!("Muse 无法显示 {label} 窗口：{error}");
            return;
        }
        if let Err(error) = window.set_focus() {
            eprintln!("Muse 无法聚焦 {label} 窗口：{error}");
        }
    }
}

/// 注册默认全局快捷键；单个按键冲突只记录诊断，不阻止 Muse 启动。
#[cfg(desktop)]
fn register_global_shortcuts(app: &tauri::App) {
    for shortcut in [
        "ctrl+shift+space",
        "ctrl+shift+i",
        "ctrl+shift+t",
        "ctrl+shift+r",
        "ctrl+shift+v",
    ] {
        if let Err(error) = app.global_shortcut().register(shortcut) {
            eprintln!("Muse 全局快捷键 {shortcut} 注册失败：{error}");
        }
    }
}

/// 初始化 Muse 多窗口客户端并启动 Tauri 运行时。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(desktop)]
    let builder = builder.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let label = if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::Space)
                {
                    Some("main")
                } else if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyI) {
                    Some("idea")
                } else if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyT) {
                    Some("task")
                } else if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyR) {
                    Some("meeting")
                } else if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyV) {
                    Some("clipboard")
                } else {
                    None
                };
                if let Some(label) = label {
                    reveal_window(app, label);
                }
            })
            .build(),
    );

    let builder = builder.setup(|app| {
        let data_dir = shared_nexus_data_dir(app.path().app_data_dir()?);
        app.manage(MuseState {
            client: reqwest::Client::new(),
            data_dir,
            connection: Mutex::new(None),
        });

        #[cfg(desktop)]
        register_global_shortcuts(app);

        Ok(())
    });

    #[cfg(desktop)]
    let builder = builder.on_window_event(|window, event| {
        if let WindowEvent::CloseRequested { api, .. } = event
            && TOOL_WINDOW_LABELS.contains(&window.label())
        {
            api.prevent_close();
            if let Err(error) = window.hide() {
                eprintln!("Muse 无法隐藏 {} 窗口：{error}", window.label());
            }
        } else if matches!(event, WindowEvent::CloseRequested { .. }) && window.label() == "main" {
            // 当前阶段尚未提供托盘退出入口，因此主窗口关闭即结束整个 Muse 进程。
            window.app_handle().exit(0);
        }
    });

    builder
        .invoke_handler(tauri::generate_handler![
            connect_service,
            get_connection_status,
            submit_idea
        ])
        .run(tauri::generate_context!())
        .expect("Muse Tauri 运行时启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::MemoryStore;
    use nexus_protocol::{CapabilityGrant, ProtocolState, Scope, serve};
    use tokio::net::TcpListener;

    /// 验证 Orbit 尚未启动时返回可读发现错误，而不是创建独立记忆库。
    #[tokio::test]
    async fn reports_missing_orbit_service() {
        let data_dir = tempfile::tempdir().expect("应创建空发现目录");
        let state = MuseState {
            client: reqwest::Client::new(),
            data_dir: data_dir.path().to_owned(),
            connection: Mutex::new(None),
        };
        let error = state.connect().await.expect_err("缺少 Orbit 时连接应失败");
        assert!(error.contains("未发现 Orbit 本地服务"));
        assert_eq!(state.status().state, "disconnected");
    }

    /// 验证 Muse 只用来源受限令牌写入，并可由 Orbit 管理令牌检索到。
    #[tokio::test]
    async fn writes_muse_idea_through_registered_connection() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("测试服务应绑定成功");
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let admin_token = "orbit-admin".to_owned();
        let server = tokio::spawn({
            let admin_token = admin_token.clone();
            async move {
                serve(
                    listener,
                    ProtocolState::new(
                        MemoryStore::open_in_memory().expect("应创建内存工作库"),
                        CapabilityGrant::new(admin_token, [Scope::Admin], None),
                    ),
                )
                .await
                .expect("测试服务应正常运行");
            }
        });
        let state = MuseState {
            client: reqwest::Client::new(),
            data_dir: PathBuf::new(),
            connection: Mutex::new(None),
        };
        state
            .register(&ServiceDiscovery {
                endpoint: endpoint.clone(),
                pid: 1,
                instance_id: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
                token: admin_token.clone(),
                protocol_version: "v1".into(),
            })
            .await
            .expect("Muse 应登记成功");
        let created = state
            .submit("Muse M3 端到端灵感".into())
            .await
            .expect("Muse 应写入成功");
        let memory = state
            .client
            .get(format!("{endpoint}/v1/memories/{}", created.id))
            .bearer_auth(&admin_token)
            .send()
            .await
            .expect("Orbit 应读取写入结果");
        let memory: serde_json::Value = parse_response(memory).await.expect("记忆响应应有效");
        assert_eq!(memory["source"], "muse");
        assert_eq!(memory["kind"], "idea");

        let connections = state
            .client
            .get(format!("{endpoint}/v1/connections"))
            .bearer_auth(&admin_token)
            .send()
            .await
            .expect("Orbit 应返回连接列表");
        let connections: serde_json::Value = parse_response(connections)
            .await
            .expect("连接列表响应应有效");
        let token_id = connections[0]["tokenId"]
            .as_str()
            .expect("连接应包含令牌标识");
        let revoked = state
            .client
            .delete(format!("{endpoint}/v1/connections/{token_id}"))
            .bearer_auth(&admin_token)
            .send()
            .await
            .expect("Orbit 应撤销 Muse 授权");
        assert_eq!(revoked.status(), reqwest::StatusCode::NO_CONTENT);
        let error = state
            .submit("撤销后的草稿".into())
            .await
            .expect_err("撤销后写入应失败");
        assert!(error.contains("授权已失效"));
        assert_eq!(state.status().state, "disconnected");

        state
            .register(&ServiceDiscovery {
                endpoint,
                pid: 1,
                instance_id: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
                token: admin_token,
                protocol_version: "v1".into(),
            })
            .await
            .expect("撤销后应可重新登记");
        state
            .submit("重新连接后的草稿".into())
            .await
            .expect("重新连接后应可重试写入");
        server.abort();
    }
}
