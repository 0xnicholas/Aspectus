# justfile

# 默认命令
default:
    @just --list

# 启动开发环境
dev:
    docker compose up -d
    cargo run -p aspectus-server

# 运行 migration
migrate:
    sqlx migrate run

# 回滚 migration
migrate-rollback:
    sqlx migrate revert

# 运行全部测试
test:
    cargo test --workspace

# 运行测试（含输出）
test-verbose:
    cargo test --workspace -- --nocapture

# 代码检查
lint:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features

# 自动格式化
fmt:
    cargo fmt --all

# 清理
clean:
    cargo clean
    docker compose down -v
