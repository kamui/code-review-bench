# PLAN-operations.md — Phase 4: Operations

> **Objective:** Build Production order tracking, Shipment tracking, the operational Dashboard, and wire the Activity Timeline into deal detail — completing the full operational picture. This is Milestone 4 from `sales-ops-crm.md`.

---

## ⚠️ CRITICAL: Read Before You Start

1. **Next.js 16.2.2** — Uses `proxy.ts` NOT `middleware.ts`. Do NOT create middleware.
2. **Do NOT run `npm install`** — All deps are installed. Check `package.json` for proof.
3. **Zod v4.3.6** — Use `import { z } from 'zod'`. Do NOT use `zod/v4`.
4. **React Hook Form v7.72.0 + @hookform/resolvers v5.2.2** — Already installed.
5. **shadcn/ui components** — Already in `src/components/ui/`: button, button-variants, card, dialog, dropdown-menu, input, label, select, separator, sheet, table, tabs, tooltip, avatar, badge, sonner, checkbox. Do NOT re-install.
6. **`buttonVariants`** — Import from `@/components/ui/button-variants` (NOT from `button.tsx`). This was separated in Phase 2 to avoid Server Component crashes.
7. **`Button` with links** — Do NOT use `<Button asChild>`. Use `<Link className={cn(buttonVariants({ variant: "..." }))}>`. The `asChild` prop crashes with `@base-ui/react`.
8. **Supabase clients** — `src/lib/supabase/client.ts` (browser) and `src/lib/supabase/server.ts` (server) already exist.
9. **Existing types** — `src/types/database.types.ts` has ALL table types including `production_orders` and `shipments` with Row/Insert/Update variants and all Enums.
10. **Existing constants** — `src/lib/constants.ts` has `PRODUCTION_STATUSES` (5 statuses: not_started, in_progress, quality_check, completed, delayed) and `SHIPMENT_STATUSES` (5 statuses: preparing, dispatched, in_transit, delivered, delayed).
11. **Existing utils** — `src/lib/utils.ts` has `cn()`, `formatCurrency()`, `formatDate()`, `formatDateTime()`, `timeAgo()`, `getDaysUntil()`.
12. **Existing shared components** — `StatusBadge`, `EmptyState`, `PageHeader`, `SearchInput`, `ConfirmDialog`, `LoadingSpinner`, `DateDisplay`, `ActivityTimeline` in `src/components/shared/`.
13. **Existing hooks** — `useAuth.ts`, `useClients.ts`, `useDeals.ts`, `useTasks.ts`, `useVendors.ts` in `src/hooks/`.
14. **Existing services** — `activities.service.ts`, `clients.service.ts`, `deals.service.ts`, `tasks.service.ts`, `vendors.service.ts` in `src/services/`.
15. **Server Actions pattern** — See `src/app/(dashboard)/deals/actions.ts`: uses `useActionState` with `(_prevState: unknown, formData: FormData)` signature, `cleanFormData` helper to convert `''` → `null`, try/catch with `revalidatePath` + `redirect`.
16. **Form component pattern** — See `src/components/deals/DealForm.tsx`: uses `useActionState`, `useFormStatus` for pending state, `useForm` from react-hook-form with `zodResolver`, hidden inputs for select fields, separate `SubmitButton` component.
17. **Hook pattern** — See `src/hooks/useDeals.ts`: `'use client'`, uses browser Supabase client, `useCallback` + `useEffect` with `// eslint-disable-next-line react-hooks/set-state-in-effect` comment.
18. **Database views exist** — `deals_with_client`, `tasks_with_context`, `dashboard_summary`. The `dashboard_summary` view returns: `active_deals`, `overdue_tasks`, `due_today`, `delayed_production`, `shipments_in_transit`.
19. **Stub pages exist** — `/production/page.tsx` and `/shipments/page.tsx` are EmptyState placeholders. `/page.tsx` (dashboard) has skeleton cards placeholder. These will be OVERWRITTEN.
20. **Deal detail has placeholders** — `/deals/[id]/page.tsx` has disabled Production and Shipments tabs (line 190-195). These must be ENABLED and wired up.
21. **Vendor detail has placeholder** — `/vendors/[id]/page.tsx` has "Order history will be available after Phase 4" placeholder (line 118-126). This must be REPLACED with real data.
22. **`logActivity()` pattern** — Import from `@/services/activities.service`. Pass `Omit<ActivityInsert, 'actor_id' | 'id' | 'created_at'>`. The actor_id is auto-filled from session.

### Supabase Project

| Key | Value |
|-----|-------|
| **Project ID** | `kwubpqzdburgjxqruejc` |
| **Project URL** | `https://kwubpqzdburgjxqruejc.supabase.co` |

### Database Schema (already exists — DO NOT run migrations)

**production_orders** — `id` (uuid PK), `deal_id` (uuid NOT NULL → deals), `vendor_id` (uuid? → vendors), `quantity` (int NOT NULL), `status` (production_status default 'not_started'), `start_date` (date?), `expected_completion_date` (date?), `actual_completion_date` (date?), `unit_cost` (numeric?), `notes` (text?), `created_by` (uuid? → profiles), `created_at`, `updated_at`

**shipments** — `id` (uuid PK), `deal_id` (uuid NOT NULL → deals), `courier_name` (text?), `tracking_number` (text?), `status` (shipment_status default 'preparing'), `dispatch_date` (date?), `expected_delivery_date` (date?), `actual_delivery_date` (date?), `recipient_name` (text?), `delivery_address` (text?), `notes` (text?), `created_by` (uuid? → profiles), `created_at`, `updated_at`

**production_status enum:** `not_started`, `in_progress`, `quality_check`, `completed`, `delayed`

**shipment_status enum:** `preparing`, `dispatched`, `in_transit`, `delivered`, `delayed`

**activity_event enum (relevant):** `production_started`, `production_completed`, `shipment_created`, `shipment_dispatched`, `shipment_delivered`

---

## Task Checklist

### PHASE 4A — Production Module (Service → Schema → Hook → Actions → Components → Pages)

