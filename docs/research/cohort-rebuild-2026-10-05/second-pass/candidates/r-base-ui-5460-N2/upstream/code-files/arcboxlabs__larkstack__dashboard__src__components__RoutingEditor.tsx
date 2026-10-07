import { Button } from "@base-ui/react/button";
import { Combobox } from "@base-ui/react/combobox";
import { Input } from "@base-ui/react/input";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import * as stylex from "@stylexjs/stylex";
import { useEffect, useRef, useState } from "react";
import { invalidate, routingTag } from "../lib/cache";
import { errMessage } from "../lib/errors";
import {
  type ChatInfo,
  type Destination,
  type DestKind,
  getRouting,
  getRoutingSpec,
  listRoutingChats,
  listRoutingUsers,
  putRouting,
  type RoutingConfig,
  type RoutingSpec,
  type UserInfo,
} from "../lib/routing-api";
import { useData, useMutation } from "../lib/tayori";
import { banner, button, card, mono } from "../theme/shared";
import { colors, effects, fonts, radii } from "../theme/tokens.stylex";
import { typo } from "../theme/typography";
import { Select } from "./Select";
import { Spinner } from "./Spinner";

// A refined settings panel: strong type hierarchy, structured rule cards,
// segmented event chips, aligned destination rows — surface ladder + hairlines.
const s = stylex.create({
  editor: {
    padding: "1.4rem 1.45rem 1.2rem",
  },
  lead: {
    color: colors.muted,
    margin: "0 0 1.4rem",
    maxWidth: "64ch",
  },
  // Picker-unavailable banner inside the editor — spacing above the rules.
  notice: {
    marginBottom: "1.4rem",
  },
  // Sibling spacing between sections; apply to every section after the first.
  sectionSpaced: {
    marginTop: "1.85rem",
  },
  sectionHead: {
    display: "flex",
    alignItems: "baseline",
    gap: "0.6rem",
    flexWrap: "wrap",
    paddingBottom: "0.6rem",
    marginBottom: "1rem",
    borderBottomWidth: "1px",
    borderBottomStyle: "solid",
    borderBottomColor: colors.hairline,
  },
  sectionTitle: {
    color: colors.ink,
  },
  sectionHint: {
    color: colors.muted,
  },
  // Per-app subject explanation (spec.subject.help), tucked under the head rule.
  sectionHelp: {
    color: colors.muted,
    margin: "-0.6rem 0 1.1rem",
    maxWidth: "64ch",
  },
  empty: {
    color: colors.mutedSoft,
    margin: "0 0 0.8rem",
  },
  // Rule card — one surface step above the editor card, hairline only.
  rule: {
    position: "relative",
    borderColor: colors.hairline,
    borderStyle: "solid",
    borderWidth: "1px",
    borderRadius: radii.lg,
    backgroundColor: colors.surfacePop,
    padding: "0.5rem 1.1rem 1.1rem",
    marginBottom: "0.75rem",
  },
  ruleHead: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "0.5rem",
    height: "2.1rem",
    marginBottom: "0.35rem",
    borderBottomWidth: "1px",
    borderBottomStyle: "solid",
    borderBottomColor: colors.hairlineSoft,
  },
  // typo.label supplies the caps/size/tracking; only the mono face is local.
  ruleBadge: {
    fontFamily: fonts.mono,
    color: colors.mutedSoft,
  },
  // Remove-rule — quiet text button, reddens on hover.
  removeRule: {
    padding: "0.32rem 0.6rem",
    borderColor: "transparent",
    borderStyle: "solid",
    borderWidth: "1px",
    borderRadius: radii.sm,
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in srgb, ${colors.error} 11%, transparent)`,
    },
    color: { default: colors.muted, ":hover": colors.error },
    cursor: "pointer",
    transitionProperty: "background-color, color",
    transitionDuration: "0.15s",
  },
  // Field block — label (with inline hint) stacked above the control.
  field: {
    display: "grid",
    gap: "0.4rem",
    marginTop: "0.9rem",
  },
  fieldLabel: {
    display: "flex",
    alignItems: "baseline",
    gap: "0.5rem",
    color: colors.muted,
  },
  // Sits inline inside the caps label — undo the inherited uppercase.
  fieldHint: {
    textTransform: "none",
    color: colors.mutedSoft,
  },
  // Inputs — recessed surface, hairline, lavender focus ring.
  input: {
    padding: "0.46rem 0.65rem",
    borderColor: {
      default: colors.hairlineStrong,
      ":focus": colors.hairlineStrong,
      ":focus-visible": colors.hairlineStrong,
    },
    borderStyle: "solid",
    borderWidth: "1px",
    borderRadius: radii.md,
    backgroundColor: colors.canvasSoft,
    color: colors.ink,
    outline: { ":focus": "none", ":focus-visible": "none" },
    boxShadow: {
      ":focus": effects.focusRing,
      ":focus-visible": effects.focusRing,
    },
    "::placeholder": {
      color: colors.mutedSoft,
    },
    transitionProperty: "border-color, background-color, box-shadow",
    transitionDuration: "0.15s",
  },
  fullWidth: {
    width: "100%",
  },
  // Event toggle chips — segmented; invert to ink on select (button-inverse).
  chips: {
    display: "flex",
    flexWrap: "wrap",
    gap: "0.4rem",
  },
  chip: {
    display: "inline-flex",
    alignItems: "center",
    gap: "0.3rem",
    padding: "0.38rem 0.72rem",
    borderColor: { default: colors.hairlineStrong, ":hover": colors.mutedSoft },
    borderStyle: "solid",
    borderWidth: "1px",
    borderRadius: radii.pill,
    backgroundColor: colors.surfaceCard,
    color: { default: colors.body, ":hover": colors.ink },
    cursor: "pointer",
    userSelect: "none",
    transitionProperty: "background-color, border-color, color",
    transitionDuration: "0.15s",
  },
  chipPressed: {
    backgroundColor: colors.ink,
    color: colors.canvas,
    borderColor: colors.ink,
  },
  chipCheck: {
    display: "inline-flex",
    fontSize: "0.6875rem",
    marginLeft: "-0.1rem",
  },
  // Destination / mapping rows — aligned flex: kind, target (grows), remove.
  dests: {
    marginTop: "0.9rem",
  },
  destLabel: {
    display: "block",
    color: colors.muted,
    marginBottom: "0.5rem",
  },
  dest: {
    display: "flex",
    alignItems: "center",
    gap: "0.5rem",
    marginBottom: "0.5rem",
  },
  // The target field grows; kind select / arrow / remove stay fixed.
  destGrow: {
    flex: "1",
    minWidth: 0,
  },
  arrow: {
    flex: "none",
    color: colors.mutedSoft,
    fontSize: "0.875rem",
  },
  // Kind select — looks like a routing input with a chevron (base pill styling
  // comes from Select; this ports only the overrides).
  kindSelect: {
    flex: "none",
    minWidth: "9rem",
    padding: "0.46rem 0.65rem",
    borderRadius: radii.md,
    borderColor: colors.hairlineStrong,
    backgroundColor: colors.canvasSoft,
  },
  // Ghost icon button (remove a row).
  iconBtn: {
    flex: "none",
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "1.95rem",
    height: "1.95rem",
    fontSize: "1.125rem",
    lineHeight: 1,
    borderColor: "transparent",
    borderStyle: "solid",
    borderWidth: "1px",
    borderRadius: radii.md,
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in srgb, ${colors.error} 11%, transparent)`,
    },
    color: { default: colors.muted, ":hover": colors.error },
    cursor: "pointer",
    transitionProperty: "background-color, color",
    transitionDuration: "0.15s",
  },
  // Add buttons — dashed, become solid on hover.
  add: {
    display: "inline-flex",
    alignItems: "center",
    gap: "0.4rem",
    padding: "0.44rem 0.85rem",
    borderColor: { default: colors.hairlineStrong, ":hover": colors.mutedSoft },
    borderStyle: { default: "dashed", ":hover": "solid" },
    borderWidth: "1px",
    borderRadius: radii.md,
    backgroundColor: { default: "transparent", ":hover": colors.surfaceCard },
    color: { default: colors.body, ":hover": colors.ink },
    cursor: "pointer",
    transitionProperty: "border-color, background-color, color",
    transitionDuration: "0.15s",
  },
  // typo.callout base with the interactive-label weight kept.
  addSubtle: {
    fontWeight: 500,
    padding: "0.34rem 0.7rem",
    marginTop: "0.1rem",
  },
  addIcon: {
    fontSize: "1rem",
    lineHeight: 1,
    color: colors.muted,
  },
  // Footer — divider + primary save (lavender CTA) + inline feedback.
  footer: {
    display: "flex",
    alignItems: "center",
    gap: "0.9rem",
    marginTop: "1.85rem",
    paddingTop: "1.15rem",
    borderTopWidth: "1px",
    borderTopStyle: "solid",
    borderTopColor: colors.hairline,
  },
  feedbackOk: {
    color: colors.success,
  },
  feedbackError: {
    color: colors.error,
    wordBreak: "break-word",
  },
  feedbackDirty: {
    color: colors.warning,
  },
  // Combobox (searchable chat/user picker) — input + inset chevron trigger.
  comboControl: {
    position: "relative",
    display: "flex",
    alignItems: "center",
    width: "100%",
    minWidth: 0,
  },
  comboInput: {
    width: "100%",
    paddingRight: "1.7rem",
  },
  comboTrigger: {
    position: "absolute",
    right: "0.5rem",
    display: "inline-flex",
    alignItems: "center",
    borderWidth: 0,
    backgroundColor: "transparent",
    color: { default: colors.muted, ":hover": colors.ink },
    padding: 0,
    cursor: "pointer",
    transitionProperty: "color",
    transitionDuration: "0.15s",
  },
  selectIcon: {
    fontSize: "0.6875rem",
    color: colors.muted,
  },
  comboPopup: {
    backgroundColor: colors.surfacePop,
    color: colors.ink,
    borderColor: colors.hairlineStrong,
    borderStyle: "solid",
    borderWidth: "1px",
    borderRadius: radii.md,
    padding: "0.3rem",
    boxShadow: effects.popShadow,
    zIndex: 10,
    width: "var(--anchor-width)",
    minWidth: "15rem",
    maxHeight: "16rem",
    overflowY: "auto",
  },
  comboEmpty: {
    padding: "0.5rem",
    color: colors.muted,
  },
  comboItem: {
    display: "flex",
    alignItems: "flex-start",
    gap: "0.4rem",
    padding: "0.4rem 0.55rem 0.4rem 0.35rem",
    borderRadius: radii.sm,
    cursor: "pointer",
    userSelect: "none",
    outline: "none",
  },
  comboItemHighlighted: {
    backgroundColor: colors.surfaceStrong,
  },
  itemIndicator: {
    width: "1rem",
    display: "inline-flex",
    justifyContent: "center",
    fontSize: "0.75rem",
    color: colors.primaryHover,
  },
  comboItemText: {
    display: "flex",
    flexDirection: "column",
    gap: "0.05rem",
    // Deliberate sub-ramp leading: keeps the stacked name + id pair compact.
    lineHeight: 1.2,
  },
  comboItemId: {
    color: colors.mutedSoft,
  },
});

