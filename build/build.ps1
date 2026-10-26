# build.ps1 - WinMemCleaner 构建脚本
#
# 用法：
#   .\build.ps1            # Release 构建
#   .\build.ps1 -Debug     # Debug 构建
#   .\build.ps1 -Clean     # 清理编译缓存

param(
    [switch]$Debug,
    [switch]$Clean
)

$ErrorActionPreference = "Stop"

# 路径设置
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SrcDir = Split-Path -Parent $ScriptDir
$BuildRoot = Split-Path -Parent $SrcDir

$TempDir = Join-Path $BuildRoot ".temp"
# 编译中间产物统一放 .temp/（CARGO_HOME 沿用机器默认，避免每次重建注册表缓存）
$env:CARGO_TARGET_DIR = Join-Path $TempDir "target"

# 清理
if ($Clean) {
    Write-Host "清理编译缓存..." -ForegroundColor Yellow
    if (Test-Path $env:CARGO_TARGET_DIR) {
        Remove-Item -Path $env:CARGO_TARGET_DIR -Recurse -Force
    }
    Write-Host "清理完成" -ForegroundColor Green
    exit 0
}

# 构建
$Profile = if ($Debug) { "debug" } else { "release" }
Write-Host "构建 ($Profile)..." -ForegroundColor Cyan

Push-Location $SrcDir
try {
    if ($Debug) {
        cargo build
    } else {
        cargo build --release
    }
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    # 复制 exe 到 .build/
    $ExeSrc = Join-Path $env:CARGO_TARGET_DIR "$Profile\WinMemCleaner.exe"
    $BuildDir = Join-Path $BuildRoot ".build"
    if (!(Test-Path $BuildDir)) { New-Item -ItemType Directory -Path $BuildDir -Force | Out-Null }
    $ExeDst = Join-Path $BuildDir "WinMemCleaner.exe"
    if (Test-Path $ExeSrc) {
        Copy-Item $ExeSrc $ExeDst -Force
        $Size = (Get-Item $ExeDst).Length
        Write-Host "构建成功: $ExeDst ($([math]::Round($Size/1KB))KB)" -ForegroundColor Green
    }
} finally {
    Pop-Location
}