- [ ] **1.1** Create `src/services/production.service.ts`
- [ ] **1.2** Create `src/lib/validations/production.schema.ts`
- [ ] **1.3** Create `src/hooks/useProduction.ts`
- [ ] **1.4** Create `src/app/(dashboard)/production/actions.ts`
- [ ] **1.5** Create `src/components/production/ProductionTable.tsx`
- [ ] **1.6** Create `src/components/production/ProductionForm.tsx`
- [ ] **1.7** Create `src/components/production/StatusUpdate.tsx`
- [ ] **1.8** Replace `src/app/(dashboard)/production/page.tsx` — production list
- [ ] **1.9** Create `src/app/(dashboard)/production/new/page.tsx`
- [ ] **1.10** Create `src/app/(dashboard)/production/[id]/page.tsx` — detail view

### PHASE 4B — Shipments Module (Service → Schema → Hook → Actions → Components → Pages)

- [ ] **2.1** Create `src/services/shipments.service.ts`
- [ ] **2.2** Create `src/lib/validations/shipment.schema.ts`
- [ ] **2.3** Create `src/hooks/useShipments.ts`
- [ ] **2.4** Create `src/app/(dashboard)/shipments/actions.ts`
- [ ] **2.5** Create `src/components/shipments/ShipmentTable.tsx`
- [ ] **2.6** Create `src/components/shipments/ShipmentForm.tsx`
- [ ] **2.7** Create `src/components/shipments/StatusUpdate.tsx`
- [ ] **2.8** Replace `src/app/(dashboard)/shipments/page.tsx` — shipment list
- [ ] **2.9** Create `src/app/(dashboard)/shipments/new/page.tsx`
- [ ] **2.10** Create `src/app/(dashboard)/shipments/[id]/page.tsx` — detail view

### PHASE 4C — Dashboard (Service → Hook → Components → Page)

- [ ] **3.1** Create `src/services/dashboard.service.ts`
- [ ] **3.2** Create `src/hooks/useDashboard.ts`
- [ ] **3.3** Create `src/components/dashboard/StatCard.tsx`
- [ ] **3.4** Create `src/components/dashboard/TasksWidget.tsx`
- [ ] **3.5** Create `src/components/dashboard/DealsWidget.tsx`
- [ ] **3.6** Create `src/components/dashboard/ProductionWidget.tsx`
- [ ] **3.7** Replace `src/app/(dashboard)/page.tsx` — full dashboard

### PHASE 4D — Integration (Wire production + shipments into deal detail + vendor detail)

- [ ] **4.1** Modify `src/app/(dashboard)/deals/[id]/page.tsx` — Enable Production + Shipments tabs with real data
- [ ] **4.2** Modify `src/app/(dashboard)/vendors/[id]/page.tsx` — Replace placeholder with real production orders

### PHASE 4E — Verification

- [ ] **5.1** Run `npm run build` — fix ALL errors
- [ ] **5.2** Run `npm run dev` — verify production CRUD flow
- [ ] **5.3** Run `npm run dev` — verify shipments CRUD flow
- [ ] **5.4** Run `npm run dev` — verify dashboard renders correctly
- [ ] **5.5** Run `npm run dev` — verify deal detail shows production + shipments + activity

---

## Detailed Instructions Per Task

---

### 1.1 — Production Service Layer

**File: `src/services/production.service.ts`** (NEW)

Uses the **server** Supabase client. Mirrors `deals.service.ts` pattern exactly.

```typescript
import { createClient } from '@/lib/supabase/server'
import type { Database } from '@/types/database.types'
import { logActivity } from './activities.service'

type ProductionRow = Database['public']['Tables']['production_orders']['Row']
type ProductionInsert = Database['public']['Tables']['production_orders']['Insert']
type ProductionUpdate = Database['public']['Tables']['production_orders']['Update']

// Extended type with joined vendor + deal info
export type ProductionWithContext = ProductionRow & {
  vendor_name: string | null
  deal_name: string | null
  company_name: string | null
}

export async function getProductionOrders(filters?: {
  search?: string
  status?: string
  vendorId?: string
}) {
  const supabase = await createClient()

  let query = supabase
    .from('production_orders')
    .select(`
      *,
      vendors:vendor_id(name),
      deals:deal_id(deal_name, clients:client_id(company_name))
    `)
    .order('created_at', { ascending: false })

  if (filters?.status) {
    query = query.eq('status', filters.status)
  }
  if (filters?.vendorId) {
    query = query.eq('vendor_id', filters.vendorId)
  }

  const { data, error } = await query
  if (error) throw error

  // Flatten the nested joins into ProductionWithContext shape
  return (data || []).map((row: Record<string, unknown>) => ({
    ...row,
    vendor_name: (row.vendors as Record<string, unknown> | null)?.name as string | null ?? null,
    deal_name: (row.deals as Record<string, unknown> | null)?.deal_name as string | null ?? null,
    company_name: ((row.deals as Record<string, unknown> | null)?.clients as Record<string, unknown> | null)?.company_name as string | null ?? null,
    vendors: undefined,
    deals: undefined,
  })) as ProductionWithContext[]
}

export async function getProductionByDeal(dealId: string) {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('production_orders')
    .select(`*, vendors:vendor_id(name)`)
    .eq('deal_id', dealId)
    .order('created_at', { ascending: false })

  if (error) throw error
  return (data || []).map((row: Record<string, unknown>) => ({
    ...row,
    vendor_name: (row.vendors as Record<string, unknown> | null)?.name as string | null ?? null,
    vendors: undefined,
  })) as (ProductionRow & { vendor_name: string | null })[]
}

export async function getProductionByVendor(vendorId: string) {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('production_orders')
    .select(`*, deals:deal_id(deal_name)`)
    .eq('vendor_id', vendorId)
    .order('created_at', { ascending: false })

  if (error) throw error
  return (data || []).map((row: Record<string, unknown>) => ({
    ...row,
    deal_name: (row.deals as Record<string, unknown> | null)?.deal_name as string | null ?? null,
    deals: undefined,
  })) as (ProductionRow & { deal_name: string | null })[]
}

export async function getProductionOrder(id: string) {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('production_orders')
    .select(`
      *,
      vendors:vendor_id(name, contact_person, phone, email),
      deals:deal_id(deal_name, clients:client_id(company_name))
    `)
    .eq('id', id)
    .single()

  if (error) throw error
  return data
}

export async function createProductionOrder(
  orderData: Omit<ProductionInsert, 'created_by'>
) {
  const supabase = await createClient()
  const { data: { user } } = await supabase.auth.getUser()

  const { data, error } = await supabase
    .from('production_orders')
    .insert({ ...orderData, created_by: user?.id })
    .select()
    .single()

  if (error) throw error

  await logActivity({
    event_type: 'production_started',
    deal_id: orderData.deal_id,
    production_id: data.id,
    metadata: { quantity: orderData.quantity, status: 'not_started' },
  })

  return data
}

export async function updateProductionOrder(id: string, orderData: ProductionUpdate) {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('production_orders')
    .update(orderData)
    .eq('id', id)
    .select()
    .single()

  if (error) throw error
  return data
}

export async function updateProductionStatus(
  id: string,
  newStatus: string,
  previousStatus: string
) {
  const supabase = await createClient()

  const updateData: ProductionUpdate = {
    status: newStatus as ProductionRow['status'],
  }

  // Auto-fill dates based on status transitions
  if (newStatus === 'in_progress' && previousStatus === 'not_started') {
    updateData.start_date = new Date().toISOString().split('T')[0]
  }
  if (newStatus === 'completed') {
    updateData.actual_completion_date = new Date().toISOString().split('T')[0]
  }

  const { data, error } = await supabase
    .from('production_orders')
    .update(updateData)
    .eq('id', id)
    .select()
    .single()

  if (error) throw error

  const eventType = newStatus === 'completed' ? 'production_completed' : 'production_started'
  await logActivity({
    event_type: eventType,
    deal_id: data.deal_id,
    production_id: data.id,
    metadata: { from: previousStatus, to: newStatus },
  })

  return data
}

export async function getDelayedOrders() {
  const supabase = await createClient()
  const today = new Date().toISOString().split('T')[0]

  const { data, error } = await supabase
    .from('production_orders')
    .select(`*, vendors:vendor_id(name), deals:deal_id(deal_name)`)
    .or(`status.eq.delayed,and(expected_completion_date.lt.${today},status.neq.completed)`)
    .order('expected_completion_date', { ascending: true })

  if (error) throw error
  return (data || []).map((row: Record<string, unknown>) => ({
    ...row,
    vendor_name: (row.vendors as Record<string, unknown> | null)?.name as string | null ?? null,
    deal_name: (row.deals as Record<string, unknown> | null)?.deal_name as string | null ?? null,
    vendors: undefined,
    deals: undefined,
  }))
}
```

