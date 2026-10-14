import { Field } from "@base-ui/react/field";
import { MinusIcon, PlusIcon } from "lucide-react";
import { useState } from "react";
import { Controller, useFieldArray, useFormContext, useWatch } from "react-hook-form";
import type { z } from "zod";
import { IconButton } from "#/components/icon-button";
import { ImageUpload } from "#/components/image-upload";
import { MaterialCombobox } from "#/components/material-combobox";
import { OperationCombobox } from "#/components/operation-combobox";
import * as StyledField from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from "#/components/ui/input-group";
import { materialsListQueryOptions } from "#/lib/query-options";
import { useOfflineAwareQuery } from "#/lib/use-offline-aware-query";
import type { budgetFormSchema } from "#/server/functions/budgets";

type BudgetFormValues = z.infer<typeof budgetFormSchema>;

type BudgetFormFieldsProps = {
  imageUrl: string | null;
  onFileSelect?: (file: File | null) => void;
  onDeleteImage?: () => void;
};

export function BudgetFormFields({ imageUrl, onFileSelect, onDeleteImage }: BudgetFormFieldsProps) {
  const { control, setValue } = useFormContext<BudgetFormValues>();
  const deleteImage = useWatch({ control, name: "deleteImage" });

  const {
    fields: materialFields,
    append: appendMaterial,
    remove: removeMaterial,
  } = useFieldArray({ control, name: "materials" });

  const {
    fields: operationFields,
    append: appendOperation,
    remove: removeOperation,
  } = useFieldArray({ control, name: "operations" });

  const [draftMaterial, setDraftMaterial] = useState({ materialId: "", quantity: "" });
  const draftMaterialValid =
    draftMaterial.materialId !== "" && draftMaterial.quantity.trim() !== "";

  function commitMaterial() {
    if (!draftMaterialValid) return;
    appendMaterial(draftMaterial);
    setDraftMaterial({ materialId: "", quantity: "" });
  }

  const [draftOperation, setDraftOperation] = useState({ operationId: "", durationMinutes: 0 });
  const draftOperationValid =
    draftOperation.operationId !== "" && draftOperation.durationMinutes > 0;

  function commitOperation() {
    if (!draftOperationValid) return;
    appendOperation(draftOperation);
    setDraftOperation({ operationId: "", durationMinutes: 0 });
  }

  return (
    <>
      <StyledField.FieldGroup>
        <Controller
          name="name"
          control={control}
          render={({
            field: { name, ref, value, onBlur, onChange },
            fieldState: { invalid, error },
          }) => (
            <Field.Root name={name} invalid={invalid}>
              <Field.Label render={<StyledField.FieldLabel />}>Nombre</Field.Label>
              <Field.Control
                value={value}
                onBlur={onBlur}
                onValueChange={onChange}
                ref={ref}
                placeholder="Ej: Vestido de novia tipo A"
                required
                render={<Input />}
              />
              <Field.Error render={<StyledField.FieldError />}>{error?.message}</Field.Error>
            </Field.Root>
          )}
        />

        <Controller
          name="description"
          control={control}
          render={({
            field: { name, ref, value, onBlur, onChange },
            fieldState: { invalid, error },
          }) => (
            <Field.Root name={name} invalid={invalid}>
              <Field.Label render={<StyledField.FieldLabel />}>Descripción</Field.Label>
              <Field.Control
                value={value}
                onBlur={onBlur}
                onValueChange={onChange}
                ref={ref}
                placeholder="Opcional"
                render={<Input />}
              />
              <Field.Error render={<StyledField.FieldError />}>{error?.message}</Field.Error>
            </Field.Root>
          )}
        />

        <Controller
          name="hourlyRate"
          control={control}
          render={({
            field: { name, ref, value, onBlur, onChange },
            fieldState: { invalid, error },
          }) => (
            <Field.Root name={name} invalid={invalid}>
              <Field.Label render={<StyledField.FieldLabel />}>Tarifa horaria ($)</Field.Label>
              <Field.Control
                value={value}
                onBlur={onBlur}
                onValueChange={onChange}
                ref={ref}
                type="number"
                step="0.01"
                placeholder="0.00"
                required
                render={<Input />}
              />
              <Field.Error render={<StyledField.FieldError />}>{error?.message}</Field.Error>
            </Field.Root>
          )}
        />

        <div className="space-y-1.5">
          <span className="font-medium text-sm">Imagen</span>
          <ImageUpload
            initialImage={deleteImage ? null : (imageUrl ?? null)}
            onClear={() => {
              onFileSelect?.(null);
              onDeleteImage?.();
            }}
            onFileSelect={(file) => {
              onFileSelect?.(file);
            }}
          />
        </div>
      </StyledField.FieldGroup>

      {/* Materials */}
      <div className="space-y-2">
        <span className="font-medium text-sm">Materiales</span>

        {materialFields.map((field, i) => (
          <div key={field.id} className="flex items-center gap-2">
            <Controller
              name={`materials.${i}.materialId`}
              control={control}
              render={({ field: { value, onChange } }) => (
                <MaterialCombobox value={value} onChange={onChange} />
              )}
            />
            <MaterialQuantityField index={i} />
            <IconButton
              label="Quitar material"
              type="button"
              variant="ghost"
              onClick={() => removeMaterial(i)}
            >
              <MinusIcon />
            </IconButton>
          </div>
        ))}

        <div className="flex items-center gap-2">
          <MaterialCombobox
            value={draftMaterial.materialId}
            onChange={(materialId) => setDraftMaterial((d) => ({ ...d, materialId }))}
          />
          <DraftMaterialQuantityField
            materialId={draftMaterial.materialId}
            value={draftMaterial.quantity}
            onChange={(quantity) => setDraftMaterial((d) => ({ ...d, quantity }))}
            onEnter={commitMaterial}
          />
          <IconButton
            label="Agregar material"
            type="button"
            variant="ghost"
            disabled={!draftMaterialValid}
            onClick={commitMaterial}
          >
            <PlusIcon />
          </IconButton>
        </div>
      </div>

      {/* Operations */}
      <div className="space-y-2">
        <span className="font-medium text-sm">Operaciones</span>

        {operationFields.map((field, i) => (
          <div key={field.id} className="flex items-center gap-2">
            <Controller
              name={`operations.${i}.operationId`}
              control={control}
              render={({ field: { value, onChange } }) => (
                <OperationCombobox
                  value={value}
                  onChange={(operationId, defaultDurationMinutes) => {
                    onChange(operationId);
                    setValue(`operations.${i}.durationMinutes`, defaultDurationMinutes);
                  }}
                />
              )}
            />
            <Controller
              name={`operations.${i}.durationMinutes`}
              control={control}
              render={({ field: { value, onChange, onBlur, ref } }) => (
                <InputGroup className="w-28">
                  <InputGroupInput
                    type="number"
                    placeholder="Minutos"
                    value={value || ""}
                    onBlur={onBlur}
                    ref={ref}
                    onChange={(e) => onChange(Number(e.target.value) || 0)}
                  />
                  <InputGroupAddon align="inline-end">
                    <InputGroupText>min</InputGroupText>
                  </InputGroupAddon>
                </InputGroup>
              )}
            />
            <IconButton
              label="Quitar operación"
              type="button"
              variant="ghost"
              onClick={() => removeOperation(i)}
            >
              <MinusIcon />
            </IconButton>
          </div>
        ))}

        <div className="flex items-center gap-2">
          <OperationCombobox
            value={draftOperation.operationId}
            onChange={(operationId, defaultDurationMinutes) =>
              setDraftOperation({ operationId, durationMinutes: defaultDurationMinutes })
            }
          />
          <InputGroup className="w-28">
            <InputGroupInput
              type="number"
              placeholder="Minutos"
              value={draftOperation.durationMinutes || ""}
              onChange={(e) =>
                setDraftOperation((d) => ({ ...d, durationMinutes: Number(e.target.value) || 0 }))
              }
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  commitOperation();
                }
              }}
            />
            <InputGroupAddon align="inline-end">
              <InputGroupText>min</InputGroupText>
            </InputGroupAddon>
          </InputGroup>
          <IconButton
            label="Agregar operación"
            type="button"
            variant="ghost"
            disabled={!draftOperationValid}
            onClick={commitOperation}
          >
            <PlusIcon />
          </IconButton>
        </div>
      </div>
    </>
  );
}

