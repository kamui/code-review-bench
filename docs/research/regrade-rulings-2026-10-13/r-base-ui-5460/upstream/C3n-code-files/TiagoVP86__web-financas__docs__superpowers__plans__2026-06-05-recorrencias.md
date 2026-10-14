# Recorrências Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement recurring transactions — users define templates that automatically generate Lancamentos on demand via a "Gerar lançamentos" button.

**Architecture:** New `Recorrencia` model links to `Lancamento` via optional FK (`recorrenciaId`). Generation logic loops active recorrencias with `proximaGeracao <= now`, creates PENDENTE lancamentos, and advances the date per frequency. Client components (RecorrenciasClient, RecorrenciaModal, RecorrenciaCard) manage state via fetch to API routes. Page is a server component that SSR-fetches the list.

**Tech Stack:** Next.js 16 App Router, Prisma/Neon, TypeScript, Tailwind CSS, sonner, date-fns (`addDays`/`addMonths`/`addYears`), shadcn/ui (Dialog, Card, Button, Input, Select, Badge)

---

### Task 1: Update Prisma schema

**Files:**
- Modify: `prisma/schema.prisma`

- [ ] **Step 1: Add `Frequencia` enum**

In `prisma/schema.prisma`, add after the `Status` enum:

```prisma
enum Frequencia {
  SEMANAL
  QUINZENAL
  MENSAL
  ANUAL
}
```

- [ ] **Step 2: Add `Recorrencia` model**

Add after the `AnaliseExtrato` model:

```prisma
model Recorrencia {
  id             String      @id @default(cuid())
  descricao      String
  valor          Decimal     @db.Decimal(10, 2)
  tipo           Tipo
  frequencia     Frequencia
  diaVencimento  Int
  mes            Int?
  categoriaId    String?
  categoria      Categoria?  @relation(fields: [categoriaId], references: [id])
  totalParcelas  Int?
  parcelaAtual   Int         @default(0)
  ativa          Boolean     @default(true)
  proximaGeracao DateTime
  userId         String
  user           User        @relation(fields: [userId], references: [id], onDelete: Cascade)
  lancamentos    Lancamento[]
  createdAt      DateTime    @default(now())

  @@index([userId])
}
```

- [ ] **Step 3: Add `recorrenciaId` to `Lancamento`**

Inside the `Lancamento` model, add after `createdAt`:

```prisma
  recorrenciaId  String?
  recorrencia    Recorrencia? @relation(fields: [recorrenciaId], references: [id], onDelete: SetNull)
```

- [ ] **Step 4: Add relations to `User` and `Categoria`**

In `User` model, add:
```prisma
  recorrencias     Recorrencia[]
```

In `Categoria` model, add:
```prisma
  recorrencias     Recorrencia[]
```

- [ ] **Step 5: Validate schema**

```bash
cd C:/Projetos/web-financas && npx prisma validate
```
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add prisma/schema.prisma
git commit -m "feat(schema): add Recorrencia model, Frequencia enum, and recorrenciaId on Lancamento"
```

---

### Task 2: Run Prisma migration

**Files:**
- Create: `prisma/migrations/[timestamp]_add_recorrencia/migration.sql` (auto-generated)

- [ ] **Step 1: Run migration**

```bash
cd C:/Projetos/web-financas && npx prisma migrate dev --name add_recorrencia
```
Expected output includes: `✓ Generated Prisma Client`

- [ ] **Step 2: Verify TypeScript compiles**

```bash
npx tsc --noEmit
```
Expected: no errors. `db.recorrencia` now exists in the generated client.

- [ ] **Step 3: Commit**

```bash
git add prisma/migrations
git commit -m "feat(db): migrate — add Recorrencia table and recorrenciaId to Lancamento"
```

---

### Task 3: Shared types

**Files:**
- Create: `src/types/recorrencia.ts`

- [ ] **Step 1: Create types file**

```typescript
// src/types/recorrencia.ts

export type Frequencia = "SEMANAL" | "QUINZENAL" | "MENSAL" | "ANUAL"

