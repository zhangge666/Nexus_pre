# 本文件执行 Orbit M4 卡片、复习、RAG、Completion Provider 与桌面 IPC 的可重复验收。

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

Set-Location $root

# 运行单项验收命令，并在任一命令失败时立即终止。
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

Invoke-Checked "Completion Provider 与本地回退" {
    cargo test -p nexus-ai
}
Invoke-Checked "卡片、ReviewState 与 FSRS 调度" {
    cargo test -p nexus-core --test review
}
Invoke-Checked "RAG、引用、卡片和复习协议契约" {
    cargo test -p nexus-protocol
}
Invoke-Checked "Orbit Tauri M4 IPC 与 Key 脱敏" {
    cargo test -p orbit-app
}
Invoke-Checked "Orbit 前端类型检查" {
    pnpm --filter '@nexus/orbit' check
}
Invoke-Checked "Orbit 前端生产构建" {
    pnpm --filter '@nexus/orbit' build
}
Invoke-Checked "M4 Rust Clippy" {
    cargo clippy -p nexus-ai -p nexus-core -p nexus-protocol -p orbit-app --all-targets -- -D warnings
}

Write-Host "`nOrbit M4 智能化验收通过。" -ForegroundColor Green
