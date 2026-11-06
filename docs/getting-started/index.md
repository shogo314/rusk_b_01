# はじめに

最初のプロトタイプは Rusk のソースを C++20 に変換します。Rust の stable ツールチェーン（edition 2024 と let chains 対応の Rust 1.88 以降）と、GCC または Clang の C++20 コンパイラが必要です。テストでは `g++` を使います。

## ビルドと実行

リポジトリのルートで実行します。

```bash
cargo build --release --locked
./target/release/rusk compile examples/aplusb.rusk -o /tmp/aplusb.cpp
g++ -std=c++20 -O2 /tmp/aplusb.cpp -o /tmp/aplusb
printf '10 32\n' | /tmp/aplusb
```

出力は `42` です。提出するときは生成された C++ を使います。Rusk の処理系は実行時に必要ありません。

`examples/aplusb.rusk` の内容：

```text
in a, b: Int;
wt a + b;
```

`compile` の `-o` を省略すると、カレントディレクトリに `a.cpp` を生成します。構文・型などの検査に失敗した場合は出力ファイルを書きません。

## 現在の範囲

基本型の入出力、整数の四則演算、変数の共有・コピー・代入、ブロックスコープ、比較、条件分岐、`while` / `loop` が使えます。配列、`for`、Rusk で書く関数定義はまだありません。整数列の合計の例は `examples/sum.rusk` にあります。

書き方は [言語リファレンス](../reference/index.md)、処理系の構成は [開発ガイド](../development/index.md) を参照してください。
