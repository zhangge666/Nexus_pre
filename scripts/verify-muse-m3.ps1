# 本文件执行 Muse M3 最小来源的协议、桌面壳、前端与 Orbit 回显验收。

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

Set-Location $root

# 运行单项验收命令，并在失败时立即终止整个 M3 验证。
function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Label,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Command
    )

    Write-Host "`n==> $Label" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label 失败，退出码：$LASTEXITCODE"
    }
}

Invoke-Checked "Memory Protocol Muse 授权、撤销与写入契约" {
    cargo test -p nexus-protocol registers_lists_and_revokes_muse_connection
}
Invoke-Checked "Muse Rust 端到端写入" {
    cargo test -p muse-app
}
Invoke-Checked "Orbit 事件与连接管理回显" {
    cargo test -p orbit-app
}
Invoke-Checked "Muse 前端类型检查" {
    pnpm --filter '@nexus/muse' check
}
Invoke-Checked "Muse 前端构建" {
    pnpm --filter '@nexus/muse' build
}
Invoke-Checked "Orbit 连接与回显类型检查" {
    pnpm --filter '@nexus/orbit' check
}
Invoke-Checked "Memory Protocol TypeScript 客户端测试" {
    pnpm --filter '@nexus/protocol-client' test
}

Write-Host "`nMuse M3 最小来源验收通过。" -ForegroundColor Green