> **WHY flattened joins?** Supabase returns nested objects for `.select('*, vendors:vendor_id(name)')`. We flatten them into `vendor_name` etc. for easy use in components, matching the pattern used in `deals_with_client` view.

> **WHY auto-fill dates?** When moving to `in_progress`, auto-set `start_date`. When moving to `completed`, auto-set `actual_completion_date`. This saves clicks and ensures data consistency.

---

### 1.2 — Production Validation Schema

**File: `src/lib/validations/production.schema.ts`** (NEW)

```typescript
import { z } from 'zod'

export const productionSchema = z.object({
  deal_id: z.string().min(1, 'Deal is required'),
  vendor_id: z.string().optional().or(z.literal('')),
  quantity: z.coerce.number().int().positive('Quantity must be positive'),
  status: z.enum([
    'not_started', 'in_progress', 'quality_check', 'completed', 'delayed'
  ]).optional(),
  start_date: z.string().optional().or(z.literal('')),
  expected_completion_date: z.string().optional().or(z.literal('')),
  actual_completion_date: z.string().optional().or(z.literal('')),
  unit_cost: z.coerce.number().positive().optional().or(z.literal('')),
  notes: z.string().max(2000).optional().or(z.literal('')),
})

export type ProductionFormValues = z.infer<typeof productionSchema>
```

---

### 1.3 — useProduction Hook

**File: `src/hooks/useProduction.ts`** (NEW)

Mirror `useDeals.ts` pattern exactly. Uses **browser** Supabase client.

```typescript
'use client'

import { useCallback, useEffect, useState } from 'react'
import { createClient } from '@/lib/supabase/client'

export type ProductionOrderHook = {
  id: string
  deal_id: string
  vendor_id: string | null
  quantity: number
  status: string
  start_date: string | null
  expected_completion_date: string | null
  actual_completion_date: string | null
  unit_cost: number | null
  notes: string | null
  created_at: string
  updated_at: string
  vendor_name: string | null
  deal_name: string | null
  company_name: string | null
}

export function useProduction(filters?: { status?: string; vendorId?: string }) {
  const [orders, setOrders] = useState<ProductionOrderHook[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const fetchOrders = useCallback(async () => {
    setLoading(true)
    setError(null)
    const supabase = createClient()

    let query = supabase
      .from('production_orders')
      .select(`
        *,
        vendors:vendor_id(name),
        deals:deal_id(deal_name, clients:client_id(company_name))
      `)
      .order('created_at', { ascending: false })

    if (filters?.status) query = query.eq('status', filters.status)
    if (filters?.vendorId) query = query.eq('vendor_id', filters.vendorId)

    const { data, error: fetchError } = await query

    if (fetchError) {
      setError(fetchError.message)
    } else {
      // Flatten nested joins
      const flattened = (data || []).map((row: Record<string, unknown>) => ({
        ...row,
        vendor_name: (row.vendors as Record<string, unknown> | null)?.name ?? null,
        deal_name: (row.deals as Record<string, unknown> | null)?.deal_name ?? null,
        company_name: ((row.deals as Record<string, unknown> | null)?.clients as Record<string, unknown> | null)?.company_name ?? null,
        vendors: undefined,
        deals: undefined,
      }))
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setOrders(flattened as ProductionOrderHook[])
    }

    setLoading(false)
  }, [filters?.status, filters?.vendorId])

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    fetchOrders()
  }, [fetchOrders])

  return { orders, loading, error, refetch: fetchOrders }
}
```

---

### 1.4 — Production Server Actions

**File: `src/app/(dashboard)/production/actions.ts`** (NEW)

Follow the EXACT pattern from `src/app/(dashboard)/deals/actions.ts`.

