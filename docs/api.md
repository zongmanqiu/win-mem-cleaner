# WinMemCleaner API 参考

> 核心模块的公开接口文档。

## core::util — 工具函数

### `pub fn wide(s: &str) -> Vec<u16>`
将 UTF-8 字符串转换为以 NUL 结尾的 UTF-16 宽字符向量。
用于 Win32 API 的字符串参数。

```rust
let wide = wide("Hello");
// wide = [72, 101, 108, 108, 111, 0]
```

### `pub fn wide_to_string(buf: &[u16]) -> String`
将 UTF-16 缓冲区转换为 String。遇第一个 NUL 停止。

---

## core::config — 配置持久化

### `pub struct AppConfig`
应用配置，JSON 序列化/反序列化。

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `interval_minutes` | `u32` | `5` | 方式一（定时）：清理间隔（分钟） |
| `interval_level` | `u32` | `1` | 方式一：清理强度 1=标准 2=深度（兼容旧字段名 `level`） |
| `threshold_percent` | `u32` | `85` | 方式二（阈值）：物理内存占用超过该百分比时触发 |
| `threshold_level` | `u32` | `2` | 方式二：清理强度 1=标准 2=深度 |
| `fullscreen_avoid` | `bool` | `true` | 全屏/游戏中自动避让 |
| `autostart` | `bool` | `false` | 开机启动 |

两种方式互相独立、可同时生效；`interval_minutes` 与 `threshold_percent` 只接受整数，
`threshold_percent` 上限取 99（100% 永远不会被超过）。

### `pub fn config_path() -> Option<PathBuf>`
配置文件路径：与 exe 同级的 `WinMemCleaner.json`（便携程序，不往用户目录写文件）。
取不到 exe 路径时返回 `None`，此时不读也不写。

### `pub fn load() -> AppConfig`
从 `config_path()` 加载配置；文件不存在或取不到路径时返回默认值。

### `pub fn save(cfg: &AppConfig)`
将配置写入 `config_path()`；路径不可用时静默跳过。

### `AppConfig::defaults() -> Self`
返回默认配置。

### `AppConfig::sanitize(&mut self)`
校验并修正配置值（clamp 间隔与阈值百分比、重置非法强度）。

---

## core::mem — 内存清理核心

### 信息采集

#### `pub fn mem_info() -> MEMORYSTATUSEX`
获取系统内存信息（物理内存、页面文件等）。

#### `pub fn physical_usage_percent() -> u32`
当前物理内存使用百分比 (0..=100)。

#### `pub fn avail_bytes() -> u64`
当前可用物理内存字节数。

#### `pub fn memory_snapshot() -> MemSnapshot`
采集物理/虚拟内存/系统缓存的完整快照。

```rust
pub struct MemSnapshot {
    pub phys_total: u64,      // 物理内存总量
    pub phys_used: u64,       // 物理内存已用
    pub phys_percent: u32,    // 物理内存使用率 = 已用/总量
    pub page_total: u64,      // 提交上限（= 物理内存 + 页面文件），不是页面文件大小
    pub page_used: u64,       // 提交量（commit charge）
    pub page_percent: u32,    // 提交量 / 提交上限
    pub cache_bytes: u64,     // 系统缓存实时字节（≈ 可回收的缓存/备用内存）
    pub cache_percent: u32,   // 系统缓存占**物理内存总量**的百分比
}
```

#### `pub fn system_cache_bytes() -> u64`
系统缓存实时字节数。取自文档化 API `GetPerformanceInfo().SystemCache`（该结构所有数值字段单位都是
「页」，需乘 `PageSize`）。历史实现读的是未文档化的
`NtQuerySystemInformation(SystemFileCacheInformation)`，结构布局靠猜、且把「缓存峰值」当分母显示，
同机实测差 13 倍（0.36 GB vs 4.75 GB），已弃用。

> 注意：`page_percent`（提交量/提交上限）与 `cache_percent`（缓存占物理内存比）都不等于
> 「内存紧张程度」。自动清理的阈值判据只用 `phys_percent`，理由见 README「运行环境」。

#### `pub fn format_bytes(bytes: u64) -> String`
格式化字节为可读字符串（如 "1.5 GB"）。

### 权限

#### `pub fn is_admin() -> bool`
判断当前进程是否以管理员权限运行（通过 TokenElevation 检查）。

#### `pub fn raise_privileges()`
提升本进程所需的全部特权（SeIncreaseQuotaPrivilege 等）。

### 清理

#### `pub fn clean_by_level(level: u32) -> String`
按档位执行清理：
- `1`（标准）：合并物理内存页 + 裁剪进程工作集 + 清理系统文件缓存 + 低优先级备用内存
- `2`（深度）：标准全部 + 系统级清空工作集 + 清空 Standby + 冲刷 Modified 页

#### `pub fn is_game_or_fullscreen() -> bool`
判断当前是否在全屏/游戏/演示状态（用于自动清理避让）。

### 底层清理函数

| 函数 | 说明 |
|------|------|
| `clear_system_working_sets()` | 系统级清空所有进程工作集 |
| `clear_system_file_cache()` | 清空系统文件缓存 |
| `flush_modified_page_list()` | 冲刷 Modified 页列表 |
| `clear_standby()` | 清空 Standby 备用内存 |
| `clear_standby_low_priority()` | 清空低优先级 Standby |
| `trim_process(pid)` | 修剪单个进程的工作集 |
| `trim_all_processes(opts)` | 遍历所有进程修剪工作集 |

---

## core::scheduler — 自动调度

### `pub struct Scheduler`
后台自动清理调度器（每 5 秒轮询一次）。

### `Scheduler::new(cfg: Arc<RwLock<AppConfig>>) -> Self`
创建调度器实例。

### `Scheduler::start(&self)`
启动后台清理线程，两种方式各自独立判定：
- 定时：距上次触发 ≥ `interval_minutes` → 按 `interval_level` 清理
- 阈值：**物理内存**占用 > `threshold_percent`% → 按 `threshold_level` 清理；
  触发后有 60 秒静默期，避免占用长期停留在阈值上方时反复清理；
  只判物理内存，提交量与缓存比值不参与（见 README「运行环境」）
- 两种方式在同一轮都到期时，优先执行阈值方式（内存压力优先）
- 两种触发都受全屏避让约束

### `Scheduler::stop(&self)`
停止清理线程。
