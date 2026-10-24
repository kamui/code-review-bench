import { useTranslation } from "react-i18next";
import { AlertDialog, Button, Field, Tooltip } from "@case-study/ui";
import { useAppDispatch, useAppSelector } from "@/store/hooks";
import { rejectTaskAsync } from "@/store/slices/taskSlice";
import type { Task } from "@/api/tasks/taskController";
import styles from "./Content.module.css";
import { useEffect, useRef } from "react";
import { formatDate } from "@/utils/date";
import { useForm } from "@tanstack/react-form";
import { z } from "zod";

interface ContentProps {
  task: Task;
  setOpenDialog: (open: boolean) => void;
}

const rejectSchema = z.object({
  reason: z.string().min(3, "pendingTasks.rejectDialog.reasonMin"),
});

export function Content({ task, setOpenDialog }: ContentProps) {
  const abortController = useRef<AbortController | null>(null);

  const { t, i18n } = useTranslation();
  const dispatch = useAppDispatch();
  const { isLoading } = useAppSelector((state) => state.tasks);

  const form = useForm({
    defaultValues: {
      reason: "",
    },
    validators: {
      onChange: rejectSchema,
      onSubmit: rejectSchema,
    },
    onSubmit: async ({ value }) => {
      abortController.current = new AbortController();
      const response = await dispatch(
        rejectTaskAsync(
          { taskId: task.id, rejectionReason: value.reason },
          { signal: abortController.current.signal },
        ),
      );

      if (rejectTaskAsync.fulfilled.match(response)) {
        setOpenDialog(false);
      }
    },
  });

  useEffect(() => {
    return () => {
      abortController.current?.abort();
    };
  }, []);

  return (
    <>
      <div className={styles.dialogHeader}>
        <AlertDialog.Title>
          {t("pendingTasks.rejectDialog.title")}
        </AlertDialog.Title>
        <AlertDialog.Close className={styles.dialogCloseButton}>
          ✕
        </AlertDialog.Close>
      </div>

      <form
        onSubmit={(e) => {
          e.preventDefault();
          e.stopPropagation();
          form.handleSubmit();
        }}
      >
        <div className={styles.dialogContent}>
          <AlertDialog.Description className={styles.dialogDescription}>
            {t("pendingTasks.rejectDialog.description")}
          </AlertDialog.Description>

          <div className={styles.taskInfo}>
            <div className={styles.taskTitle}>{task.title}</div>
            <div className={styles.taskDetailsGrid}>
              <div className={styles.taskDetail}>
                <span className={styles.detailLabel}>
                  {t("tasks.table.owner")}:
                </span>
                <span className={styles.detailValue}>{task.createdBy}</span>
              </div>
              <div className={styles.taskDetail}>
                <span className={styles.detailLabel}>
                  {t("tasks.table.date")}:
                </span>
                <span className={styles.detailValue}>
                  {formatDate(task.createdAt, i18n.language)}
                </span>
              </div>
            </div>
          </div>

          <div className={styles.form}>
            <form.Field
              name="reason"
              children={(field) => (
                <Field.Root>
                  <Field.Label required>
                    {t("pendingTasks.rejectDialog.reasonLabel")}
                  </Field.Label>
                  <Field.Control
                    render={
                      <textarea
                        style={{
                          minHeight: "6rem",
                          resize: "vertical",
                          padding: "0.75rem",
                        }}
                      />
                    }
                    id={field.name}
                    name={field.name}
                    value={field.state.value}
                    onBlur={field.handleBlur}
                    onChange={(e) => field.handleChange(e.target.value)}
                    placeholder={t("pendingTasks.rejectDialog.placeholder")}
                  />
                  <Field.Error
                    match={
                      field.state.meta.isTouched && !field.state.meta.isValid
                    }
                  >
                    {t(field.state.meta.errors?.[0]?.message ?? "")}
                  </Field.Error>
                </Field.Root>
              )}
            />
          </div>
        </div>

        <div className={styles.dialogFooter}>
          <AlertDialog.Close className={styles.cancelButton} type="button">
            {t("common.cancel")}
          </AlertDialog.Close>
          <form.Subscribe
            selector={(state) => [state.canSubmit, state.isSubmitting]}
          >
            {([canSubmit, isSubmitting]) => (
              <Tooltip.Provider>
                <Tooltip.Root>
                  <Tooltip.Trigger
                    render={(props, state) => (
                      <span {...props} {...state} tabIndex={-1}>
                        <Button
                          type="submit"
                          data-variant="danger"
                          className={styles.confirmButton}
                          disabled={!canSubmit || isSubmitting || isLoading}
                        >
                          {isSubmitting || isLoading
                            ? t("common.loading")
                            : t("pendingTasks.reject")}
                        </Button>
                      </span>
                    )}
                  />
                  <Tooltip.Portal>
                    <Tooltip.Positioner>
                      <Tooltip.Popup>
                        <Tooltip.Arrow />
                        {canSubmit
                          ? t("pendingTasks.reject")
                          : t("common.formInvalid")}
                      </Tooltip.Popup>
                    </Tooltip.Positioner>
                  </Tooltip.Portal>
                </Tooltip.Root>
              </Tooltip.Provider>
            )}
          </form.Subscribe>
        </div>
      </form>
    </>
  );
}
