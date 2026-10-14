import { NumberField } from "@base-ui/react/number-field";
import { CalendarIcon } from "lucide-react";
import { type ComponentProps, useEffect, useEffectEvent, useId, useRef } from "react";
import type { Labels } from "react-day-picker";
import { ja } from "react-day-picker/locale/ja";

import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useFieldContext } from "@/hooks/form-context";
import { formatCalendarDate, parseCalendarDate } from "@/lib/calendar-date";
import { normalizeFieldErrors } from "@/lib/field-errors";
import { formatCalendarDateLabel } from "@/lib/format-calendar-date-label";

/**
 * フォームの配線部品。
 * form.AppField の内側で使い、useFieldContext で field を受け取る (公式 form composition)。
 * shadcn 正典の invalid / disabled 両属性ペア (`data-invalid` + `aria-invalid`、
 * `data-disabled` + `disabled`) と、`aria-describedby` ⇄ `FieldError` の id 一致を
 * 部品内部で構造的に保証する。useFieldContext のジェネリクスは呼び出し側 AppField の
 * 値型と一致させる契約 (公式 docs と同じ trusted generic)。
 */

/**
 * fieldValue は部品内部では使わず、消費側の field.state.value を受けて値型を突き合わせる
 * ためだけに存在する。useFieldContext のジェネリクスは実フィールドと型で結びつかないため、
 * これが唯一の突き合わせ経路となる (TanStack/form#1240 のメンテナ回答)。
 * 比較した案は docs/guides/forms-and-inputs.md「`fieldValue` で値型を突き合わせる理由」。
 */
interface FieldValueTypeCheckProps<T> {
  fieldValue: T;
}

/**
 * 転送 prop の型は転送先コンポーネントの ComponentProps から Pick で導出する
 * (出処の明示 + 転送先の型変更への自動追随)。全面スプレッド (Omit + rest) は
 * controlled prop (value / onChange / id / aria-*) の上書きや Field 規約外の
 * className 直渡しの経路を開くため採らない — 許可する prop を Pick で列挙する。
 */
interface FormTextFieldProps
  extends
    Pick<
      ComponentProps<typeof Input>,
      "type" | "inputMode" | "autoFocus" | "disabled" | "placeholder"
    >,
    FieldValueTypeCheckProps<string> {
  label: string;
  /** handleChange 前に入力値を整形する (例: 数字のみに制限する) */
  sanitize?: (raw: string) => string;
}

/**
 * Text / Number / Select / Date フィールドが共有する状態導出。
 * id 2 つと invalid の計算を 1 箇所に集め、フィールド種別を増やすときの写し漏れを防ぐ。
 * (FormCheckboxField は FieldError 非対応の別形なので使わない)。errors と invalid の取り方は
 * `docs/guides/forms-and-inputs.md`「`fieldComponents` の部品を書く」
 */
function useFormFieldState<T>() {
  const field = useFieldContext<T>();
  const id = useId();
  const errorId = useId();
  const rawErrors = field.state.meta.errors;
  // warn は描画のたびではなく、描画できない検証エラーが表示されたときに出す。form-core は errorMap が
  // 変わらない限り同じ errors の配列を返すので、focus の出入りなどの描き直しでは走らない
  useEffect(() => {
    const { unrenderable } = normalizeFieldErrors(rawErrors);
    if (unrenderable.length > 0) {
      console.warn("[form-fields] 描画できない形式の検証エラーを代替文言へ丸めました", {
        errors: unrenderable,
      });
    }
  }, [rawErrors]);
  return {
    field,
    id,
    errorId,
    errors: normalizeFieldErrors(rawErrors).errors,
    invalid: !field.state.meta.isValid,
  };
}

