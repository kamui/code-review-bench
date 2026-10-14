import { AlertDialog } from "@base-ui/react/alert-dialog";
import { Button } from "@base-ui/react/button";
import { Field } from "@base-ui/react/field";
import * as stylex from "@stylexjs/stylex";
import { useEffect, useState } from "react";
import { type Control, Controller, useForm } from "react-hook-form";
import { Checkbox } from "../components/Checkbox";
import { LarkBinding } from "../components/LarkBinding";
import { RoutingEditor } from "../components/RoutingEditor";
import { Select } from "../components/Select";
import {
  invalidate,
  TAG_LINEAR_SETTINGS,
  TAG_LINEAR_USER_MAP,
} from "../lib/cache";
import { errMessage } from "../lib/errors";
import {
  deleteUserMap,
  getLinearSettings,
  type LinearSettings,
  listUserMap,
  putLinearSettings,
  upsertUserMap,
} from "../lib/linear-api";
import { useData, useMutation } from "../lib/tayori";
// Aliased: react-hook-form render props also destructure a `field`.
import {
  button,
  card,
  dialog,
  field as fieldStyle,
  filters,
  text,
} from "../theme/shared";
import { colors } from "../theme/tokens.stylex";
import { typo } from "../theme/typography";

const s = stylex.create({
  subsystem: {
    color: colors.muted,
    marginBottom: "0.6rem",
  },
  fields: {
    display: "grid",
    gap: "0.45rem",
    marginTop: "0.7rem",
    paddingTop: "0.7rem",
    borderTopColor: colors.hairlineStrong,
    borderTopStyle: "dashed",
    borderTopWidth: "1px",
  },
  filtersSpaced: {
    marginTop: "0.75rem",
  },
  // The user-map card sits further below the settings card than the default
  // stacked-card rhythm.
  userMapCard: {
    marginTop: "1.5rem",
  },
});

type Feedback = { tone: "ok" | "error"; text: string } | null;

// ── Settings ───────────────────────────────────────────────────────────────

// The form mirrors the wire shape but holds lead-days as an editable CSV string.
interface SettingsForm extends Omit<LinearSettings, "reminder_lead_days"> {
  reminder_lead_days: string;
}

const DEFAULT_FORM: SettingsForm = {
  subscriber_on_comment: true,
  subscriber_on_status_change: true,
  subscriber_on_any_update: false,
  reminders_enabled: true,
  reminder_recipients: "assignee",
  reminder_lead_days: "7, 3, 1, 0",
  reminder_overdue_max_days: 7,
  reminder_check_interval_hours: 6,
  reminder_timezone: "UTC",
};

function wireToForm(w: LinearSettings): SettingsForm {
  return { ...w, reminder_lead_days: w.reminder_lead_days.join(", ") };
}

function formToWire(f: SettingsForm): LinearSettings {
  return {
    subscriber_on_comment: f.subscriber_on_comment,
    subscriber_on_status_change: f.subscriber_on_status_change,
    subscriber_on_any_update: f.subscriber_on_any_update,
    reminders_enabled: f.reminders_enabled,
    reminder_recipients: f.reminder_recipients,
    reminder_lead_days: f.reminder_lead_days
      .split(",")
      .map((s) => parseInt(s.trim(), 10))
      .filter((n) => Number.isFinite(n) && n >= 0),
    reminder_overdue_max_days: f.reminder_overdue_max_days,
    reminder_check_interval_hours: f.reminder_check_interval_hours,
    reminder_timezone: f.reminder_timezone.trim(),
  };
}

// Boolean fields of the settings form — the ones rendered as checkboxes.
type BoolField =
  | "subscriber_on_comment"
  | "subscriber_on_status_change"
  | "subscriber_on_any_update"
  | "reminders_enabled";

