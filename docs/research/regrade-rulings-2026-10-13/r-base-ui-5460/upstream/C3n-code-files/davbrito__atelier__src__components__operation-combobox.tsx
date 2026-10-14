import { Field } from "@base-ui/react/field";
import { useDebouncer } from "@tanstack/react-pacer";
import { useMutation } from "@tanstack/react-query";
import { useServerFn } from "@tanstack/react-start";
import { Loader2Icon, PlusIcon } from "lucide-react";
import { useEffect, useState } from "react";
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
import { Button } from "#/components/ui/button";
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
import { toast } from "#/components/ui/toast.tsx";
import { WriteButton } from "#/components/write-button";
import { operationByIdQueryOptions, queryKeys } from "#/lib/query-options";
import { useOfflineAwareQuery } from "#/lib/use-offline-aware-query";
import { createOperation, listOperations } from "#/server/functions/operations";

type RawOperation = {
  id: string;
  name: string;
  defaultDurationMinutes: number;
};

type OperationComboboxProps = {
  value: string;
  onChange: (operationId: string, defaultDurationMinutes: number) => void;
};

export function OperationCombobox({ value, onChange }: OperationComboboxProps) {
  const listFn = useServerFn(listOperations);
  const createFn = useServerFn(createOperation);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [search, setSearch] = useState("");
  const [newName, setNewName] = useState("");
  const [newDuration, setNewDuration] = useState("60");

  const searchDebouncer = useDebouncer((next: string) => setSearch(next), { wait: 300 });

  useEffect(() => {
    searchDebouncer.maybeExecute(inputValue);
  }, [inputValue, searchDebouncer.maybeExecute]);

  const {
    data: rawOperations = [],
    isFetching,
    offlineUnavailable: searchUnavailable,
  } = useOfflineAwareQuery<RawOperation[]>({
    queryKey: queryKeys.operationsSearch(search),
    queryFn: async () => (await listFn({ data: { page: 1, pageSize: 20, search } })).items,
    staleTime: 10_000,
  });

  // The selected operation may not be in the current (filtered) results —
  // fetch it directly so its label still renders when the combobox is closed.
  const { data: selectedOperation, offlineUnavailable: selectedUnavailable } = useOfflineAwareQuery(
    {
      ...operationByIdQueryOptions(value),
      enabled: !!value,
    },
  );

  const operationItems = rawOperations.map((o) => ({
    id: o.id,
    label: o.name,
    defaultDurationMinutes: o.defaultDurationMinutes,
  }));

  const selectedItem =
    operationItems.find((item) => item.id === value) ??
    (selectedOperation && selectedOperation.id === value
      ? {
          id: selectedOperation.id,
          label: selectedOperation.name,
          defaultDurationMinutes: selectedOperation.defaultDurationMinutes,
        }
      : null) ??
    // Offline, a value whose details were never loaded must not look like an empty field.
    (value && selectedUnavailable
      ? { id: value, label: OFFLINE_SELECTION_LABEL, defaultDurationMinutes: 0 }
      : null);

  const createMutation = useMutation({
    mutationFn: createFn,
    onSuccess: (newOperation) => {
      toast.add({ type: "success", description: "Operación creada y seleccionada" });
      onChange(newOperation.id, newOperation.defaultDurationMinutes);
      setDialogOpen(false);
      setNewName("");
      setNewDuration("60");
    },
    onError: () => toast.add({ type: "error", description: "Error al crear la operación" }),
  });

  function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!newName.trim()) return;
    createMutation.mutate({
      data: {
        name: newName.trim(),
        defaultDurationMinutes: Number(newDuration) || 60,
      },
    });
  }

  function openCreateDialog() {
    setNewName(inputValue);
    setNewDuration("60");
    setDialogOpen(true);
  }

  return (
    <>
      <Combobox
        items={operationItems}
        itemToStringValue={(item: (typeof operationItems)[number]) => item.label}
        value={selectedItem}
        onValueChange={(item) => {
          if (item) onChange(item.id, item.defaultDurationMinutes);
        }}
        onInputValueChange={(val) => setInputValue(typeof val === "string" ? val : "")}
        filter={null}
      >
        <ComboboxInput placeholder="Buscar operación..." className="w-full" showTrigger />

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
                Crear nueva operación en el catálogo
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
          <form onSubmit={handleCreate}>
            <AlertDialogHeader>
              <AlertDialogTitle>Nueva operación</AlertDialogTitle>
              <AlertDialogDescription>
                Agrega {newName ? `"${newName}"` : "una operación"} al catálogo.
              </AlertDialogDescription>
            </AlertDialogHeader>

            <StyledField.FieldGroup className="py-4">
              <Field.Root name="name" render={<StyledField.Field />}>
                <Field.Label render={<StyledField.FieldLabel />}>Nombre</Field.Label>
                <Field.Control
                  value={newName}
                  onChange={(e) => setNewName((e.target as HTMLInputElement).value)}
                  placeholder="Ej: Corte de tela"
                  required
                  render={<Input />}
                />
                <Field.Error render={<StyledField.FieldError />} />
              </Field.Root>

              <Field.Root name="defaultDurationMinutes" render={<StyledField.Field />}>
                <Field.Label render={<StyledField.FieldLabel />}>
                  Duración por defecto (min)
                </Field.Label>
                <Field.Control
                  type="number"
                  placeholder="60"
                  value={newDuration}
                  onChange={(e) => setNewDuration((e.target as HTMLInputElement).value)}
                  render={<Input />}
                />
                <Field.Error render={<StyledField.FieldError />} />
              </Field.Root>
            </StyledField.FieldGroup>

            <AlertDialogFooter>
              <AlertDialogCancel>
                <Button variant="outline" type="button">
                  Cancelar
                </Button>
              </AlertDialogCancel>
              <WriteButton type="submit" disabled={createMutation.isPending}>
                {createMutation.isPending && <Loader2Icon className="mr-2 size-4 animate-spin" />}
                Crear operación
              </WriteButton>
            </AlertDialogFooter>
          </form>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
