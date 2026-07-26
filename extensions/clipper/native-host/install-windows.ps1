# 本脚本为当前用户安装 Nexus Clipper Native Messaging 宿主并保存来源受限令牌。
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[a-p]{32}$')]
    [string]$ExtensionId,
    [string]$Endpoint = 'http://127.0.0.1:4111',
    [string]$Token = ''
)

$ErrorActionPreference = 'Stop'
$HostName = 'com.nexus.clipper'
$InstallDir = Join-Path $env:LOCALAPPDATA 'Nexus\Clipper'
$ConfigDir = Join-Path $env:APPDATA 'Nexus'
$ScriptPath = Join-Path $PSScriptRoot 'nexus_clipper_host.py'
$PythonPath = (Get-Command python -ErrorAction Stop).Source

if (-not $Token) {
    $SecureToken = Read-Host '粘贴 Orbit 为 clipper 生成的令牌' -AsSecureString
    $Pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($SecureToken)
    try { $Token = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($Pointer) }
    finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($Pointer) }
}
if (-not $Token.Trim()) { throw '令牌不能为空' }
if (-not (Test-Path -LiteralPath $ScriptPath)) { throw "未找到宿主脚本：$ScriptPath" }

New-Item -ItemType Directory -Force -Path $InstallDir, $ConfigDir | Out-Null
Copy-Item -LiteralPath $ScriptPath -Destination (Join-Path $InstallDir 'nexus_clipper_host.py') -Force

$Config = @{
    endpoint = $Endpoint.TrimEnd('/')
    token = $Token.Trim()
    source = 'external:clipper'
} | ConvertTo-Json
$ConfigPath = Join-Path $ConfigDir 'clipper.json'
[IO.File]::WriteAllText($ConfigPath, $Config, [Text.UTF8Encoding]::new($false))

$LauncherPath = Join-Path $InstallDir 'nexus-clipper-host.cmd'
$Launcher = "@echo off`r`nset `"NEXUS_CLIPPER_CONFIG=$ConfigPath`"`r`n`"$PythonPath`" `"$InstallDir\nexus_clipper_host.py`"`r`n"
[IO.File]::WriteAllText($LauncherPath, $Launcher, [Text.ASCIIEncoding]::new())

$Manifest = @{
    name = $HostName
    description = 'Nexus browser clipper native host'
    path = $LauncherPath
    type = 'stdio'
    allowed_origins = @("chrome-extension://$ExtensionId/")
} | ConvertTo-Json -Depth 4
$ManifestPath = Join-Path $InstallDir "$HostName.json"
[IO.File]::WriteAllText($ManifestPath, $Manifest, [Text.UTF8Encoding]::new($false))

foreach ($Browser in @('Google\Chrome', 'Microsoft\Edge')) {
    $RegistryPath = "HKCU:\Software\$Browser\NativeMessagingHosts\$HostName"
    New-Item -Path $RegistryPath -Force | Out-Null
    Set-Item -Path $RegistryPath -Value $ManifestPath
}

Write-Host "Nexus Clipper 宿主已安装：$ManifestPath"
