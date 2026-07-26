# Nexus Clipper

Chrome/Edge Manifest V3 扩展通过 Native Messaging 把当前网页写入本地 Memory Protocol，令牌不会进入页面或扩展存储。

## 安装

1. 在 Orbit「连接与隐私」创建应用标识 `clipper`，只授予 `memory:write`。
2. 在浏览器扩展管理页开启开发者模式，选择“加载已解压的扩展程序”，指向本目录并复制扩展 ID。
3. 运行：

```powershell
.\native-host\install-windows.ps1 -ExtensionId <扩展 ID>
```

4. 粘贴 Orbit 只展示一次的令牌，重新加载扩展。

每条剪藏记忆固定标记为 `source=external:clipper`、`kind=clip`，并在 `meta.url` 中保留原始网页地址。