function CheckboxField({
  label,
  name,
  control,
}: {
  label: string;
  name: BoolField;
  control: Control<SettingsForm>;
}) {
  return (
    <Field.Root className={stylex.props(fieldStyle.row).className}>
      <Field.Label className={stylex.props(fieldStyle.label).className}>
        {label}
      </Field.Label>
      <Controller
        control={control}
        name={name}
        render={({ field }) => (
          <Checkbox
            checked={!!field.value}
            onCheckedChange={field.onChange}
            inputRef={field.ref}
            name={field.name}
          />
        )}
      />
    </Field.Root>
  );
}

function SettingsCard() {
  const { data, error } = useData(getLinearSettings, {
    cacheTags: [TAG_LINEAR_SETTINGS],
  });
  const { register, handleSubmit, reset, control } = useForm<SettingsForm>({
    defaultValues: DEFAULT_FORM,
  });
  const [feedback, setFeedback] = useState<Feedback>(null);

  // Hydrate the form once settings load.
  useEffect(() => {
    if (data) reset(wireToForm(data));
  }, [data, reset]);

  const save = useMutation(putLinearSettings, {
    onSuccess: () => void invalidate(TAG_LINEAR_SETTINGS),
  });

  const onSave = handleSubmit(async (form) => {
    setFeedback(null);
    try {
      await save.trigger({ body: formToWire(form) });
      setFeedback({ tone: "ok", text: "settings saved" });
    } catch (e) {
      setFeedback({ tone: "error", text: errMessage(e) });
    }
  });

  return (
    <div {...stylex.props(card.base, card.stacked)}>
      <div {...stylex.props(typo.mono12, s.subsystem)}>behavior settings</div>
      {error !== undefined && (
        <p className="error">Failed to load: {errMessage(error)}</p>
      )}

      <p {...stylex.props(text.help)}>
        Subscriber fan-out &amp; due-date reminders need{" "}
        <code>LINEAR_API_KEY</code> set (to resolve subscriber emails / poll due
        dates). Changes apply live — no restart.
      </p>

      <div {...stylex.props(s.fields)}>
        <CheckboxField
          label="Notify subscribers on comments"
          name="subscriber_on_comment"
          control={control}
        />
        <CheckboxField
          label="Notify subscribers on status changes"
          name="subscriber_on_status_change"
          control={control}
        />
        <CheckboxField
          label="Notify subscribers on any field update"
          name="subscriber_on_any_update"
          control={control}
        />
        <CheckboxField
          label="Enable due-date reminders"
          name="reminders_enabled"
          control={control}
        />

        <Field.Root className={stylex.props(fieldStyle.row).className}>
          <Field.Label className={stylex.props(fieldStyle.label).className}>
            Reminder recipients
          </Field.Label>
          <Controller
            control={control}
            name="reminder_recipients"
            render={({ field }) => (
              <Select
                trigger={[fieldStyle.input, fieldStyle.selectTrigger]}
                value={field.value}
                onValueChange={field.onChange}
                options={[
                  { value: "assignee", label: "Assignee only" },
                  {
                    value: "assignee_and_subscribers",
                    label: "Assignee + all subscribers",
                  },
                ]}
              />
            )}
          />
        </Field.Root>

        <Field.Root className={stylex.props(fieldStyle.row).className}>
          <Field.Label className={stylex.props(fieldStyle.label).className}>
            Reminder lead days
          </Field.Label>
          <Field.Control
            className={stylex.props(fieldStyle.input).className}
            placeholder="7, 3, 1, 0"
            {...register("reminder_lead_days")}
          />
        </Field.Root>

        <Field.Root className={stylex.props(fieldStyle.row).className}>
          <Field.Label className={stylex.props(fieldStyle.label).className}>
            Overdue reminders cap (days)
          </Field.Label>
          <Field.Control
            className={stylex.props(fieldStyle.input).className}
            type="number"
            min={0}
            {...register("reminder_overdue_max_days", { valueAsNumber: true })}
          />
        </Field.Root>

        <Field.Root className={stylex.props(fieldStyle.row).className}>
          <Field.Label className={stylex.props(fieldStyle.label).className}>
            Check interval (hours)
          </Field.Label>
          <Field.Control
            className={stylex.props(fieldStyle.input).className}
            type="number"
            min={1}
            {...register("reminder_check_interval_hours", {
              valueAsNumber: true,
            })}
          />
        </Field.Root>

        <Field.Root className={stylex.props(fieldStyle.row).className}>
          <Field.Label className={stylex.props(fieldStyle.label).className}>
            Timezone (IANA)
          </Field.Label>
          <Field.Control
            className={stylex.props(fieldStyle.input).className}
            placeholder="UTC"
            {...register("reminder_timezone")}
          />
        </Field.Root>
      </div>

      <div {...stylex.props(filters.row, s.filtersSpaced)}>
        <Button
          className={stylex.props(button.secondary).className}
          type="button"
          onClick={onSave}
          disabled={save.isMutating}
        >
          {save.isMutating ? "Saving…" : "Save settings"}
        </Button>
        {feedback && (
          <span
            {...stylex.props(
              feedback.tone === "ok" ? text.resultOk : text.resultError,
            )}
          >
            {feedback.text}
          </span>
        )}
      </div>
    </div>
  );
}

