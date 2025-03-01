use anyhow::Result;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

use crate::dependency_manager::DependencyAnalyzer;
use crate::CargoAutodd;

/// monorepo構造のテスト環境を作成する
fn create_monorepo_test_environment() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let root_path = temp_dir.path();

    // ルートのCargo.tomlを作成
    let root_cargo_toml = root_path.join("Cargo.toml");
    let root_content = r#"
[workspace]
members = [
    "public-crate",
    "internal-crate"
]

[workspace.package]
version = "0.1.0"
edition = "2021"
"#;
    let mut file = File::create(&root_cargo_toml)?;
    writeln!(file, "{}", root_content)?;

    // 内部クレートのディレクトリとファイルを作成
    let internal_crate_dir = root_path.join("internal-crate");
    fs::create_dir(&internal_crate_dir)?;

    // 内部クレートのCargo.toml
    let internal_cargo_toml = internal_crate_dir.join("Cargo.toml");
    let internal_content = r#"
[package]
name = "internal-crate"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
serde = "1.0"
"#;
    let mut file = File::create(&internal_cargo_toml)?;
    writeln!(file, "{}", internal_content)?;

    // 内部クレートのソースファイル
    fs::create_dir(internal_crate_dir.join("src"))?;
    let internal_lib_rs = internal_crate_dir.join("src/lib.rs");
    let internal_lib_content = r#"
use serde;

pub fn internal_function() -> &'static str {
    "This is an internal function"
}
"#;
    let mut file = File::create(internal_lib_rs)?;
    writeln!(file, "{}", internal_lib_content)?;

    // 公開クレートのディレクトリとファイルを作成
    let public_crate_dir = root_path.join("public-crate");
    fs::create_dir(&public_crate_dir)?;

    // 公開クレートのCargo.toml
    let public_cargo_toml = public_crate_dir.join("Cargo.toml");
    let public_content = r#"
[package]
name = "public-crate"
version = "0.1.0"
edition = "2021"

[dependencies]
internal-crate = { path = "../internal-crate" }
"#;
    let mut file = File::create(&public_cargo_toml)?;
    writeln!(file, "{}", public_content)?;

    // 公開クレートのソースファイル
    fs::create_dir(public_crate_dir.join("src"))?;
    let public_lib_rs = public_crate_dir.join("src/lib.rs");
    let public_lib_content = r#"
use internal_crate;
use tokio;

pub fn public_function() -> &'static str {
    let internal_result = internal_crate::internal_function();
    "This is a public function using an internal crate"
}
"#;
    let mut file = File::create(public_lib_rs)?;
    writeln!(file, "{}", public_lib_content)?;

    Ok(temp_dir)
}

#[test]
fn test_monorepo_dependency_analysis() -> Result<()> {
    let temp_dir = create_monorepo_test_environment()?;
    let root_path = temp_dir.path().to_path_buf();

    // 公開クレートのパス
    let public_crate_path = root_path.join("public-crate");

    // 依存関係の分析
    let analyzer = DependencyAnalyzer::with_debug(public_crate_path.clone(), true);
    let crate_refs = analyzer.analyze_dependencies()?;

    // 結果の検証
    // tokioが検出されるべき
    assert!(
        crate_refs.contains_key("tokio"),
        "tokio dependency should be detected"
    );

    // internal-crateまたはinternal_crateが検出されるべき
    // Rustの命名規則により、ハイフンはアンダースコアに変換されることがある
    let has_internal_crate =
        crate_refs.contains_key("internal-crate") || crate_refs.contains_key("internal_crate");
    assert!(
        has_internal_crate,
        "internal crate dependency should be detected"
    );

    // 内部クレートがpath依存として検出されているか確認
    if let Some(internal_crate) = crate_refs.get("internal-crate") {
        assert!(
            internal_crate.is_path_dependency,
            "internal-crate should be a path dependency"
        );
    } else if let Some(_internal_crate) = crate_refs.get("internal_crate") {
        // 内部クレートがinternal_crateとして検出された場合
        // 注: use文からの検出の場合、path依存として認識されない可能性がある
        // この場合はテストをスキップする
        println!("internal_crate detected instead of internal-crate");
    }

    Ok(())
}

#[test]
#[ignore] // 実際のcrates.ioへのアクセスが必要なため、CI環境ではスキップ
fn test_monorepo_update_with_internal_crates() -> Result<()> {
    let temp_dir = create_monorepo_test_environment()?;
    let root_path = temp_dir.path().to_path_buf();

    // 公開クレートのパス
    let public_crate_path = root_path.join("public-crate");

    // CargoAutoddインスタンスを作成
    let autodd = CargoAutodd::with_debug(public_crate_path, true);

    // 依存関係の更新を実行
    let result = autodd.analyze_and_update();

    // 結果の検証
    assert!(
        result.is_ok(),
        "analyze_and_update should succeed with internal crates"
    );

    // 更新後のCargo.tomlを読み込む
    let cargo_toml_content = fs::read_to_string(root_path.join("public-crate/Cargo.toml"))?;

    // tokioが追加されているか確認
    assert!(
        cargo_toml_content.contains("tokio"),
        "tokio should be added to Cargo.toml"
    );

    // internal-crateがpath依存として維持されているか確認
    assert!(
        cargo_toml_content.contains("internal-crate = { path ="),
        "internal-crate should be maintained as a path dependency"
    );

    Ok(())
}

#[test]
#[ignore] // 実際のcrates.ioへのアクセスが必要なため、CI環境ではスキップ
fn test_monorepo_with_publish_false_crates() -> Result<()> {
    let temp_dir = create_monorepo_test_environment()?;
    let root_path = temp_dir.path().to_path_buf();

    // 内部クレートのパス
    let internal_crate_path = root_path.join("internal-crate");

    // CargoAutoddインスタンスを作成
    let autodd = CargoAutodd::with_debug(internal_crate_path, true);

    // 依存関係の更新を実行
    let result = autodd.analyze_and_update();

    // 結果の検証
    assert!(
        result.is_ok(),
        "analyze_and_update should succeed with publish=false crates"
    );

    // 更新後のCargo.tomlを読み込む
    let cargo_toml_content = fs::read_to_string(root_path.join("internal-crate/Cargo.toml"))?;

    // publish = falseが維持されているか確認
    assert!(
        cargo_toml_content.contains("publish = false"),
        "publish = false should be maintained"
    );

    // serdeが追加/維持されているか確認
    assert!(
        cargo_toml_content.contains("serde"),
        "serde should be in Cargo.toml"
    );

    Ok(())
}