```typescript
'use server'

import { redirect } from 'next/navigation'
import { revalidatePath } from 'next/cache'
import {
  createProductionOrder,
  updateProductionOrder,
  updateProductionStatus,
} from '@/services/production.service'
import { productionSchema } from '@/lib/validations/production.schema'

function cleanFormData(data: Record<string, unknown>) {
  const cleaned: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(data)) {
    cleaned[key] = (value === '' || value === undefined) ? null : value
  }
  return cleaned
}

export async function createProductionAction(_prevState: unknown, formData: FormData) {
  const raw = Object.fromEntries(formData.entries())
  const parsed = productionSchema.safeParse(raw)

  if (!parsed.success) {
    return { error: parsed.error.flatten().fieldErrors }
  }

  try {
    await createProductionOrder(
      cleanFormData(parsed.data) as Parameters<typeof createProductionOrder>[0]
    )
  } catch (err: unknown) {
    return { message: err instanceof Error ? err.message : String(err) }
  }

  revalidatePath('/production')
  revalidatePath('/deals')
  redirect('/production')
}

export async function updateProductionAction(
  id: string,
  _prevState: unknown,
  formData: FormData
) {
  const raw = Object.fromEntries(formData.entries())
  const parsed = productionSchema.safeParse(raw)

  if (!parsed.success) {
    return { error: parsed.error.flatten().fieldErrors }
  }

  try {
    await updateProductionOrder(
      id,
      cleanFormData(parsed.data) as Parameters<typeof updateProductionOrder>[1]
    )
  } catch (err: unknown) {
    return { message: err instanceof Error ? err.message : String(err) }
  }

  revalidatePath(`/production/${id}`)
  revalidatePath('/production')
  revalidatePath('/deals')
  redirect(`/production/${id}`)
}

export async function updateProductionStatusAction(
  id: string,
  newStatus: string,
  previousStatus: string
) {
  try {
    await updateProductionStatus(id, newStatus, previousStatus)
  } catch (err: unknown) {
    return { message: err instanceof Error ? err.message : String(err) }
  }

  revalidatePath('/production')
  revalidatePath('/deals')
  return { success: true }
}
```

---

### 1.5 — ProductionTable Component

**File: `src/components/production/ProductionTable.tsx`** (NEW)

`'use client'` component. Renders a data table of production orders.

**Props:** `orders: ProductionOrderHook[]`

**Columns:** Deal Name (link to `/deals/[deal_id]`), Vendor (link to `/vendors/[vendor_id]`), Quantity, Status (StatusBadge with PRODUCTION_STATUSES), Start Date (DateDisplay), Expected Completion (DateDisplay), Actions (view link).

**Row styling:** If `status === 'delayed'` OR (`expected_completion_date < today` AND `status !== 'completed'`), apply `bg-red-50 text-red-900` to the row. Use `getDaysUntil(expected_completion_date) < 0` check.

**Empty state:** If no orders → render `EmptyState` with "Create Production Order" CTA.

---

### 1.6 — ProductionForm Component

**File: `src/components/production/ProductionForm.tsx`** (NEW)

`'use client'` component. Follow `DealForm.tsx` pattern exactly.

**Props:** `initialData?: ProductionFormValues & { id: string }`, `deals: { id: string; deal_name: string }[]`, `vendors: { id: string; name: string }[]`, `defaultDealId?: string`

**Fields (2-column grid):**
- `deal_id` (required select from `deals` prop — use hidden input pattern from DealForm)
- `vendor_id` (optional select from `vendors` prop — use hidden input pattern)
- `quantity` (required Input type="number")
- `unit_cost` (optional Input type="number" step="0.01")
- `status` (select from `PRODUCTION_STATUSES` — only shown in edit mode, default 'not_started' for create)
- `start_date` (Input type="date")
- `expected_completion_date` (Input type="date")
- `actual_completion_date` (Input type="date" — only shown in edit mode)
- `notes` (textarea, span 2 cols)

**Uses:** `useActionState` with `createProductionAction` or `updateProductionAction.bind(null, id)`, `useForm` with `zodResolver(productionSchema)`, `SubmitButton` pattern.

**Footer:** Cancel link (to `/production` or `/production/[id]`) + SubmitButton ("Create Order" / "Update Order").

> **CRITICAL:** Same hidden input pattern for select fields as `DealForm.tsx`. shadcn `Select` does NOT serialize to FormData automatically.

---

### 1.7 — StatusUpdate Component (Production)

**File: `src/components/production/StatusUpdate.tsx`** (NEW)

`'use client'` component. Inline one-click status change dropdown.

**Props:** `orderId: string`, `currentStatus: string`

**Behavior:**
1. Renders a shadcn `Select` showing current status with colored dot
2. On value change: calls `updateProductionStatusAction(orderId, newStatus, currentStatus)`
3. Shows `toast.success('Status updated to {newStatus}')` on success
4. Shows `toast.error(message)` on failure
5. Calls `router.refresh()` to update server component data

```typescript
'use client'

import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { PRODUCTION_STATUSES } from '@/lib/constants'
import { updateProductionStatusAction } from '@/app/(dashboard)/production/actions'
import { toast } from 'sonner'

interface StatusUpdateProps {
  orderId: string
  currentStatus: string
}

export function ProductionStatusUpdate({ orderId, currentStatus }: StatusUpdateProps) {
  const [status, setStatus] = useState(currentStatus)
  const router = useRouter()

  async function handleStatusChange(newStatus: string) {
    const prev = status
    setStatus(newStatus) // optimistic
    const result = await updateProductionStatusAction(orderId, newStatus, prev)
    if (result?.message) {
      setStatus(prev) // rollback
      toast.error(result.message)
    } else {
      toast.success(`Status updated to ${PRODUCTION_STATUSES.find(s => s.value === newStatus)?.label || newStatus}`)
      router.refresh()
    }
  }

  return (
    <Select value={status} onValueChange={handleStatusChange}>
      <SelectTrigger className="w-[160px] h-8 text-xs">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {PRODUCTION_STATUSES.map(s => (
          <SelectItem key={s.value} value={s.value}>
            <span className="flex items-center gap-2">
              <span className={`h-2 w-2 rounded-full ${s.color}`} />
              {s.label}
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
```

---

