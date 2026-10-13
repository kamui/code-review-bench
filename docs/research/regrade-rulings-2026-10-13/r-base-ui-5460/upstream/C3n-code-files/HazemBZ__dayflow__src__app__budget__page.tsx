"use client";

import { useState, useEffect, useCallback } from "react";
import { startOfWeek, format, parseISO, addDays } from "date-fns";
import { Plus, AlertCircle, CheckCircle2 } from "lucide-react";

import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Progress as ProgressPrimitive } from "@base-ui/react/progress";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";

import { PageScroll } from "@/components/ui/page-scroll";

import { getTimeLogsForWeek, logTime } from "@/lib/actions/weekly";
import { TIME_CATEGORIES } from "@/lib/constants";

type TimeLog = Awaited<ReturnType<typeof getTimeLogsForWeek>>[number];

function getMonday(): string {
  const now = new Date();
  const monday = startOfWeek(now, { weekStartsOn: 1 });
  return format(monday, "yyyy-MM-dd");
}

function getProgressColor(pct: number): string {
  if (pct < 80) return "bg-green-500";
  if (pct <= 100) return "bg-yellow-500";
  return "bg-red-500";
}

const defaultCat = TIME_CATEGORIES[0]?.value ?? "";

export default function BudgetPage() {
  const weekStart = getMonday();
  const mondayDate = parseISO(weekStart);
  const sundayDate = addDays(mondayDate, 6);
  const weekLabel = `${format(mondayDate, "MMM d")} – ${format(sundayDate, "MMM d, yyyy")}`;

  // ── Data state ──────────────────────────────────────────────────────
  const [timeLogs, setTimeLogs] = useState<TimeLog[]>([]);
  const [loading, setLoading] = useState(true);

  // ── Dialog state ────────────────────────────────────────────────────
  const [dialogOpen, setDialogOpen] = useState(false);
  const [formDate, setFormDate] = useState(() => new Date().toISOString().slice(0, 10));
  const [formCategory, setFormCategory] = useState<string>(defaultCat);
  const [formHours, setFormHours] = useState(1);
  const [formDescription, setFormDescription] = useState("");
  const [saving, setSaving] = useState(false);

  // ── Load data ───────────────────────────────────────────────────────
  useEffect(() => {
    async function load() {
      try {
        const logs = await getTimeLogsForWeek(weekStart);
        setTimeLogs(logs);
      } catch (e) {
        console.error("Failed to load time logs", e);
      } finally {
        setLoading(false);
      }
    }
    load();
  }, [weekStart]);

  // ── Derived: category totals ────────────────────────────────────────
  const categoryHours: Record<string, number> = {};
  for (const log of timeLogs) {
    categoryHours[log.category] = (categoryHours[log.category] ?? 0) + log.hours;
  }

  const totalHours = Object.values(categoryHours).reduce((a, b) => a + b, 0);
  const totalBudget = TIME_CATEGORIES.reduce((sum, cat) => sum + cat.max, 0);

  // ── Derived: daily breakdown ────────────────────────────────────────
  const daysMap: Record<string, TimeLog[]> = {};
  for (const log of timeLogs) {
    if (!daysMap[log.date]) daysMap[log.date] = [];
    daysMap[log.date].push(log);
  }
  const sortedDays = Object.entries(daysMap).sort(([a], [b]) => b.localeCompare(a));

  // ── Log time handler ────────────────────────────────────────────────
  const handleLogTime = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formDate || !formCategory || formHours <= 0) return;
    setSaving(true);
    try {
      await logTime({
        date: formDate,
        category: formCategory,
        hours: formHours,
        description: formDescription || undefined,
      });
      const logs = await getTimeLogsForWeek(weekStart);
      setTimeLogs(logs);
      setDialogOpen(false);
      setFormDate(new Date().toISOString().slice(0, 10));
      setFormCategory(defaultCat);
      setFormHours(1);
      setFormDescription("");
    } catch (e) {
      console.error("Failed to log time", e);
    } finally {
      setSaving(false);
    }
  }, [formDate, formCategory, formHours, formDescription, weekStart]);

  // ── Render ──────────────────────────────────────────────────────────
  if (loading) {
    return (
      <div className="mx-auto max-w-5xl space-y-6 py-6 px-4 md:px-6 lg:px-8">
        {/* Header */}
        <div className="space-y-3 rounded-xl border bg-background px-5 py-4 shadow-sm">
          <div className="h-7 w-36 animate-pulse rounded bg-muted" />
          <div className="h-4 w-56 animate-pulse rounded bg-muted/60" />
          <div className="h-4 w-40 animate-pulse rounded bg-muted/60" />
          <div className="h-9 w-28 animate-pulse rounded-lg bg-muted" />
        </div>

        {/* Category grid */}
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} className="space-y-3 rounded-xl border bg-card p-5 shadow-sm">
              <div className="flex items-center gap-2">
                <div className="h-2.5 w-2.5 animate-pulse rounded-full bg-muted" />
                <div className="h-5 w-24 animate-pulse rounded bg-muted" />
              </div>
              <div className="flex items-center justify-between">
                <div className="h-8 w-16 animate-pulse rounded bg-muted" />
                <div className="h-5 w-5 animate-pulse rounded bg-muted" />
              </div>
              <div className="h-1.5 w-full animate-pulse rounded-full bg-muted" />
            </div>
          ))}
        </div>

        {/* Daily Breakdown */}
        <div className="space-y-4 rounded-xl border bg-card p-5 shadow-sm">
          <div className="space-y-2">
            <div className="h-5 w-32 animate-pulse rounded bg-muted" />
            <div className="h-3 w-48 animate-pulse rounded bg-muted/60" />
          </div>
          <div className="space-y-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="space-y-2">
                <div className="flex items-center justify-between">
                  <div className="h-4 w-36 animate-pulse rounded bg-muted/60" />
                  <div className="h-3 w-10 animate-pulse rounded bg-muted/60" />
                </div>
                <div className="h-8 w-full animate-pulse rounded-lg bg-muted/30" />
                <div className="h-8 w-3/4 animate-pulse rounded-lg bg-muted/20" />
              </div>
            ))}
          </div>
        </div>

        {/* Weekly Summary */}
        <div className="space-y-4 rounded-xl border bg-card p-5 shadow-sm">
          <div className="space-y-2">
            <div className="h-5 w-32 animate-pulse rounded bg-muted" />
            <div className="h-3 w-56 animate-pulse rounded bg-muted/60" />
          </div>
          <div className="space-y-3">
            {Array.from({ length: 6 }).map((_, i) => (
              <div key={i} className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <div className="h-3 w-20 animate-pulse rounded bg-muted/60" />
                  <div className="h-3 w-24 animate-pulse rounded bg-muted/60" />
                </div>
                <div className="h-2 w-full animate-pulse rounded-full bg-muted" />
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  return (
    <PageScroll
      header={
        <div className="bg-background px-5 py-2 rounded-xl shadow-sm border flex flex-col gap-3">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">
              Time Budget
            </h1>
            <p className="mt-1 text-sm text-muted-foreground">{weekLabel}</p>
            <p className="mt-1 text-sm text-muted-foreground">
              <span className="tabular-nums">{totalHours.toFixed(1)}h</span> /{" "}
              <span className="tabular-nums">{totalBudget}h</span> total
            </p>
          </div>
          <Button onClick={() => setDialogOpen(true)} className="self-start">
            <Plus className="size-4" />
            Log Time
          </Button>
        </div>
      }
      maxWidth="max-w-5xl"
      scrollContentClass="space-y-6 pt-6"
    >

      {/* ── Budget Overview Cards ───────────────────────────── */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {TIME_CATEGORIES.map((cat) => {
          const logged = categoryHours[cat.value] ?? 0;
          const rawPct = (logged / cat.max) * 100;
          const pct = Math.min(Math.round(rawPct), 100);
          const indicatorColor = getProgressColor(rawPct);
          const overBudget = rawPct > 100;

          return (
            <Card key={cat.value}>
              <CardHeader>
                <CardTitle>
                  <div className="flex items-center gap-2">
                    <div
                      className={`h-2.5 w-2.5 shrink-0 rounded-full ${cat.color}`}
                    />
                    <span className="truncate">{cat.label}</span>
                  </div>
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-2xl font-semibold tabular-nums">
                    {logged.toFixed(1)}
                    <span className="text-sm font-normal text-muted-foreground">
                      /{cat.max}h
                    </span>
                  </span>
                  {overBudget ? (
                    <AlertCircle className="size-5 shrink-0 text-red-500" />
                  ) : (
                    <CheckCircle2 className="size-5 shrink-0 text-green-500" />
                  )}
                </div>
                <ProgressPrimitive.Root
                  value={pct}
                  className="flex flex-wrap gap-3"
                >
                  <div className="relative flex h-1.5 w-full items-center overflow-x-hidden rounded-full bg-muted">
                    <ProgressPrimitive.Indicator
                      className={`h-full rounded-full ${indicatorColor} transition-all`}
                    />
                  </div>
                </ProgressPrimitive.Root>
              </CardContent>
            </Card>
          );
        })}
      </div>

      {/* ── Daily Breakdown ─────────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle>Daily Breakdown</CardTitle>
          <CardDescription>
            Time logged each day this week
          </CardDescription>
        </CardHeader>
        <CardContent>
          {sortedDays.length === 0 ? (
            <p className="py-6 text-center text-sm text-muted-foreground">
              No time logged this week yet
            </p>
          ) : (
            <div className="space-y-6">
              {sortedDays.map(([date, logs]) => {
                const dayDate = parseISO(date);
                const dayLabel = format(dayDate, "EEEE, MMM d");
                const dayTotal = logs.reduce((sum, l) => sum + l.hours, 0);

                return (
                  <div key={date}>
                    <div className="flex items-center justify-between">
                      <h3 className="text-sm font-medium">{dayLabel}</h3>
                      <span className="text-xs text-muted-foreground tabular-nums">
                        {dayTotal.toFixed(1)}h
                      </span>
                    </div>
                    <div className="mt-2 space-y-1.5">
                      {logs.map((log) => {
                        const catConfig = TIME_CATEGORIES.find(
                          (c) => c.value === log.category,
                        );
                        return (
                          <div
                            key={log.id}
                            className="flex items-center gap-2 rounded-lg bg-muted/30 px-3 py-2 text-sm"
                          >
                            <Badge
                              variant="outline"
                              className="shrink-0 font-normal"
                            >
                              {catConfig?.label ?? log.category}
                            </Badge>
                            <span className="tabular-nums text-muted-foreground">
                              {log.hours}h
                            </span>
                            {log.description && (
                              <>
                                <span className="text-muted-foreground/50">
                                  ·
                                </span>
                                <span className="truncate text-muted-foreground">
                                  {log.description}
                                </span>
                              </>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      {/* ── Weekly Summary ──────────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle>Weekly Summary</CardTitle>
          <CardDescription>
            Hours per category vs weekly budget
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {TIME_CATEGORIES.map((cat) => {
            const logged = categoryHours[cat.value] ?? 0;
            const pct = Math.min((logged / cat.max) * 100, 100);

            return (
              <div key={cat.value} className="space-y-1.5">
                <div className="flex items-center justify-between text-sm">
                  <span className="font-medium">{cat.label}</span>
                  <span className="tabular-nums text-muted-foreground">
                    {logged.toFixed(1)}h / {cat.max}h
                  </span>
                </div>
                <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
                  <div
                    className={`h-full rounded-full ${cat.color} transition-all`}
                    style={{ width: `${pct}%` }}
                  />
                </div>
              </div>
            );
          })}
          {timeLogs.length === 0 && (
            <p className="py-6 text-center text-sm text-muted-foreground">
              No time logged this week yet
            </p>
          )}
        </CardContent>
      </Card>

      {/* ── Log Time Dialog ─────────────────────────────────── */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Log Time</DialogTitle>
            <DialogDescription>
              Record time spent on a category this week
            </DialogDescription>
          </DialogHeader>
          <form onSubmit={handleLogTime}>
            <div className="space-y-4 py-2">
              {/* Date */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-muted-foreground">
                  Date
                </label>
                <Input
                  type="date"
                  value={formDate}
                  onChange={(e) => setFormDate(e.target.value)}
                  required
                />
              </div>

              {/* Category */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-muted-foreground">
                  Category
                </label>
                <Select
                  value={formCategory}
                  onValueChange={(val) => { if (val) setFormCategory(val); }}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {TIME_CATEGORIES.map((cat) => (
                      <SelectItem key={cat.value} value={cat.value}>
                        {cat.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* Hours */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-muted-foreground">
                  Hours
                </label>
                <Input
                  type="number"
                  step={0.5}
                  min={0.5}
                  value={formHours}
                  onChange={(e) =>
                    setFormHours(parseFloat(e.target.value) || 0)
                  }
                  required
                />
              </div>

              {/* Description */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-muted-foreground">
                  Description{" "}
                  <span className="font-normal text-muted-foreground/60">
                    (optional)
                  </span>
                </label>
                <Input
                  value={formDescription}
                  onChange={(e) => setFormDescription(e.target.value)}
                  placeholder="What did you work on?"
                />
              </div>
            </div>
            <DialogFooter className="mt-4">
              <Button type="submit" disabled={saving}>
                {saving ? "Saving…" : "Save"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </PageScroll>
  );
}
