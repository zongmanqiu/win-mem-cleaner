# build.ps1 - WinMemCleaner 构建脚本
#
# 用法：
#   .\build.ps1                          # Release 构建 → .build\WinMemCleaner.exe
#   .\build.ps1 -Debug                   # Debug 构建
#   .\build.ps1 -Clean                   # 清理编译缓存
#   .\build.ps1 -Deploy <运行目录>        # 构建后把 exe 拷进「实际运行的目录」
#
# 为什么要 -Deploy：开机启动注册的是「当前 exe 所在路径」。只更新 .build 而不动
# 运行目录，等于改了个没人执行的文件。

param(
    [switch]$Debug,
    [switch]$Clean,
    [string]$Deploy = ""
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

    # 部署到实际运行目录（该目录里的 exe 若正在运行会占用文件，拷贝会直接报错而不是静默跳过）
    if ($Deploy) {
        if (!(Test-Path $Deploy)) { throw "部署目录不存在: $Deploy" }
        $RunExe = Join-Path $Deploy "WinMemCleaner.exe"
        Copy-Item $ExeSrc $RunExe -Force
        Write-Host "已部署到运行目录: $RunExe" -ForegroundColor Green
        Write-Host "  提示：程序下次启动会把开机启动项刷成这个路径（自愈）" -ForegroundColor DarkGray
    }
} finally {
    Pop-Location
}
