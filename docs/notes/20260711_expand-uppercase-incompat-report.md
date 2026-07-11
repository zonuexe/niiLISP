<!--
Provenance: reported from the ~/works/lambda1-grandprix λ-calculus experiment
(extending https://gist.github.com/kosh04/262332). Copied here verbatim so the
analysis is not lost with the scratchpad.

Resolution: FIXED — `(expand expr)` now auto-expands upper-case-initial symbols
bound to a non-nil value of ANY type (the manual's PROLOG form), restoring
ADR-0027. See CHANGELOG [Unreleased] "Fixed" and the church-numeral test in
`src/eval.rs`. Trade-off recorded there: PLUS/MULT on nested church numerals now
hit the gist's documented reused-variable hazard under dynamic scoping — the same
as real newLISP, which is authoritative.
-->

# niiLISP `expand` 非互換レポート（newLISP を正とする）

対象: niiLISP（Rust 実装、`cargo build --release` で構築） vs newLISP 10.7.5
文脈: λ計算トランスレータが生成する newLISP コードを niiLISP で実行したところ動かず。原因を切り分けた。

## 結論（根本原因）

**`(expand expr)` を「明示シンボル無し」で呼んだときの自動展開が niiLISP に無い。**

- newLISP: 明示シンボルを省略すると、**先頭が大文字**で現在束縛されているシンボルを自動的にその値へ展開する（暗黙モード）。
- niiLISP: 明示シンボル無しの `expand` は**何も展開せず式をそのまま返す**。
- 明示シンボル付き `(expand expr 'sym)` は**両者一致**（差異なし）。

これにより、ダイナミックスコープ下で擬似レキシカルクロージャを作る定番イディオム
`(define-macro (LAMBDA) (append (lambda) (expand (args))))`（kosh04）が niiLISP では機能しない。

## 比較マトリクス（同一入力）

| # | 入力 | newLISP（期待） | niiLISP（現象） | 判定 |
|---|------|-----------------|-----------------|------|
| A | `(let (N 5) (expand '(f N)))` | `(f 5)` | `(f N)` | **差異** |
| B | `(let (n 5) (expand '(f n)))` | `(f n)` | `(f n)` | 一致 |
| C | `(let (N 5) (expand '(f N) 'N))` | `(f 5)` | `(f 5)` | 一致 |
| D | `(let (n 5) (expand '(f n) 'n))` | `(f 5)` | `(f 5)` | 一致 |
| E | 下記 LAMBDA マクロ ＋ `((K 1) 2)` | `1` | `nil` | **差異** |

- A: 明示シンボル無し・大文字 → newLISP は自動展開、niiLISP は無展開。**これが差異の核心。**
- B: 明示シンボル無し・小文字 → 両者とも無展開（＝newLISP の自動展開は「先頭大文字」限定）。
- C/D: 明示シンボル付き → 両者一致（niiLISP の `expand` の明示モードは正しく動く）。

## ケース E（クロージャの実挙動）

入力:
```lisp
(define-macro (LAMBDA) (append (lambda) (expand (args))))
(define K (LAMBDA (X) (LAMBDA (Y) X)))
(println ((K 1) 2))
```
- newLISP（期待）: `1`
  - `(K 1)` で X=1 が束縛された状態で内側 `(LAMBDA (Y) X)` が生成され、`(expand (args))` が大文字 X を 1 に焼き込む → `(lambda (Y) 1)` → 適用して `1`。
- niiLISP（現象）: `nil`
  - `(expand (args))` が無展開のため X が焼き込まれない → `(lambda (Y) X)` のまま。適用時に X はダイナミック環境から消えており未定義 → `nil`。

## 実害（元の症状）

トランスレータの church 数ヘルパー:
```lisp
(define (encodeInt N)
  (LAMBDA (F) (LAMBDA (X)
    (let (acc X) (dotimes (i N) (set 'acc (F acc))) acc))))
```
- newLISP: 正常（N が焼き込まれ `(dotimes (i 5) ...)` になる）。
- niiLISP: `niilisp: dotimes: count must be a number`
  - N が焼き込まれず、束縛が切れた後に `dotimes` が symbol `N`（非数値）を受け取り失敗。

## 再現

```sh
# niiLISP（Rust）
cargo build --release            # -> target/release/niilisp
./target/release/niilisp exmatrix.lsp
./target/release/niilisp kmacro.lsp
# newLISP
newlisp exmatrix.lsp
newlisp kmacro.lsp
```
（`exmatrix.lsp` は A–D、`kmacro.lsp` は E の入力）

## newLISP の仕様（参考）

newLISP マニュアル `expand`: `(expand expr [sym-1 ... ])`。シンボルを省略した場合、**先頭が大文字**で現在のコンテキストに値を持つシンボルが展開される。小文字始まりのシンボルは自動展開の対象外（マトリクス B と整合）。

## niiLISP を newLISP 準拠にするための提案

`(expand expr)`（明示シンボル無し）で、**現在束縛されている「先頭大文字」のシンボルを自動的に値へ展開する**モードを実装する。これで:
- マトリクス A が `(f 5)` になる、
- ケース E が `1` になる、
- `(define-macro (LAMBDA) (append (lambda) (expand (args))))` によるクロージャ手法が動作する、
- トランスレータ生成コードが niiLISP でもそのまま緑になる。

明示モード（C/D）は既に一致しているため、追加すべきは「暗黙・大文字自動展開」のみ。
