# E2E Test - End-to-End テスト実行

このコマンドは、cargo-autodd の機能を包括的にテストします。

## 使用方法

```
/e2e-test [オプション]
```

## オプション

- `--verbose`: 詳細な出力を表示
- `--quick`: 主要なテストのみ実行
- `--all`: 全てのテスト（unit + integration + E2E）を実行

## 実行内容

以下のE2Eテストを順次実行します：

### 1. 基本機能テスト
- [ ] 依存関係の自動検出
- [ ] Cargo.toml の更新
- [ ] dry-run モードの動作確認

### 2. dev-dependencies テスト
- [ ] tests/ ディレクトリからの検出
- [ ] [dev-dependencies] セクションへの追加

### 3. 設定ファイルテスト
- [ ] .cargo-autodd.toml の読み込み
- [ ] exclude 設定の適用
- [ ] essential 設定の適用

### 4. ワークスペーステスト
- [ ] workspace プロジェクトの検出
- [ ] path 依存関係のハンドリング

### 5. サブコマンドテスト
- [ ] report サブコマンド
- [ ] security サブコマンド
- [ ] update サブコマンド

## テスト手順

$ARGUMENTS

### Step 1: ビルド確認

```bash
cargo build
```

### Step 2: ユニットテスト実行

```bash
cargo test
```

### Step 3: E2Eテストスクリプト実行

```bash
./scripts/e2e-test.sh
```

### Step 4: 手動検証（オプション）

テスト用プロジェクトを作成して動作確認：

```bash
# テストディレクトリ作成
cd /tmp && rm -rf cargo-autodd-manual-test && mkdir cargo-autodd-manual-test && cd cargo-autodd-manual-test

# 最小限のプロジェクト作成
cat > Cargo.toml << 'EOF'
[package]
name = "manual-test"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

mkdir -p src tests

cat > src/main.rs << 'EOF'
use serde::Serialize;
use tokio::runtime::Runtime;

fn main() {
    println!("Hello!");
}
EOF

cat > tests/integration.rs << 'EOF'
use tempfile::TempDir;

#[test]
fn test_example() {}
EOF

# dry-run で確認
cargo autodd --dry-run

# 実際に更新
cargo autodd

# 結果確認
cat Cargo.toml
```

## 期待される結果

### 成功時の出力例

```
========================================
  cargo-autodd E2E Test Suite
========================================

[BUILD] Building cargo-autodd...
[BUILD] Build successful

Running E2E tests...

[TEST] Basic dependency detection
[PASS] Basic dependency detection

[TEST] Dev-dependencies detection
[PASS] Dev-dependencies detection

... (中略) ...

========================================
  Test Summary
========================================
  Passed: 10
  Failed: 0

All tests passed!
```

### 失敗時の対応

テストが失敗した場合：

1. 失敗したテスト名を確認
2. `--debug` フラグで詳細ログを取得
3. 関連するユニットテストを個別実行

```bash
# 特定のテストを実行
cargo test test_name -- --nocapture

# デバッグモードで実行
cargo run -- autodd --debug
```

## 関連コマンド

- `/quality-check` - コード品質チェック（fmt, clippy, test）
- `/mutation-test` - Mutation testing の実行

## トラブルシューティング

### E2Eスクリプトが見つからない

```bash
chmod +x ./scripts/e2e-test.sh
```

### テストがタイムアウトする

crates.io へのネットワークアクセスが必要なテストは `--ignored` で実行：

```bash
cargo test -- --ignored
```

### 権限エラー

```bash
# スクリプトに実行権限を付与
chmod +x ./scripts/e2e-test.sh
```
