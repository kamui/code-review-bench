"use client";

import { useState } from "react";
import { useQuery, useMutation } from "convex/react";
import { api } from "@/convex/_generated/api";
import { useUser } from "@/lib/user-context";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { Button } from "@base-ui/react/button";
import { Field } from "@base-ui/react/field";
import { Input } from "@base-ui/react/input";
import { positiveGoal } from "@/lib/nutrition";
import { ArrowLeft, Heart } from "@phosphor-icons/react";

export default function SettingsPage() {
  const { userId, clearUser } = useUser();
  const user = useQuery(api.users.get, userId ? { id: userId } : "skip");
  const updateGoal = useMutation(api.users.updateGoal);
  const updateName = useMutation(api.users.updateName);
  const router = useRouter();

  const [goalInput, setGoalInput] = useState("");
  const [proteinInput, setProteinInput] = useState<string | null>(null);
  const [fiberInput, setFiberInput] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [nameInput, setNameInput] = useState("");
  const [goalSaved, setGoalSaved] = useState(false);
  const [nameSaved, setNameSaved] = useState(false);

  if (!userId || !user) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="w-6 h-6 border-2 border-mist-800 border-t-mist-100 rounded-full animate-spin" />
      </div>
    );
  }

  async function handleSaveGoal(e: React.FormEvent) {
    e.preventDefault();
    if (!userId || !user || saving) return;
    setError("");
    setSaving(true);
    try {
      await updateGoal({
        id: userId,
        dailyCalorieGoal: positiveGoal(
          goalInput.trim() ? Number(goalInput) : user.dailyCalorieGoal,
        ),
        ...(proteinInput === null
          ? {}
          : {
              dailyProteinGoal: proteinInput.trim()
                ? positiveGoal(Number(proteinInput))
                : null,
            }),
        ...(fiberInput === null
          ? {}
          : {
              dailyFiberGoal: fiberInput.trim()
                ? positiveGoal(Number(fiberInput))
                : null,
            }),
      });
      setGoalInput("");
      setProteinInput(null);
      setFiberInput(null);
      setGoalSaved(true);
      setTimeout(() => setGoalSaved(false), 2000);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not save goals.");
    } finally {
      setSaving(false);
    }
  }

  async function handleSaveName(e: React.FormEvent) {
    e.preventDefault();
    if (!userId || !nameInput.trim() || saving) return;
    setError("");
    setSaving(true);
    try {
      await updateName({ id: userId, name: nameInput.trim() });
      setNameInput("");
      setNameSaved(true);
      setTimeout(() => setNameSaved(false), 2000);
    } catch {
      setError("Could not save name. Please try again.");
    } finally {
      setSaving(false);
    }
  }

  function handleSwitchUser() {
    clearUser();
    router.push("/select");
  }

  return (
    <div className="px-4 pt-6 max-w-lg mx-auto">
      <div className="flex items-center gap-3 mb-1">
        <Link
          href="/"
          aria-label="Back to home"
          className="text-mist-400 hover:text-mist-100 transition-colors"
        >
          <ArrowLeft size={20} weight="bold" />
        </Link>
        <h1 className="text-xl font-bold text-mist-50">Settings</h1>
      </div>
      <p className="text-mist-400 text-sm mb-6">Manage your profile</p>

      {error && (
        <p role="alert" className="text-sm text-mist-200 mb-4">
          {error}
        </p>
      )}
      {/* Profile info */}
      <div className="bg-mist-900 border border-mist-800 rounded-xl p-4 mb-6">
        <p className="text-mist-500 text-xs uppercase tracking-wide mb-1">
          Current profile
        </p>
        <p className="text-mist-50 font-semibold text-lg">{user?.name}</p>
        <p className="text-mist-400 text-sm mt-0.5">
          Daily goal:{" "}
          <span className="text-mist-50 font-medium">
            {user?.dailyCalorieGoal.toLocaleString()} cal
          </span>
        </p>
      </div>

      {/* Change name */}
      <div className="mb-5">
        <h2 className="text-mist-50 font-medium mb-3">Change name</h2>
        <form onSubmit={handleSaveName} className="flex gap-3">
          <Field.Root className="flex-1">
            <Input
              type="text"
              aria-label="Name"
              maxLength={80}
              value={nameInput}
              onChange={(e) => setNameInput(e.target.value)}
              placeholder={user?.name ?? "Name"}
              className="w-full bg-mist-900 text-mist-50 rounded-xl px-4 py-3 border border-mist-800 focus:outline-none focus:border-mist-400 placeholder:text-mist-600"
            />
          </Field.Root>
          <Button
            type="submit"
            disabled={!nameInput.trim() || saving}
            className="bg-mist-100 hover:bg-mist-200 disabled:bg-mist-800 disabled:text-mist-600 text-mist-950 font-semibold px-4 rounded-xl transition-colors whitespace-nowrap"
          >
            {nameSaved ? "Saved!" : "Save"}
          </Button>
        </form>
      </div>

      <div className="mb-8">
        <h2 className="text-mist-50 font-medium mb-3">Daily goals</h2>
        <form onSubmit={handleSaveGoal} className="flex flex-col gap-3">
          {[
            {
              label: "Calories (kcal)",
              value: goalInput,
              current: user.dailyCalorieGoal,
              change: setGoalInput,
              required: true,
            },
            {
              label: "Protein (g)",
              value: proteinInput ?? String(user.dailyProteinGoal ?? ""),
              current: user.dailyProteinGoal,
              change: setProteinInput,
              required: false,
            },
            {
              label: "Fiber (g)",
              value: fiberInput ?? String(user.dailyFiberGoal ?? ""),
              current: user.dailyFiberGoal,
              change: setFiberInput,
              required: false,
            },
          ].map((field) => (
            <Field.Root key={field.label}>
              <Field.Label className="block text-sm text-mist-400 mb-1">
                {field.label}
              </Field.Label>
              <Input
                type="number"
                min="0"
                step="any"
                value={field.value}
                onChange={(e) => field.change(e.target.value)}
                placeholder={
                  field.current === undefined
                    ? "Not set"
                    : String(field.current)
                }
                className="w-full bg-mist-900 text-mist-50 rounded-xl px-4 py-3 border border-mist-800 focus:outline-none focus:border-mist-400 placeholder:text-mist-600"
              />
            </Field.Root>
          ))}
          <p className="text-xs text-mist-500">
            Leave protein or fiber blank to remove its goal. Goals use grams per
            day.
          </p>
          <Button
            type="submit"
            disabled={
              saving ||
              (!goalInput && proteinInput === null && fiberInput === null)
            }
            className="bg-mist-100 text-mist-950 rounded-xl py-3 font-semibold disabled:opacity-40"
          >
            {goalSaved ? "Saved!" : saving ? "Saving..." : "Save goals"}
          </Button>
        </form>
      </div>

      {/* Switch user */}
      <div className="border-t border-mist-800 pt-6">
        <Button
          onClick={handleSwitchUser}
          className="w-full bg-mist-800 hover:bg-mist-700 text-mist-200 font-medium py-3 rounded-xl transition-colors"
        >
          Switch profile
        </Button>
      </div>

      <footer className="mt-8 mb-4 text-center text-mist-600 text-xs leading-relaxed">
        <p className="flex items-center justify-center gap-1">
          Grown in the Midwest{" "}
          <Heart weight="fill" className="text-rose-500" size={12} />
        </p>
        <p>Yskas v{process.env.NEXT_PUBLIC_APP_VERSION}</p>
      </footer>
    </div>
  );
}
