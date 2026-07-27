# Nilo

Niloは、Rustで実装された小さく読みやすいプログラミング言語です。単一の`nilo`コマンドに、インタプリタ、状態を保持するREPL、モジュール、実行時型検証、プロジェクトマニフェスト、テストランナー、開発者向け解析機能をまとめています。

> **現在の状態:** Nilo 0.2はアルファ版です。実験や小さなツールには利用できますが、1.0までは構文や標準ライブラリが変更される可能性があります。

## インストール

ビルド済みリリースを整備している間は、Rustツールチェーンからインストールできます。

```bash
cargo install --git https://github.com/nullx2-x/nilo-lang
nilo --version
```

このリポジトリを直接ビルドする場合:

```bash
cargo build --release
./target/release/nilo examples/main.nilo
```

## すぐに試す

```bash
# ファイルを直接実行
nilo examples/main.nilo

# Nilo.tomlで指定したエントリを実行
nilo run

# コマンドライン上のコードを実行
nilo -e 'print("Hello from Nilo")'

# 状態を保持するREPL
nilo

# 構文確認とテスト
nilo check examples/main.nilo
nilo test

# 新規プロジェクト作成
nilo init my-app
cd my-app
nilo run
```

## コード例

```nilo
from "math_tools" import add;
import "std/json" as json;

type Point {
    x: int;
    y: int;
}

func magnitude_squared(point: Point) -> int {
    return point.x * point.x + point.y * point.y;
}

let point: Point = Point(3, 4);
let values: list<int> = [1, 2, 3];
push(values, add(point.x, point.y));

print(json.stringify({
    "point": point,
    "score": magnitude_squared(point),
    "values": values
}, true));
```

## 主な機能

- `let`、関数、再帰、クロージャ、レコード型
- 整数、浮動小数、真偽値、文字列、リスト、map、`nil`
- `if` / `else if` / `else`、`while`、`for ... in`、`break`、`continue`
- `int`、`User?`、`list<str>`、`map<str, any>`などの実行時型検証
- 変数、レコードフィールド、リスト添字、map添字への代入
- `export`、`import`、`from ... import ...`によるファイルモジュール
- `std/json`、`std/regex`、`std/fs`、`std/http`、`std/time`、`std/list`、`std/string`、`std/math`
- 行・列とソース抜粋を表示するエラー診断
- CLI、永続REPL、プロジェクト初期化、テスト、トークン表示、AST表示

## コマンド

```text
nilo                         REPLを開始
nilo <file.nilo>             ソースファイルを実行
nilo run [file.nilo]         ファイルまたはNilo.tomlのentryを実行
nilo eval <source>           コード文字列を実行
nilo -e <source>             コード文字列を実行
nilo check <file.nilo>       実行せず構文を確認
nilo test [path]             *_test.niloを再帰的に実行
nilo init [path] [--name N]  新規プロジェクトを作成
nilo tokens <file.nilo>      トークンをJSONで表示
nilo ast <file.nilo>         ASTをJSONで表示
```

## プロジェクト構成

`Nilo.toml`を置いた通常のディレクトリがNiloパッケージになります。

```toml
[package]
name = "my-app"
version = "0.1.0"
entry = "src/main.nilo"

[exports]
main = "src/main.nilo"
```

詳細は[言語ガイド](docs/LANGUAGE.md)と[パッケージガイド](docs/PACKAGES.md)を参照してください。

## 開発

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo run -- examples/main.nilo
cargo run -- test tests
```

実装は字句解析、構文解析、AST、値、環境、インタプリタ、標準ライブラリ、CLIに分離しています。将来、フロントエンドを維持したままバイトコードVMやネイティブコンパイラへ発展させられる構造です。

## ライセンス

MIT
