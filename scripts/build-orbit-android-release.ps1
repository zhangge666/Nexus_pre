# 本脚本统一构建、签名并验证 Orbit Android release APK/AAB，签名密码只从环境变量读取。

[CmdletBinding()]
param(
    [string]$VersionName = "",
    [int]$VersionCode = 0,
    [ValidateSet("aarch64", "armv7", "i686", "x86_64")]
    [string[]]$Targets = @("aarch64", "armv7", "x86_64"),
    [switch]$PreviewSigning,
    [switch]$Unsigned,
    [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$TauriDirectory = Join-Path $Root "apps\orbit\src-tauri"
$AndroidDirectory = Join-Path $TauriDirectory "gen\android"
$ConfigPath = Join-Path $TauriDirectory "tauri.conf.json"

Set-Location $Root

if ($PreviewSigning -and $Unsigned) {
    throw "PreviewSigning 与 Unsigned 不能同时使用。"
}

# 返回必需环境变量；错误信息只包含变量名，不输出任何凭据内容。
function Get-RequiredEnvironmentVariable {
    param([Parameter(Mandatory = $true)][string]$Name)

    $value = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value)) {
        throw "缺少环境变量 $Name。"
    }
    return $value
}

# 在 Android SDK 中选择版本号最高的 Build-Tools。
function Get-LatestBuildToolsDirectory {
    param([Parameter(Mandatory = $true)][string]$AndroidHome)

    $root = Join-Path $AndroidHome "build-tools"
    if (-not (Test-Path -LiteralPath $root)) {
        throw "Android Build-Tools 目录不存在：$root"
    }
    $directory = Get-ChildItem -LiteralPath $root -Directory |
        Sort-Object { try { [version]$_.Name } catch { [version]"0.0" } } -Descending |
        Select-Object -First 1
    if ($null -eq $directory) {
        throw "Android Build-Tools 尚未安装。"
    }
    return $directory.FullName
}

# 执行外部命令，并在非零退出码时立即中止发布流程。
function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )

    Write-Host "`n==> $Label" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label 失败，退出码：$LASTEXITCODE"
    }
}

$baseConfig = Get-Content -LiteralPath $ConfigPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]::IsNullOrWhiteSpace($VersionName)) {
    $VersionName = [string]$baseConfig.version
}
if ($VersionName -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
    throw "VersionName 必须是合法 SemVer：$VersionName"
}
if ($VersionCode -eq 0) {
    $VersionCode = [int]$baseConfig.bundle.android.versionCode
}
if ($VersionCode -lt 1 -or $VersionCode -gt 2100000000) {
    throw "VersionCode 必须位于 1..2100000000。"
}

$AndroidHome = if (-not [string]::IsNullOrWhiteSpace($env:ANDROID_HOME)) {
    $env:ANDROID_HOME
} else {
    Get-RequiredEnvironmentVariable "ANDROID_SDK_ROOT"
}
$env:ANDROID_HOME = $AndroidHome
$env:ANDROID_SDK_ROOT = $AndroidHome
$BuildTools = Get-LatestBuildToolsDirectory $AndroidHome
$ApkSigner = Join-Path $BuildTools "apksigner.bat"
$ZipAlign = Join-Path $BuildTools "zipalign.exe"
$Aapt2 = Join-Path $BuildTools "aapt2.exe"
$JavaHome = Get-RequiredEnvironmentVariable "JAVA_HOME"
$JarSigner = Join-Path $JavaHome "bin\jarsigner.exe"
$Jar = Join-Path $JavaHome "bin\jar.exe"

foreach ($tool in @($ApkSigner, $ZipAlign, $Aapt2, $JarSigner, $Jar)) {
    if (-not (Test-Path -LiteralPath $tool)) {
        throw "发布工具不存在：$tool"
    }
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $Root "dist\orbit-android\$VersionName-$VersionCode"
}