// ── Editable shape: rows carry a stable client key (avoids index keys) and
//    alert_labels is edited as a CSV string. ──────────────────────────────────

interface DestRow extends Destination {
  key: number;
}
interface RuleRow {
  key: number;
  match: string;
  events: string[];
  destinations: DestRow[];
}
interface UserMapRow {
  key: number;
  username: string;
  lark_email: string;
}
interface EditState {
  rules: RuleRow[];
  default_destinations: DestRow[];
  user_map: UserMapRow[];
  alert_labels: string;
}

type Feedback = { tone: "ok" | "error"; text: string } | null;

export interface RoutingEditorProps {
  /** App name; backs the API base `/api/apps/<appName>/routing`. */
  appName: string;
}

export function RoutingEditor({ appName }: RoutingEditorProps) {
  const { data: spec, error: specError } = useData(getRoutingSpec, {
    path: { app: appName },
  });
  const { data, error } = useData(getRouting, {
    path: { app: appName },
    cacheTags: [routingTag(appName)],
  });
  // The bot's chats + reachable users power the searchable pickers; absent (503)
  // when the app is stopped or has no bot — the fields then fall back to manual
  // entry (retries are disabled globally, so a stopped app doesn't retry-storm).
  const { data: chats, error: chatsError } = useData(
    listRoutingChats,
    spec?.features.chat_picker && { path: { app: appName } },
  );
  const { data: users, error: usersError } = useData(
    listRoutingUsers,
    spec?.features.user_picker && { path: { app: appName } },
  );
  const [edit, setEdit] = useState<EditState | null>(null);
  const [feedback, setFeedback] = useState<Feedback>(null);
  // Wire-shape snapshot of the last loaded/saved config, for the dirty check.
  const baseline = useRef<string | null>(null);
  const keyer = useRef(0);
  const nextKey = () => {
    keyer.current += 1;
    return keyer.current;
  };

  // Hydrate the editable state once the config loads. Uses a local key counter
  // (seeded from the ref) so the effect depends only on `data`.
  useEffect(() => {
    if (!data) return;
    let k = keyer.current;
    const nk = () => {
      k += 1;
      return k;
    };
    const dests = (ds: Destination[]): DestRow[] =>
      ds.map((d) => ({ key: nk(), kind: d.kind, target: d.target }));
    setEdit({
      rules: data.rules.map((r) => ({
        key: nk(),
        match: r.match,
        events: r.events,
        destinations: dests(r.destinations),
      })),
      default_destinations: dests(data.default_destinations),
      user_map: data.user_map.map((m) => ({
        key: nk(),
        username: m.username,
        lark_email: m.lark_email,
      })),
      alert_labels: data.alert_labels.join(", "),
    });
    keyer.current = k;
    // The server config is already in wire shape, so it doubles as the dirty
    // baseline (matches what `toWire` produces from a freshly-hydrated edit).
    baseline.current = JSON.stringify(data);
  }, [data]);

  const save = useMutation(putRouting, {
    onSuccess: () => void invalidate(routingTag(appName)),
  });

  // Dirty when the edited config diverges from the loaded/saved baseline.
  const dirty =
    edit !== null &&
    spec !== undefined &&
    baseline.current !== null &&
    JSON.stringify(toWire(edit, spec)) !== baseline.current;

  // Warn before a full page unload (reload / close) while there are unsaved
  // edits. In-app navigation isn't guarded — that needs a data router.
  useEffect(() => {
    if (!dirty) return;
    const onBeforeUnload = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [dirty]);

  if (error) {
    return <p className="error">Failed to load: {errMessage(error)}</p>;
  }
  if (specError) {
    return (
      <p className="error">
        Failed to load routing spec: {errMessage(specError)}
      </p>
    );
  }
  if (!edit || !spec) {
    return <Spinner />;
  }

  const onSave = async () => {
    setFeedback(null);
    try {
      await save.trigger({ path: { app: appName }, body: toWire(edit, spec) });
      setFeedback({ tone: "ok", text: "routing saved" });
    } catch (e) {
      setFeedback({ tone: "error", text: errMessage(e) });
    }
  };

  // Pickers are offered by the spec but the chat/user list failed to load —
  // the app is stopped or has no bound bot. Manual entry still works.
  const pickersUnavailable =
    (spec.features.chat_picker && !!chatsError) ||
    (spec.features.user_picker && !!usersError);

  const newDest = (): DestRow => ({ key: nextKey(), kind: "chat", target: "" });

  const addRule = () =>
    setEdit((s) =>
      s
        ? {
            ...s,
            rules: [
              ...s.rules,
              {
                key: nextKey(),
                match: "",
                events: [],
                destinations: [newDest()],
              },
            ],
          }
        : s,
    );
  const removeRule = (key: number) =>
    setEdit((s) =>
      s ? { ...s, rules: s.rules.filter((r) => r.key !== key) } : s,
    );

  return (
    <div {...stylex.props(card.base, card.stacked, s.editor)}>
      <p {...stylex.props(typo.body, s.lead)}>
        Route events to Lark group chats or DMs — pick from the bot's chats and
        users, or type a <code {...stylex.props(mono.inlineCode)}>chat_id</code>{" "}
        / email. Changes apply live, no restart. Delivery needs a bound{" "}
        <code {...stylex.props(mono.inlineCode)}>lark_app</code> bot.
      </p>

      {pickersUnavailable && (
        <div {...stylex.props(banner.warn, s.notice)}>
          <span>
            Chat / user pickers are unavailable — the app is stopped or has no
            bound Lark app. Enter a <code>chat_id</code> / <code>open_id</code>{" "}
            / email by hand, or start the app to pick from a list.
          </span>
        </div>
      )}

      {/* ── Rules ── */}
      <section>
        <div {...stylex.props(s.sectionHead)}>
          <span {...stylex.props(typo.title3, s.sectionTitle)}>
            Routing rules
          </span>
          <span {...stylex.props(typo.callout, s.sectionHint)}>
            every matching rule contributes its destinations
          </span>
        </div>

        {spec.subject.help && (
          <p {...stylex.props(typo.callout, s.sectionHelp)}>
            {spec.subject.help}
          </p>
        )}

        {edit.rules.length === 0 && (
          <p {...stylex.props(typo.body, s.empty)}>
            No rules yet — unmatched events fall through to the defaults below.
          </p>
        )}

        {edit.rules.map((rule, i) => (
          <div key={rule.key} {...stylex.props(s.rule)}>
            <div {...stylex.props(s.ruleHead)}>
              <span {...stylex.props(typo.label, s.ruleBadge)}>
                Rule {i + 1}
              </span>
              <Button
                type="button"
                className={stylex.props(typo.callout, s.removeRule).className}
                onClick={() => removeRule(rule.key)}
              >
                Remove
              </Button>
            </div>

            <div {...stylex.props(s.field)}>
              <label
                {...stylex.props(typo.label, s.fieldLabel)}
                htmlFor={`match-${rule.key}`}
              >
                {spec.subject.label}
                <span {...stylex.props(typo.footnote, s.fieldHint)}>
                  exact, “{spec.subject.placeholder}/*”, or “*” for all
                </span>
              </label>
              <Input
                id={`match-${rule.key}`}
                className={stylex.props(typo.body, s.input).className}
                placeholder={spec.subject.placeholder}
                value={rule.match}
                onChange={(e) =>
                  setEdit((s) =>
                    patchRule(s, rule.key, (r) => ({
                      ...r,
                      match: e.target.value,
                    })),
                  )
                }
              />
            </div>

            <div {...stylex.props(s.field)}>
              <span {...stylex.props(typo.label, s.fieldLabel)}>
                Events
                <span {...stylex.props(typo.footnote, s.fieldHint)}>
                  none selected = all
                </span>
              </span>
              <ToggleGroup
                className={stylex.props(s.chips).className}
                multiple
                value={rule.events}
                onValueChange={(events) =>
                  setEdit((s) =>
                    patchRule(s, rule.key, (r) => ({ ...r, events })),
                  )
                }
              >
                {spec.events.map((opt) => (
                  <Toggle
                    key={opt.value}
                    value={opt.value}
                    className={(state) =>
                      stylex.props(
                        typo.bodyMedium,
                        s.chip,
                        state.pressed && s.chipPressed,
                      ).className ?? ""
                    }
                    aria-label={opt.label}
                    title={opt.description}
                  >
                    {/* Rendered only while selected so the flex gap collapses
                        cleanly when hidden (mirrors the old display:none). */}
                    {rule.events.includes(opt.value) && (
                      <span {...stylex.props(s.chipCheck)}>✓</span>
                    )}
                    {opt.label}
                  </Toggle>
                ))}
              </ToggleGroup>
            </div>

            <DestinationList
              dests={rule.destinations}
              chats={chats}
              users={users}
              onChange={(ds) =>
                setEdit((s) =>
                  patchRule(s, rule.key, (r) => ({ ...r, destinations: ds })),
                )
              }
              onAdd={() =>
                setEdit((s) =>
                  patchRule(s, rule.key, (r) => ({
                    ...r,
                    destinations: [...r.destinations, newDest()],
                  })),
                )
              }
            />
          </div>
        ))}

        <Button
          type="button"
          className={stylex.props(typo.bodyMedium, s.add).className}
          onClick={addRule}
        >
          <span {...stylex.props(s.addIcon)}>+</span> Add rule
        </Button>
      </section>

      {/* ── Default destinations ── */}
      <section {...stylex.props(s.sectionSpaced)}>
        <div {...stylex.props(s.sectionHead)}>
          <span {...stylex.props(typo.title3, s.sectionTitle)}>
            Default destinations
          </span>
          <span {...stylex.props(typo.callout, s.sectionHint)}>
            used when no rule matches — empty drops the event
          </span>
        </div>
        <DestinationList
          dests={edit.default_destinations}
          chats={chats}
          users={users}
          hideLabel
          onChange={(ds) =>
            setEdit((s) => (s ? { ...s, default_destinations: ds } : s))
          }
          onAdd={() =>
            setEdit((s) =>
              s
                ? {
                    ...s,
                    default_destinations: [
                      ...s.default_destinations,
                      newDest(),
                    ],
                  }
                : s,
            )
          }
        />
      </section>

      {/* ── Reviewer user map ── */}
      {spec.features.user_map && (
        <section {...stylex.props(s.sectionSpaced)}>
          <div {...stylex.props(s.sectionHead)}>
            <span {...stylex.props(typo.title3, s.sectionTitle)}>
              Reviewer user map
            </span>
            <span {...stylex.props(typo.callout, s.sectionHint)}>
              source username → Lark email
            </span>
          </div>
          {edit.user_map.map((m) => (
            <div key={m.key} {...stylex.props(s.dest)}>
              <Input
                className={
                  stylex.props(typo.body, s.input, s.destGrow).className
                }
                placeholder="username"
                value={m.username}
                onChange={(e) =>
                  setEdit((s) =>
                    patchUser(s, m.key, (u) => ({
                      ...u,
                      username: e.target.value,
                    })),
                  )
                }
              />
              <span {...stylex.props(s.arrow)}>→</span>
              <Input
                className={
                  stylex.props(typo.body, s.input, s.destGrow).className
                }
                placeholder="lark@email"
                value={m.lark_email}
                onChange={(e) =>
                  setEdit((s) =>
                    patchUser(s, m.key, (u) => ({
                      ...u,
                      lark_email: e.target.value,
                    })),
                  )
                }
              />
              <Button
                type="button"
                className={stylex.props(s.iconBtn).className}
                aria-label="Remove mapping"
                onClick={() =>
                  setEdit((s) =>
                    s
                      ? {
                          ...s,
                          user_map: s.user_map.filter((u) => u.key !== m.key),
                        }
                      : s,
                  )
                }
              >
                ×
              </Button>
            </div>
          ))}
          <Button
            type="button"
            className={stylex.props(typo.callout, s.add, s.addSubtle).className}
            onClick={() =>
              setEdit((s) =>
                s
                  ? {
                      ...s,
                      user_map: [
                        ...s.user_map,
                        { key: nextKey(), username: "", lark_email: "" },
                      ],
                    }
                  : s,
              )
            }
          >
            <span {...stylex.props(s.addIcon)}>+</span> Add mapping
          </Button>
        </section>
      )}

      {/* ── Alert labels ── */}
      {spec.features.alert_labels && (
        <section {...stylex.props(s.sectionSpaced)}>
          <div {...stylex.props(s.sectionHead)}>
            <span {...stylex.props(typo.title3, s.sectionTitle)}>
              Alert labels
            </span>
            <span {...stylex.props(typo.callout, s.sectionHint)}>
              comma-separated; these labels trigger an alert card
            </span>
          </div>
          <Input
            className={stylex.props(typo.body, s.input, s.fullWidth).className}
            placeholder="bug, urgent, p0"
            value={edit.alert_labels}
            onChange={(e) =>
              setEdit((s) => (s ? { ...s, alert_labels: e.target.value } : s))
            }
          />
        </section>
      )}

      <div {...stylex.props(s.footer)}>
        <Button
          type="button"
          className={stylex.props(button.primary).className}
          onClick={onSave}
          disabled={save.isMutating || !dirty}
        >
          {save.isMutating ? "Saving…" : "Save routing"}
        </Button>
        {/* A save error always wins; otherwise show the dirty hint, else the
            last "saved" confirmation. */}
        {feedback?.tone === "error" ? (
          <span {...stylex.props(typo.callout, s.feedbackError)}>
            {feedback.text}
          </span>
        ) : dirty ? (
          <span {...stylex.props(typo.callout, s.feedbackDirty)}>
            unsaved changes
          </span>
        ) : (
          feedback?.tone === "ok" && (
            <span {...stylex.props(typo.callout, s.feedbackOk)}>
              {feedback.text}
            </span>
          )
        )}
      </div>
    </div>
  );
}

