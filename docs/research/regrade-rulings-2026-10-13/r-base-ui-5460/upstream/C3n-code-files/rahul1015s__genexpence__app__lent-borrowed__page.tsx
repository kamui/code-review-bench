"use client"

import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react"
import type { AccordionValue } from "@base-ui/react/accordion"
import { ArrowDownRight, ArrowUpRight, Filter } from "lucide-react"
import { DashboardLayout } from "@/components/Dashboard/DashboardLayout"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { api, getApiErrorMessage } from "@/lib/http"
import { formatCurrency } from "@/lib/utils"
import { toast } from "sonner"
import {
  LentBorrowedTable,
  type LentBorrowedRow,
} from "@/components/Dashboard/LentBorrowedTable"
import { useAppLock } from "@/contexts/AppLockContext"

type LentBorrowedType = "lent" | "borrowed"
type LentBorrowedStatus = "pending" | "settled"

const sheetClasses = "!top-auto !right-0 !bottom-0 !left-0 !max-w-none !translate-x-0 !translate-y-0 gap-0 rounded-t-[22px] border-x-0 border-b-0 p-0 sm:!top-1/2 sm:!right-auto sm:!bottom-auto sm:!left-1/2 sm:!max-w-lg sm:!-translate-x-1/2 sm:!-translate-y-1/2 sm:rounded-2xl sm:border sm:p-0"

export default function LentBorrowedPage() {
  return (
    <DashboardLayout>
      <LentBorrowedPageContent />
    </DashboardLayout>
  )
}

