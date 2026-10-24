import React from "react";
import { revalidateLogic, useForm } from "@tanstack/react-form";
import { z } from "zod";
import { Field, Button, Tooltip } from "@case-study/ui";
import styles from "./Login.module.css";
import { useAppDispatch, useAppSelector } from "@/store/hooks";
import { adminLoginAsync } from "@/store/slices/authSlice";
import { ThemeSwitcher } from "@/components/ThemeSwitcher/ThemeSwitcher";
import { LanguageSwitcher } from "@/components/LanguageSwitcher/LanguageSwitcher";
import { useTranslation } from "react-i18next";

const loginSchema = z.object({
  email: z
    .string()
    .min(1, { message: "login.emailRequired" })
    .pipe(z.email({ message: "login.emailInvalid" })),
  password: z
    .string()
    .min(1, { message: "login.passwordRequired" })
    .min(6, { message: "login.passwordMin" }),
});

const loginDefaultValues = {
  email: "",
  password: "",
};

const Login: React.FC = () => {
  const { t } = useTranslation();
  const dispatch = useAppDispatch();
  const { isLoading, error } = useAppSelector((state) => state.auth);

  const form = useForm({
    defaultValues: loginDefaultValues,
    validationLogic: revalidateLogic({
      mode: "submit",
      modeAfterSubmission: "change",
    }),
    validators: {
      onChange: loginSchema,
      onSubmit: loginSchema,
    },
    onSubmit: async ({ value }) => {
      await dispatch(adminLoginAsync(value));
    },
  });

  return (
    <div className={styles.container}>
      <div className={styles.switchers}>
        <ThemeSwitcher />
        <LanguageSwitcher />
      </div>
      <div className={styles.loginCard}>
        <h1 className={styles.title}>{t("login.title")}</h1>
        <p className={styles.subtitle}>{t("login.subtitle")}</p>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            e.stopPropagation();
            form.handleSubmit();
          }}
          className={styles.form}
        >
          <form.Field
            name="email"
            children={(field) => (
              <Field.Root>
                <Field.Label required>{t("login.emailLabel")}</Field.Label>
                <Field.Control
                  id={field.name}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => field.handleChange(e.target.value)}
                  type="email"
                  autoComplete="email"
                  placeholder={t("login.emailPlaceholder")}
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
                <Field.Label required>{t("login.passwordLabel")}</Field.Label>
                <Field.Control
                  id={field.name}
                  name={field.name}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => field.handleChange(e.target.value)}
                  type="password"
                  autoComplete="current-password"
                  placeholder="••••••"
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
          <div className={styles.actions}>
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
                            disabled={!canSubmit || isSubmitting || isLoading}
                            className={styles.submitButton}
                          >
                            {isSubmitting || isLoading
                              ? t("login.signingIn")
                              : t("login.submitButton")}
                          </Button>
                        </span>
                      )}
                    />
                    <Tooltip.Portal>
                      <Tooltip.Positioner>
                        <Tooltip.Popup>
                          <Tooltip.Arrow />
                          {canSubmit
                            ? t("login.submitButton")
                            : t("common.formInvalid")}
                        </Tooltip.Popup>
                      </Tooltip.Positioner>
                    </Tooltip.Portal>
                  </Tooltip.Root>
                </Tooltip.Provider>
              )}
            </form.Subscribe>
            {error && <div className={styles.generalError}>{t(error)}</div>}
          </div>
        </form>
      </div>
    </div>
  );
};

export default Login;
