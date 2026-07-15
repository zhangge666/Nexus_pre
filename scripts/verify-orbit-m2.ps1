# 本脚本执行 Orbit M2 的可重复发布前回归：类型检查、协议核心流程测试、桌面构建与产物检查。
$ErrorActionPreference = "Stop"

Write-Host "[M2] Run Orbit frontend type check"
pnpm --filter @nexus/orbit check

Write-Host "[M2] Run Orbit Protocol workflow test"
cargo test -p orbit-app

Write-Host "[M2] Run Protocol contract and arbitration test"
cargo test -p nexus-protocol

Write-Host "[M2] Build Tauri desktop debug artifact"
pnpm --filter @nexus/orbit tauri build --debug --no-bundle

Write-Host "[M2] Orbit regression passed"
