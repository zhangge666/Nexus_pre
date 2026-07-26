# 本脚本统一验收 M6 协议授权、SDK、MCP、浏览器剪藏和 Orbit 连接面板。
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )
    Write-Host "==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name 失败，退出码 $LASTEXITCODE"
    }
}

Invoke-Checked 'Memory Protocol 契约测试' { cargo test -p nexus-protocol --test http_contract }
Invoke-Checked 'Orbit Rust 编译' { cargo check -p orbit-app }
Invoke-Checked 'Orbit 前端构建' { pnpm --filter '@nexus/orbit' build }
Invoke-Checked 'Memory Protocol TypeScript 客户端测试' { pnpm --filter '@nexus/protocol-client' test }
Invoke-Checked 'TypeScript SDK 测试' { pnpm --filter '@nexus/sdk-ts' test }
Invoke-Checked 'MCP Server 测试' { pnpm --filter '@nexus/mcp-server' test }
Invoke-Checked 'Python SDK/CLI 测试' { python -m unittest discover -s sdk/python/tests -v }
Invoke-Checked 'Native Messaging 宿主测试' { python -m unittest discover -s extensions/clipper/native-host/tests -v }
Invoke-Checked '浏览器扩展脚本检查' { node --check extensions/clipper/popup.js }
Invoke-Checked '浏览器扩展 Manifest 检查' { node -e "JSON.parse(require('fs').readFileSync('extensions/clipper/manifest.json','utf8'))" }

Write-Host 'M6 外联验收通过。'
