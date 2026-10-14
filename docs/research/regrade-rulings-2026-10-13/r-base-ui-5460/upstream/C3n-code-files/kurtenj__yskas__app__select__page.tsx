"use client";

import { useState } from "react";
import { useQuery, useMutation } from "convex/react";
import { api } from "@/convex/_generated/api";
import { useUser } from "@/lib/user-context";
import { useRouter } from "next/navigation";
import { Id } from "@/convex/_generated/dataModel";
import { Button } from "@base-ui/react/button";
import { Field } from "@base-ui/react/field";
import { Input } from "@base-ui/react/input";
import { positiveGoal } from "@/lib/nutrition";
import { CaretRight } from "@phosphor-icons/react";

export default function SelectPage() {
  const users = useQuery(api.users.list);
  const createUser = useMutation(api.users.create);
  const { setUserId } = useUser();
  const router = useRouter();

  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState("");
  const [goal, setGoal] = useState("1800");
  const [error, setError] = useState("");
  const [creating, setCreating] = useState(false);

  function selectUser(id: Id<"users">) {
    setUserId(id);
    router.push("/");
  }

  function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim() || !goal) return;
    let dailyCalorieGoal: number;
    try {
      dailyCalorieGoal = positiveGoal(Number(goal));
    } catch {
      setError("Enter a positive calorie goal.");
      return;
    }
    setError("");
    setCreating(true);
    createUser({
      name: name.trim(),
      dailyCalorieGoal,
    })
      .then((id) => {
        setUserId(id);
        router.push("/");
      })
      .catch(() => setError("Could not create profile. Please try again."))
      .finally(() => setCreating(false));
  }

  if (users === undefined) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="w-6 h-6 border-2 border-mist-800 border-t-mist-100 rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-mist-950 flex flex-col items-center justify-center px-6">
      <div className="w-full max-w-sm">
        <div className="text-center mb-8">
          <h1 className="text-2xl font-bold text-mist-50">
            Who&apos;s tracking?
          </h1>
          <p className="text-mist-400 mt-1 text-sm">
            Select your profile to continue
          </p>
        </div>

        {error && (
          <p role="alert" className="text-sm text-mist-200 mb-3">
            {error}
          </p>
        )}
        {!showCreate ? (
          <div className="space-y-3">
            {users.map((user) => (
              <Button
                key={user._id}
                onClick={() => selectUser(user._id)}
                className="w-full bg-mist-900 hover:bg-mist-800 border border-mist-700 rounded-xl px-5 py-4 text-left transition-colors"
              >
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-mist-50 font-semibold">{user.name}</p>
                    <p className="text-mist-400 text-sm mt-0.5">
                      Goal: {user.dailyCalorieGoal.toLocaleString()} cal/day
                    </p>
                  </div>
                  <CaretRight size={20} className="text-mist-600" />
                </div>
              </Button>
            ))}

            <Button
              onClick={() => setShowCreate(true)}
              className="w-full border border-dashed border-mist-700 hover:border-mist-400 rounded-xl px-5 py-4 text-mist-500 hover:text-mist-200 transition-colors"
            >
              Add profile
            </Button>
          </div>
        ) : (
          <form onSubmit={handleCreate} className="space-y-4">
            <Field.Root>
              <Field.Label className="block text-mist-400 text-sm mb-1.5">
                Name
              </Field.Label>
              <Input
                type="text"
                maxLength={80}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. Jonathan"
                className="w-full bg-mist-900 text-mist-50 rounded-xl px-4 py-3 border border-mist-800 focus:outline-none focus:border-mist-400 placeholder:text-mist-600"
                autoFocus
              />
            </Field.Root>

            <Field.Root>
              <Field.Label className="block text-mist-400 text-sm mb-1.5">
                Daily calorie goal
              </Field.Label>
              <Input
                type="number"
                value={goal}
                onChange={(e) => setGoal(e.target.value)}
                min="0.1"
                step="any"
                className="w-full bg-mist-900 text-mist-50 rounded-xl px-4 py-3 border border-mist-800 focus:outline-none focus:border-mist-400 placeholder:text-mist-600"
              />
              <p className="text-mist-500 text-xs mt-1.5">
                Typical deficit goal: 1,500–1,800 cal/day
              </p>
            </Field.Root>

            <div className="flex gap-3">
              <Button
                type="button"
                onClick={() => {
                  setShowCreate(false);
                  setName("");
                  setGoal("1800");
                }}
                className="flex-1 bg-mist-800 hover:bg-mist-700 text-mist-200 rounded-xl py-3 font-medium transition-colors"
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={!name.trim() || !goal || creating}
                className="flex-1 bg-mist-100 hover:bg-mist-200 disabled:bg-mist-800 disabled:text-mist-600 text-mist-950 rounded-xl py-3 font-semibold transition-colors"
              >
                {creating ? "Creating..." : "Create"}
              </Button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
