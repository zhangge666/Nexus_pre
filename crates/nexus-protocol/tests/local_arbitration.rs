//! 本文件验证本地服务锁文件仲裁、发现记录和持有者退出后的接管行为。

use nexus_protocol::LocalServiceClaim;

/// 验证首个进程成为持有者、后续实例成为客户端，租约释放后可重新接管。
#[tokio::test]
async fn arbitrates_and_hands_over_local_service() {
    let directory = tempfile::tempdir().expect("临时目录应创建成功");
    let first = LocalServiceClaim::acquire(directory.path())
        .await
        .expect("首个实例应完成仲裁");
    let (lease, listener, first_discovery) = match first {
        LocalServiceClaim::Holder {
            lease,
            listener,
            discovery,
        } => (lease, listener, discovery),
        LocalServiceClaim::Client(_) => panic!("首个实例应成为持有者"),
    };
    let second = LocalServiceClaim::acquire(directory.path())
        .await
        .expect("第二个实例应发现持有者");
    match second {
        LocalServiceClaim::Client(discovery) => assert_eq!(discovery, first_discovery),
        LocalServiceClaim::Holder { .. } => panic!("锁占用时不应产生第二个持有者"),
    }

    drop(listener);
    drop(lease);
    let replacement = LocalServiceClaim::acquire(directory.path())
        .await
        .expect("租约释放后应允许接管");
    match replacement {
        LocalServiceClaim::Holder { discovery, .. } => {
            assert_ne!(discovery.instance_id, first_discovery.instance_id);
        }
        LocalServiceClaim::Client(_) => panic!("原持有者退出后应成为新持有者"),
    }
}
