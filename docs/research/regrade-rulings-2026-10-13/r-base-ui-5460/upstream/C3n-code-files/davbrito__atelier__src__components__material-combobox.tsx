import { Field } from "@base-ui/react/field";
import { useDebouncer } from "@tanstack/react-pacer";
import { useMutation } from "@tanstack/react-query";
import { useServerFn } from "@tanstack/react-start";
import { Loader2Icon, PlusIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { ImageUpload } from "#/components/image-upload";
import {
  OFFLINE_SELECTION_LABEL,
  OfflineSearchUnavailable,
} from "#/components/offline-unavailable";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "#/components/ui/alert-dialog";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "#/components/ui/combobox";
import * as StyledField from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#/components/ui/select";
import { toast } from "#/components/ui/toast.tsx";
import { WriteButton } from "#/components/write-button";
import { materialByIdQueryOptions, queryKeys } from "#/lib/query-options";
import { UNIT_OPTIONS, type Unit } from "#/lib/units";
import { useOfflineAwareQuery } from "#/lib/use-offline-aware-query";
import { createMaterial, listMaterials } from "#/server/functions/materials";

type RawMaterial = {
  id: string;
  name: string;
  unit: string;
  currentPrice: string;
};

type MaterialComboboxProps = {
  value: string;
  onChange: (materialId: string) => void;
};

export function MaterialCombobox({ value, onChange }: MaterialComboboxProps) {
  const listFn = useServerFn(listMaterials);
  const createFn = useServerFn(createMaterial);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [search, setSearch] = useState("");
  const [newName, setNewName] = useState("");
  const [newUnit, setNewUnit] = useState<Unit>("unit");
  const [newPrice, setNewPrice] = useState("");
  const [newImage, setNewImage] = useState<string | null>(null);

  const searchDebouncer = useDebouncer((next: string) => setSearch(next), { wait: 300 });

  useEffect(() => {
    searchDebouncer.maybeExecute(inputValue);
  }, [inputValue, searchDebouncer.maybeExecute]);

  const {
    data: rawMaterials = [],
    isFetching,
    offlineUnavailable: searchUnavailable,
  } = useOfflineAwareQuery<RawMaterial[]>({
    queryKey: queryKeys.materialsSearch(search),
    queryFn: async () => (await listFn({ data: { page: 1, pageSize: 20, search } })).items,
    staleTime: 10_000,
  });

  // The selected material may not be in the current (filtered) results —
  // fetch it directly so its label still renders when the combobox is closed.
  const { data: selectedMaterial, offlineUnavailable: selectedUnavailable } = useOfflineAwareQuery({
    ...materialByIdQueryOptions(value),
    enabled: !!value,
  });

  const materialItems = rawMaterials.map((m) => ({
    id: m.id,
    label: `${m.name} (${m.unit})`,
  }));

  const selectedItem =
    materialItems.find((item) => item.id === value) ??
    (selectedMaterial && selectedMaterial.id === value
      ? { id: selectedMaterial.id, label: `${selectedMaterial.name} (${selectedMaterial.unit})` }
      : null) ??
    // Offline, a value whose details were never loaded must not look like an empty field.
    (value && selectedUnavailable ? { id: value, label: OFFLINE_SELECTION_LABEL } : null);

  const createMutation = useMutation({
    mutationFn: createFn,
    onSuccess: (newMaterial) => {
      toast.add({ type: "success", description: "Material creado y seleccionado" });
      onChange(newMaterial.id);
      setDialogOpen(false);
      setNewName("");
      setNewUnit("unit");
      setNewPrice("");
      setNewImage(null);
    },
    onError: () => toast.add({ type: "error", description: "Error al crear el material" }),
  });

  function handleCreate(e: React.SubmitEvent) {
    e.stopPropagation();
    e.preventDefault();
    if (!newName.trim()) return;
    createMutation.mutate({
      data: { name: newName.trim(), unit: newUnit, currentPrice: newPrice || "0" },
    });
  }

  function openCreateDialog() {
    setNewName(inputValue);
    setNewUnit("unit");
    setNewPrice("");
    setNewImage(null);
    setDialogOpen(true);
  }

  return (
    <>
      <Combobox
        items={materialItems}
        itemToStringValue={(item: (typeof materialItems)[number]) => item.label}
        value={selectedItem}
        onValueChange={(item) => {
          if (item) onChange(item.id);
        }}
        onInputValueChange={(val) => setInputValue(typeof val === "string" ? val : "")}
        filter={null}
      >
        <ComboboxInput placeholder="Buscar material..." className="w-full" showTrigger />

        <ComboboxContent>
          <ComboboxEmpty>
            {searchUnavailable ? (
              <OfflineSearchUnavailable />
            ) : isFetching ? (
              <span className="flex items-center gap-2 px-3 py-2 text-muted-foreground text-xs">
                <Loader2Icon className="size-3 animate-spin" />
                Buscando...
              </span>
            ) : (
              <button
                type="button"
                onClick={openCreateDialog}
                className="flex w-full items-center gap-2 px-3 py-2 text-xs hover:bg-accent hover:text-accent-foreground"
              >
                <PlusIcon className="size-3" />
                Crear nuevo material en el catálogo
              </button>
            )}
          </ComboboxEmpty>
          <ComboboxList>
            {(item) => (
              <ComboboxItem key={item.id} value={item}>
                {item.label}
              </ComboboxItem>
            )}
          </ComboboxList>
        </ComboboxContent>
      </Combobox>

      <AlertDialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <AlertDialogContent className="sm:max-w-sm">
          <form className="contents" onSubmit={handleCreate}>
            <AlertDialogHeader>
              <AlertDialogTitle>Nuevo material</AlertDialogTitle>
              <AlertDialogDescription>
                Agrega {newName ? `"${newName}"` : "un material"} al catálogo.
              </AlertDialogDescription>
            </AlertDialogHeader>

            <StyledField.FieldGroup className="py-4">
              <Field.Root name="name" render={<StyledField.Field />}>
                <Field.Label render={<StyledField.FieldLabel />}>Nombre</Field.Label>
                <Field.Control
                  value={newName}
                  onChange={(e) => setNewName((e.target as HTMLInputElement).value)}
                  placeholder="Nombre del material"
                  required
                  render={<Input />}
                />
                <Field.Error render={<StyledField.FieldError />} />
              </Field.Root>

              <Field.Root name="unit" render={<StyledField.Field />}>
                <Field.Label render={<StyledField.FieldLabel />}>Unidad de medida</Field.Label>
                <Select
                  items={UNIT_OPTIONS}
                  value={newUnit}
                  onValueChange={(val) => val && setNewUnit(val)}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {UNIT_OPTIONS.map(({ label, value }) => (
                      <SelectItem key={value} value={value}>
                        {label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Field.Error render={<StyledField.FieldError />} />
              </Field.Root>

              <Field.Root name="currentPrice" render={<StyledField.Field />}>
                <Field.Label render={<StyledField.FieldLabel />}>Precio actual ($)</Field.Label>
                <Field.Control
                  type="number"
                  step="0.01"
                  placeholder="0.00"
                  value={newPrice}
                  onChange={(e) => setNewPrice((e.target as HTMLInputElement).value)}
                  render={<Input />}
                />
                <Field.Error render={<StyledField.FieldError />} />
              </Field.Root>

              <StyledField.Field>
                <StyledField.FieldLabel>Imagen</StyledField.FieldLabel>
                <ImageUpload
                  initialImage={newImage}
                  onClear={() => setNewImage(null)}
                  onFileSelect={() => {}}
                />
              </StyledField.Field>
            </StyledField.FieldGroup>

            <AlertDialogFooter>
              <AlertDialogCancel>Cancelar</AlertDialogCancel>
              <WriteButton type="submit" disabled={createMutation.isPending}>
                {createMutation.isPending && <Loader2Icon className="mr-2 size-4 animate-spin" />}
                Crear material
              </WriteButton>
            </AlertDialogFooter>
          </form>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
