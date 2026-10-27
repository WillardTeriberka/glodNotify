# gold-notify

使用 Rust + SQLite 常驻运行的黄金行情监控：按节假日接口判定交易日，在交易日每约 10 秒拉取行情入库；当某品种采样价刷新当日最低或最高时，通过飞书群机器人 Webhook 推送文本消息。

## 配置

复制 `.env.example` 为 `.env` 并填写：

- `DATABASE_PATH`：SQLite 文件路径（默认 `./data/gold.db`）。
- `FEISHU_WEBHOOK_URL`：飞书机器人 Webhook 地址（勿提交到仓库）。
- 可选：`TRADING_POLL_SECS`（默认 10）、`IDLE_POLL_SECS`（默认 3600，非交易日休眠间隔）。
- 建议设置 `TZ`（例如 `Asia/Shanghai`）以与 `trade_date` 对齐。

日志：`RUST_LOG=gold_notify=info,sqlx=warn`。

## 本地运行

需要安装 Rust（`rustup`）。首次构建会生成 `Cargo.lock`。

```bash
cp .env.example .env
# 编辑 .env
cargo run --release
```

## 交叉编译 Windows 可执行文件（无需在 Windows 装编译环境）

在 Linux 上构建出可直接运行的 Windows `.exe`，Windows 侧零工具链、零依赖：

```bash
# 一次性准备（Debian/Ubuntu）
rustup target add x86_64-pc-windows-gnu
sudo apt install gcc-mingw-w64-x86-64

# 构建（或直接执行 ./build-windows.sh）
cargo build --release --target x86_64-pc-windows-gnu
```

产物位于 `target/x86_64-pc-windows-gnu/release/gold-notify.exe`（已通过 `.cargo/config.toml` 静态链接 C 运行时），拷贝到 Windows 后与 `data/`、`.env` 放同一目录即可运行。

## Docker / WSL

在仓库根目录：

```bash
cp .env.example .env
# 编辑 .env
docker compose build
docker compose up -d
```

`docker-compose.yml` 将宿主机 `./data` 挂载到容器内 `/data`，并设置 `DATABASE_PATH=/data/gold.db`，避免容器重建丢失数据库。

在 WSL 中长期运行：使用 `docker compose` 或 `docker run` 均可，注意宿主机目录持久化与 `.env` 中的 Webhook。

## 数据库

表结构见 `migrations/20240323000001_init.sql`（`t_workday`、`t_gold_sample`、`t_daily_extreme`）。

## 业务说明摘要

- **交易日**：`https://api.jiejiariapi.com/v1/workdays/{year}` 返回的日期在本地 `t_workday` 中存在且 `is_workday = 1`；若当天无记录则视为非交易日。
- **行情**：`https://www.huilvbiao.com/api/gold_indexApi?t={unix_ms}`，解析全部 `hq_str_*` 变量；首个可解析浮点字段作为采样价。
- **飞书**：`POST` JSON，`msg_type` 为 `text`，`content.text` 为正文。

## 许可证

按项目需要自行补充。