// ── User map ─────────────────────────────────────────────────────────────────

interface MappingForm {
  linear_email: string;
  lark_email: string;
  note: string;
}

const EMPTY_MAPPING: MappingForm = {
  linear_email: "",
  lark_email: "",
  note: "",
};

function UserMapCard() {
  const { data: rows, error } = useData(listUserMap, {
    cacheTags: [TAG_LINEAR_USER_MAP],
  });

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors },
  } = useForm<MappingForm>({ defaultValues: EMPTY_MAPPING });
  const [feedback, setFeedback] = useState<Feedback>(null);
  const [target, setTarget] = useState<string | null>(null);

  const save = useMutation(upsertUserMap, {
    onSuccess: () => void invalidate(TAG_LINEAR_USER_MAP),
  });
  const remove = useMutation(deleteUserMap, {
    onSuccess: () => void invalidate(TAG_LINEAR_USER_MAP),
  });

  const onSave = handleSubmit(async (form) => {
    setFeedback(null);
    try {
      await save.trigger({
        body: {
          linear_email: form.linear_email.trim(),
          lark_email: form.lark_email.trim(),
          note: form.note.trim() || null,
        },
      });
      setFeedback({ tone: "ok", text: `mapped ${form.linear_email.trim()}` });
      reset(EMPTY_MAPPING);
    } catch (e) {
      setFeedback({ tone: "error", text: errMessage(e) });
    }
  });

  const confirmDelete = async () => {
    if (!target) return;
    setFeedback(null);
    try {
      await remove.trigger({ path: { linear_email: target } });
    } catch (e) {
      setFeedback({ tone: "error", text: errMessage(e) });
    } finally {
      setTarget(null);
    }
  };

  return (
    <div {...stylex.props(card.base, s.userMapCard)}>
      <div {...stylex.props(typo.mono12, s.subsystem)}>
        user map (Linear → Lark email)
      </div>
      <p {...stylex.props(text.help)}>
        Override the DM target when a person's Linear and Lark emails differ.
        When they match, no entry is needed.
      </p>

      <div {...stylex.props(s.fields)}>
        <Field.Root
          className={stylex.props(fieldStyle.row).className}
          invalid={!!errors.linear_email}
        >
          <Field.Label className={stylex.props(fieldStyle.label).className}>
            linear_email<span {...stylex.props(fieldStyle.required)}> *</span>
          </Field.Label>
          <Field.Control
            className={stylex.props(fieldStyle.input).className}
            placeholder="alice@linear.example"
            {...register("linear_email", {
              required: "linear_email is required",
            })}
          />
          {errors.linear_email && (
            <Field.Error
              className={stylex.props(fieldStyle.error).className}
              match
            >
              {errors.linear_email.message}
            </Field.Error>
          )}
        </Field.Root>
        <Field.Root
          className={stylex.props(fieldStyle.row).className}
          invalid={!!errors.lark_email}
        >
          <Field.Label className={stylex.props(fieldStyle.label).className}>
            lark_email<span {...stylex.props(fieldStyle.required)}> *</span>
          </Field.Label>
          <Field.Control
            className={stylex.props(fieldStyle.input).className}
            placeholder="alice@lark.example"
            {...register("lark_email", { required: "lark_email is required" })}
          />
          {errors.lark_email && (
            <Field.Error
              className={stylex.props(fieldStyle.error).className}
              match
            >
              {errors.lark_email.message}
            </Field.Error>
          )}
        </Field.Root>
        <Field.Root className={stylex.props(fieldStyle.row).className}>
          <Field.Label className={stylex.props(fieldStyle.label).className}>
            note
          </Field.Label>
          <Field.Control
            className={stylex.props(fieldStyle.input).className}
            placeholder="optional"
            {...register("note")}
          />
        </Field.Root>
      </div>

      <div {...stylex.props(filters.row, s.filtersSpaced)}>
        <Button
          className={stylex.props(button.secondary).className}
          type="button"
          onClick={onSave}
          disabled={save.isMutating}
        >
          {save.isMutating ? "Saving…" : "Save mapping"}
        </Button>
        {feedback && (
          <span
            {...stylex.props(
              feedback.tone === "ok" ? text.resultOk : text.resultError,
            )}
          >
            {feedback.text}
          </span>
        )}
      </div>

      {error !== undefined && (
        <p className="error">Failed to load: {errMessage(error)}</p>
      )}
      {rows && rows.length > 0 && (
        <table style={{ marginTop: "1.5rem" }}>
          <thead>
            <tr>
              <th>linear_email</th>
              <th>lark_email</th>
              <th>note</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {rows.map((m) => (
              <tr key={m.linear_email}>
                <td>
                  <code>{m.linear_email}</code>
                </td>
                <td>
                  <code>{m.lark_email}</code>
                </td>
                <td className="muted">{m.note ?? ""}</td>
                <td style={{ textAlign: "right", whiteSpace: "nowrap" }}>
                  <Button
                    className={
                      stylex.props(button.action, button.actionError).className
                    }
                    onClick={() => setTarget(m.linear_email)}
                  >
                    Delete
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {rows && rows.length === 0 && (
        <p {...stylex.props(text.help)}>No overrides yet.</p>
      )}

      <AlertDialog.Root
        open={target !== null}
        onOpenChange={(open) => {
          if (!open) setTarget(null);
        }}
      >
        <AlertDialog.Portal>
          <AlertDialog.Backdrop
            className={stylex.props(dialog.backdrop).className}
          />
          <AlertDialog.Popup className={stylex.props(dialog.popup).className}>
            <AlertDialog.Title className={stylex.props(dialog.title).className}>
              Delete mapping for "{target}"?
            </AlertDialog.Title>
            <AlertDialog.Description
              className={stylex.props(dialog.desc).className}
            >
              DMs will fall back to the Linear email. This cannot be undone.
            </AlertDialog.Description>
            <div {...stylex.props(dialog.actions)}>
              <AlertDialog.Close
                className={stylex.props(button.action).className}
                disabled={remove.isMutating}
              >
                Cancel
              </AlertDialog.Close>
              <Button
                className={
                  stylex.props(button.action, button.actionError).className
                }
                type="button"
                onClick={confirmDelete}
                disabled={remove.isMutating}
              >
                {remove.isMutating ? "Deleting…" : "Delete"}
              </Button>
            </div>
          </AlertDialog.Popup>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </div>
  );
}

export function Linear() {
  return (
    <section>
      <h2>Linear</h2>
      <LarkBinding appName="linear" />
      <RoutingEditor appName="linear" />
      <SettingsCard />
      <UserMapCard />
    </section>
  );
}