export interface RecorrenciaItem {
  id: string
  descricao: string
  valor: number
  tipo: "RECEITA" | "DESPESA"
  frequencia: Frequencia
  diaVencimento: number
  mes: number | null
  categoriaId: string | null
  categoriaNome: string | null
  categoriaCor: string | null
  totalParcelas: number | null
  parcelaAtual: number
  ativa: boolean
  proximaGeracao: string // ISO string
  createdAt: string
}

export interface CriarRecorrenciaBody {
  descricao: string
  valor: number
  tipo: "RECEITA" | "DESPESA"
  frequencia: Frequencia
  diaVencimento: number
  mes?: number
  categoriaId?: string | null
  totalParcelas?: number | null
  dataInicio: string // YYYY-MM-DD
}

export interface AtualizarRecorrenciaBody extends CriarRecorrenciaBody {
  scope: "futuros" | "todos"
}

export interface GerarResponse {
  gerados: number
}
```

- [ ] **Step 2: Verify TypeScript**

```bash
cd C:/Projetos/web-financas && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/types/recorrencia.ts
git commit -m "feat(types): add shared types for recorrencias"
```

---

### Task 4: Generation logic

**Files:**
- Create: `src/lib/recorrencia.ts`

- [ ] **Step 1: Create lib file**

```typescript
// src/lib/recorrencia.ts
import { addDays, addMonths, addYears } from "date-fns"
import { db } from "@/lib/db"
import type { Frequencia } from "@/types/recorrencia"

export function calcularProximaGeracao(atual: Date, frequencia: Frequencia): Date {
  switch (frequencia) {
    case "SEMANAL":   return addDays(atual, 7)
    case "QUINZENAL": return addDays(atual, 15)
    case "MENSAL":    return addMonths(atual, 1)
    case "ANUAL":     return addYears(atual, 1)
  }
}

export async function gerarLancamentos(userId: string): Promise<{ gerados: number }> {
  const now = new Date()

  const recorrencias = await db.recorrencia.findMany({
    where: {
      userId,
      ativa: true,
      proximaGeracao: { lte: now },
    },
  })

  let gerados = 0

  for (const rec of recorrencias) {
    let proxima = rec.proximaGeracao
    let parcelaAtual = rec.parcelaAtual

    while (
      proxima <= now &&
      (rec.totalParcelas === null || parcelaAtual < rec.totalParcelas)
    ) {
      await db.lancamento.create({
        data: {
          descricao: rec.descricao,
          valor: rec.valor,
          tipo: rec.tipo,
          data: proxima,
          status: "PENDENTE",
          categoriaId: rec.categoriaId ?? null,
          recorrenciaId: rec.id,
          userId,
        },
      })

      parcelaAtual++
      gerados++
      proxima = calcularProximaGeracao(proxima, rec.frequencia as Frequencia)
    }

    await db.recorrencia.update({
      where: { id: rec.id },
      data: {
        parcelaAtual,
        proximaGeracao: proxima,
        ativa:
          rec.totalParcelas !== null && parcelaAtual >= rec.totalParcelas
            ? false
            : rec.ativa,
      },
    })
  }

  return { gerados }
}
```

- [ ] **Step 2: Verify TypeScript**

```bash
cd C:/Projetos/web-financas && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/lib/recorrencia.ts
git commit -m "feat(lib): add recorrencia generation logic and date calculation helpers"
```

---

### Task 5: API routes — CRUD + toggle

**Files:**
- Create: `src/app/api/recorrencias/route.ts`
- Create: `src/app/api/recorrencias/[id]/route.ts`
- Create: `src/app/api/recorrencias/[id]/toggle/route.ts`

- [ ] **Step 1: Create GET + POST route**

```typescript
// src/app/api/recorrencias/route.ts
import { NextRequest, NextResponse } from "next/server"
import { auth } from "@/lib/auth"
import { db } from "@/lib/db"
import type { CriarRecorrenciaBody } from "@/types/recorrencia"