### 1.8 — Production List Page (REPLACE stub)

**File: `src/app/(dashboard)/production/page.tsx`** (OVERWRITE)

Server Component. Pattern matches the clients list page.

1. Reads optional `?status=` from URL searchParams
2. Calls `getProductionOrders({ status })` from service
3. Renders `PageHeader` title "Production Orders" + "Create Order" button linking to `/production/new`
4. Renders filter bar with status dropdown (all PRODUCTION_STATUSES + "All")
5. If empty → `EmptyState` with CTA
6. If orders exist → `ProductionTable` with inline `ProductionStatusUpdate` per row

> **NOTE:** The `ProductionTable` renders `ProductionStatusUpdate` inside each row's Status column for one-click updates.

---

### 1.9 — Create Production Order Page

**File: `src/app/(dashboard)/production/new/page.tsx`** (NEW)

Server Component:
1. Read optional `?deal_id=` from searchParams
2. Fetch deals list (id + deal_name) via `getDeals()`
3. Fetch vendors list (id + name) via `getVendors()` from vendors service
4. Render `PageHeader title="New Production Order"` + `ProductionForm`

```tsx
import { PageHeader } from '@/components/shared/PageHeader'
import { ProductionForm } from '@/components/production/ProductionForm'
import { getDeals } from '@/services/deals.service'
import { getVendors } from '@/services/vendors.service'

export default async function NewProductionPage({
  searchParams,
}: {
  searchParams: Promise<{ deal_id?: string }>
}) {
  const { deal_id } = await searchParams
  const [deals, vendors] = await Promise.all([getDeals(), getVendors()])

  return (
    <div>
      <PageHeader title="New Production Order" />
      <ProductionForm
        deals={deals.map(d => ({ id: d.id, deal_name: d.deal_name }))}
        vendors={vendors.map(v => ({ id: v.id, name: v.name }))}
        defaultDealId={deal_id}
      />
    </div>
  )
}
```

---

### 1.10 — Production Detail Page

**File: `src/app/(dashboard)/production/[id]/page.tsx`** (NEW)

Server Component:
1. Call `getProductionOrder(id)` — if not found → `notFound()`
2. Show order details in a Card layout similar to deal detail
3. Display: vendor info, deal link, quantity, unit_cost, status with `ProductionStatusUpdate` inline, dates (start, expected, actual), notes
4. Include "Edit" button linking to a potential edit page (or inline edit via StatusUpdate)
5. Include back link to `/production`

**Layout:**
- Header: "Production Order" + status badge + vendor name
- Grid: deal info card (link to deal) + vendor info card (link to vendor)
- Details: quantity, unit_cost, dates
- Notes section
- Timestamps (created_at, updated_at)

---

### 2.1 — Shipments Service Layer

**File: `src/services/shipments.service.ts`** (NEW)

Mirrors `production.service.ts` pattern. Uses **server** Supabase client.

```typescript
import { createClient } from '@/lib/supabase/server'
import type { Database } from '@/types/database.types'
import { logActivity } from './activities.service'

type ShipmentRow = Database['public']['Tables']['shipments']['Row']
type ShipmentInsert = Database['public']['Tables']['shipments']['Insert']
type ShipmentUpdate = Database['public']['Tables']['shipments']['Update']

export type ShipmentWithContext = ShipmentRow & {
  deal_name: string | null
  company_name: string | null
}

export async function getShipments(filters?: { search?: string; status?: string }) {
  const supabase = await createClient()

  let query = supabase
    .from('shipments')
    .select(`*, deals:deal_id(deal_name, clients:client_id(company_name))`)
    .order('created_at', { ascending: false })

  if (filters?.status) query = query.eq('status', filters.status)

  const { data, error } = await query
  if (error) throw error

  return (data || []).map((row: Record<string, unknown>) => ({
    ...row,
    deal_name: (row.deals as Record<string, unknown> | null)?.deal_name ?? null,
    company_name: ((row.deals as Record<string, unknown> | null)?.clients as Record<string, unknown> | null)?.company_name ?? null,
    deals: undefined,
  })) as ShipmentWithContext[]
}

export async function getShipmentsByDeal(dealId: string) {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('shipments')
    .select('*')
    .eq('deal_id', dealId)
    .order('created_at', { ascending: false })

  if (error) throw error
  return (data || []) as ShipmentRow[]
}

export async function getShipment(id: string) {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('shipments')
    .select(`*, deals:deal_id(deal_name, clients:client_id(company_name, contact_person))`)
    .eq('id', id)
    .single()

  if (error) throw error
  return data
}

export async function createShipmentRecord(shipmentData: Omit<ShipmentInsert, 'created_by'>) {
  const supabase = await createClient()
  const { data: { user } } = await supabase.auth.getUser()

  const { data, error } = await supabase
    .from('shipments')
    .insert({ ...shipmentData, created_by: user?.id })
    .select()
    .single()

  if (error) throw error

  await logActivity({
    event_type: 'shipment_created',
    deal_id: shipmentData.deal_id,
    shipment_id: data.id,
    metadata: { tracking_number: shipmentData.tracking_number },
  })

  return data
}

export async function updateShipmentRecord(id: string, shipmentData: ShipmentUpdate) {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('shipments')
    .update(shipmentData)
    .eq('id', id)
    .select()
    .single()

  if (error) throw error
  return data
}

export async function updateShipmentStatus(id: string, newStatus: string, previousStatus: string) {
  const supabase = await createClient()

  const updateData: ShipmentUpdate = {
    status: newStatus as ShipmentRow['status'],
  }

  // Auto-fill dates based on status transitions
  if (newStatus === 'dispatched' && previousStatus === 'preparing') {
    updateData.dispatch_date = new Date().toISOString().split('T')[0]
  }
  if (newStatus === 'delivered') {
    updateData.actual_delivery_date = new Date().toISOString().split('T')[0]
  }

  const { data, error } = await supabase
    .from('shipments')
    .update(updateData)
    .eq('id', id)
    .select()
    .single()

  if (error) throw error

  // Map status to appropriate event type
  let eventType: 'shipment_dispatched' | 'shipment_delivered' | 'shipment_created' = 'shipment_created'
  if (newStatus === 'dispatched' || newStatus === 'in_transit') eventType = 'shipment_dispatched'
  if (newStatus === 'delivered') eventType = 'shipment_delivered'

  await logActivity({
    event_type: eventType,
    deal_id: data.deal_id,
    shipment_id: data.id,
    metadata: { from: previousStatus, to: newStatus },
  })

  return data
}

export async function getInTransitShipments() {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('shipments')
    .select(`*, deals:deal_id(deal_name)`)
    .eq('status', 'in_transit')
    .order('expected_delivery_date', { ascending: true })

  if (error) throw error
  return (data || []).map((row: Record<string, unknown>) => ({
    ...row,
    deal_name: (row.deals as Record<string, unknown> | null)?.deal_name ?? null,
    deals: undefined,
  }))
}
```