function DestinationList({
  dests,
  chats,
  users,
  onChange,
  onAdd,
  hideLabel = false,
}: {
  dests: DestRow[];
  chats: ChatInfo[] | undefined;
  users: UserInfo[] | undefined;
  onChange: (ds: DestRow[]) => void;
  onAdd: () => void;
  /** Omit the "Destinations" sub-label (the section header already names it). */
  hideLabel?: boolean;
}) {
  const patch = (key: number, fn: (d: DestRow) => DestRow) =>
    onChange(dests.map((x) => (x.key === key ? fn(x) : x)));
  // Picker sources: chats keyed by chat_id, users keyed by open_id.
  const chatItems = chats?.map((c) => ({ value: c.chat_id, label: c.name }));
  const userItems = users?.map((u) => ({ value: u.open_id, label: u.name }));
  return (
    <div {...stylex.props(s.dests)}>
      {!hideLabel && <span {...stylex.props(s.destLabel)}>Destinations</span>}
      {dests.map((d) => (
        <div key={d.key} {...stylex.props(s.dest)}>
          <Select
            trigger={s.kindSelect}
            value={d.kind}
            onValueChange={(v) =>
              // Switching kind clears the target — a chat_id and a user id aren't
              // interchangeable, and the picker source differs.
              patch(d.key, (x) => ({ ...x, kind: v as DestKind, target: "" }))
            }
            options={[
              { value: "chat", label: "Group chat" },
              { value: "dm", label: "Direct message" },
            ]}
          />
          <PickerField
            items={d.kind === "chat" ? chatItems : userItems}
            value={d.target}
            onChange={(target) => patch(d.key, (x) => ({ ...x, target }))}
            searchPlaceholder={
              d.kind === "chat" ? "Search group chats…" : "Search users…"
            }
            manualPlaceholder={
              d.kind === "chat" ? "chat_id (oc_…)" : "open_id / email"
            }
            emptyLabel={
              d.kind === "chat" ? "No matching chats" : "No matching users"
            }
          />
          <Button
            type="button"
            className={stylex.props(s.iconBtn).className}
            aria-label="Remove destination"
            onClick={() => onChange(dests.filter((x) => x.key !== d.key))}
          >
            ×
          </Button>
        </div>
      ))}
      <Button
        type="button"
        className={stylex.props(typo.callout, s.add, s.addSubtle).className}
        onClick={onAdd}
      >
        <span {...stylex.props(s.addIcon)}>+</span> Add destination
      </Button>
    </div>
  );
}