export function FormTextField({
  label,
  placeholder,
  type,
  inputMode,
  autoFocus,
  disabled,
  sanitize,
}: FormTextFieldProps) {
  const { field, id, errorId, errors, invalid } = useFormFieldState<string>();

  return (
    <Field
      // ラベルの destructive 色は registry の Field が `data-[invalid=true]:text-destructive` で
      // 持ち、FieldLabel はそれを継承する。JS で色を足さない (ADR-0022)
      data-invalid={invalid || undefined}
      data-disabled={disabled || undefined}
    >
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Input
        id={id}
        type={type}
        inputMode={inputMode}
        // oxlint-disable-next-line jsx-a11y/no-autofocus -- 既定は無効で、消費側が明示的に渡したときだけ転送する。ダイアログ内の先頭フィールドのように妥当な場面があるかは消費側でしか判定できない
        autoFocus={autoFocus}
        disabled={disabled}
        placeholder={placeholder}
        value={field.state.value}
        onChange={(event) =>
          field.handleChange(sanitize ? sanitize(event.target.value) : event.target.value)
        }
        onBlur={field.handleBlur}
        aria-invalid={invalid}
        aria-describedby={invalid ? errorId : undefined}
      />
      <FieldError id={errorId} errors={errors} />
    </Field>
  );
}

/**
 * 数値フィールドは「空」を値として持てる必要があるため `number | null` を扱う。
 * `NumberField` の `onValueChange` が空入力を `null` で返すため、`Number("")` が 0 になる
 * 経路を通らない。
 */
interface FormNumberFieldProps
  extends
    Pick<ComponentProps<typeof NumberField.Root>, "disabled">,
    FieldValueTypeCheckProps<number | null> {
  label: string;
}

export function FormNumberField({ label, disabled }: FormNumberFieldProps) {
  const { field, id, errorId, errors, invalid } = useFormFieldState<number | null>();

  return (
    <Field data-invalid={invalid || undefined} data-disabled={disabled || undefined}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      {/* type="number" は使わない。GOV.UK Design System が利用者テストの結果として外している:
          NVDA の要素一覧で unlabeled になる、Dragon で音声入力できない、ホイールで値が
          無言に増減する。NumberField は推奨形の type="text" + inputmode="numeric" を出し、
          パースとロケール整形を自前で持つ。onValueChange が number | null をそのまま返すので、
          空文字と数値でない入力を自前で畳む処理も要らなくなる */}
      <NumberField.Root
        value={field.state.value}
        disabled={disabled}
        onValueChange={(value) => {
          field.handleChange(value);
        }}
      >
        <NumberField.Input
          id={id}
          render={<Input />}
          onBlur={field.handleBlur}
          aria-invalid={invalid}
          aria-describedby={invalid ? errorId : undefined}
        />
      </NumberField.Root>
      <FieldError id={errorId} errors={errors} />
    </Field>
  );
}

interface FormSelectFieldProps<T extends string>
  extends
    Pick<ComponentProps<typeof Select>, "disabled">,
    Pick<ComponentProps<typeof SelectValue>, "placeholder">,
    FieldValueTypeCheckProps<T> {
  label: string;
  options: readonly { value: T; label: string }[];
}