> **WHY auto-fill dates?** When dispatching, auto-set `dispatch_date`. When delivering, auto-set `actual_delivery_date`. Same consistency pattern as production.

---

### 2.2 — Shipment Validation Schema

**File: `src/lib/validations/shipment.schema.ts`** (NEW)

```typescript
import { z } from 'zod'

export const shipmentSchema = z.object({
  deal_id: z.string().min(1, 'Deal is required'),
  courier_name: z.string().max(255).optional().or(z.literal('')),
  tracking_number: z.string().max(255).optional().or(z.literal('')),
  status: z.enum([
    'preparing', 'dispatched', 'in_transit', 'delivered', 'delayed'
  ]).optional(),
  dispatch_date: z.string().optional().or(z.literal('')),
  expected_delivery_date: z.string().optional().or(z.literal('')),
  actual_delivery_date: z.string().optional().or(z.literal('')),
  recipient_name: z.string().max(255).optional().or(z.literal('')),
  delivery_address: z.string().max(500).optional().or(z.literal('')),
  notes: z.string().max(2000).optional().or(z.literal('')),
})

export type ShipmentFormValues = z.infer<typeof shipmentSchema>
```

---

### 2.3 — useShipments Hook

**File: `src/hooks/useShipments.ts`** (NEW)

Mirror `useProduction.ts` pattern. Uses **browser** Supabase client. Queries `shipments` table with deal join. State: `shipments`, `loading`, `error`. Filters: `status`. Returns `{ shipments, loading, error, refetch }`.

---

### 2.4 — Shipment Server Actions

**File: `src/app/(dashboard)/shipments/actions.ts`** (NEW)

Mirror `production/actions.ts` exactly. Functions:
- `createShipmentAction(_prevState, formData)` — parse with `shipmentSchema`, `cleanFormData`, call `createShipmentRecord`, revalidate `/shipments` + `/deals`, redirect
- `updateShipmentAction(id, _prevState, formData)` — same pattern for update
- `updateShipmentStatusAction(id, newStatus, previousStatus)` — call `updateShipmentStatus`, revalidate

---

### 2.5 — ShipmentTable Component

**File: `src/components/shipments/ShipmentTable.tsx`** (NEW)

`'use client'` component. Data table of shipments.

**Columns:** Deal Name (link), Courier, Tracking # (with copy-to-clipboard button), Status (StatusBadge with SHIPMENT_STATUSES), Dispatch Date, Expected Delivery, Actions.

**Copy-to-clipboard:** Tracking number cell includes a small `Copy` icon button. On click: `navigator.clipboard.writeText(tracking_number)` + `toast.success('Tracking number copied')`.

**Row styling:** Red background for `status === 'delayed'` OR (`expected_delivery_date < today` AND `status !== 'delivered'`).

---

### 2.6 — ShipmentForm Component

**File: `src/components/shipments/ShipmentForm.tsx`** (NEW)

Follow `ProductionForm.tsx` pattern.

**Props:** `initialData?`, `deals: { id: string; deal_name: string }[]`, `defaultDealId?: string`

**Fields (2-column grid):**
- `deal_id` (required select — hidden input pattern)
- `courier_name` (Input)
- `tracking_number` (Input)
- `status` (select — edit mode only, default 'preparing')
- `recipient_name` (Input)
- `delivery_address` (Input — span 2 cols)
- `dispatch_date` (Input type="date")
- `expected_delivery_date` (Input type="date")
- `actual_delivery_date` (Input type="date" — edit mode only)
- `notes` (textarea, span 2 cols)

---

### 2.7 — StatusUpdate Component (Shipments)

**File: `src/components/shipments/StatusUpdate.tsx`** (NEW)

Mirror `production/StatusUpdate.tsx` exactly but uses `SHIPMENT_STATUSES` and `updateShipmentStatusAction`.

---

### 2.8 — Shipments List Page (REPLACE stub)

**File: `src/app/(dashboard)/shipments/page.tsx`** (OVERWRITE)

Server Component. Same pattern as production list page:
1. Read `?status=` from searchParams
2. Call `getShipments({ status })`
3. `PageHeader` "Shipment Tracking" + "Create Shipment" button
4. Status filter dropdown
5. `ShipmentTable` with inline `ShipmentStatusUpdate`

---

### 2.9 — Create Shipment Page

**File: `src/app/(dashboard)/shipments/new/page.tsx`** (NEW)

Server Component:
1. Read optional `?deal_id=` from searchParams
2. Fetch deals list
3. Render `PageHeader` + `ShipmentForm`

---

### 2.10 — Shipment Detail Page

**File: `src/app/(dashboard)/shipments/[id]/page.tsx`** (NEW)

Server Component:
1. Call `getShipment(id)` — if not found → `notFound()`
2. Header: tracking number + status badge
3. Deal info card (link to deal)
4. Shipping details: courier, tracking (with copy button), recipient, delivery address
5. Dates: dispatch, expected delivery, actual delivery
6. Inline status update dropdown
7. Notes, timestamps

---

### 3.1 — Dashboard Service

**File: `src/services/dashboard.service.ts`** (NEW)

Uses **server** Supabase client. Fetches data for all 4 dashboard widgets.

