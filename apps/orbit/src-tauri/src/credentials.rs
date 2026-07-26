//! 本文件封装 Orbit 云端 Completion API Key 的系统凭据库读写，禁止将密钥写入设置文件。
#[cfg(not(mobile))]
use keyring::Entry;

#[cfg(not(mobile))]
const SERVICE_NAME: &str = "com.nexus.orbit.completion";

/// 从系统凭据库读取指定云端 Provider 的 API Key；不存在或不可用时返回空。
#[cfg(not(mobile))]
pub fn load_api_key(provider: &str) -> Option<String> {
    let account = credential_account(provider)?;
    Entry::new(SERVICE_NAME, account)
        .ok()?
        .get_password()
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 将指定云端 Provider 的 API Key 保存到操作系统凭据库，设置 JSON 永远不会接触该值。
#[cfg(not(mobile))]
pub fn save_api_key(provider: &str, api_key: &str) -> Result<(), String> {
    let account = credential_account(provider)
        .ok_or_else(|| "当前 Provider 不需要保存 API Key".to_owned())?;
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("API Key 不能为空".into());
    }
    let entry = Entry::new(SERVICE_NAME, account).map_err(|_| "无法打开系统凭据库".to_owned())?;
    entry
        .set_password(api_key)
        .map_err(|_| "无法保存 API Key 到系统凭据库".to_owned())
}

/// 返回会使用系统凭据库的云端 Provider 帐号，纯本地模式不产生任何密钥条目。
#[cfg(not(mobile))]
fn credential_account(provider: &str) -> Option<&'static str> {
    match provider {
        "claude" => Some("claude"),
        "openai" => Some("openai"),
        "custom" => Some("custom"),
        _ => None,
    }
}