type DraftMaterialQuantityFieldProps = {
  materialId: string;
  value: string;
  onChange: (value: string) => void;
  onEnter: () => void;
};

/** Quantity input for the draft (not-yet-added) material row. */
function DraftMaterialQuantityField({
  materialId,
  value,
  onChange,
  onEnter,
}: DraftMaterialQuantityFieldProps) {
  const { data } = useOfflineAwareQuery(materialsListQueryOptions({ page: 1, pageSize: 1000 }));
  const catalogMaterials = data?.items ?? [];
  const unit = catalogMaterials.find((m) => m.id === materialId)?.unit ?? "";

  return (
    <InputGroup className="w-40">
      <InputGroupInput
        type="number"
        step="0.01"
        placeholder="Cantidad"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            onEnter();
          }
        }}
      />
      <InputGroupAddon align="inline-end">
        <InputGroupText>{unit}</InputGroupText>
      </InputGroupAddon>
    </InputGroup>
  );
}

/**
 * Quantity input for one row of the materials array. Reads `materialId` with
 * useWatch so the unit suffix updates when the material is (re)selected —
 * useFieldArray's `field.materialId` is a snapshot from the field's mount and
 * never changes.
 */
function MaterialQuantityField({ index }: { index: number }) {
  const { control } = useFormContext<BudgetFormValues>();
  const materialId = useWatch({ control, name: `materials.${index}.materialId` });
  const { data } = useOfflineAwareQuery(materialsListQueryOptions({ page: 1, pageSize: 1000 }));
  const catalogMaterials = data?.items ?? [];
  const unit = catalogMaterials.find((m) => m.id === materialId)?.unit ?? "";

  return (
    <Controller
      name={`materials.${index}.quantity`}
      control={control}
      render={({ field: { value, onChange, onBlur, ref } }) => (
        <InputGroup className="w-40">
          <InputGroupInput
            type="number"
            step="0.01"
            placeholder="Cantidad"
            value={value}
            onBlur={onBlur}
            ref={ref}
            onChange={(e) => onChange(e.target.value)}
          />
          <InputGroupAddon align="inline-end">
            <InputGroupText>{unit}</InputGroupText>
          </InputGroupAddon>
        </InputGroup>
      )}
    />
  );
}
