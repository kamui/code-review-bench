import { Field } from "@base-ui/react/field";
import { Form } from "@base-ui/react/form";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useServerFn } from "@tanstack/react-start";
import { cn } from "cn";
import { Loader2Icon, PencilIcon, PlusIcon, ScissorsIcon, Trash2Icon } from "lucide-react";
import { useState } from "react";
import * as z from "zod";
import { EmptyState } from "#/components/empty-state";
import { PageHeader } from "#/components/page-header";
import { PageShell } from "#/components/page-shell";
import { Pagination } from "#/components/pagination";
import { ResponsiveList } from "#/components/responsive-list";
import { ListSkeleton } from "#/components/skeletons";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "#/components/ui/alert-dialog";
import { Button } from "#/components/ui/button";
import * as StyledField from "#/components/ui/field";
import { Input } from "#/components/ui/input";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "#/components/ui/sheet";
import { toast } from "#/components/ui/toast.tsx";
import { WriteButton } from "#/components/write-button";
import { useCreateSheetParam } from "#/hooks/use-create-sheet-param";
import { useIsMobile } from "#/hooks/use-mobile";
import { useWriteGuard } from "#/lib/connectivity";
import { operationsListQueryOptions, queryKeys } from "#/lib/query-options";
import { useSuspenseQuery } from "#/lib/use-suspense-query";
import {
  createOperation,
  deleteOperation,
  type listOperations,
  updateOperation,
} from "#/server/functions/operations";

const PAGE_SIZE = 20;

const operationsSearchSchema = z.object({
  nuevo: z.boolean().optional().catch(undefined),
  page: z.number().int().min(1).default(1).catch(1),
});

export const Route = createFileRoute("/_app/app/_workspace/operations/")({
  component: OperationsPage,
  validateSearch: operationsSearchSchema,
  loaderDeps: ({ search: { page } }) => ({ page }),
  context: ({ deps: { page } }) => ({
    listOptions: operationsListQueryOptions({ page, pageSize: PAGE_SIZE }),
  }),
  loader: ({ context: { queryClient, listOptions } }) =>
    void queryClient.prefetchQuery(listOptions),
  pendingComponent: () => <ListSkeleton page />,
});

type Operation = Awaited<ReturnType<typeof listOperations>>["items"][number];

