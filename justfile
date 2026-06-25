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

# 运行全部测试（需要已启动 docker compose 并执行 migration）
test:
    cargo test --workspace --lib
    cargo test -p aspectus-server --test http_tests
    cargo test -p aspectus-server --test integration_test
    cargo test -p aspectus-server --test feature_test
    cargo test -p aspectus-server --test e2e_test
    cargo test -p aspectus-server --test bench_test

# 运行测试（含输出）
test-verbose:
    cargo test --workspace --lib -- --nocapture
    cargo test -p aspectus-server --test http_tests -- --nocapture

# 代码检查（与 CI 一致，warning 视为 error）
lint:
    cargo fmt --all -- --check
    RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features

# 自动格式化
fmt:
    cargo fmt --all

# 清理
clean:
    cargo clean
    docker compose down -v
