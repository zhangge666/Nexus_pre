//! 本文件实现进程内记忆事件广播，为 UI 刷新和协议订阅提供统一事件源。

use std::sync::{Arc, Mutex, mpsc};

use uuid::Uuid;

use crate::{CoreError, Result};

/// 表示记忆完成事务提交后的领域事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    /// 一条记忆已经创建。
    MemoryCreated {
        /// 已创建的记忆标识。
        id: Uuid,
    },
    /// 一条记忆已经更新。
    MemoryUpdated {
        /// 已更新的记忆标识。
        id: Uuid,
    },
    /// 一条记忆及其关联索引已经删除。
    MemoryDeleted {
        /// 已删除的记忆标识。
        id: Uuid,
    },
}

/// 表示一个按发送顺序接收核心事件的订阅。
pub struct EventSubscription {
    receiver: mpsc::Receiver<CoreEvent>,
}

impl EventSubscription {
    /// 在指定时限内等待下一条事件。
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<CoreEvent> {
        self.receiver.recv_timeout(timeout).ok()
    }
}

/// 管理当前进程内全部事件订阅者。
#[derive(Default, Clone)]
pub(crate) struct EventBus {
    subscribers: Arc<Mutex<Vec<mpsc::Sender<CoreEvent>>>>,
}

impl EventBus {
    /// 注册新的事件订阅并返回独立接收端。
    pub(crate) fn subscribe(&self) -> Result<EventSubscription> {
        let (sender, receiver) = mpsc::channel();
        self.subscribers
            .lock()
            .map_err(|_| CoreError::StoreUnavailable)?
            .push(sender);
        Ok(EventSubscription { receiver })
    }

    /// 广播已提交事件，并清理已经断开的订阅。
    pub(crate) fn publish(&self, event: CoreEvent) -> Result<()> {
        self.subscribers
            .lock()
            .map_err(|_| CoreError::StoreUnavailable)?
            .retain(|subscriber| subscriber.send(event.clone()).is_ok());
        Ok(())
    }
}