function LentBorrowedPageContent() {
  const [items, setItems] = useState<LentBorrowedRow[]>([])
  const [currency, setCurrency] = useState("₹")
  const [isLoading, setIsLoading] = useState(true)
  const [isSaving, setIsSaving] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editOpen, setEditOpen] = useState(false)
  const [accordionValue, setAccordionValue] = useState<AccordionValue>([])
  const [viewItem, setViewItem] = useState<LentBorrowedRow | null>(null)
  const { isLocked } = useAppLock()

  const [filters, setFilters] = useState({
    type: "all",
    status: "all",
    search: "",
    limit: "10",
  })

  const [page, setPage] = useState(1)
  const [totalPages, setTotalPages] = useState(1)

  const [form, setForm] = useState({
    personName: "",
    phoneNumber: "",
    amount: "",
    type: "lent" as LentBorrowedType,
    date: new Date().toISOString().split("T")[0],
    dueDate: "",
    status: "pending" as LentBorrowedStatus,
    notes: "",
  })

  const totals = useMemo(() => {
    const lent = items
      .filter((item) => item.type === "lent" && item.status === "pending")
      .reduce((sum, item) => sum + item.amount, 0)
    const borrowed = items
      .filter((item) => item.type === "borrowed" && item.status === "pending")
      .reduce((sum, item) => sum + item.amount, 0)
    return { lent, borrowed }
  }, [items])

  const fetchItems = useCallback(
    async (pageNum: number = 1) => {
      try {
        setIsLoading(true)
        const params = new URLSearchParams()
        params.set("page", pageNum.toString())
        params.set("limit", filters.limit)
        if (filters.type !== "all") params.set("type", filters.type)
        if (filters.status !== "all") params.set("status", filters.status)
        if (filters.search.trim()) params.set("search", filters.search.trim())
        const { data } = await api.get(`/api/lent-borrowed?${params.toString()}`)
        setItems(data.items || [])
        setCurrency(data.currency || "₹")
        setTotalPages(data.pagination?.pages || 1)
        setPage(pageNum)
      } catch (error) {
        toast.error(getApiErrorMessage(error, "Failed to load lent & borrowed"))
      } finally {
        setIsLoading(false)
      }
    },
    [filters]
  )

  useEffect(() => {
    if (isLocked) {
      setIsLoading(false)
      return
    }

    void fetchItems(1)
  }, [fetchItems, isLocked])

  const handleSearch = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    void fetchItems(1)
  }

  const resetFilters = () => {
    setFilters({
      type: "all",
      status: "all",
      search: "",
      limit: "10",
    })
    void fetchItems(1)
  }

  const resetForm = () => {
    setForm({
      personName: "",
      phoneNumber: "",
      amount: "",
      type: "lent",
      date: new Date().toISOString().split("T")[0],
      dueDate: "",
      status: "pending",
      notes: "",
    })
    setEditingId(null)
  }

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()

    if (!form.personName.trim() || !form.amount) {
      toast.error("Person name and amount are required")
      return
    }

    try {
      setIsSaving(true)
      const payload = {
        personName: form.personName.trim(),
        phoneNumber: form.phoneNumber.trim(),
        amount: Number(form.amount),
        type: form.type,
        date: form.date,
        dueDate: form.dueDate || null,
        status: form.status,
        notes: form.notes.trim(),
      }

      if (editingId) {
        await api.put(`/api/lent-borrowed/${editingId}`, payload)
        toast.success("Record updated")
      } else {
        await api.post("/api/lent-borrowed", payload)
        toast.success("Record created")
      }

      resetForm()
      setAccordionValue([])
      setEditOpen(false)
      await fetchItems(page)
    } catch (error) {
      toast.error(getApiErrorMessage(error, "Failed to save record"))
    } finally {
      setIsSaving(false)
    }
  }

  const startEdit = (item: LentBorrowedRow) => {
    setEditingId(item._id)
    setForm({
      personName: item.personName,
      phoneNumber: item.phoneNumber || "",
      amount: item.amount.toString(),
      type: item.type,
      date: item.date.split("T")[0],
      dueDate: item.dueDate ? item.dueDate.split("T")[0] : "",
      status: item.status,
      notes: item.notes || "",
    })
    setAccordionValue([])
    setEditOpen(true)
  }

  const cancelEdit = () => {
    resetForm()
    setEditOpen(false)
  }

  const markSettled = async (item: LentBorrowedRow) => {
    try {
      await api.put(`/api/lent-borrowed/${item._id}`, { status: "settled" })
      toast.success("Marked as settled")
      await fetchItems(page)
    } catch (error) {
      toast.error(getApiErrorMessage(error, "Failed to update status"))
    }
  }

  const deleteItem = async (id: string) => {
    try {
      await api.delete(`/api/lent-borrowed/${id}`)
      toast.success("Record deleted")
      await fetchItems(page)
    } catch (error) {
      toast.error(getApiErrorMessage(error, "Failed to delete record"))
    }
  }

  const formFields = (
    <>
      <div className="space-y-2">
        <Label htmlFor="person-name">Person name</Label>
        <Input
          id="person-name"
          value={form.personName}
          onChange={(event) => setForm((current) => ({ ...current, personName: event.target.value }))}
          placeholder="Name"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="phone-number">Phone number</Label>
        <Input
          id="phone-number"
          value={form.phoneNumber}
          onChange={(event) => setForm((current) => ({ ...current, phoneNumber: event.target.value }))}
          placeholder="Optional"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="amount">Amount</Label>
        <Input
          id="amount"
          type="number"
          min="0.01"
          step="0.01"
          value={form.amount}
          onChange={(event) => setForm((current) => ({ ...current, amount: event.target.value }))}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="type">Type</Label>
        <Select
          value={form.type}
          onValueChange={(value) =>
            setForm((current) => ({ ...current, type: value as LentBorrowedType }))
          }
        >
          <SelectTrigger id="type">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="lent">Lent</SelectItem>
            <SelectItem value="borrowed">Borrowed</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div className="space-y-2">
        <Label htmlFor="date">Date</Label>
        <Input
          id="date"
          type="date"
          value={form.date}
          onChange={(event) => setForm((current) => ({ ...current, date: event.target.value }))}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="due-date">Due date</Label>
        <Input
          id="due-date"
          type="date"
          value={form.dueDate}
          onChange={(event) => setForm((current) => ({ ...current, dueDate: event.target.value }))}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="status">Status</Label>
        <Select
          value={form.status}
          onValueChange={(value) =>
            setForm((current) => ({ ...current, status: value as LentBorrowedStatus }))
          }
        >
          <SelectTrigger id="status">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="pending">Pending</SelectItem>
            <SelectItem value="settled">Settled</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div className="space-y-2 sm:col-span-2">
        <Label htmlFor="notes">Notes</Label>
        <Textarea
          id="notes"
          value={form.notes}
          onChange={(event) => setForm((current) => ({ ...current, notes: event.target.value }))}
          placeholder="Optional notes"
        />
      </div>
      <div className="flex flex-wrap gap-2 sm:col-span-2">
        <Button type="submit" disabled={isSaving}>
          {isSaving ? "Saving..." : editingId ? "Update record" : "Add record"}
        </Button>
        {editingId ? (
          <Button type="button" variant="outline" onClick={cancelEdit}>
            Cancel edit
          </Button>
        ) : null}
      </div>
    </>
  )

  if (isLocked) {
    return null
  }

  return (
    <>
      <div className="space-y-4 sm:space-y-6">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
          <div>
            <h1 className="text-2xl font-bold sm:text-3xl">Lent & Borrowed</h1>
            <p className="text-sm text-muted-foreground sm:text-base">
              Track temporary money movements without impacting cashflow totals.
            </p>
          </div>
          <Button
            variant="outline"
            onClick={() => {
              resetForm()
              setAccordionValue(["add-record"])
              window.scrollTo({ top: 0, behavior: "smooth" })
            }}
          >
            Add new record
          </Button>
        </div>

        <div className="grid gap-3 md:grid-cols-2 sm:gap-4">
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm font-medium">Outstanding Lent</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="flex items-center justify-between">
                <p className="text-xl font-bold text-emerald-600 sm:text-2xl">
                  {formatCurrency(totals.lent, currency)}
                </p>
                <ArrowUpRight className="h-5 w-5 text-emerald-600" />
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm font-medium">Outstanding Borrowed</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="flex items-center justify-between">
                <p className="text-xl font-bold text-amber-600 sm:text-2xl">
                  {formatCurrency(totals.borrowed, currency)}
                </p>
                <ArrowDownRight className="h-5 w-5 text-amber-600" />
              </div>
            </CardContent>
          </Card>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Add record</CardTitle>
          </CardHeader>
          <CardContent>
            <Accordion value={accordionValue} onValueChange={(value) => setAccordionValue(value ?? [])}>
              <AccordionItem value="add-record">
                <AccordionTrigger className="rounded-xl border border-border bg-muted/40 px-4 py-4 text-sm font-semibold hover:no-underline hover:bg-muted/60">
                  <div className="pr-4">
                    <div>Add new record details</div>
                    <div className="mt-1 text-xs font-normal text-muted-foreground">
                      Open the form to log money you lent or borrowed.
                    </div>
                  </div>
                </AccordionTrigger>
                <AccordionContent>
                  <form onSubmit={handleSubmit} className="grid gap-4 sm:grid-cols-2">
                    {formFields}
                  </form>
                </AccordionContent>
              </AccordionItem>
            </Accordion>
          </CardContent>
        </Card>

        <Card className="personal-panel rounded-[26px] border-border/70">
          <CardHeader className="pb-4">
            <CardTitle className="flex items-center gap-2 text-base">
              <Filter className="h-4 w-4" />
              Filters
            </CardTitle>
            <p className="text-sm text-muted-foreground">
              Search by person, narrow by type or status, and keep the record list focused.
            </p>
          </CardHeader>
          <CardContent className="pt-0">
            <form onSubmit={handleSearch} className="space-y-4">
              <div className="rounded-2xl border border-border/60 bg-background/55 p-4 sm:p-5">
                <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <p className="text-sm font-medium text-foreground">Record controls</p>
                    <p className="text-xs text-muted-foreground">
                      Focus the ledger before checking due amounts or settling balances.
                    </p>
                  </div>
                  <span className="rounded-full border border-border/70 bg-background/80 px-3 py-1 text-xs font-medium text-muted-foreground">
                    {items.length} records in view
                  </span>
                </div>

                <div className="grid gap-4 md:grid-cols-4">
                  <div className="space-y-2 md:col-span-2">
                    <Label htmlFor="search">Search</Label>
                    <Input
                      id="search"
                      placeholder="Search by person, phone, notes"
                      value={filters.search}
                      onChange={(event) => setFilters((current) => ({ ...current, search: event.target.value }))}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="filter-type">Type</Label>
                    <Select
                      value={filters.type}
                      onValueChange={(value) =>
                        setFilters((current) => ({ ...current, type: value ?? "all" }))
                      }
                    >
                      <SelectTrigger id="filter-type" className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="all">All types</SelectItem>
                        <SelectItem value="lent">Lent</SelectItem>
                        <SelectItem value="borrowed">Borrowed</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="filter-status">Status</Label>
                    <Select
                      value={filters.status}
                      onValueChange={(value) =>
                        setFilters((current) => ({ ...current, status: value ?? "all" }))
                      }
                    >
                      <SelectTrigger id="filter-status" className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="all">All statuses</SelectItem>
                        <SelectItem value="pending">Pending</SelectItem>
                        <SelectItem value="settled">Settled</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <div className="mt-4 grid gap-4 sm:grid-cols-[minmax(0,1fr)_auto_auto] sm:items-end">
                  <div className="space-y-2">
                    <Label htmlFor="limit">Per Page</Label>
                    <Select
                      value={filters.limit}
                      onValueChange={(value) =>
                        setFilters((current) => ({ ...current, limit: value ?? "10" }))
                      }
                    >
                      <SelectTrigger id="limit" className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="10">10</SelectItem>
                        <SelectItem value="25">25</SelectItem>
                        <SelectItem value="50">50</SelectItem>
                        <SelectItem value="100">100</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <Button type="button" variant="outline" className="w-full sm:w-auto" onClick={resetFilters}>
                    Reset
                  </Button>
                  <Button type="submit" className="w-full sm:w-auto">Apply filters</Button>
                </div>
              </div>
            </form>
          </CardContent>
        </Card>

        <LentBorrowedTable
          items={items}
          currency={currency}
          loading={isLoading}
          currentPage={page}
          totalPages={totalPages}
          onPageChange={fetchItems}
          onView={(item) => setViewItem(item)}
          onEdit={startEdit}
          onDelete={(item) => deleteItem(item._id)}
          onSettle={markSettled}
        />
      </div>

      <Dialog open={Boolean(viewItem)} onOpenChange={(open) => !open && setViewItem(null)}>
        <DialogContent className={sheetClasses}>
          <DialogHeader className="border-b px-4 py-3 sm:border-b-0 sm:px-0 sm:py-0">
            <div className="mx-auto mb-2 h-1.5 w-12 rounded-full bg-muted sm:hidden" />
            <DialogTitle className="px-0 sm:px-4 sm:pt-4">Lent/Borrowed Details</DialogTitle>
            <DialogDescription className="px-0 sm:px-4">Review this record before making changes.</DialogDescription>
          </DialogHeader>
          {viewItem ? (
            <div className="space-y-4 px-4 pb-6 pt-4 sm:px-4 sm:pb-4">
              <div className="rounded-xl border p-4">
                <p className="text-sm text-muted-foreground">Person</p>
                <p className="mt-1 text-lg font-semibold">{viewItem.personName}</p>
                <p className="text-xs text-muted-foreground">
                  {viewItem.phoneNumber || "No phone number"}
                </p>
              </div>
              <div className="grid gap-4 sm:grid-cols-2">
                <div className="rounded-xl border p-4">
                  <p className="text-sm text-muted-foreground">Amount</p>
                  <p className="mt-1 text-lg font-semibold">
                    {formatCurrency(viewItem.amount, currency)}
                  </p>
                </div>
                <div className="rounded-xl border p-4">
                  <p className="text-sm text-muted-foreground">Type</p>
                  <p className="mt-1 text-lg font-semibold capitalize">{viewItem.type}</p>
                </div>
                <div className="rounded-xl border p-4">
                  <p className="text-sm text-muted-foreground">Status</p>
                  <p className="mt-1 text-lg font-semibold capitalize">{viewItem.status}</p>
                </div>
                <div className="rounded-xl border p-4">
                  <p className="text-sm text-muted-foreground">Date</p>
                  <p className="mt-1 text-lg font-semibold">
                    {new Date(viewItem.date).toLocaleDateString("en-IN", {
                      year: "numeric",
                      month: "long",
                      day: "numeric",
                    })}
                  </p>
                </div>
                <div className="rounded-xl border p-4">
                  <p className="text-sm text-muted-foreground">Due Date</p>
                  <p className="mt-1 text-lg font-semibold">
                    {viewItem.dueDate
                      ? new Date(viewItem.dueDate).toLocaleDateString("en-IN", {
                          year: "numeric",
                          month: "long",
                          day: "numeric",
                        })
                      : "—"}
                  </p>
                </div>
              </div>
              {viewItem.notes ? (
                <div className="rounded-xl border p-4">
                  <p className="text-sm text-muted-foreground">Notes</p>
                  <p className="mt-1 whitespace-pre-wrap">{viewItem.notes}</p>
                </div>
              ) : null}
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => setViewItem(null)}>
                  Close
                </Button>
                <Button
                  onClick={() => {
                    startEdit(viewItem)
                    setViewItem(null)
                  }}
                >
                  Edit Record
                </Button>
              </div>
            </div>
          ) : null}
        </DialogContent>
      </Dialog>

      <Dialog
        open={editOpen}
        onOpenChange={(open) => {
          if (!open) {
            cancelEdit()
          }
        }}
      >
        <DialogContent className={sheetClasses}>
          <DialogHeader className="border-b px-4 py-3 sm:border-b-0 sm:px-0 sm:py-0">
            <div className="mx-auto mb-2 h-1.5 w-12 rounded-full bg-muted sm:hidden" />
            <DialogTitle className="px-0 sm:px-4 sm:pt-4">Edit record</DialogTitle>
            <DialogDescription className="px-0 sm:px-4">Update the details and save.</DialogDescription>
          </DialogHeader>
          <form onSubmit={handleSubmit} className="grid gap-4 px-4 pb-6 pt-4 sm:px-4 sm:pb-4 sm:grid-cols-2">
            {formFields}
          </form>
        </DialogContent>
      </Dialog>
    </>
  )
}

