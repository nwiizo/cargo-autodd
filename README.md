# 📦 cargo-autodd

A Cargo subcommand that automatically manages dependencies in your Rust projects.

## 🔍 Overview

cargo-autodd simplifies Rust dependency management by automatically adding required crates to your Cargo.toml based on `use` statements and `extern crate` declarations in your code. 

![cargo-autodd demo](.github/cargo-autodd_01.gif)

## ✨ Features

- 🔎 Scans Rust source files for imports
- 🤖 Automatically detects required dependencies
- ⬆️ Updates Cargo.toml with the latest stable versions
- 🗑️ Removes unused dependencies
- 🛠️ Leverages rust-analyzer for better code analysis (when available)
- 🧹 Removes unused dependencies
- 📊 Generates dependency usage reports
- 🔒 Checks for security vulnerabilities
- 🐛 Debug mode for detailed analysis

## 📥 Installation

```bash
cargo install cargo-autodd
```

## ⚙️ Requirements

- 🦀 Rust 1.56.0 or later
- 📦 Cargo
- 🔧 rust-analyzer (optional, but recommended)

## 🚀 Usage

### Automatic Dependency Management

```bash
# Analyze and update dependencies in the current project
cargo autodd

# Run with debug mode for detailed analysis
cargo autodd --debug
# or
cargo autodd -d
```

### Update Dependencies

```bash
# Check and update all dependencies to their latest versions
cargo autodd update
```

### Generate Reports

```bash
# Generate a detailed dependency usage report
cargo autodd report
```

### Security Check

```bash
# Check for known security vulnerabilities
cargo autodd security
```

### Debug Mode

デバッグモードでは以下の詳細情報が表示されます：

- 🔍 検出されたRustファイルのパス
- 📝 処理中の各行の内容
- 🔎 検出されたuse文と基本クレート名
- 📦 ネストされたインポートの詳細
- 🔧 extern crate文の検出
- 📊 各ファイルの解析結果
- 📋 最終的なクレート参照の一覧

## 🔄 How It Works

1. 📝 Analyzes your Rust source files
2. 🔍 Detects import statements and external crate declarations
3. ⚡ Updates Cargo.toml with required dependencies
4. ✅ Verifies changes with `cargo check`
5. 🔒 Checks for security vulnerabilities using the RustSec Advisory Database
6. 📊 Generates detailed reports about dependency usage

## 👥 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 👤 Author

nwiizo ([@nwiizo](https://x.com/nwiizo))
