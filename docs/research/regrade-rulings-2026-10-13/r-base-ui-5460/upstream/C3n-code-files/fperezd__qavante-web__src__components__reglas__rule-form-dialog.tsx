"use client";

import * as React from "react";
import { Dialog } from "@base-ui/react/dialog";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { X } from "lucide-react";
import { QavanteButton, QavanteInput } from "@/components/qavante";
import { cn } from "@/lib/utils";
import { ApiError } from "@/lib/api/errors";
import { apiErrorToUserMessage } from "@/lib/api/error-messages";
import {
  useCreateClassificationRule,
  useUpdateClassificationRule,
  type ClassificationRule,
  type SuggestRuleResponse,
} from "@/lib/api/classification-rules";
import { useCanonicalCategories } from "@/lib/api/treasury";
import {
  CONDITION_FIELDS,
  CONFIDENCE_STEPS,
  OPERATORS,
  formToCreateRequest,
  formToUpdateRequest,
  ruleFormSchema,
  ruleToForm,
  suggestionToFormValues,
  type RuleFormValues,
} from "./rule-form-schema";

/* Editor de regla de clasificación — Addendum §17 + §20 (POST/PATCH owner/admin).
   Patrón establecido: Base UI Dialog + react-hook-form + zod (igual que
   invite-user-dialog y currency-settings-dialog). Lazy-loaded desde la view
   (next/dynamic, ssr:false) para no inflar First Load JS — admin-only.

   El mismo dialog cubre los dos casos: crear (rule=null) y editar (rule
   presente). Schema + transforms en `./rule-form-schema` (sin React,
   testeable en vitest unit). El backend re-valida (es la verdad). 403 si
   rol sin permiso (§20) — se renderiza via apiErrorToUserMessage. */

const FIELD_LABEL: Record<string, string> = {
  description: "Glosa",
  counterparty_name: "Contraparte",
  reference: "Referencia",
  amount: "Monto",
  currency_code: "Moneda",
  bank_account_id: "Cuenta bancaria",
};

const OPERATOR_LABEL: Record<string, string> = {
  equals: "es igual a",
  contains: "contiene",
  starts_with: "empieza con",
  ends_with: "termina con",
  regex: "matchea regex",
  greater_than: "mayor a",
  less_than: "menor a",
};

export interface RuleFormDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Si viene, es edit; si no (null), es create. */
  rule: ClassificationRule | null;
  /** Sugerencia §18.7 (read-only) para pre-poblar el form en modo create.
   *  El dialog hace el transform internamente — el caller pasa el response
   *  crudo del backend. Ignorado si `rule` !== null. Mantener el transform
   *  acá (vs en el caller) evita arrastrar el schema + zod al chunk del
   *  caller cuando el dialog ya es lazy. */
  suggestion?: SuggestRuleResponse | null;
}

const selectClass = cn(
  "flex h-10 w-full rounded-md border border-neutral-light bg-surface px-3 py-2 text-sm text-neutral-dark",
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-primary",
);

function resolveInitial(
  rule: ClassificationRule | null,
  suggestion?: SuggestRuleResponse | null,
): RuleFormValues {
  const base = ruleToForm(rule);
  /* Solo aplica la sugerencia en modo create (rule === null). En edit el
     snapshot de la regla manda — no queremos que una sugerencia pise
     valores reales de la regla. */
  if (rule || !suggestion) return base;
  return { ...base, ...suggestionToFormValues(suggestion) };
}