export async function GET() {
  const session = await auth()
  if (!session?.user?.id) return NextResponse.json({ error: "Unauthorized" }, { status: 401 })

  const recorrencias = await db.recorrencia.findMany({
    where: { userId: session.user.id },
    include: { categoria: { select: { nome: true, cor: true } } },
    orderBy: { createdAt: "desc" },
  })

  return NextResponse.json(
    recorrencias.map((r) => ({
      id: r.id,
      descricao: r.descricao,
      valor: Number(r.valor),
      tipo: r.tipo,
      frequencia: r.frequencia,
      diaVencimento: r.diaVencimento,
      mes: r.mes,
      categoriaId: r.categoriaId,
      categoriaNome: r.categoria?.nome ?? null,
      categoriaCor: r.categoria?.cor ?? null,
      totalParcelas: r.totalParcelas,
      parcelaAtual: r.parcelaAtual,
      ativa: r.ativa,
      proximaGeracao: r.proximaGeracao.toISOString(),
      createdAt: r.createdAt.toISOString(),
    }))
  )
}

export async function POST(req: NextRequest) {
  const session = await auth()
  if (!session?.user?.id) return NextResponse.json({ error: "Unauthorized" }, { status: 401 })
  const userId = session.user.id

  const body = (await req.json()) as CriarRecorrenciaBody

  if (!body.descricao || !body.valor || !body.tipo || !body.frequencia || !body.dataInicio) {
    return NextResponse.json({ error: "Campos obrigatórios faltando" }, { status: 400 })
  }

  const recorrencia = await db.recorrencia.create({
    data: {
      descricao: body.descricao,
      valor: body.valor,
      tipo: body.tipo,
      frequencia: body.frequencia,
      diaVencimento: body.diaVencimento,
      mes: body.mes ?? null,
      categoriaId: body.categoriaId ?? null,
      totalParcelas: body.totalParcelas ?? null,
      proximaGeracao: new Date(body.dataInicio),
      userId,
    },
  })

  return NextResponse.json({ id: recorrencia.id }, { status: 201 })
}
```

- [ ] **Step 2: Create PUT + DELETE route**

```typescript
// src/app/api/recorrencias/[id]/route.ts
import { NextRequest, NextResponse } from "next/server"
import { auth } from "@/lib/auth"
import { db } from "@/lib/db"
import type { AtualizarRecorrenciaBody } from "@/types/recorrencia"

export async function PUT(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const session = await auth()
  if (!session?.user?.id) return NextResponse.json({ error: "Unauthorized" }, { status: 401 })
  const userId = session.user.id
  const { id } = await params

  const existing = await db.recorrencia.findFirst({ where: { id, userId } })
  if (!existing) return NextResponse.json({ error: "Not found" }, { status: 404 })

  const body = (await req.json()) as AtualizarRecorrenciaBody

  await db.recorrencia.update({
    where: { id },
    data: {
      descricao: body.descricao,
      valor: body.valor,
      tipo: body.tipo,
      frequencia: body.frequencia,
      diaVencimento: body.diaVencimento,
      mes: body.mes ?? null,
      categoriaId: body.categoriaId ?? null,
      totalParcelas: body.totalParcelas ?? null,
    },
  })

  if (body.scope === "todos") {
    await db.lancamento.updateMany({
      where: { recorrenciaId: id, status: "PENDENTE" },
      data: {
        descricao: body.descricao,
        valor: body.valor,
        tipo: body.tipo,
        categoriaId: body.categoriaId ?? null,
      },
    })
  }

  return NextResponse.json({ ok: true })
}

export async function DELETE(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const session = await auth()
  if (!session?.user?.id) return NextResponse.json({ error: "Unauthorized" }, { status: 401 })
  const userId = session.user.id
  const { id } = await params

  const existing = await db.recorrencia.findFirst({ where: { id, userId } })
  if (!existing) return NextResponse.json({ error: "Not found" }, { status: 404 })

  await db.recorrencia.delete({ where: { id } })
  return NextResponse.json({ ok: true })
}
```

- [ ] **Step 3: Create toggle route**

```typescript
// src/app/api/recorrencias/[id]/toggle/route.ts
import { NextRequest, NextResponse } from "next/server"
import { auth } from "@/lib/auth"
import { db } from "@/lib/db"