/**
 * 選んでいた値が候補から消えたことを、Base UI の `onValueChange(null)` で検出しない。
 * 値の解決はこの部品が引き取り、`null` と options に無い値は表示を保ったまま warn に残す。
 * 候補から消えた値の保存は止めない。`options` が描画中に変わりうるとき (query や別の
 * フィールドから来るとき) は、消費側が `form.AppField` の validators で止める
 * (docs/guides/forms-and-inputs.md「Select の値を解決する」)。
 *
 * Base UI の自己リセットは公式 docs の Select に書かれていない (同梱の docs を `reset` /
 * `onValueChange` で検索、2026-09-23、1.8.0)。実装は `select/positioner/SelectPositioner.mjs`
 * の `onMapChange` にあり、1.8.0 のソース上の扱いは次のとおり (読み取りで、挙動は未実測)。
 *
 * | 条件                                                             | ソース上の扱い                        |
 * | ---------------------------------------------------------------- | ------------------------------------- |
 * | 項目が 1 件も登録されていない (`valuesRef.current.length === 0`) | 何もしない                            |
 * | 初回の登録 (`prevSize === 0`)                                    | 何もしない                            |
 * | 単一選択で現在値が `null`                                        | 何もしない                            |
 * | 現在値が候補に無く、マウント時の値が候補にある                   | マウント時の値へ戻す。`null` は来ない |
 * | 現在値もマウント時の値も候補に無い                               | `null` で `setValue` する             |
 *
 * 通知が来ない条件がある。項目の登録の変化を拾う `CompositeList` は件数に加えて要素の同一性も
 * 比べ、件数が変わらないときに何もしない分岐は 1.8.0 で撤去された (CHANGELOG v1.8.0、
 * mui/base-ui#5469)。版ごとに経路が変わるので、自己リセットに任せる案は採らない。
 * 項目がいつ登録されるか (トリガーを一度もフォーカスしていない間は登録されないか) は未確認。
 * この部品を包まずに `Select` を使うときの書き方は
 * docs/guides/forms-and-inputs.md「Select の値を解決する」。
 *
 * 出典: Base UI Select (https://base-ui.com/react/components/select)、CHANGELOG
 * (https://github.com/mui/base-ui/blob/master/CHANGELOG.md)、mui/base-ui#5469
 * (https://github.com/mui/base-ui/pull/5469)、`SelectPositioner` の実装
 * (https://github.com/mui/base-ui/blob/master/packages/react/src/select/positioner/SelectPositioner.tsx)
 */
