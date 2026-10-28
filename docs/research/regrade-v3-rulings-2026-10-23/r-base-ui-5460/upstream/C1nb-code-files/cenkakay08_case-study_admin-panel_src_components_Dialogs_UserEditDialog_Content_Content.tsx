import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { revalidateLogic, useForm } from "@tanstack/react-form";
import { z } from "zod";
import { Field, Select, Button, Dialog, Tooltip } from "@case-study/ui";
import { useAppDispatch } from "@/store/hooks";
import { updateUserAsync } from "@/store/slices/userSlice";
import {
  type AdminUser,
  USER_ROLES,
  type UserRole,
} from "@/api/users/userController";
import styles from "./Content.module.css";

const editUserSchema = z
  .object({
    name: z.string().min(1, { message: "userManagement.form.nameRequired" }),
    email: z
      .string()
      .min(1, { message: "login.emailRequired" })
      .pipe(z.email({ message: "login.emailInvalid" })),
    password: z.string().refine((val) => !val || val.length >= 6, {
      message: "login.passwordMin",
    }),
    confirmPassword: z.string(),
    role: z.enum([USER_ROLES.ADMIN, USER_ROLES.MODERATOR, USER_ROLES.VIEWER]),
  })
  .refine((data) => data.password === data.confirmPassword, {
    message: "userManagement.form.passwordsDoNotMatch",
    path: ["confirmPassword"],
  });

const ROLES: UserRole[] = [
  USER_ROLES.ADMIN,
  USER_ROLES.MODERATOR,
  USER_ROLES.VIEWER,
];

interface ContentProps {
  user: AdminUser;
  setOpen: (open: boolean) => void;
}

export function Content({ user, setOpen }: ContentProps) {
  const { t } = useTranslation();
  const dispatch = useAppDispatch();

  const abortController = useRef<AbortController | null>(null);

  const form = useForm({
    defaultValues: {
      name: user.name,
      email: user.email,
      password: "",
      confirmPassword: "",
      role: user.role as UserRole,
    },
    validationLogic: revalidateLogic({
      mode: "submit",
      modeAfterSubmission: "change",
    }),
    validators: {
      onChange: editUserSchema,
      onSubmit: editUserSchema,
    },
    onSubmit: async ({ value }) => {
      abortController.current = new AbortController();

      const result = await dispatch(
        updateUserAsync(
          {
            userId: user.id,
            user: {
              name: value.name,
              email: value.email,
              ...(value.password ? { password: value.password } : {}),
              role: value.role,
            },
          },
          { signal: abortController.current.signal },
        ),
      );
      if (updateUserAsync.fulfilled.match(result)) {
        setOpen(false);
      }
    },
  });

  useEffect(() => {
    return () => {
      abortController.current?.abort();
    };
  }, []);

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        e.stopPropagation();
        form.handleSubmit();
      }}
      className={styles.dialogForm}
    >
      <div className={styles.dialogHeader}>
        <Dialog.Title>{t("userManagement.editDialog.title")}</Dialog.Title>
        <Dialog.Close className={styles.dialogCloseButton}>✕</Dialog.Close>
      </div>

      <div className={styles.dialogContent}>
        <Dialog.Description className={styles.dialogDescription}>
          {t("userManagement.editDialog.description")}
        </Dialog.Description>

        <div className={styles.formFields}>
          <form.Field
            name="name"
            children={(field) => (
              <Field.Root>
                <Field.Label required>
                  {t("userManagement.form.name")}
                </Field.Label>
                <Field.Control
                  id={field.name}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => field.handleChange(e.target.value)}
                  placeholder={t("userManagement.form.namePlaceholder")}
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

          <form.Field
            name="email"
            children={(field) => (
              <Field.Root>
                <Field.Label required>
                  {t("userManagement.form.email")}
                </Field.Label>
                <Field.Control
                  id={field.name}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => field.handleChange(e.target.value)}
                  type="email"
                  placeholder={t("userManagement.form.emailPlaceholder")}
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

          <form.Field
            name="password"
            children={(field) => (
              <Field.Root>
                <Field.Label>
                  {t("userManagement.form.newPassword")}
                </Field.Label>
                <Field.Control
                  id={field.name}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => field.handleChange(e.target.value)}
                  type="password"
                  autoComplete="new-password"
                  placeholder={t("userManagement.form.newPasswordPlaceholder")}
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

          <form.Field
            name="confirmPassword"
            children={(field) => (
              <Field.Root>
                <Field.Label>
                  {t("userManagement.form.confirmNewPassword")}
                </Field.Label>
                <Field.Control
                  id={field.name}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => field.handleChange(e.target.value)}
                  type="password"
                  autoComplete="new-password"
                  placeholder={t(
                    "userManagement.form.confirmNewPasswordPlaceholder",
                  )}
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

          <form.Field
            name="role"
            children={(field) => (
              <Field.Root>
                <Field.Label required>
                  {t("userManagement.form.role")}
                </Field.Label>
                <Select.Root
                  value={field.state.value}
                  onValueChange={(val) => field.handleChange(val as UserRole)}
                >
                  <Select.Trigger>
                    <Select.Value>{field.state.value}</Select.Value>
                    <Select.Icon>
                      <Select.ChevronUpDownIcon />
                    </Select.Icon>
                  </Select.Trigger>
                  <Select.Portal>
                    <Select.Positioner sideOffset={8}>
                      <Select.Popup>
                        <Select.List>
                          {ROLES.map((role) => (
                            <Select.Item key={role} value={role}>
                              <Select.ItemText>{role}</Select.ItemText>
                              <Select.ItemIndicator>
                                <Select.CheckIcon />
                              </Select.ItemIndicator>
                            </Select.Item>
                          ))}
                        </Select.List>
                      </Select.Popup>
                    </Select.Positioner>
                  </Select.Portal>
                </Select.Root>
              </Field.Root>
            )}
          />
        </div>
      </div>

      <div className={styles.dialogFooter}>
        <Dialog.Close className={styles.cancelButton}>
          {t("common.cancel")}
        </Dialog.Close>
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
                        className={styles.submitButton}
                        disabled={!canSubmit || isSubmitting}
                      >
                        {isSubmitting
                          ? t("common.loading")
                          : t("userManagement.save")}
                      </Button>
                    </span>
                  )}
                />
                <Tooltip.Portal>
                  <Tooltip.Positioner>
                    <Tooltip.Popup>
                      <Tooltip.Arrow />
                      {canSubmit
                        ? t("userManagement.save")
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
  );
}
