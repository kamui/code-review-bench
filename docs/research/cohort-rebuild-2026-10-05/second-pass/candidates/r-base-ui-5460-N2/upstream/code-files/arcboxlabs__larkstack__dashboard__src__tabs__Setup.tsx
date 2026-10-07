import { Button } from "@base-ui/react/button";
import { Field } from "@base-ui/react/field";
import * as stylex from "@stylexjs/stylex";
import { useEffect, useState } from "react";
import { Link } from "react-router";
import { RegisterLarkApp } from "../components/RegisterLarkApp";
import { Select } from "../components/Select";
import { TAG_CONSOLE_AUTH, TAG_LARK_APPS } from "../lib/cache";
import { errMessage } from "../lib/errors";
import { useData, useMutation } from "../lib/tayori";
import { getConsoleAuth, listLarkApps, putConsoleAuth } from "../sdk";
import { banner, button, field, filters, text } from "../theme/shared";
import { colors, radii } from "../theme/tokens.stylex";
import { typo } from "../theme/typography";

const s = stylex.create({
  steps: {
    listStyle: "none",
    margin: 0,
    padding: 0,
    display: "grid",
    gap: "2.25rem",
  },
  stepHead: {
    display: "flex",
    alignItems: "center",
    gap: "0.65rem",
    marginBottom: "0.6rem",
  },
  stepNum: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "1.6rem",
    height: "1.6rem",
    borderRadius: radii.pill,
    borderColor: colors.hairlineStrong,
    borderStyle: "solid",
    borderWidth: "1px",
    backgroundColor: colors.surfaceCard,
    color: colors.ink,
    // typo.callout supplies the 12px badge digit; the weight bumps to 600
    // locally so the number holds its own inside the pill.
    fontWeight: 600,
  },
  // Step 2's number dims until step 1 (register an app) is done.
  stepNumMuted: {
    color: colors.muted,
  },
  stepTitle: {
    color: colors.ink,
  },
  done: {
    color: colors.success,
  },
  callback: {
    display: "inline-block",
    padding: "0.4rem 0.65rem",
    borderColor: colors.hairline,
    borderStyle: "solid",
    borderWidth: "1px",
    borderRadius: radii.sm,
    backgroundColor: colors.canvas,
    color: colors.ink,
    wordBreak: "break-all",
  },
  // The action-fields block, with extra room below the callback URL.
  fields: {
    display: "grid",
    gap: "0.45rem",
    marginTop: "1rem",
    paddingTop: "0.7rem",
    borderTopColor: colors.hairlineStrong,
    borderTopStyle: "dashed",
    borderTopWidth: "1px",
  },
  // The admin-list textarea, merged onto field.input (font via typo.mono13).
  admins: {
    minHeight: "3rem",
    resize: "vertical",
  },
  filtersSpaced: {
    marginTop: "0.5rem",
  },
});