interface PickerItem {
  /** The stored value: a chat_id or a user open_id. */
  value: string;
  /** The human label: a chat or user display name. */
  label: string;
}

/**
 * A destination-target field: a searchable Select over `items` (the bot's chats
 * or reachable users), matched by display name but storing the underlying id.
 * Since the bot can only deliver to chats/users it can reach, picking from the
 * fetched list is also the correct constraint. Falls back to a plain text input
 * when the list is unavailable (app stopped / no bot / 503), so a `chat_id`,
 * `open_id`, or email can still be entered by hand.
 */
function PickerField({
  items,
  value,
  onChange,
  searchPlaceholder,
  manualPlaceholder,
  emptyLabel,
}: {
  items: PickerItem[] | undefined;
  value: string;
  onChange: (value: string) => void;
  searchPlaceholder: string;
  manualPlaceholder: string;
  emptyLabel: string;
}) {
  if (!items || items.length === 0) {
    return (
      <Input
        className={stylex.props(typo.body, s.input, s.destGrow).className}
        placeholder={manualPlaceholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    );
  }

  // Build the candidate id list keyed to labels. Include the current value as a
  // synthetic entry when it's a saved id (or email) not among the fetched items,
  // so it still shows and stays selected.
  const byId = new Map(items.map((i) => [i.value, i.label]));
  const ids = items.map((i) => i.value);
  if (value && !byId.has(value)) {
    ids.unshift(value);
    byId.set(value, value);
  }
  const labelOf = (id: string) => byId.get(id) ?? id;

  return (
    <Combobox.Root
      items={ids}
      value={value || null}
      onValueChange={(v) => onChange((v as string | null) ?? "")}
      itemToStringLabel={labelOf}
    >
      <span {...stylex.props(s.comboControl, s.destGrow)}>
        <Combobox.Input
          className={stylex.props(typo.body, s.input, s.comboInput).className}
          placeholder={searchPlaceholder}
        />
        <Combobox.Trigger
          className={stylex.props(s.comboTrigger).className}
          aria-label="Open"
        >
          <Combobox.Icon className={stylex.props(s.selectIcon).className}>
            ▾
          </Combobox.Icon>
        </Combobox.Trigger>
      </span>
      <Combobox.Portal>
        <Combobox.Positioner sideOffset={4} align="start">
          <Combobox.Popup className={stylex.props(s.comboPopup).className}>
            <Combobox.Empty
              className={stylex.props(typo.callout, s.comboEmpty).className}
            >
              {emptyLabel}
            </Combobox.Empty>
            <Combobox.List>
              {(id: string) => (
                <Combobox.Item
                  key={id}
                  value={id}
                  className={(state) =>
                    stylex.props(
                      typo.body,
                      s.comboItem,
                      state.highlighted && s.comboItemHighlighted,
                    ).className ?? ""
                  }
                >
                  <Combobox.ItemIndicator
                    className={stylex.props(s.itemIndicator).className}
                  >
                    ✓
                  </Combobox.ItemIndicator>
                  <span {...stylex.props(s.comboItemText)}>
                    <span>{byId.get(id) ?? id}</span>
                    <span {...stylex.props(typo.mono11, s.comboItemId)}>
                      {id}
                    </span>
                  </span>
                </Combobox.Item>
              )}
            </Combobox.List>
          </Combobox.Popup>
        </Combobox.Positioner>
      </Combobox.Portal>
    </Combobox.Root>
  );
}

// ── Helpers ──────────────────────────────────────────────────────────────────

function patchRule(
  s: EditState | null,
  key: number,
  fn: (r: RuleRow) => RuleRow,
): EditState | null {
  if (!s) return s;
  return { ...s, rules: s.rules.map((r) => (r.key === key ? fn(r) : r)) };
}

function patchUser(
  s: EditState | null,
  key: number,
  fn: (u: UserMapRow) => UserMapRow,
): EditState | null {
  if (!s) return s;
  return { ...s, user_map: s.user_map.map((u) => (u.key === key ? fn(u) : u)) };
}

function toWire(e: EditState, spec: RoutingSpec): RoutingConfig {
  const dest = (d: DestRow): Destination => ({
    kind: d.kind,
    target: d.target.trim(),
  });
  return {
    rules: e.rules.map((r) => ({
      match: r.match.trim(),
      events: r.events,
      destinations: r.destinations.map(dest).filter((d) => d.target.length > 0),
    })),
    default_destinations: e.default_destinations
      .map(dest)
      .filter((d) => d.target.length > 0),
    user_map: spec.features.user_map
      ? e.user_map
          .map((m) => ({
            username: m.username.trim(),
            lark_email: m.lark_email.trim(),
          }))
          .filter((m) => m.username.length > 0 && m.lark_email.length > 0)
      : [],
    alert_labels: spec.features.alert_labels
      ? e.alert_labels
          .split(",")
          .map((s) => s.trim())
          .filter((s) => s.length > 0)
      : [],
  };
}