export function FormSelectField<T extends string>({
  label,
  options,
  placeholder,
  disabled,
}: FormSelectFieldProps<T>) {
  const { field, id, errorId, errors, invalid } = useFormFieldState<T>();
  const labelId = useId();

  return (
    <Field data-invalid={invalid || undefined} data-disabled={disabled || undefined}>
      <FieldLabel id={labelId} htmlFor={id}>
        {label}
      </FieldLabel>
      <Select
        value={field.state.value}
        onValueChange={(value) => {
          // 候補が入れ替わったとき Base UI は現在値を null で通知してくることがある。この
          // 自己リセットに委ねると form の値が黙って消えるため、値の解決はここで引き取る。
          if (value === null) {
            console.warn("[FormSelectField] 候補から現在値が消えました。値は保持します", {
              currentValue: field.state.value,
              options,
            });
            return;
          }

          // options との突合により、手書き型ガードなしで union 値を構造的に narrow する。
          const selected = options.find((option) => option.value === value);
          if (selected === undefined) {
            // SelectItem は options だけから描画するため通常は到達しない。表示は壊さず、
            // Base UI との配線不整合を検出できるよう値と突合元を fail-loud に残す。
            console.warn("[FormSelectField] options にない値を受け取りました", {
              value,
              options,
            });
            return;
          }
          field.handleChange(selected.value);
        }}
        disabled={disabled}
        // Base UI の items は ReadonlyArray<{ label, value }> を直接受ける
        // (SelectRoot.d.ts:111)。options の安定性は消費側 (useMemo / モジュール定数) の責務。
        items={options}
      >
        {/* trigger は button の combobox なので、見える label を aria-labelledby で指す
            (docs/guides/accessibility.md「accessible name を与える」) */}
        <SelectTrigger
          id={id}
          className="w-full"
          aria-labelledby={labelId}
          onBlur={field.handleBlur}
          aria-invalid={invalid}
          aria-describedby={invalid ? errorId : undefined}
        >
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent aria-labelledby={labelId}>
          <SelectGroup>
            {options.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
      <FieldError id={errorId} errors={errors} />
    </Field>
  );
}

interface FormCheckboxFieldProps
  extends Pick<ComponentProps<typeof Checkbox>, "disabled">, FieldValueTypeCheckProps<boolean> {
  label: string;
}

export function FormCheckboxField({ label, disabled }: FormCheckboxFieldProps) {
  const field = useFieldContext<boolean>();
  const id = useId();
  const labelId = useId();
  const errors = field.state.meta.errors;

  // 検証を持たない真偽値フィールド専用なので、正規の利用では errors は常に空で
  // data-invalid / FieldError は不要。誤って validators を付けた場合は、表示できない
  // エラーで submit が止まったことを追跡できるよう fail-loud に警告する。
  // horizontal variant の FieldLabel 幅は direct child selector に依存する。FieldError のために
  // FieldLabel を FieldContent で包むと layout が変わるので、検証つき checkbox が必要になったら
  // この direct child 制約の解決から始める。
  // warn を出す契機は errors の変化だけで、label は出したときの値を読めば足りる (`docs/guides/react/effects.md`「依存に置く値と Effect Event で読む値」)
  const warnUnrenderable = useEffectEvent((unrenderable: readonly unknown[]) => {
    console.warn("[FormCheckboxField] 検証エラーを表示できません (FieldError 非対応)", {
      label,
      errors: unrenderable,
    });
  });
  useEffect(() => {
    if (errors.length > 0) {
      warnUnrenderable(errors);
    }
  }, [errors]);

  // registry の horizontal Field は子の FieldLabel を flex-auto で伸ばし、Field 自身は w-full なので、
  // フォームの幅いっぱいまでラベルが伸びて右の余白でもトグルする。shadcn の単独チェックボックスの例は
  // 器の幅を w-56 に絞っており、行を中身の幅に縮めるのはその形に合わせたもの
  return (
    <Field orientation="horizontal" className="w-fit" data-disabled={disabled || undefined}>
      {/* Base UI の Checkbox は htmlFor の label からも名前を写すが、写すのはクライアントだけなので
          aria-labelledby で指す (docs/guides/accessibility.md「accessible name を与える」) */}
      <Checkbox
        id={id}
        checked={field.state.value}
        disabled={disabled}
        aria-labelledby={labelId}
        onCheckedChange={field.handleChange}
      />
      <FieldLabel id={labelId} htmlFor={id}>
        {label}
      </FieldLabel>
    </Field>
  );
}

/**
 * Calendar のラベルの上書き。日本語のラベルは locale に渡す react-day-picker の ja が持ち、ここでは
 * nav の名前 (labelNav) だけを変える。ja の「ナビゲーションバー」は、nav の名前に「ナビゲーション」を
 * 含めない規範 (docs/guides/accessibility.md「ナビゲーションを組む」) に合わない。上書きは locale を
 * 組み直さず、react-day-picker docs「Advanced Translations」の Override ARIA labels のとおり labels で渡す。
 * 月の表の名前は labelGrid (aria-label) のままにする。同じ年月は見出しの文字として DOM にもあり、
 * WAI-ARIA 1.2 の aria-label の定義は見出しを aria-labelledby で指す形を勧める (SHOULD) が、その形にしない。
 * 見出しに id を付けて指すと、animate のとき react-day-picker が月の DOM を cloneNode で複製して id が重なり
 * (`useAnimation.js`)、表が前の月の見出しで名付けられうる。aria-labelledby が効く間は、消費側が渡す
 * labelGrid も無視される (react-day-picker 10.0.2、2026-10-07 に確認)。名前の付け方の規範は
 * docs/guides/accessibility.md「accessible name を与える」にある。
 */
const CALENDAR_LABELS: Partial<Labels> = {
  labelNav: () => "月の切り替え",
};

interface FormDateFieldProps
  extends
    Pick<ComponentProps<typeof PopoverTrigger>, "disabled">,
    FieldValueTypeCheckProps<string | null> {
  label: string;
  /** 値が無いときにトリガーへ出す文言 (例: 期日なし) */
  emptyText: string;
}

/**
 * 暦の日付 (`YYYY-MM-DD`、ADR-0031) を Calendar で選ぶフィールド。値が無いことを null で持つ。
 * 組み方は shadcn docs「Date Picker」の Popover + Calendar + Button。`Date` との変換は
 * `src/lib/calendar-date.ts` が行い、form の値に `Date` を入れない。
 *
 * - 選択中の日をもう一度押すと外れる。Calendar に `required` を付けない (react-day-picker docs「Single Mode」)
 * - Popover の中に解除のボタンも置く。日を選んでも Popover は閉じない (shadcn の例と同じ非制御)
 * - 検証の blur は、開かずにトリガーを通り過ぎたときと、Popover を閉じたときに起こす。開いて
 *   フォーカスが popup へ移るときの blur では起こさない。日を選んでいる最中にエラーを出さない
 * - トリガーの名前はラベルと表示中の値をつないで作る。`htmlFor` だけだと button の名前がラベルに
 *   なり、選んだ日が読み上げに出ない
 * - shadcn の例の `p-0` (Popup) と文字色・太さ (トリガー) は付けない。`parts/` からは layout の
 *   class しか渡せない (ADR-0011)
 */
export function FormDateField({ label, emptyText, disabled }: FormDateFieldProps) {
  const { field, id, errorId, errors, invalid } = useFormFieldState<string | null>();
  const labelId = useId();
  const valueId = useId();
  // 開いている間の blur (フォーカスが popup へ移るとき) は検証の契機にしない。描画には使わない
  // ので state ではなく ref で持つ
  const openRef = useRef(false);
  const value = field.state.value;
  const selected = value === null ? undefined : parseCalendarDate(value);

  return (
    <Field data-invalid={invalid || undefined} data-disabled={disabled || undefined}>
      <FieldLabel id={labelId} htmlFor={id}>
        {label}
      </FieldLabel>
      <Popover
        onOpenChange={(open) => {
          openRef.current = open;
          if (!open) {
            field.handleBlur();
          }
        }}
      >
        <PopoverTrigger
          id={id}
          disabled={disabled}
          render={<Button variant="outline" className="w-full justify-start" />}
          onBlur={() => {
            if (!openRef.current) {
              field.handleBlur();
            }
          }}
          aria-labelledby={`${labelId} ${valueId}`}
          aria-invalid={invalid}
          aria-describedby={invalid ? errorId : undefined}
        >
          <CalendarIcon aria-hidden />
          <span id={valueId}>
            {value === null ? emptyText : formatCalendarDateLabel(value, "long")}
          </span>
        </PopoverTrigger>
        {/* popup は role="dialog" になる。Popover.Title を置かないので、名前はラベルから取る */}
        <PopoverContent align="start" className="w-auto" aria-labelledby={labelId}>
          <Calendar
            mode="single"
            selected={selected}
            // 開いたら選択中の日、無ければ今日へフォーカスを移す (react-day-picker の autoFocus)。
            // Popover の既定の移し先 (最初の tabbable の「前の月へ」) に上書きされないことは
            // form-fields.test.tsx のキーボード操作のテストが見る
            // oxlint-disable-next-line jsx-a11y/no-autofocus -- DOM の autofocus ではなく react-day-picker の prop。Calendar は利用者が Popover を開いたときにだけ mount され、react-day-picker の型定義はユーザー操作の後に表示する場合に使うよう勧める
            autoFocus
            // 開く月は選択中の日の月。無ければ今日の月 (react-day-picker の getInitialMonth)
            defaultMonth={selected}
            onSelect={(date) => {
              field.handleChange(date === undefined ? null : formatCalendarDate(date));
            }}
            locale={ja}
            labels={CALENDAR_LABELS}
          />
          <Button
            type="button"
            variant="ghost"
            disabled={value === null}
            // キーボードで押して無効になっても、フォーカスをこのボタンに残す (Base UI Button docs)
            focusableWhenDisabled
            onClick={() => {
              field.handleChange(null);
            }}
          >
            {`${label}をクリア`}
          </Button>
        </PopoverContent>
      </Popover>
      <FieldError id={errorId} errors={errors} />
    </Field>
  );
}