export async function POST(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const session = await auth()
  if (!session?.user?.id) return NextResponse.json({ error: "Unauthorized" }, { status: 401 })
  const userId = session.user.id
  const { id } = await params

  const existing = await db.recorrencia.findFirst({ where: { id, userId } })
  if (!existing) return NextResponse.json({ error: "Not found" }, { status: 404 })

  await db.recorrencia.update({
    where: { id },
    data: { ativa: !existing.ativa },
  })
  return NextResponse.json({ ok: true })
}
```

- [ ] **Step 4: Verify TypeScript**

```bash
cd C:/Projetos/web-financas && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add src/app/api/recorrencias/
git commit -m "feat(api): add recorrencias CRUD, toggle, routes"
```

---

### Task 6: API route — gerar lançamentos

**Files:**
- Create: `src/app/api/recorrencias/gerar/route.ts`

- [ ] **Step 1: Create gerar route**

```typescript
// src/app/api/recorrencias/gerar/route.ts
import { NextResponse } from "next/server"
import { auth } from "@/lib/auth"
import { gerarLancamentos } from "@/lib/recorrencia"

export async function POST() {
  const session = await auth()
  if (!session?.user?.id) return NextResponse.json({ error: "Unauthorized" }, { status: 401 })

  const result = await gerarLancamentos(session.user.id)
  return NextResponse.json(result)
}
```

- [ ] **Step 2: Verify TypeScript**

```bash
cd C:/Projetos/web-financas && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/app/api/recorrencias/gerar/route.ts
git commit -m "feat(api): add POST /api/recorrencias/gerar — triggers lancamento generation"
```

---

### Task 7: RecorrenciaModal component

**Files:**
- Create: `src/components/recorrencias/recorrencia-modal.tsx`

Note: `Dialog` from `@/components/ui/dialog` uses `@base-ui/react/dialog` internally. The modal is controlled via `open` prop — no `DialogTrigger` needed here, avoiding the button-in-button issue.

- [ ] **Step 1: Create modal component**

```typescript
// src/components/recorrencias/recorrencia-modal.tsx
"use client"

import { useState } from "react"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import type {
  RecorrenciaItem,
  CriarRecorrenciaBody,
  AtualizarRecorrenciaBody,
  Frequencia,
} from "@/types/recorrencia"

const DIAS_SEMANA = ["Domingo", "Segunda", "Terça", "Quarta", "Quinta", "Sexta", "Sábado"]
const MESES_NOMES = [
  "Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho",
  "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro",
]

interface Categoria {
  id: string
  nome: string
}

interface Props {
  open: boolean
  onClose: () => void
  onSaved: () => void
  recorrencia?: RecorrenciaItem
  categorias: Categoria[]
}

