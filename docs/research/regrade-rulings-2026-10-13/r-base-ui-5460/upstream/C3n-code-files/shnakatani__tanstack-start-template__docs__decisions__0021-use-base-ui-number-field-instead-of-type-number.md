# ADR-0021: 数値入力に `type="number"` を使わず Base UI の NumberField に寄せる

- Status: Accepted
- Date: 2026-09-02

## Context

`type="number"` は数値入力の素直な選択に見えるが、支援技術と入力手段で既知の不具合を抱える。
GOV.UK Design System は利用者テストの結果として number パターンからこれを外し、`type="text"` + `inputmode="numeric"` を推奨形にしている。

| 症状                                                             | 影響                                     |
| ---------------------------------------------------------------- | ---------------------------------------- |
| NVDA の要素一覧で unlabeled として並ぶ                           | スクリーンリーダーでフィールドを探せない |
| Dragon NaturallySpeaking で音声入力できない                      | 音声操作の利用者が入力できない           |
| 入力欄にカーソルを置いたままページをスクロールすると値が増減する | **値が無言で書き換わる**                 |

3 つめはこのリポジトリが一貫して潰してきた silent failure に当たる。利用者は変わったことに気付かず、検証も通る。

実装側にも負債があった。`type="number"` は数値に解釈できない入力を `value=""` + `validity.badInput` で返し、`Number("")` は 0 を返す。
そのため commit する値を決める自前の関数を持ち、空入力とパース不能を `null` へ畳んでいた。

## Decision

**数値入力は `@base-ui/react` の `NumberField` を使う。`type="number"` は使わない。**

`NumberField.Input` が出す DOM を実測した (2026-09-02、`@base-ui/react` 1.7.0)。

```html
<input
  inputmode="numeric"
  autocomplete="off"
  spellcheck="false"
  aria-roledescription="Number field"
  type="text"
  value="3"
/>
```

推奨形と一致する。パースとロケール整形は `NumberField` が担い、`onValueChange` が `number | null` をそのまま返すため、自前の畳み込みは要らなくなる。

`Field` / `FieldLabel` / `FieldError` の構成と見た目は変えない。組み方は `docs/guides/forms-and-inputs.md`「数値の入力欄を組む」にある。

### 検討した選択肢

| 案                                       | 評価                                                                                            | 採否     |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------- | -------- |
| `NumberField` へ移す                     | 推奨形の DOM を出し、パースとロケール整形を持つ。`@base-ui/react` は既存の依存で追加はゼロ      | **採用** |
| `type="number"` を続ける                 | 上表の 3 症状が残る。自前のパースも維持することになる                                           | 却下     |
| `type="text"` + `inputMode` を自前で組む | 推奨形の DOM にはなるが、パースとロケール整形を自前で持つ。`NumberField` と同じものを再実装する | 却下     |
| shadcn registry の number-field を入れる | `base-vega` に存在しない (`/r/styles/base-vega/number-field.json` が 404、2026-09-02 確認)      | 却下     |

## Consequences

- ロールが `spinbutton` から `textbox` へ変わる。テストでの取り方は `docs/guides/testing/user-interactions.md`「入力部品を操作する」にある
- ブラウザの `validity.badInput` は起きない。数値として読める前置部分が採られ、表示も commit した値へ正規化される (`2e` → `2`)。入力欄の表示と form の値が食い違ったまま残ることはない
- `NumberField` は `role="spinbutton"` も `aria-valuenow` も付けない。上表の不具合を避ける代償として、値の範囲を ARIA で伝える経路は無くなる。範囲を伝える必要があるフィールドでは `Field` の説明文で補う
- Base UI が推奨形をやめたときは再評価する。判断の根拠は上表の 3 症状であって、ライブラリの選択ではない
- 上流で `role="spinbutton"` と `aria-valuemin` / `aria-valuemax` を持たせる案が議論されている (mui/base-ui#4226、2026-09-02 時点 open)。入れば上の代償が消えるので、close したら本 ADR の Consequences を見直す

## 出典

- GOV.UK Design System が `type="number"` を外した経緯: https://technology.blog.gov.uk/2020/02/24/why-the-gov-uk-design-system-team-changed-the-input-type-for-numbers/
- Base UI Number Field: https://base-ui.com/react/components/number-field
- NumberField へ min/max と spinbutton ロールを持たせる議論: https://github.com/mui/base-ui/issues/4226
