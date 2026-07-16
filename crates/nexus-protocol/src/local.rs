//! 本文件实现本地 Memory Protocol 服务的锁文件仲裁、端点发现和持有者租约。

use std::{
    fs::{self, File, OpenOptions},
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

const LOCK_FILE_NAME: &str = "memory-service.lock";
const DISCOVERY_FILE_NAME: &str = "memory-service.json";

/// 从各 Tauri 应用自己的数据目录推导 Nexus 产品族共享的数据目录。
#[must_use]
pub fn shared_nexus_data_dir(app_data_dir: impl AsRef<Path>) -> PathBuf {
    app_data_dir
        .as_ref()
        .parent()
        .unwrap_or_else(|| app_data_dir.as_ref())
        .join("com.nexus.shared")
}

/// 读取共享目录中的本地服务发现记录并校验其回环端点仍然可达。
pub async fn discover_local_service(data_dir: impl AsRef<Path>) -> io::Result<ServiceDiscovery> {
    let discovery = read_discovery(&data_dir.as_ref().join(DISCOVERY_FILE_NAME))?;
    verify_loopback_endpoint(&discovery).await?;
    Ok(discovery)
}

/// 表示其他本地应用连接 Memory Protocol 所需的发现信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceDiscovery {
    /// HTTP 回环端点。
    pub endpoint: String,
    /// 当前持有者进程标识。
    pub pid: u32,
    /// 当前持有者实例公开标识。
    pub instance_id: Uuid,
    /// 本地短期 capability token。
    pub token: String,
    /// Memory Protocol 主版本。
    pub protocol_version: String,
}

/// 持有文件排他锁并在释放时清理发现记录。
pub struct LocalServiceLease {
    lock_file: File,
    discovery_path: PathBuf,
    instance_id: Uuid,
}

impl LocalServiceLease {
    /// 返回当前持有者实例标识。
    #[must_use]
    pub const fn instance_id(&self) -> Uuid {
        self.instance_id
    }
}

impl Drop for LocalServiceLease {
    /// 仅删除仍属于当前实例的发现记录，并释放排他锁。
    fn drop(&mut self) {
        if read_discovery(&self.discovery_path)
            .is_ok_and(|record| record.instance_id == self.instance_id)
        {
            let _ = fs::remove_file(&self.discovery_path);
        }
        let _ = FileExt::unlock(&self.lock_file);
    }
}

/// 表示当前进程取得持有权，或发现另一个健康持有者。
pub enum LocalServiceClaim {
    /// 当前进程应启动服务并保留租约。
    Holder {
        /// 排他锁租约。
        lease: LocalServiceLease,
        /// 已绑定的随机回环端口。
        listener: TcpListener,
        /// 已发布的发现信息。
        discovery: ServiceDiscovery,
    },
    /// 当前进程应作为客户端连接已有服务。
    Client(ServiceDiscovery),
}

impl LocalServiceClaim {
    /// 竞争本地服务持有权；锁已占用时读取并验证发现端点。
    pub async fn acquire(data_dir: impl AsRef<Path>) -> io::Result<Self> {
        let data_dir = data_dir.as_ref();
        fs::create_dir_all(data_dir)?;
        let lock_path = data_dir.join(LOCK_FILE_NAME);
        let discovery_path = data_dir.join(DISCOVERY_FILE_NAME);
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;

        match lock_file.try_lock_exclusive() {
            Ok(()) => become_holder(lock_file, discovery_path).await,
            Err(error) if lock_is_contended(&error) => {
                let discovery = read_discovery(&discovery_path)?;
                verify_loopback_endpoint(&discovery).await?;
                Ok(Self::Client(discovery))
            }
            Err(error) => Err(error),
        }
    }
}

/// 识别 Unix `WouldBlock` 与 Windows `ERROR_LOCK_VIOLATION` 锁竞争结果。
fn lock_is_contended(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock || error.raw_os_error() == Some(33)
}

/// 绑定随机回环端口、生成短期凭据并原子发布发现记录。
async fn become_holder(lock_file: File, discovery_path: PathBuf) -> io::Result<LocalServiceClaim> {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).await?;
    let address = listener.local_addr()?;
    let instance_id = Uuid::new_v4();
    let discovery = ServiceDiscovery {
        endpoint: format!("http://{address}"),
        pid: std::process::id(),
        instance_id,
        token: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
        protocol_version: crate::protocol_version().into(),
    };
    write_discovery(&discovery_path, &discovery)?;
    Ok(LocalServiceClaim::Holder {
        lease: LocalServiceLease {
            lock_file,
            discovery_path,
            instance_id,
        },
        listener,
        discovery,
    })
}

/// 使用同目录临时文件替换发现记录，避免客户端读取半条 JSON。
fn write_discovery(path: &Path, discovery: &ServiceDiscovery) -> io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(discovery).map_err(io::Error::other)?,
    )?;
    restrict_discovery_permissions(&temporary)?;
    fs::rename(temporary, path)
}

/// 在 Unix 上把包含短期令牌的发现文件限制为当前用户读写。
#[cfg(unix)]
fn restrict_discovery_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

/// Windows 应用数据目录沿用当前用户 ACL，不额外改写继承权限。
#[cfg(not(unix))]
fn restrict_discovery_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// 读取并解析发现记录。
fn read_discovery(path: &Path) -> io::Result<ServiceDiscovery> {
    serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
}

/// 验证发现端点仍可连接且严格位于回环地址。
async fn verify_loopback_endpoint(discovery: &ServiceDiscovery) -> io::Result<()> {
    let address = discovery
        .endpoint
        .strip_prefix("http://")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "发现端点不是本地 HTTP"))?
        .parse::<SocketAddr>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if !address.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "发现端点不是回环地址",
        ));
    }
    TcpStream::connect(address).await.map(|_| ())
}
