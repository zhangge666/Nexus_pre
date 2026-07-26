//! 本文件封装 Orbit 云端 Completion API Key 的系统凭据库读写，禁止将密钥写入设置文件。
use keyring::Entry;

#[cfg(not(mobile))]
const SERVICE_NAME: &str = "com.nexus.orbit.completion";
#[cfg(mobile)]
const SYNC_SERVICE_NAME: &str = "com.nexus.orbit.sync";
#[cfg(mobile)]
const SYNC_ACCOUNT: &str = "remote-access-token";

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

/// 从当前平台凭据后端读取移动端远程 Memory Protocol 的访问令牌。
#[cfg(mobile)]
pub fn load_sync_token() -> Option<String> {
    Entry::new(SYNC_SERVICE_NAME, SYNC_ACCOUNT)
        .ok()?
        .get_password()
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 将移动端远程访问令牌交给平台凭据后端，禁止写入普通设置文件。
#[cfg(mobile)]
pub fn save_sync_token(token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("远程访问令牌不能为空".into());
    }
    let entry =
        Entry::new(SYNC_SERVICE_NAME, SYNC_ACCOUNT).map_err(|_| "无法打开系统凭据库".to_owned())?;
    entry
        .set_password(token)
        .map_err(|_| "无法保存远程访问令牌到系统凭据库".to_owned())
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