```typescript
import { createClient } from '@/lib/supabase/server'

export async function getDashboardSummary() {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('dashboard_summary')
    .select('*')
    .single()

  if (error) throw error
  return data as {
    active_deals: number | null
    overdue_tasks: number | null
    due_today: number | null
    delayed_production: number | null
    shipments_in_transit: number | null
  }
}

export async function getRecentDeals(limit = 5) {
  const supabase = await createClient()
  const { data, error } = await supabase
    .from('deals')
    .select('id, deal_name, stage, total_value, currency, clients:client_id(company_name)')
    .eq('is_archived', false)
    .order('updated_at', { ascending: false })
    .limit(limit)

  if (error) throw error
  return (data || []).map((row: Record<string, unknown>) => ({
    ...row,
    company_name: (row.clients as Record<string, unknown> | null)?.company_name ?? null,
    clients: undefined,
  }))
}

export async function getOverdueTasks(limit = 5) {
  const supabase = await createClient()
  const today = new Date().toISOString().split('T')[0]

  const { data, error } = await supabase
    .from('tasks')
    .select('id, title, due_date, priority, status, deals:deal_id(deal_name)')
    .or(`status.eq.overdue,and(due_date.lt.${today},status.neq.done)`)
    .order('due_date', { ascending: true })
    .limit(limit)

  if (error) throw error
  return (data || []).map((row: Record<string, unknown>) => ({
    ...row,
    deal_name: (row.deals as Record<string, unknown> | null)?.deal_name ?? null,
    deals: undefined,
  }))
}

export async function getDelayedProduction(limit = 5) {
  const supabase = await createClient()
  const today = new Date().toISOString().split('T')[0]

  const { data, error } = await supabase
    .from('production_orders')
    .select('id, quantity, status, expected_completion_date, vendors:vendor_id(name), deals:deal_id(deal_name)')
    .or(`status.eq.delayed,and(expected_completion_date.lt.${today},status.neq.completed)`)
    .order('expected_completion_date', { ascending: true })
    .limit(limit)

  if (error) throw error
  return (data || []).map((row: Record<string, unknown>) => ({
    ...row,
    vendor_name: (row.vendors as Record<string, unknown> | null)?.name ?? null,
    deal_name: (row.deals as Record<string, unknown> | null)?.deal_name ?? null,
    vendors: undefined,
    deals: undefined,
  }))
}
```

> **WHY a `dashboard_summary` view?** This view already exists in the database (Phase 1 migration). It pre-aggregates counts to avoid N+1 client-side queries. We just `.select('*').single()` it.

---

### 3.2 — useDashboard Hook

**File: `src/hooks/useDashboard.ts`** (NEW)

**NOT needed.** Dashboard page is a Server Component that calls `dashboard.service.ts` directly at the top level. No client-side reactivity needed for the dashboard — it refreshes on navigation.

> Skip this task. Mark as done immediately.

---

### 3.3 — StatCard Component

**File: `src/components/dashboard/StatCard.tsx`** (NEW)

Server Component (no `'use client'`).

**Props:** `title: string`, `value: number`, `icon: LucideIcon`, `href: string`, `color: string` (tailwind bg class like `bg-orange-500`), `description?: string`

**Design:**
- Card with left-colored border (4px, `border-l-4 border-l-{color}`)
- Icon with colored background circle
- Large bold number (`text-3xl font-bold`)
- Small label (`text-xs uppercase text-muted-foreground`)
- Entire card is clickable via wrapping with Next.js `Link`
- Hover: subtle shadow increase

```tsx
import { Card, CardContent } from '@/components/ui/card'
import Link from 'next/link'
import type { LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'

interface StatCardProps {
  title: string
  value: number
  icon: LucideIcon
  href: string
  color: string
  description?: string
}

export function StatCard({ title, value, icon: Icon, href, color, description }: StatCardProps) {
  return (
    <Link href={href} className="block group">
      <Card className={cn(
        "border-l-4 transition-all hover:shadow-md cursor-pointer",
        color
      )}>
        <CardContent className="p-5 flex items-center gap-4">
          <div className={cn("p-2.5 rounded-full bg-muted/50 group-hover:scale-110 transition-transform")}>
            <Icon className="h-5 w-5 text-muted-foreground" />
          </div>
          <div className="space-y-0.5">
            <p className="text-3xl font-bold tracking-tight">{value}</p>
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">{title}</p>
            {description && <p className="text-xs text-muted-foreground">{description}</p>}
          </div>
        </CardContent>
      </Card>
    </Link>
  )
}
```

---

### 3.4 — TasksWidget

**File: `src/components/dashboard/TasksWidget.tsx`** (NEW)

Server Component. Shows overdue + due-today tasks.

**Props:** `tasks: { id, title, due_date, priority, status, deal_name }[]`

**Renders:** A Card with "Urgent Tasks" title. List of task items. Each item shows:
- Priority dot color (from TASK_PRIORITIES constant)
- Title (link to `/tasks` — or individual task page if available)
- Due date formatted with `timeAgo()` — red text if overdue
- Deal name in small muted text

If empty → render italic "All caught up! 🎉" message

Footer: "View all tasks →" link to `/tasks`

---

### 3.5 — DealsWidget

**File: `src/components/dashboard/DealsWidget.tsx`** (NEW)

Server Component. Shows recent active deals.

**Props:** `deals: { id, deal_name, stage, total_value, currency, company_name }[]`

**Renders:** Card with "Active Pipeline" title. List of deal items:
- Deal name (link to `/deals/[id]`)
- Stage badge (StatusBadge with DEAL_STAGES)
- Value formatted with `formatCurrency()`
- Company name in small text

Footer: "View pipeline →" link to `/deals`

---

### 3.6 — ProductionWidget

**File: `src/components/dashboard/ProductionWidget.tsx`** (NEW)

Server Component. Shows delayed/overdue production orders.

**Props:** `orders: { id, quantity, status, expected_completion_date, vendor_name, deal_name }[]`

**Renders:** Card with "Production Alerts" title + warning icon. Red-tinted header if any orders exist.

Each item:
- Deal name (link to `/production/[id]`)
- Vendor name
- Expected date with "X days overdue" in red using `getDaysUntil()`
- Quantity

If empty → green "All on schedule! ✅" message

Footer: "View all production →" link to `/production`

---

### 3.7 — Dashboard Page (REPLACE placeholder)

**File: `src/app/(dashboard)/page.tsx`** (OVERWRITE)

Server Component. The main homepage of the app.

