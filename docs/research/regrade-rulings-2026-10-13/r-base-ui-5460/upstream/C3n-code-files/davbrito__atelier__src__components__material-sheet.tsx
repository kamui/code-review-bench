import { Field } from "@base-ui/react/field";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useServerFn } from "@tanstack/react-start";
import { cn } from "cn";
import {
  ArrowDownToLineIcon,
  ArrowUpFromLineIcon,
  Loader2Icon,
  SlidersHorizontalIcon,
} from "lucide-react";
import { useState } from "react";
import { Controller, useForm, useWatch } from "react-hook-form";
import { ColorPicker } from "#/components/color-picker";
import { ImageUpload } from "#/components/image-upload";
import { OfflineUnavailable } from "#/components/offline-unavailable";
import { Badge } from "#/components/ui/badge";
import { Button } from "#/components/ui/button";
import * as StyledField from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "#/components/ui/sheet";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "#/components/ui/tabs";
import { toast } from "#/components/ui/toast.tsx";
import { WriteButton } from "#/components/write-button";
import { useIsMobile } from "#/hooks/use-mobile";
import { formatDate, formatNumber } from "#/lib/format";
import { materialInventoryQueryOptions, queryKeys } from "#/lib/query-options";
import { UNIT_OPTIONS, type Unit, unitSchema } from "#/lib/units";
import { useOfflineAwareQuery } from "#/lib/use-offline-aware-query";
import { setEntityImage } from "#/server/functions/images";
import { registerMovement } from "#/server/functions/inventory";
import { createMaterial, type getMaterialById, updateMaterial } from "#/server/functions/materials";

type Material = Awaited<ReturnType<typeof getMaterialById>>;

type MaterialSheetProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  editingMaterial: Material | null;
};

type MaterialFormValues = {
  name: string;
  currentPrice: string;
  unit: Unit;
  color: string;
  colorName: string;
  deleteImage: boolean;
  file: File | null;
};

type MovementFormValues = {
  type: "entry" | "exit" | "adjustment";
  quantity: string;
  note: string;
};

const MOVEMENT_TYPES = [
  {
    value: "entry" as const,
    label: "Entrada",
    icon: ArrowDownToLineIcon,
    selected: "border-green-500 bg-green-50 dark:bg-green-950 text-green-700 dark:text-green-400",
  },
  {
    value: "exit" as const,
    label: "Salida",
    icon: ArrowUpFromLineIcon,
    selected: "border-red-500 bg-red-50 dark:bg-red-950 text-red-700 dark:text-red-400",
  },
  {
    value: "adjustment" as const,
    label: "Ajuste",
    icon: SlidersHorizontalIcon,
    selected: "border-blue-500 bg-blue-50 dark:bg-blue-950 text-blue-700 dark:text-blue-400",
  },
];

const MOVEMENT_TYPE_LABELS: Record<"entry" | "exit" | "adjustment", string> = {
  entry: "Entrada",
  exit: "Salida",
  adjustment: "Ajuste",
};

function formatRelativeDate(date: Date | string): string {
  const diff = Date.now() - new Date(date).getTime();
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 30) return formatDate(date, "numeric");
  if (days > 0) return `hace ${days} día${days > 1 ? "s" : ""}`;
  if (hours > 0) return `hace ${hours} hora${hours > 1 ? "s" : ""}`;
  if (minutes > 0) return `hace ${minutes} min`;
  return "ahora mismo";
}

type InventoryTabProps = {
  materialId: string;
  unit: string;
  enabled: boolean;
};

