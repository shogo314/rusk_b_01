# Rusk β

競技プログラミング向け言語 **Rusk のβ版**を開発・管理するリポジトリです。

作者自身にとっての書きやすさと、競技プログラミングで十分な実行速度を重視しています。Python と C++ に慣れた使い方を出発点に、言語を設計しています。

## 現在の状態

Rust と Parol で最小プロトタイプを実装しています。C++20 に変換し、基本型の入出力、整数演算、共有・コピー・代入、比較、条件分岐、while ループが動きます。C++ の型・演算の実装は `lib/core.rusk` に置いています。

実装に合わせて細部を決めていくため、設計ノートには合意した方針と、検討中・保留中の案が含まれます。配列・for ループ・Rusk で書く関数定義などは未実装です。

## 試す

Rust stable と GCC/Clang の C++20 環境を使います。

```bash
cargo run --locked -- compile examples/aplusb.rusk -o /tmp/aplusb.cpp
g++ -std=c++20 -O2 /tmp/aplusb.cpp -o /tmp/aplusb
printf '10 32\n' | /tmp/aplusb
```

出力は `42` です。[導入手順](docs/getting-started/index.md) と [実装の構成・検証方法](docs/development/index.md) も参照してください。

## 設計ノート

[design/README.md](design/README.md) を入口として、思想、値の共有とコピー、型、構文、関数、標準ライブラリなどの記録をまとめています。

設計ノートは設計の判断基準や理由を残すもので、実装済みの機能を説明する言語ドキュメントとは区別します。

## ドキュメントの方針

利用者向けドキュメントは、他の人にも理解できる内容を目指し、実装済みの機能に合わせて整備します。[docs/](docs/index.md) の Markdown を MkDocs Material で構築し、[GitHub Pages](https://shogo314.github.io/rusk_b_01/) で公開します。

ローカルでのプレビューと公開の手順は、[ドキュメントの編集と公開](docs/development/documentation.md) を参照してください。

```bash
python -m pip install -r requirements-docs.txt
python -m mkdocs serve
```