function OperationsPage() {
  const { listOptions } = Route.useRouteContext();
  const { page, nuevo } = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const queryClient = useQueryClient();
  const createFn = useServerFn(createOperation);
  const updateFn = useServerFn(updateOperation);
  const deleteFn = useServerFn(deleteOperation);
  const isMobile = useIsMobile();

  const [isSheetOpen, setIsSheetOpen] = useCreateSheetParam(nuevo, () =>
    navigate({ search: (prev) => ({ ...prev, nuevo: undefined }), replace: true }),
  );
  const [editingOperation, setEditingOperation] = useState<Operation | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [formKey, setFormKey] = useState(0);

  const { data } = useSuspenseQuery(listOptions);
  const operations = data.items;
  const total = data.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  const writeGuard = useWriteGuard();
  const createMutation = useMutation({
    mutationFn: createFn,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.operations });
      toast.add({ type: "success", description: "Operación creada correctamente" });
      setIsSheetOpen(false);
    },
    onError: () => toast.add({ type: "error", description: "Error al crear la operación" }),
  });

  const updateMutation = useMutation({
    mutationFn: updateFn,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.operations });
      toast.add({ type: "success", description: "Operación actualizada" });
      setIsSheetOpen(false);
      setEditingOperation(null);
    },
    onError: () => toast.add({ type: "error", description: "Error al actualizar la operación" }),
  });

  const deleteMutation = useMutation({
    mutationFn: deleteFn,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.operations });
      toast.add({ type: "success", description: "Operación eliminada" });
      setDeletingId(null);
    },
    onError: () => toast.add({ type: "error", description: "Error al eliminar la operación" }),
  });

  function openCreate() {
    setEditingOperation(null);
    setFormKey((k) => k + 1);
    setIsSheetOpen(true);
  }

  function handleEdit(operation: Operation) {
    setEditingOperation(operation);
    setFormKey((k) => k + 1);
    setIsSheetOpen(true);
  }

  function goToPage(nextPage: number) {
    navigate({ search: { page: nextPage } });
  }

  return (
    <PageShell>
      <PageHeader title="Operaciones" description="Operaciones de mano de obra y su costo.">
        <Button onClick={openCreate}>
          <PlusIcon className="mr-2 size-4" />
          Agregar operación
        </Button>
      </PageHeader>

      {operations.length === 0 ? (
        <EmptyState
          icon={<ScissorsIcon />}
          title="No hay operaciones"
          description="Define las tareas de mano de obra como corte, costura o patronaje."
          action={
            <Button variant="outline" onClick={openCreate}>
              Crear mi primera operación
            </Button>
          }
        />
      ) : (
        <>
          <ResponsiveList
            items={operations}
            getKey={(o) => o.id}
            actionsLabel="Acciones de la operación"
            actions={(o) => [
              { label: "Editar operación", icon: PencilIcon, onClick: () => handleEdit(o) },
              {
                label: "Eliminar operación",
                icon: Trash2Icon,
                destructive: true,
                onClick: () => setDeletingId(o.id),
              },
            ]}
            columns={[
              { id: "name", header: "Nombre", className: "font-medium", cell: (o) => o.name },
              {
                id: "duration",
                header: "Duración por defecto",
                className: "text-muted-foreground",
                cell: (o) => `${o.defaultDurationMinutes} min`,
              },
            ]}
            card={(o) => ({
              title: o.name,
              description: `${o.defaultDurationMinutes} min por defecto`,
            })}
          />
          <Pagination page={page} totalPages={totalPages} onPageChange={goToPage} />
        </>
      )}

      {/* Form Sheet */}
      <Sheet
        open={isSheetOpen}
        onOpenChange={(open) => {
          setIsSheetOpen(open);
          if (!open) setEditingOperation(null);
        }}
      >
        <SheetContent
          side={isMobile ? "bottom" : "right"}
          className={cn(isMobile && "max-h-[85dvh]")}
        >
          <SheetHeader>
            <SheetTitle>{editingOperation ? "Editar operación" : "Nueva operación"}</SheetTitle>
            <SheetDescription>Ingresa el nombre de la operación de mano de obra.</SheetDescription>
          </SheetHeader>

          <Form
            key={formKey}
            onFormSubmit={(values: { name: string }) => {
              // defaultDurationMinutes is read from Field.Control but not typed in onFormSubmit yet
              const formEl = document.querySelector("form") as HTMLFormElement;
              const durationInput = formEl?.querySelector(
                "[name='defaultDurationMinutes']",
              ) as HTMLInputElement;
              const defaultDurationMinutes = Number(durationInput?.value) || 60;

              if (editingOperation) {
                updateMutation.mutate({
                  data: { id: editingOperation.id, name: values.name, defaultDurationMinutes },
                });
              } else {
                createMutation.mutate({ data: { name: values.name, defaultDurationMinutes } });
              }
            }}
            className="flex flex-1 flex-col gap-6 p-6"
          >
            <Field.Root name="name" render={<StyledField.Field />}>
              <Field.Label render={<StyledField.FieldLabel />}>Nombre</Field.Label>
              <Field.Control
                defaultValue={editingOperation?.name ?? ""}
                placeholder="Ej: Corte de tela"
                required
                render={<Input />}
              />
            </Field.Root>

            <Field.Root name="defaultDurationMinutes" render={<StyledField.Field />}>
              <Field.Label render={<StyledField.FieldLabel />}>
                Duración por defecto (min)
              </Field.Label>
              <Field.Control
                defaultValue={String(editingOperation?.defaultDurationMinutes ?? 60)}
                type="number"
                render={<Input />}
              />
            </Field.Root>

            <SheetFooter className="mt-auto">
              <WriteButton
                type="submit"
                disabled={createMutation.isPending || updateMutation.isPending}
              >
                {(createMutation.isPending || updateMutation.isPending) && (
                  <Loader2Icon className="mr-2 size-4 animate-spin" />
                )}
                {editingOperation ? "Guardar cambios" : "Crear operación"}
              </WriteButton>
              <SheetClose render={<Button variant="outline" type="button" />}>Cancelar</SheetClose>
            </SheetFooter>
          </Form>
        </SheetContent>
      </Sheet>

      {/* Delete Confirmation */}
      <AlertDialog open={!!deletingId} onOpenChange={(open) => !open && setDeletingId(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              ¿Eliminar «{operations.find((o) => o.id === deletingId)?.name ?? "esta operación"}»?
            </AlertDialogTitle>
            <AlertDialogDescription>
              Esta acción no se puede deshacer. Se eliminará permanentemente la operación de la base
              de datos.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogAction
            onClick={() => deletingId && deleteMutation.mutate({ data: { id: deletingId } })}
            render={
              <Button
                variant="destructive"
                disabled={deleteMutation.isPending || writeGuard.disabled}
              />
            }
          >
            {deleteMutation.isPending ? "Eliminando..." : "Eliminar operación"}
          </AlertDialogAction>
          <AlertDialogFooter>
            <AlertDialogCancel render={<Button variant="outline" />}>Cancelar</AlertDialogCancel>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </PageShell>
  );
}