```tsx
import { getDashboardSummary, getRecentDeals, getOverdueTasks, getDelayedProduction } from '@/services/dashboard.service'
import { StatCard } from '@/components/dashboard/StatCard'
import { TasksWidget } from '@/components/dashboard/TasksWidget'
import { DealsWidget } from '@/components/dashboard/DealsWidget'
import { ProductionWidget } from '@/components/dashboard/ProductionWidget'
import { Briefcase, CheckSquare, Factory, Truck } from 'lucide-react'

export default async function DashboardPage() {
  const [summary, recentDeals, overdueTasks, delayedProduction] = await Promise.all([
    getDashboardSummary(),
    getRecentDeals(),
    getOverdueTasks(),
    getDelayedProduction(),
  ])

  return (
    <div className="flex flex-col gap-6">
      {/* Stat Cards Row */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <StatCard title="Active Deals" value={summary.active_deals ?? 0} icon={Briefcase} href="/deals" color="border-l-blue-500" />
        <StatCard title="Overdue Tasks" value={summary.overdue_tasks ?? 0} icon={CheckSquare} href="/tasks" color="border-l-red-500" description={`${summary.due_today ?? 0} due today`} />
        <StatCard title="Delayed Production" value={summary.delayed_production ?? 0} icon={Factory} href="/production" color="border-l-orange-500" />
        <StatCard title="In Transit" value={summary.shipments_in_transit ?? 0} icon={Truck} href="/shipments" color="border-l-emerald-500" />
      </div>

      {/* Widgets Grid */}
      <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
        <TasksWidget tasks={overdueTasks} />
        <DealsWidget deals={recentDeals} />
        <ProductionWidget orders={delayedProduction} />
      </div>
    </div>
  )
}
```

---

### 4.1 — Wire Production + Shipments into Deal Detail

**File: `src/app/(dashboard)/deals/[id]/page.tsx`** (MODIFY)

**Changes:**
1. Import `getProductionByDeal` from `@/services/production.service`
2. Import `getShipmentsByDeal` from `@/services/shipments.service`
3. Add to `Promise.all`: `getProductionByDeal(id)`, `getShipmentsByDeal(id)`
4. **REMOVE `disabled` prop** from Production and Shipments TabsTriggers (lines 190-195)
5. Add `TabsContent value="production"`: render production orders list with StatusBadge, dates, vendor name, link to `/production/[id]`
6. Add `TabsContent value="shipments"`: render shipments list with StatusBadge, tracking #, dates, link to `/shipments/[id]`
7. Add "Create Production Order" and "Create Shipment" links that pass `?deal_id={id}` as query param

**Production tab content example:**
```tsx
<TabsContent value="production" className="space-y-4">
  <div className="flex justify-between items-center mb-2">
    <h3 className="font-bold text-slate-800">Production Orders</h3>
    <Link href={`/production/new?deal_id=${id}`} className="text-xs font-bold text-primary flex items-center gap-1 hover:underline">
      <Plus className="h-3 w-3" /> New Order
    </Link>
  </div>
  {productionOrders.length > 0 ? (
    <div className="grid gap-3">
      {productionOrders.map(order => (
        <Card key={order.id} className="p-4">
          <div className="flex justify-between items-center">
            <div>
              <p className="font-medium">{order.vendor_name || 'No vendor'}</p>
              <p className="text-sm text-muted-foreground">Qty: {order.quantity}</p>
            </div>
            <div className="flex items-center gap-3">
              <StatusBadge status={order.status} items={PRODUCTION_STATUSES} />
              <Link href={`/production/${order.id}`} className="text-xs text-primary hover:underline">View →</Link>
            </div>
          </div>
        </Card>
      ))}
    </div>
  ) : (
    <div className="text-center py-12 bg-slate-50 rounded-xl border border-dashed border-slate-200">
      <p className="text-sm text-slate-500 italic">No production orders for this deal.</p>
    </div>
  )}
</TabsContent>
```

**Shipments tab content:** Same pattern but with `ShipmentRow[]`. Show tracking_number, courier_name, status, dates.

---

### 4.2 — Wire Production into Vendor Detail

**File: `src/app/(dashboard)/vendors/[id]/page.tsx`** (MODIFY)

**Changes:**
1. Import `getProductionByVendor` from `@/services/production.service`
2. Call `getProductionByVendor(id)` in the component
3. Replace the dashed placeholder Card (lines 118-126) with a real Card showing:
   - "Recent Production Orders" title
   - List of orders: deal_name (link to deal), quantity, status badge, expected_completion_date
   - If empty → "No production orders with this vendor yet." italic text
   - Footer: "View all production →" link to `/production`

---

### PHASE 4E — Verification Steps

**5.1** Run `npm run build` — Must pass with 0 errors. Check for:
- Type mismatches in service return types
- Missing imports
- Server/client boundary violations

**5.2** Start `npm run dev` and test production CRUD:
- Navigate to `/production` → should see empty state
- Click "Create Production Order" → fill form → submit → redirects to list
- Click on order → detail page loads
- Change status inline → toast appears, row updates
- Delayed orders show red highlight

**5.3** Test shipments CRUD:
- Navigate to `/shipments` → empty state
- Create shipment → form submits → redirects
- Detail page shows tracking info
- Copy-to-clipboard works for tracking number
- Status transitions auto-fill dispatch/delivery dates

**5.4** Test dashboard:
- Navigate to `/` → 4 stat cards render with real counts from `dashboard_summary` view
- Widgets show recent deals, overdue tasks, delayed production
- All links navigate correctly

**5.5** Test integration:
- Open a deal detail → tabs for Production/Shipments are ENABLED
- Create production order from deal detail (passes deal_id)
- Create shipment from deal detail (passes deal_id)
- Activity Timeline shows production_started, shipment_created events
- Vendor detail page shows production orders

---

## Done When

- [ ] Production CRUD (list, create, detail, status update) works end-to-end
- [ ] Shipments CRUD (list, create, detail, status update) works end-to-end
- [ ] Dashboard shows live stats + 3 widgets with real data
- [ ] Deal detail page has working Production + Shipments + Activity tabs
- [ ] Vendor detail page shows production order history
- [ ] `npm run build` passes with 0 errors
- [ ] All activity events logged correctly for production + shipment transitions