function InventoryTab({ materialId, unit, enabled }: InventoryTabProps) {
  const queryClient = useQueryClient();
  const registerFn = useServerFn(registerMovement);
  const unitLabel = UNIT_OPTIONS.find((o) => o.value === unit)?.label ?? unit;

  const { data, isLoading, offlineUnavailable } = useOfflineAwareQuery({
    ...materialInventoryQueryOptions(materialId),
    enabled,
  });

  const {
    control,
    handleSubmit,
    reset: resetMovementForm,
  } = useForm<MovementFormValues>({
    defaultValues: { type: "entry", quantity: "", note: "" },
  });
  const movementType = useWatch({ control, name: "type" });

  const movementMutation = useMutation({
    mutationFn: registerFn,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.materialInventory(materialId) });
      queryClient.invalidateQueries({ queryKey: queryKeys.materials });
      toast.add({ type: "success", description: "Movimiento registrado" });
      resetMovementForm();
    },
    onError: () => toast.add({ type: "error", description: "Error al registrar el movimiento" }),
  });

  const currentStock = data?.currentStock ?? "0";
  const movements = data?.movements ?? [];
  const stockNum = Number(currentStock);

  const quantityInput = useWatch({ control, name: "quantity" });
  const quantityNum = Number(quantityInput);
  const previewStock =
    quantityInput !== "" && Number.isFinite(quantityNum)
      ? movementType === "adjustment"
        ? quantityNum
        : movementType === "entry"
          ? stockNum + quantityNum
          : stockNum - quantityNum
      : null;

  // Without the inventory, the stock would read 0 and the history would look empty.
  if (offlineUnavailable) {
    return (
      <div className="p-6">
        <OfflineUnavailable variant="section" />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 overflow-auto p-6">
      {/* Stock actual */}
      <div className="rounded-lg border p-4">
        <p className="text-muted-foreground text-xs uppercase tracking-wider">Stock actual</p>
        <p
          className={cn("mt-1 font-bold text-3xl tabular-nums", stockNum < 0 && "text-destructive")}
        >
          {formatNumber(stockNum, 4)}
          <span className="ml-1.5 font-normal text-muted-foreground text-sm">{unitLabel}</span>
        </p>
      </div>

      {/* Registrar movimiento */}
      <form
        className="flex flex-col gap-4"
        onSubmit={handleSubmit((values) => {
          movementMutation.mutate({
            data: {
              materialId,
              type: values.type,
              quantity: values.quantity,
              note: values.note || undefined,
            },
          });
        })}
      >
        <h3 className="font-medium text-sm">Registrar movimiento</h3>

        <Controller
          name="type"
          control={control}
          render={({ field }) => (
            <div className="grid grid-cols-3 gap-2">
              {MOVEMENT_TYPES.map((opt) => (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => field.onChange(opt.value)}
                  className={cn(
                    "flex flex-col items-center gap-1.5 rounded-lg border p-3 font-medium text-xs transition-colors",
                    field.value === opt.value
                      ? opt.selected
                      : "border-border text-muted-foreground hover:text-foreground",
                  )}
                >
                  <opt.icon className="size-4" />
                  {opt.label}
                </button>
              ))}
            </div>
          )}
        />

        <Controller
          name="quantity"
          control={control}
          render={({ field: { name, ref, value, onBlur, onChange } }) => (
            <Field.Root name={name} render={<StyledField.Field />}>
              <Field.Label render={<StyledField.FieldLabel />}>
                {movementType === "adjustment" ? "Cantidad final en stock" : "Cantidad"}
              </Field.Label>
              <Field.Control
                value={value}
                onBlur={onBlur}
                onValueChange={onChange}
                ref={ref}
                type="number"
                step="any"
                min="0"
                placeholder="0"
                required
                render={<Input />}
              />
              {previewStock !== null && (
                <p className="text-muted-foreground text-xs">
                  Nuevo stock:{" "}
                  <span
                    className={cn(
                      "font-medium tabular-nums",
                      previewStock < 0 && "text-destructive",
                    )}
                  >
                    {formatNumber(previewStock, 4)} {unitLabel}
                  </span>
                </p>
              )}
            </Field.Root>
          )}
        />

        <Controller
          name="note"
          control={control}
          render={({ field: { name, ref, value, onBlur, onChange } }) => (
            <Field.Root name={name} render={<StyledField.Field />}>
              <Field.Label render={<StyledField.FieldLabel />}>Nota (opcional)</Field.Label>
              <Field.Control
                value={value}
                onBlur={onBlur}
                onValueChange={onChange}
                ref={ref}
                placeholder="Ej: Compra proveedor X"
                render={<Input />}
              />
            </Field.Root>
          )}
        />

        <WriteButton type="submit" disabled={movementMutation.isPending}>
          {movementMutation.isPending && <Loader2Icon className="mr-2 size-4 animate-spin" />}
          Registrar
        </WriteButton>
      </form>

      {/* Historial */}
      <div>
        <h3 className="mb-3 font-medium text-sm">Historial</h3>
        {isLoading ? (
          <div className="flex justify-center py-4">
            <Loader2Icon className="size-5 animate-spin text-muted-foreground/50" />
          </div>
        ) : movements.length === 0 ? (
          <p className="py-4 text-center text-muted-foreground text-sm">
            No hay movimientos registrados aún.
          </p>
        ) : (
          <div className="flex flex-col gap-2">
            {movements.map((m) => {
              const deltaNum = Number(m.delta);
              const isPositive = deltaNum >= 0;
              return (
                <div
                  key={m.id}
                  className="flex items-start justify-between gap-2 rounded-lg border p-3 text-xs"
                >
                  <div className="flex flex-col gap-1">
                    <div className="flex items-center gap-2">
                      <Badge
                        variant={
                          m.type === "entry"
                            ? "default"
                            : m.type === "exit"
                              ? "destructive"
                              : "outline"
                        }
                      >
                        {MOVEMENT_TYPE_LABELS[m.type]}
                      </Badge>
                      <span
                        className={cn(
                          "font-bold tabular-nums",
                          isPositive ? "text-green-600 dark:text-green-400" : "text-destructive",
                        )}
                      >
                        {isPositive ? "+" : ""}
                        {formatNumber(deltaNum, 4)} {unitLabel}
                      </span>
                    </div>
                    {m.note && <p className="text-muted-foreground">{m.note}</p>}
                    {m.createdByName && <p className="text-muted-foreground">{m.createdByName}</p>}
                  </div>
                  <span className="shrink-0 text-muted-foreground">
                    {formatRelativeDate(m.createdAt)}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

export function MaterialSheet({ open, onOpenChange, editingMaterial }: MaterialSheetProps) {
  const queryClient = useQueryClient();
  const createFn = useServerFn(createMaterial);
  const isMobile = useIsMobile();
  const [activeTab, setActiveTab] = useState<"general" | "inventario">("general");

  const { control, handleSubmit, reset, setValue } = useForm<MaterialFormValues>({
    values: {
      name: editingMaterial?.name ?? "",
      currentPrice: editingMaterial?.currentPrice ?? "",
      unit: editingMaterial ? unitSchema.catch("unit").parse(editingMaterial.unit) : "unit",
      color: editingMaterial?.color ?? "",
      colorName: editingMaterial?.colorName ?? "",
      deleteImage: false,
      file: null,
    },
  });
  const deleteImage = useWatch({ control, name: "deleteImage" });

  const isEdit = !!editingMaterial;

  // A failed image upload/commit must not block saving the material — the
  // rest of the form data is already persisted by the time this runs. So
  // this never throws; it resolves to null and callers show a soft warning.
  const commitFile = async ({
    signedUrl,
    entityId,
    imageKey,
    file,
  }: {
    signedUrl: string;
    entityId: string;
    imageKey: string;
    file: File;
  }): Promise<string | null> => {
    const uploadResponse = await fetch(signedUrl, {
      method: "PUT",
      body: file,
      headers: { "Content-Type": file.type },
    });

    if (!uploadResponse.ok) {
      console.warn("Image upload PUT failed:", uploadResponse.status);
      return null;
    }

    const commit = await setEntityImage({
      data: { entityType: "materials", entityId, imageKey },
    });

    if (!commit.success) {
      console.warn("Failed to commit image:", commit.error);
      return null;
    }

    return commit.permanentKey;
  };

  const createMutation = useMutation({
    mutationFn: async ({
      data,
      file,
    }: {
      data: Parameters<typeof createFn>[0]["data"];
      file: File | null;
    }) => {
      const fnData = { ...data, imageContentType: file?.type, imageSize: file?.size };
      const result = await createFn({ data: fnData });
      if (file && "presignedImageUrl" in result && "imageKey" in result) {
        const permanentKey = await commitFile({
          signedUrl: result.presignedImageUrl,
          entityId: result.id,
          imageKey: result.imageKey,
          file,
        });
        return { ...result, image: permanentKey, imageFailed: permanentKey === null };
      }
      return result;
    },
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.materials });
      if ("imageFailed" in result && result.imageFailed) {
        toast.add({
          type: "warning",
          description: "Material creado, pero no se pudo subir la imagen. Intenta editarlo.",
        });
      } else {
        toast.add({ type: "success", description: "Material creado correctamente" });
      }
      onOpenChange(false);
    },
    onError: () => toast.add({ type: "error", description: "Error al crear el material" }),
  });

  const updateMutation = useMutation({
    mutationFn: async ({
      data,
      file,
    }: {
      data: Parameters<typeof updateMaterial>[0]["data"];
      file: File | null;
    }) => {
      const fnData = { ...data, imageContentType: file?.type, imageSize: file?.size };
      const result = await updateMaterial({ data: fnData });
      if (file && "presignedImageUrl" in result && "imageKey" in result) {
        const permanentKey = await commitFile({
          signedUrl: result.presignedImageUrl,
          entityId: result.id,
          imageKey: result.imageKey,
          file,
        });
        return { ...result, image: permanentKey, imageFailed: permanentKey === null };
      }
      return result;
    },
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.materials });
      if ("imageFailed" in result && result.imageFailed) {
        toast.add({
          type: "warning",
          description: "Material actualizado, pero no se pudo subir la imagen. Intenta de nuevo.",
        });
      } else {
        toast.add({ type: "success", description: "Material actualizado" });
      }
      onOpenChange(false);
    },
    onError: () => toast.add({ type: "error", description: "Error al actualizar el material" }),
  });

  const isPending = createMutation.isPending || updateMutation.isPending;

  function handleOpenChange(open: boolean) {
    if (!open) {
      reset();
      setActiveTab("general");
    }
    onOpenChange(open);
  }

  const generalForm = (
    <form
      id="material-form"
      className="flex min-h-0 flex-1 flex-col gap-6 overflow-auto p-6"
      onSubmit={handleSubmit(({ file, ...values }) => {
        const data = {
          name: values.name,
          unit: values.unit,
          currentPrice: values.currentPrice,
          color: values.color,
          colorName: values.colorName,
          deleteImage: values.deleteImage,
        };
        const id = editingMaterial?.id;
        if (isEdit && id) {
          updateMutation.mutate({ data: { ...data, id }, file });
        } else {
          createMutation.mutate({ data, file });
        }
      })}
    >
      <StyledField.FieldGroup>
        {/* Name */}
        <Controller
          name="name"
          control={control}
          render={({
            field: { name, ref, value, onBlur, onChange },
            fieldState: { invalid, error },
          }) => (
            <Field.Root name={name} invalid={invalid} render={<StyledField.Field />}>
              <Field.Label render={<StyledField.FieldLabel />}>Nombre</Field.Label>
              <Field.Control
                value={value}
                onBlur={onBlur}
                onValueChange={onChange}
                ref={ref}
                placeholder="Ej: Tela de Lino"
                required
                render={<Input />}
              />
              <Field.Error render={<StyledField.FieldError />}>{error?.message}</Field.Error>
            </Field.Root>
          )}
        />

        {/* Unit */}
        <Controller
          name="unit"
          control={control}
          render={({ field, fieldState: { invalid, error } }) => (
            <Field.Root name="unit" render={<StyledField.Field />} invalid={invalid}>
              <Field.Label render={<StyledField.FieldLabel />}>Unidad de medida</Field.Label>
              <Select items={UNIT_OPTIONS} value={field.value} onValueChange={field.onChange}>
                <Field.Control
                  ref={field.ref}
                  render={
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                  }
                />
                <SelectContent>
                  {UNIT_OPTIONS.map(({ label, value }) => (
                    <SelectItem key={value} value={value}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Field.Error render={<StyledField.FieldError />}>{error?.message}</Field.Error>
            </Field.Root>
          )}
        />

        {/* Current Price */}
        <Controller
          name="currentPrice"
          control={control}
          render={({
            field: { name, ref, value, onBlur, onChange },
            fieldState: { invalid, error },
          }) => (
            <Field.Root name={name} invalid={invalid} render={<StyledField.Field />}>
              <Field.Label render={<StyledField.FieldLabel />}>Precio actual ($)</Field.Label>
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

        {/* Color */}
        <Controller
          name="color"
          control={control}
          render={({ field: { name, value, onChange }, fieldState: { invalid, error } }) => (
            <Field.Root name={name} invalid={invalid} render={<StyledField.Field />}>
              <Field.Label render={<StyledField.FieldLabel />}>Color (opcional)</Field.Label>
              <ColorPicker value={value} onValueChange={onChange} placeholder="Ej: #1E3A8A" />
              <Field.Error render={<StyledField.FieldError />}>{error?.message}</Field.Error>
            </Field.Root>
          )}
        />

        {/* Color name */}
        <Controller
          name="colorName"
          control={control}
          render={({
            field: { name, ref, value, onBlur, onChange },
            fieldState: { invalid, error },
          }) => (
            <Field.Root name={name} invalid={invalid} render={<StyledField.Field />}>
              <Field.Label render={<StyledField.FieldLabel />}>
                Nombre del color (opcional)
              </Field.Label>
              <Field.Control
                value={value}
                onBlur={onBlur}
                onValueChange={onChange}
                ref={ref}
                placeholder="Ej: Azul marino"
                render={<Input />}
              />
              <Field.Error render={<StyledField.FieldError />}>{error?.message}</Field.Error>
            </Field.Root>
          )}
        />

        {/* Image */}
        <StyledField.Field>
          <StyledField.FieldLabel>Imagen</StyledField.FieldLabel>
          <ImageUpload
            initialImage={deleteImage ? null : (editingMaterial?.image ?? null)}
            onClear={() => {
              setValue("deleteImage", true);
            }}
            onFileSelect={(file) => {
              setValue("file", file);
              if (file) setValue("deleteImage", false);
            }}
            isUploading={isPending}
          />
        </StyledField.Field>
      </StyledField.FieldGroup>
    </form>
  );

  return (
    <Sheet open={open} onOpenChange={handleOpenChange}>
      <SheetContent
        side={isMobile ? "bottom" : "right"}
        className={cn(isMobile && "max-h-[85dvh]")}
      >
        <SheetHeader>
          <SheetTitle>{isEdit ? "Editar material" : "Nuevo material"}</SheetTitle>
          <SheetDescription>
            Ingresa los detalles del material para utilizarlo en tus prendas.
          </SheetDescription>
        </SheetHeader>

        {isEdit ? (
          <Tabs
            value={activeTab}
            onValueChange={(v) => setActiveTab(v as "general" | "inventario")}
            className="flex min-h-0 flex-1 flex-col gap-0"
          >
            <TabsList className="mx-6 w-auto self-start">
              <TabsTrigger value="general">General</TabsTrigger>
              <TabsTrigger value="inventario">Inventario</TabsTrigger>
            </TabsList>
            <TabsContent value="general" className="flex min-h-0 flex-1 flex-col overflow-hidden">
              {generalForm}
            </TabsContent>
            <TabsContent
              value="inventario"
              className="flex min-h-0 flex-1 flex-col overflow-hidden"
            >
              <InventoryTab
                materialId={editingMaterial.id}
                unit={editingMaterial.unit}
                enabled={open && activeTab === "inventario"}
              />
            </TabsContent>
          </Tabs>
        ) : (
          generalForm
        )}

        {activeTab === "general" && (
          <SheetFooter className="mt-auto">
            <WriteButton type="submit" disabled={isPending} form="material-form">
              {isPending && <Loader2Icon className="mr-2 size-4 animate-spin" />}
              {isEdit ? "Guardar cambios" : "Crear material"}
            </WriteButton>
            <SheetClose render={<Button variant="outline" type="button" />}>Cancelar</SheetClose>
          </SheetFooter>
        )}
      </SheetContent>
    </Sheet>
  );
}