$KeystorePath = ""
$KeyAlias = ""
$previousStorePassword = $env:NEXUS_ANDROID_STORE_PASSWORD
$previousKeyPassword = $env:NEXUS_ANDROID_KEY_PASSWORD
if ($PreviewSigning) {
    $KeystorePath = Join-Path $HOME ".android\debug.keystore"
    $KeyAlias = "androiddebugkey"
} elseif (-not $Unsigned) {
    $KeystorePath = Get-RequiredEnvironmentVariable "NEXUS_ANDROID_KEYSTORE_PATH"
    $KeyAlias = Get-RequiredEnvironmentVariable "NEXUS_ANDROID_KEY_ALIAS"
    Get-RequiredEnvironmentVariable "NEXUS_ANDROID_STORE_PASSWORD" | Out-Null
}
if (-not $Unsigned -and -not (Test-Path -LiteralPath $KeystorePath)) {
    throw "签名密钥库不存在：$KeystorePath"
}

$overlayPath = Join-Path $Root ".tmp-orbit-android-release-$PID.json"
$overlay = @{
    version = $VersionName
    bundle = @{
        android = @{
            versionCode = $VersionCode
        }
    }
} | ConvertTo-Json -Depth 5

try {
    if ($PreviewSigning) {
        $env:NEXUS_ANDROID_STORE_PASSWORD = "android"
        $env:NEXUS_ANDROID_KEY_PASSWORD = "android"
    } elseif (-not $Unsigned -and [string]::IsNullOrWhiteSpace($env:NEXUS_ANDROID_KEY_PASSWORD)) {
        $env:NEXUS_ANDROID_KEY_PASSWORD = $env:NEXUS_ANDROID_STORE_PASSWORD
    }

    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText(
        $overlayPath,
        $overlay,
        $utf8NoBom
    )

    $tauriArguments = @(
        "--filter", "@nexus/orbit", "android:build",
        "--target"
    ) + $Targets + @(
        "--apk", "--aab", "--ci", "--config", $overlayPath
    )
    Invoke-Checked "构建 Android release APK/AAB" {
        & pnpm @tauriArguments
    }

    $unsignedApk = Join-Path $AndroidDirectory "app\build\outputs\apk\universal\release\app-universal-release-unsigned.apk"
    $unsignedAab = Join-Path $AndroidDirectory "app\build\outputs\bundle\universalRelease\app-universal-release.aab"
    foreach ($artifact in @($unsignedApk, $unsignedAab)) {
        if (-not (Test-Path -LiteralPath $artifact)) {
            throw "构建产物不存在：$artifact"
        }
    }
    [System.IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null

    $targetLabel = $Targets -join "-"
    if ($Unsigned) {
        $finalApk = Join-Path $OutputDirectory "Orbit-$VersionName-$VersionCode-$targetLabel-unsigned.apk"
        $finalAab = Join-Path $OutputDirectory "Orbit-$VersionName-$VersionCode-$targetLabel-unsigned.aab"
        Copy-Item -LiteralPath $unsignedApk -Destination $finalApk -Force
        Copy-Item -LiteralPath $unsignedAab -Destination $finalAab -Force
    } else {
        $signingLabel = if ($PreviewSigning) { "preview" } else { "release" }
        $finalApk = Join-Path $OutputDirectory "Orbit-$VersionName-$VersionCode-$targetLabel-$signingLabel-signed.apk"
        $finalAab = Join-Path $OutputDirectory "Orbit-$VersionName-$VersionCode-$targetLabel-$signingLabel-signed.aab"
        Remove-Item -LiteralPath $finalApk, $finalAab -Force -ErrorAction SilentlyContinue

        Invoke-Checked "检查 unsigned APK 对齐" {
            & $ZipAlign -c -P 16 4 $unsignedApk
        }
        Invoke-Checked "签名 APK" {
            & $ApkSigner sign `
                --ks $KeystorePath `
                --ks-key-alias $KeyAlias `
                --ks-pass "env:NEXUS_ANDROID_STORE_PASSWORD" `
                --key-pass "env:NEXUS_ANDROID_KEY_PASSWORD" `
                --v1-signing-enabled true `
                --v2-signing-enabled true `
                --v3-signing-enabled true `
                --v4-signing-enabled false `
                --out $finalApk `
                $unsignedApk
        }

        Copy-Item -LiteralPath $unsignedAab -Destination $finalAab -Force
        Invoke-Checked "签名 AAB" {
            & $JarSigner `
                -keystore $KeystorePath `
                "-storepass:env" NEXUS_ANDROID_STORE_PASSWORD `
                "-keypass:env" NEXUS_ANDROID_KEY_PASSWORD `
                -sigalg SHA256withRSA `
                -digestalg SHA-256 `
                $finalAab `
                $KeyAlias
        }
        Invoke-Checked "验证 APK 签名" {
            & $ApkSigner verify --verbose --print-certs $finalApk
        }
        Invoke-Checked "验证 AAB 签名" {
            & $JarSigner -verify $finalAab
        }
        $aabEntries = & $Jar tf $finalAab
        if ($LASTEXITCODE -ne 0) {
            throw "读取 AAB 签名条目失败，退出码：$LASTEXITCODE"
        }
        if (-not ($aabEntries | Where-Object { $_ -match '^META-INF/.+\.SF$' }) -or
            -not ($aabEntries | Where-Object { $_ -match '^META-INF/.+\.(RSA|DSA|EC)$' })) {
            throw "AAB 未包含完整的 JAR 签名条目。"
        }
    }

    Invoke-Checked "验证最终 APK 对齐" {
        & $ZipAlign -c -P 16 4 $finalApk
    }

    $apkEntries = & $Jar tf $finalApk
    if ($LASTEXITCODE -ne 0) {
        throw "读取 APK 文件条目失败，退出码：$LASTEXITCODE"
    }
    if ($Unsigned) {
        $aabEntries = & $Jar tf $finalAab
        if ($LASTEXITCODE -ne 0) {
            throw "读取 AAB 文件条目失败，退出码：$LASTEXITCODE"
        }
    }

    # 显式核对 ABI，避免生成工程中残留的 jniLibs 被误打入发布包。
    $abiMap = @{
        aarch64 = "arm64-v8a"
        armv7 = "armeabi-v7a"
        i686 = "x86"
        x86_64 = "x86_64"
    }
    $expectedAbis = @($Targets | ForEach-Object { $abiMap[$_] } | Sort-Object -Unique)
    $apkAbis = @(
        $apkEntries |
            ForEach-Object { if ($_ -match '^lib/([^/]+)/liborbit_app_lib\.so$') { $Matches[1] } } |
            Sort-Object -Unique
    )
    $aabAbis = @(
        $aabEntries |
            ForEach-Object { if ($_ -match '^base/lib/([^/]+)/liborbit_app_lib\.so$') { $Matches[1] } } |
            Sort-Object -Unique
    )
    if (Compare-Object -ReferenceObject $expectedAbis -DifferenceObject $apkAbis) {
        throw "APK ABI 与构建目标不一致；期望 $($expectedAbis -join ', ')，实际 $($apkAbis -join ', ')。"
    }
    if (Compare-Object -ReferenceObject $expectedAbis -DifferenceObject $aabAbis) {
        throw "AAB ABI 与构建目标不一致；期望 $($expectedAbis -join ', ')，实际 $($aabAbis -join ', ')。"
    }

    $badging = & $Aapt2 dump badging $finalApk
    if ($LASTEXITCODE -ne 0) {
        throw "读取 APK 元数据失败，退出码：$LASTEXITCODE"
    }
    $packageLine = $badging | Select-Object -First 1
    if ($packageLine -notmatch "name='com\.nexus\.orbit'" -or
        $packageLine -notmatch "versionCode='$VersionCode'" -or
        $packageLine -notmatch "versionName='$([regex]::Escape($VersionName))'") {
        throw "APK 包名或版本元数据与发布参数不一致：$packageLine"
    }

    $hashFile = Join-Path $OutputDirectory "SHA256SUMS.txt"
    @($finalApk, $finalAab) |
        ForEach-Object {
            $hash = Get-FileHash -LiteralPath $_ -Algorithm SHA256
            "$($hash.Hash.ToLowerInvariant())  $([System.IO.Path]::GetFileName($_))"
        } |
        Set-Content -LiteralPath $hashFile -Encoding ASCII

    Write-Host "`nOrbit Android release 产物已就绪：" -ForegroundColor Green
    Write-Host "APK: $finalApk"
    Write-Host "AAB: $finalAab"
    Write-Host "SHA256: $hashFile"
} finally {
    Remove-Item -LiteralPath $overlayPath -Force -ErrorAction SilentlyContinue
    if (-not $Unsigned) {
        $env:NEXUS_ANDROID_STORE_PASSWORD = $previousStorePassword
        $env:NEXUS_ANDROID_KEY_PASSWORD = $previousKeyPassword
    }
}