export function RecorrenciaModal({ open, onClose, onSaved, recorrencia, categorias }: Props) {
  const isEdit = !!recorrencia

  const [descricao, setDescricao] = useState(recorrencia?.descricao ?? "")
  const [valor, setValor] = useState(recorrencia ? String(recorrencia.valor) : "")
  const [tipo, setTipo] = useState<"RECEITA" | "DESPESA">(recorrencia?.tipo ?? "DESPESA")
  const [frequencia, setFrequencia] = useState<Frequencia>(recorrencia?.frequencia ?? "MENSAL")
  const [diaVencimento, setDiaVencimento] = useState(recorrencia?.diaVencimento ?? 1)
  const [mes, setMes] = useState(recorrencia?.mes ?? 1)
  const [categoriaId, setCategoriaId] = useState(recorrencia?.categoriaId ?? "")
  const [totalParcelas, setTotalParcelas] = useState(
    recorrencia?.totalParcelas ? String(recorrencia.totalParcelas) : ""
  )
  const [dataInicio, setDataInicio] = useState(() => new Date().toISOString().split("T")[0])
  const [scope, setScope] = useState<"futuros" | "todos">("futuros")
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function handleSave() {
    setLoading(true)
    setError(null)

    const base: CriarRecorrenciaBody = {
      descricao,
      valor: parseFloat(valor),
      tipo,
      frequencia,
      diaVencimento,
      mes: frequencia === "ANUAL" ? mes : undefined,
      categoriaId: categoriaId || null,
      totalParcelas: totalParcelas ? parseInt(totalParcelas) : null,
      dataInicio,
    }

    const body: CriarRecorrenciaBody | AtualizarRecorrenciaBody = isEdit
      ? { ...base, scope }
      : base

    const url = isEdit ? `/api/recorrencias/${recorrencia!.id}` : "/api/recorrencias"
    const method = isEdit ? "PUT" : "POST"

    try {
      const res = await fetch(url, {
        method,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      })
      if (!res.ok) {
        const data = await res.json()
        setError(data.error ?? "Erro ao salvar")
        return
      }
      onSaved()
      onClose()
    } catch {
      setError("Falha na conexão")
    } finally {
      setLoading(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-md max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{isEdit ? "Editar recorrência" : "Nova recorrência"}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3">
          <div className="space-y-1">
            <Label>Descrição</Label>
            <Input
              value={descricao}
              onChange={(e) => setDescricao(e.target.value)}
              placeholder="Ex: Salário, Aluguel, Netflix"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <Label>Valor (R$)</Label>
              <Input
                type="number"
                step="0.01"
                min="0"
                value={valor}
                onChange={(e) => setValor(e.target.value)}
                placeholder="0,00"
              />
            </div>
            <div className="space-y-1">
              <Label>Tipo</Label>
              <Select value={tipo} onValueChange={(v) => setTipo(v as "RECEITA" | "DESPESA")}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="RECEITA">Receita</SelectItem>
                  <SelectItem value="DESPESA">Despesa</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <Label>Frequência</Label>
              <Select value={frequencia} onValueChange={(v) => setFrequencia(v as Frequencia)}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="SEMANAL">Semanal</SelectItem>
                  <SelectItem value="QUINZENAL">Quinzenal</SelectItem>
                  <SelectItem value="MENSAL">Mensal</SelectItem>
                  <SelectItem value="ANUAL">Anual</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {frequencia === "SEMANAL" && (
              <div className="space-y-1">
                <Label>Dia da semana</Label>
                <Select
                  value={String(diaVencimento)}
                  onValueChange={(v) => setDiaVencimento(parseInt(v))}
                >
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    {DIAS_SEMANA.map((d, i) => (
                      <SelectItem key={i} value={String(i)}>{d}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}

            {(frequencia === "MENSAL" || frequencia === "ANUAL") && (
              <div className="space-y-1">
                <Label>Dia do mês (1–28)</Label>
                <Input
                  type="number"
                  min={1}
                  max={28}
                  value={diaVencimento}
                  onChange={(e) => setDiaVencimento(Math.min(28, Math.max(1, parseInt(e.target.value) || 1)))}
                />
              </div>
            )}
          </div>

          {frequencia === "ANUAL" && (
            <div className="space-y-1">
              <Label>Mês</Label>
              <Select value={String(mes)} onValueChange={(v) => setMes(parseInt(v))}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  {MESES_NOMES.map((m, i) => (
                    <SelectItem key={i} value={String(i + 1)}>{m}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <Label>Categoria</Label>
              <Select
                value={categoriaId || "none"}
                onValueChange={(v) => setCategoriaId(v === "none" ? "" : v)}
              >
                <SelectTrigger><SelectValue placeholder="Sem categoria" /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">Sem categoria</SelectItem>
                  {categorias.map((c) => (
                    <SelectItem key={c.id} value={c.id}>{c.nome}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1">
              <Label>Parcelas (vazio = indefinido)</Label>
              <Input
                type="number"
                min={1}
                value={totalParcelas}
                onChange={(e) => setTotalParcelas(e.target.value)}
                placeholder="∞"
              />
            </div>
          </div>

          {!isEdit && (
            <div className="space-y-1">
              <Label>Primeira geração em</Label>
              <Input
                type="date"
                value={dataInicio}
                onChange={(e) => setDataInicio(e.target.value)}
              />
            </div>
          )}

          {isEdit && (
            <div className="space-y-1">
              <Label>Aplicar alterações a</Label>
              <Select value={scope} onValueChange={(v) => setScope(v as "futuros" | "todos")}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="futuros">Somente lançamentos futuros</SelectItem>
                  <SelectItem value="todos">Todos os lançamentos pendentes</SelectItem>
                </SelectContent>
              </Select>
            </div>
          )}

          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose} disabled={loading}>
            Cancelar
          </Button>
          <Button
            onClick={handleSave}
            disabled={loading || !descricao.trim() || !valor}
          >
            {loading ? "Salvando..." : "Salvar"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

- [ ] **Step 2: Verify TypeScript**

```bash
cd C:/Projetos/web-financas && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/recorrencias/recorrencia-modal.tsx
git commit -m "feat(ui): add RecorrenciaModal — create/edit form with frequency-aware fields and scope selector"
```

---

### Task 8: RecorrenciaCard component

**Files:**
- Create: `src/components/recorrencias/recorrencia-card.tsx`

- [ ] **Step 1: Create card component**

```typescript
// src/components/recorrencias/recorrencia-card.tsx
"use client"

import { useState } from "react"
import { Card, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { format } from "date-fns"
import { ptBR } from "date-fns/locale"
import { Pencil, Trash2, Pause, Play } from "lucide-react"
import type { RecorrenciaItem } from "@/types/recorrencia"

const FREQUENCIA_LABEL: Record<string, string> = {
  SEMANAL: "Semanal",
  QUINZENAL: "Quinzenal",
  MENSAL: "Mensal",
  ANUAL: "Anual",
}

interface Props {
  recorrencia: RecorrenciaItem
  onEdit: (r: RecorrenciaItem) => void
  onRefresh: () => void
}

export function RecorrenciaCard({ recorrencia: r, onEdit, onRefresh }: Props) {
  const [loading, setLoading] = useState(false)

  const fmt = (v: number) =>
    v.toLocaleString("pt-BR", { style: "currency", currency: "BRL" })

  async function handleDelete() {
    if (!confirm(`Excluir "${r.descricao}"? Os lançamentos já gerados não serão removidos.`)) return
    setLoading(true)
    await fetch(`/api/recorrencias/${r.id}`, { method: "DELETE" })
    setLoading(false)
    onRefresh()
  }

  async function handleToggle() {
    setLoading(true)
    await fetch(`/api/recorrencias/${r.id}/toggle`, { method: "POST" })
    setLoading(false)
    onRefresh()
  }

  const parcelas = r.totalParcelas
    ? `Parcela ${r.parcelaAtual}/${r.totalParcelas}`
    : "Indefinida"

  return (
    <Card className={r.ativa ? "" : "opacity-50"}>
      <CardContent className="pt-4 space-y-2">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
            <p className="font-medium truncate">{r.descricao}</p>
            <p className={`text-lg font-bold ${r.tipo === "RECEITA" ? "text-green-500" : "text-red-500"}`}>
              {fmt(r.valor)}
            </p>
          </div>
          <Badge variant="outline" className="shrink-0 text-xs">
            {FREQUENCIA_LABEL[r.frequencia]}
          </Badge>
        </div>

        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span>{parcelas}</span>
          <span>
            Próxima: {format(new Date(r.proximaGeracao), "dd/MM/yyyy", { locale: ptBR })}
          </span>
        </div>

        {r.categoriaNome && (
          <div className="flex items-center gap-1">
            <span
              className="inline-block h-2 w-2 rounded-full"
              style={{ background: r.categoriaCor ?? "#94a3b8" }}
            />
            <span className="text-xs text-muted-foreground">{r.categoriaNome}</span>
          </div>
        )}

        <div className="flex items-center gap-1 pt-1">
          <Button size="sm" variant="ghost" onClick={() => onEdit(r)} disabled={loading}>
            <Pencil className="h-3.5 w-3.5" />
          </Button>
          <Button size="sm" variant="ghost" onClick={handleToggle} disabled={loading}>
            {r.ativa
              ? <Pause className="h-3.5 w-3.5" />
              : <Play className="h-3.5 w-3.5" />}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            className="text-destructive hover:text-destructive"
            onClick={handleDelete}
            disabled={loading}
          >
            <Trash2 className="h-3.5 w-3.5" />
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
```

- [ ] **Step 2: Verify TypeScript**

```bash
cd C:/Projetos/web-financas && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/recorrencias/recorrencia-card.tsx
git commit -m "feat(ui): add RecorrenciaCard — display with pause/edit/delete actions"
```

---

### Task 9: RecorrenciasClient orchestrator

**Files:**
- Create: `src/components/recorrencias/recorrencias-client.tsx`

- [ ] **Step 1: Create client component**

```typescript
// src/components/recorrencias/recorrencias-client.tsx
"use client"

import { useState } from "react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Plus, RefreshCw } from "lucide-react"
import { RecorrenciaCard } from "./recorrencia-card"
import { RecorrenciaModal } from "./recorrencia-modal"
import type { RecorrenciaItem, GerarResponse } from "@/types/recorrencia"

interface Categoria {
  id: string
  nome: string
}

interface Props {
  initialRecorrencias: RecorrenciaItem[]
  categorias: Categoria[]
}

export function RecorrenciasClient({ initialRecorrencias, categorias }: Props) {
  const [recorrencias, setRecorrencias] = useState(initialRecorrencias)
  const [modalOpen, setModalOpen] = useState(false)
  const [editTarget, setEditTarget] = useState<RecorrenciaItem | undefined>()
  const [gerando, setGerando] = useState(false)

  async function refresh() {
    const res = await fetch("/api/recorrencias")
    if (res.ok) setRecorrencias(await res.json())
  }

  async function handleGerar() {
    setGerando(true)
    try {
      const res = await fetch("/api/recorrencias/gerar", { method: "POST" })
      const data: GerarResponse = await res.json()
      if (!res.ok) {
        toast.error("Erro ao gerar lançamentos")
        return
      }
      toast.success(
        data.gerados === 0
          ? "Nenhum lançamento a gerar no momento"
          : `${data.gerados} lançamento${data.gerados !== 1 ? "s" : ""} gerado${data.gerados !== 1 ? "s" : ""}!`
      )
      await refresh()
    } catch {
      toast.error("Falha na conexão")
    } finally {
      setGerando(false)
    }
  }

  function openCreate() {
    setEditTarget(undefined)
    setModalOpen(true)
  }

  function openEdit(r: RecorrenciaItem) {
    setEditTarget(r)
    setModalOpen(true)
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h1 className="text-2xl font-bold">Recorrências</h1>
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={handleGerar} disabled={gerando}>
            <RefreshCw className={`mr-2 h-4 w-4 ${gerando ? "animate-spin" : ""}`} />
            Gerar lançamentos
          </Button>
          <Button onClick={openCreate}>
            <Plus className="mr-2 h-4 w-4" />
            Nova
          </Button>
        </div>
      </div>

      {recorrencias.length === 0 ? (
        <p className="py-12 text-center text-sm text-muted-foreground">
          Nenhuma recorrência cadastrada. Clique em "Nova" para começar.
        </p>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {recorrencias.map((r) => (
            <RecorrenciaCard
              key={r.id}
              recorrencia={r}
              onEdit={openEdit}
              onRefresh={refresh}
            />
          ))}
        </div>
      )}

      <RecorrenciaModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        onSaved={refresh}
        recorrencia={editTarget}
        categorias={categorias}
      />
    </div>
  )
}
```

- [ ] **Step 2: Verify TypeScript**

```bash
cd C:/Projetos/web-financas && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/recorrencias/recorrencias-client.tsx
git commit -m "feat(ui): add RecorrenciasClient — orchestrates list, modal, and gerar flow"
```

---

### Task 10: /recorrencias page

**Files:**
- Create: `src/app/(app)/recorrencias/page.tsx`

- [ ] **Step 1: Create server component page**

```typescript
// src/app/(app)/recorrencias/page.tsx
import type { Metadata } from "next"
import { redirect } from "next/navigation"
import { auth } from "@/lib/auth"
import { db } from "@/lib/db"
import { RecorrenciasClient } from "@/components/recorrencias/recorrencias-client"
import type { RecorrenciaItem } from "@/types/recorrencia"

export const metadata: Metadata = { title: "Recorrências" }

export default async function RecorrenciasPage() {
  const session = await auth()
  if (!session?.user?.id) redirect("/login")
  const userId = session.user.id

  const [recorrenciasRaw, categorias] = await Promise.all([
    db.recorrencia.findMany({
      where: { userId },
      include: { categoria: { select: { nome: true, cor: true } } },
      orderBy: { createdAt: "desc" },
    }),
    db.categoria.findMany({ where: { userId }, orderBy: { nome: "asc" } }),
  ])

  const recorrencias: RecorrenciaItem[] = recorrenciasRaw.map((r) => ({
    id: r.id,
    descricao: r.descricao,
    valor: Number(r.valor),
    tipo: r.tipo,
    frequencia: r.frequencia,
    diaVencimento: r.diaVencimento,
    mes: r.mes,
    categoriaId: r.categoriaId,
    categoriaNome: r.categoria?.nome ?? null,
    categoriaCor: r.categoria?.cor ?? null,
    totalParcelas: r.totalParcelas,
    parcelaAtual: r.parcelaAtual,
    ativa: r.ativa,
    proximaGeracao: r.proximaGeracao.toISOString(),
    createdAt: r.createdAt.toISOString(),
  }))

  return (
    <div className="p-6">
      <RecorrenciasClient
        initialRecorrencias={recorrencias}
        categorias={categorias.map((c) => ({ id: c.id, nome: c.nome }))}
      />
    </div>
  )
}
```

- [ ] **Step 2: Verify TypeScript**

```bash
cd C:/Projetos/web-financas && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add "src/app/(app)/recorrencias/page.tsx"
git commit -m "feat(page): add /recorrencias server component page"
```

---

### Task 11: Sidebar update

**Files:**
- Modify: `src/components/layout/sidebar.tsx`

- [ ] **Step 1: Add RefreshCw import and navItem**

In `src/components/layout/sidebar.tsx`:

1. Add `RefreshCw` to the lucide-react import:
```typescript
import {
  LayoutDashboard,
  Receipt,
  BarChart3,
  Sparkles,
  Settings,
  LogOut,
  Wallet,
  Menu,
  FileSearch,
  RefreshCw,
} from "lucide-react"
```

2. Update `navItems` array — add Recorrências between Lançamentos and Relatórios:
```typescript
const navItems = [
  { href: "/dashboard",     label: "Dashboard",     icon: LayoutDashboard },
  { href: "/lancamentos",   label: "Lançamentos",   icon: Receipt },
  { href: "/recorrencias",  label: "Recorrências",  icon: RefreshCw },
  { href: "/relatorios",    label: "Relatórios",    icon: BarChart3 },
  { href: "/ia",            label: "Análise IA",    icon: Sparkles },
  { href: "/extrato",       label: "Extrato",       icon: FileSearch },
  { href: "/configuracoes", label: "Configurações", icon: Settings },
]
```

- [ ] **Step 2: Verify TypeScript**

```bash
cd C:/Projetos/web-financas && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/layout/sidebar.tsx
git commit -m "feat(nav): add Recorrências link to sidebar"
```