export function RuleFormDialog({ open, onOpenChange, rule, suggestion }: RuleFormDialogProps) {
  const isEdit = Boolean(rule);
  const create = useCreateClassificationRule();
  const update = useUpdateClassificationRule();
  const categoriesQuery = useCanonicalCategories();
  const [submitError, setSubmitError] = React.useState<string | null>(null);

  const {
    control,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<RuleFormValues>({
    resolver: zodResolver(ruleFormSchema),
    defaultValues: resolveInitial(rule, suggestion),
    mode: "onBlur",
  });

  /* Re-sincronizar el form si cambia el snapshot (post-mutación, al abrir
     en modo edit con otra regla, o al recibir una sugerencia distinta). */
  React.useEffect(() => {
    if (open) {
      reset(resolveInitial(rule, suggestion));
      setSubmitError(null);
    }
  }, [open, rule, suggestion, reset]);

  async function onSubmit(values: RuleFormValues) {
    setSubmitError(null);
    try {
      if (isEdit && rule) {
        await update.mutateAsync({ ruleId: rule.id, body: formToUpdateRequest(values) });
      } else {
        await create.mutateAsync(formToCreateRequest(values));
      }
      onOpenChange(false);
    } catch (err) {
      if (err instanceof ApiError) {
        setSubmitError(apiErrorToUserMessage(err));
      } else {
        setSubmitError("Error inesperado. Reintenta.");
      }
    }
  }

  const title = isEdit ? "Editar regla" : "Nueva regla de clasificación";
  const categories = categoriesQuery.data?.items ?? [];

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Backdrop className="fixed inset-0 z-40 bg-neutral-dark/40 backdrop-blur-sm data-[open]:animate-in data-[closed]:animate-out" />
        <Dialog.Popup className="fixed left-1/2 top-1/2 z-50 max-h-[90vh] w-full max-w-lg -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-lg bg-surface p-6 shadow-xl">
          <div className="mb-4 flex items-start justify-between">
            <Dialog.Title className="text-lg font-semibold text-neutral-dark">{title}</Dialog.Title>
            <Dialog.Close
              aria-label="Cerrar"
              className="rounded-md p-1 text-neutral-mid hover:bg-neutral-light/40 hover:text-neutral-dark"
            >
              <X className="h-5 w-5" />
            </Dialog.Close>
          </div>
          <Dialog.Description className="mb-4 text-sm text-neutral-mid">
            Las reglas clasifican automáticamente los movimientos similares en el futuro. Se evalúan
            de menor a mayor prioridad (1 corre antes que 100). Qavante nunca borra reglas, puedes
            desactivarlas si dejan de servir.
          </Dialog.Description>

          <form onSubmit={handleSubmit(onSubmit)} noValidate className="space-y-4">
            <div className="space-y-1">
              <label htmlFor="rule-name" className="text-sm font-medium text-neutral-dark">
                Nombre
              </label>
              <Controller
                control={control}
                name="name"
                render={({ field }) => (
                  <QavanteInput
                    id="rule-name"
                    placeholder="Ej: Sueldo Fernando"
                    value={field.value}
                    onValueChange={field.onChange}
                    onBlur={field.onBlur}
                    invalid={Boolean(errors.name)}
                    aria-required="true"
                    aria-describedby={errors.name ? "rule-name-error" : undefined}
                  />
                )}
              />
              {errors.name && (
                <p id="rule-name-error" className="text-xs text-danger-500" role="alert">
                  {errors.name.message}
                </p>
              )}
            </div>

            <fieldset className="space-y-2 rounded-md border border-neutral-light bg-neutral-light/20 p-3">
              <legend className="px-1 text-xs font-medium uppercase tracking-wide text-neutral-mid">
                Condición
              </legend>
              <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                <div className="space-y-1">
                  <label htmlFor="rule-field" className="text-sm font-medium text-neutral-dark">
                    Campo
                  </label>
                  <Controller
                    control={control}
                    name="condition_field"
                    render={({ field }) => (
                      <select
                        id="rule-field"
                        value={field.value}
                        onChange={(e) => field.onChange(e.target.value)}
                        aria-invalid={Boolean(errors.condition_field) || undefined}
                        aria-required="true"
                        className={cn(selectClass, errors.condition_field && "border-danger-500")}
                      >
                        {CONDITION_FIELDS.map((f) => (
                          <option key={f} value={f}>
                            {FIELD_LABEL[f]}
                          </option>
                        ))}
                      </select>
                    )}
                  />
                </div>
                <div className="space-y-1">
                  <label htmlFor="rule-operator" className="text-sm font-medium text-neutral-dark">
                    Operador
                  </label>
                  <Controller
                    control={control}
                    name="operator"
                    render={({ field }) => (
                      <select
                        id="rule-operator"
                        value={field.value}
                        onChange={(e) => field.onChange(e.target.value)}
                        aria-invalid={Boolean(errors.operator) || undefined}
                        aria-required="true"
                        className={cn(selectClass, errors.operator && "border-danger-500")}
                      >
                        {OPERATORS.map((o) => (
                          <option key={o} value={o}>
                            {OPERATOR_LABEL[o]}
                          </option>
                        ))}
                      </select>
                    )}
                  />
                </div>
              </div>
              <div className="space-y-1">
                <label htmlFor="rule-value" className="text-sm font-medium text-neutral-dark">
                  Valor
                </label>
                <Controller
                  control={control}
                  name="condition_value"
                  render={({ field }) => (
                    <QavanteInput
                      id="rule-value"
                      placeholder="Ej: SUELDO FERNANDO"
                      value={field.value}
                      onValueChange={field.onChange}
                      onBlur={field.onBlur}
                      invalid={Boolean(errors.condition_value)}
                      aria-required="true"
                      aria-describedby={errors.condition_value ? "rule-value-error" : undefined}
                    />
                  )}
                />
                {errors.condition_value && (
                  <p id="rule-value-error" className="text-xs text-danger-500" role="alert">
                    {errors.condition_value.message}
                  </p>
                )}
              </div>
            </fieldset>

            <div className="space-y-1">
              <label htmlFor="rule-category" className="text-sm font-medium text-neutral-dark">
                Categoría canónica <span className="text-neutral-mid">(opcional)</span>
              </label>
              <Controller
                control={control}
                name="canonical_category"
                render={({ field }) => (
                  <select
                    id="rule-category"
                    value={field.value}
                    onChange={(e) => field.onChange(e.target.value)}
                    className={selectClass}
                    disabled={categoriesQuery.isLoading}
                  >
                    <option value="">Sin categoría</option>
                    {categories.map((c) => (
                      <option key={c.code} value={c.code}>
                        {c.label}
                      </option>
                    ))}
                  </select>
                )}
              />
              {categoriesQuery.isError && (
                <p className="text-xs text-warning-700" role="alert">
                  No pudimos cargar las categorías, vas a poder guardar la regla sin ella.
                </p>
              )}
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1">
                <label htmlFor="rule-priority" className="text-sm font-medium text-neutral-dark">
                  Prioridad
                </label>
                <Controller
                  control={control}
                  name="priority"
                  render={({ field }) => (
                    <input
                      id="rule-priority"
                      type="number"
                      min={1}
                      max={1000}
                      step={1}
                      value={field.value}
                      onChange={(e) => field.onChange(e.target.valueAsNumber)}
                      onBlur={field.onBlur}
                      aria-invalid={Boolean(errors.priority) || undefined}
                      aria-required="true"
                      aria-describedby={
                        errors.priority ? "rule-priority-error" : "rule-priority-hint"
                      }
                      className={cn(
                        "flex h-10 w-full rounded-md border bg-surface px-3 py-2 text-sm text-neutral-dark",
                        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-primary",
                        errors.priority ? "border-danger-500" : "border-neutral-light",
                      )}
                    />
                  )}
                />
                <p id="rule-priority-hint" className="text-xs text-neutral-mid">
                  Menor = corre antes (§17.6).
                </p>
                {errors.priority && (
                  <p id="rule-priority-error" className="text-xs text-danger-500" role="alert">
                    {errors.priority.message}
                  </p>
                )}
              </div>
              <div className="space-y-1">
                <label htmlFor="rule-confidence" className="text-sm font-medium text-neutral-dark">
                  Confianza
                </label>
                <Controller
                  control={control}
                  name="confidence"
                  render={({ field }) => (
                    <select
                      id="rule-confidence"
                      value={field.value}
                      onChange={(e) => field.onChange(parseFloat(e.target.value))}
                      className={selectClass}
                    >
                      {CONFIDENCE_STEPS.map((c) => (
                        <option key={c} value={c}>
                          {Math.round(c * 100)}%
                        </option>
                      ))}
                    </select>
                  )}
                />
                <p className="text-xs text-neutral-mid">Qué tan seguro es el match.</p>
              </div>
            </div>

            {submitError && (
              <div
                role="alert"
                className="rounded-xl border border-danger-500/40 bg-danger-500/10 p-3 text-sm text-danger-500"
              >
                {submitError}
              </div>
            )}

            <div className="flex justify-end gap-2 pt-2">
              <Dialog.Close render={<QavanteButton variant="ghost" type="button" />}>
                Cancelar
              </Dialog.Close>
              <QavanteButton type="submit" loading={isSubmitting}>
                {isEdit ? "Guardar cambios" : "Crear regla"}
              </QavanteButton>
            </div>
          </form>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