/// Split an admin list pasted as comma / whitespace / newline separated.
function parseAdmins(raw: string): string[] {
  return raw
    .split(/[\s,]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

/// First-run guided setup: register a Lark app, then bind it as the console's
/// sign-in client. Binding is the moment the console stops being open — this
/// screen makes that explicit and hands the operator straight to the login flow.
export function Setup() {
  const { data: registry } = useData(listLarkApps, {
    cacheTags: [TAG_LARK_APPS],
  });
  const { data: current } = useData(getConsoleAuth, {
    cacheTags: [TAG_CONSOLE_AUTH],
  });
  const apps = registry?.lark_apps ?? [];
  const hasApp = apps.length > 0;

  const callbackUrl = `${window.location.origin}/auth/callback`;

  const [larkApp, setLarkApp] = useState("");
  const [admins, setAdmins] = useState("");
  const [error, setError] = useState<string | null>(null);

  // Prefill from any existing binding, and default the dropdown to the first
  // registered app once the registry loads.
  useEffect(() => {
    if (current?.lark_app) setLarkApp(current.lark_app);
    else if (apps[0]) setLarkApp((v) => v || apps[0].name);
    if (current?.admins?.length) setAdmins(current.admins.join(", "));
  }, [current, apps]);

  const bind = useMutation(putConsoleAuth);

  const onSecure = async () => {
    setError(null);
    if (!larkApp) {
      setError("Choose a Lark app to sign in with.");
      return;
    }
    try {
      await bind.trigger({
        body: {
          lark_app: larkApp,
          admins: parseAdmins(admins),
        },
      });
      // Binding now enforces sign-in; hand off to the OAuth flow.
      window.location.assign("/auth/login");
    } catch (e) {
      setError(errMessage(e));
    }
  };

  return (
    <section>
      <h2>Secure your console</h2>
      <div {...stylex.props(banner.warn, banner.top)}>
        ⚠ This console is <strong>open</strong> — anyone who can reach it has
        full admin access. Bind a Lark app below to require sign-in.
      </div>

      <ol {...stylex.props(s.steps)}>
        <li>
          <div {...stylex.props(s.stepHead)}>
            <span {...stylex.props(typo.callout, s.stepNum)}>1</span>
            <span {...stylex.props(typo.title3, s.stepTitle)}>
              Register a Lark app
            </span>
            {hasApp && (
              <span {...stylex.props(typo.callout, s.done)}>✓ done</span>
            )}
          </div>
          <p {...stylex.props(text.help)}>
            Create a custom app in the{" "}
            <a
              href="https://open.larksuite.com/app"
              target="_blank"
              rel="noreferrer"
            >
              Lark Developer Console
            </a>
            , then add its credentials here. Saving live-tests them against
            Lark.
          </p>
          {hasApp ? (
            <p {...stylex.props(text.help)}>
              Registered: {apps.map((a) => a.name).join(", ")}. Add or manage
              more in the <Link to="/lark-apps">Lark Apps</Link> tab.
            </p>
          ) : (
            <RegisterLarkApp onSaved={(n) => setLarkApp(n)} />
          )}
        </li>

        <li>
          <div {...stylex.props(s.stepHead)}>
            <span
              {...stylex.props(
                typo.callout,
                s.stepNum,
                !hasApp && s.stepNumMuted,
              )}
            >
              2
            </span>
            <span {...stylex.props(typo.title3, s.stepTitle)}>
              Bind console sign-in
            </span>
          </div>
          <p {...stylex.props(text.help)}>
            In your Lark app's security settings, register this redirect URI and
            grant the user-info permission:
          </p>
          <code {...stylex.props(typo.mono13, s.callback)}>{callbackUrl}</code>

          <div {...stylex.props(s.fields)}>
            <Field.Root className={stylex.props(field.row).className}>
              <Field.Label
                className={stylex.props(field.label).className}
                htmlFor="setup-lark-app"
              >
                sign in with
              </Field.Label>
              <Select
                id="setup-lark-app"
                trigger={[field.input, field.selectTrigger]}
                value={larkApp}
                onValueChange={setLarkApp}
                disabled={!hasApp}
                options={
                  hasApp
                    ? apps.map((a) => ({ value: a.name, label: a.name }))
                    : [{ value: "", label: "register an app first" }]
                }
              />
            </Field.Root>
            <Field.Root className={stylex.props(field.row).className}>
              <Field.Label
                className={stylex.props(field.label).className}
                htmlFor="setup-admins"
              >
                admin emails
              </Field.Label>
              <Field.Control
                id="setup-admins"
                className={
                  stylex.props(field.input, typo.mono13, s.admins).className
                }
                placeholder="you@example.com, teammate@example.com"
                value={admins}
                onChange={(e) => setAdmins(e.target.value)}
                disabled={!hasApp}
                render={<textarea />}
              />
            </Field.Root>
          </div>
          <p {...stylex.props(text.help)}>
            Only these Lark accounts may sign in. <strong>Leave empty</strong>{" "}
            to allow any user in your tenant.
          </p>
          <p {...stylex.props(text.help)}>
            After saving, sign-in is required immediately — make sure you can
            sign in with one of the emails above (or an empty list). If you get
            locked out, clear <code>[console].lark_app</code> in{" "}
            <code>config.toml</code> on the server to reopen it.
          </p>

          <div {...stylex.props(filters.row, s.filtersSpaced)}>
            <Button
              className={stylex.props(button.secondary).className}
              type="button"
              onClick={onSecure}
              disabled={!hasApp || bind.isMutating}
            >
              {bind.isMutating ? "Securing…" : "Secure & sign in"}
            </Button>
            <Link
              className={stylex.props(button.action).className}
              to="/status"
            >
              Skip for now
            </Link>
            {error && <span {...stylex.props(text.resultError)}>{error}</span>}
          </div>
        </li>
      </ol>
    </section>
  );
}
