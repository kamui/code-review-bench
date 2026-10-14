# Bug Report: Select Tipe Dompet Tidak Berfungsi di Dalam Dialog (Round 2)

**Project:** Finance Memo  
**File:** `WalletForm.tsx`, `select.tsx`, `dialog.tsx`  
**Status:** 🔴 Critical — Dropdown posisi meleset + opsi tidak bisa dipilih  
**Reporter:** Code Review (Senior Dev / QA / PM)

---

## Konteks

Setelah fix pertama (`defaultValue` → `value`), masalah ternyata belum selesai. Screenshot menunjukkan dua gejala baru:

1. **Dropdown muncul jauh di pojok kanan bawah** — bukan di bawah trigger
2. **Opsi Bank dan E-Wallet tetap tidak bisa diklik**

Setelah review `dialog.tsx` dan `select.tsx`, ditemukan bahwa **library yang digunakan bukan Radix UI — melainkan `@base-ui/react` (Base UI dari tim MUI).** Kedua library ini punya API dan perilaku yang sama sekali berbeda, sehingga asumsi fix pertama sebagian tidak tepat.

---

## Temuan & Root Cause

### Temuan 1 — Koreksi Fix Sebelumnya: `asChild` SALAH untuk Base UI

Saran saya di round 1 untuk mengganti `DialogTrigger` menggunakan pola `asChild` adalah **pattern Radix UI, bukan Base UI**.

```tsx
// ❌ Saran round 1 — pattern Radix UI, tidak berlaku di sini
<DialogTrigger asChild>
  <Button>...</Button>
</DialogTrigger>

// ✅ Base UI memang menggunakan render prop — kode aslimu sudah benar
<DialogTrigger render={<Button .../>} />
```

> Ini tidak menyebabkan bug select, tapi perlu diluruskan agar tidak membingungkan.

---

### Temuan 2 — Root Cause Posisi Dropdown Meleset

Di `select.tsx`, default value `alignItemWithTrigger` di-set `true`:

```tsx
// select.tsx
alignItemWithTrigger = true,  // ← ini penyebab posisi meleset
```

Base UI Select dengan `alignItemWithTrigger={true}` mencoba **menyejajarkan item yang sedang terpilih dengan posisi trigger secara visual**. Di dalam Dialog yang menggunakan `transform: translate(-50%, -50%)`, kalkulasi posisi ini kacau.

**Mengapa?** CSS `transform` pada ancestor element menciptakan *containing block* baru untuk `position: fixed`. Akibatnya, koordinat anchor yang dihitung Base UI merujuk ke containing block yang salah, sehingga dropdown meleset jauh ke pojok.

---

### Temuan 3 — Root Cause Opsi Tidak Bisa Diklik: Base UI Dialog Focus Trap

Base UI Dialog secara default menjalankan **modal focus trap** yang memblokir semua interaksi dengan elemen di luar DOM subtree-nya.

`Select.Positioner` mem-portal kontennya ke `<body>` (di luar subtree Dialog), sehingga **Base UI Dialog menganggap klik ke dropdown sebagai "outside interaction"** dan menelan event tersebut sebelum seleksi sempat terjadi.

Ini bukan bug di library — ini by design — tapi perlu dihandle secara eksplisit di level komponen.

---

## Fix

### Fix 1 — `select.tsx`: Ubah Default `alignItemWithTrigger` ke `false`

```tsx
// select.tsx — baris default value di SelectContent
// Sebelum:
alignItemWithTrigger = true,

// Sesudah:
alignItemWithTrigger = false,
```

---

### Fix 2 — `WalletForm.tsx`: Guard Dialog + Controlled Select Open State

Solusinya adalah mengontrol state `open` Select secara manual, lalu mencegah Dialog menutup selama Select masih terbuka.

```tsx
'use client'

import { useState } from 'react'
import { Plus } from 'lucide-react'
import { useForm, Controller, SubmitHandler } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { 
  Dialog, 
  DialogContent, 
  DialogHeader, 
  DialogTitle, 
  DialogTrigger 
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { createWallet } from '@/actions/wallets'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'

const schema = z.object({
  name: z.string().min(1, 'Nama dompet wajib diisi'),
  type: z.string().min(1, 'Tipe dompet wajib dipilih'),
  balance: z.number().min(0, 'Saldo minimal 0'),
  color: z.string().min(1),
})

type WalletData = z.infer<typeof schema>

export function WalletForm() {
  const [open, setOpen] = useState(false)
  // Track apakah Select sedang terbuka
  const [selectOpen, setSelectOpen] = useState(false)

  const { register, handleSubmit, control, reset, formState: { isSubmitting, errors } } = useForm<WalletData>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: '',
      type: 'cash',
      balance: 0,
      color: '#10b981',
    }
  })

  const onSubmit: SubmitHandler<WalletData> = async (data) => {
    try {
      await createWallet(data)
      setOpen(false)
      reset()
      toast.success('Dompet baru telah ditambahkan.')
    } catch (error) {
      toast.error('Terjadi kesalahan saat menambah dompet.')
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        // Jangan tutup Dialog kalau Select masih terbuka
        if (selectOpen) return
        setOpen(v)
        if (!v) reset()
      }}
    >
      {/* ✅ render prop adalah pattern yang benar untuk Base UI */}
      <DialogTrigger
        render={
          <Button type="button" className="bg-emerald-600 hover:bg-emerald-700 shadow-lg shadow-emerald-600/20">
            <Plus className="h-4 w-4 mr-2" />
            Tambah Dompet
          </Button>
        }
      />
      <DialogContent className="sm:max-w-[425px]">
        <DialogHeader>
          <DialogTitle>Tambah Dompet Baru</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit(onSubmit)} className="space-y-6 pt-4">
          <div className="space-y-2">
            <Label htmlFor="name">Nama Dompet</Label>
            <Input
              id="name"
              placeholder="Contoh: Tabungan Utama"
              {...register('name')}
              className={errors.name ? 'border-destructive' : ''}
            />
            {errors.name && <p className="text-xs text-destructive mt-1">{errors.name.message}</p>}
          </div>

          <div className="space-y-2">
            <Label>Tipe Dompet</Label>
            <Controller
              name="type"
              control={control}
              render={({ field }) => (
                <Select
                  value={field.value}
                  onValueChange={(val) => {
                    field.onChange(val)
                    setSelectOpen(false)
                  }}
                  open={selectOpen}
                  onOpenChange={setSelectOpen}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="Pilih tipe" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="cash">Tunai (Cash)</SelectItem>
                    <SelectItem value="bank">Bank</SelectItem>
                    <SelectItem value="ewallet">E-Wallet</SelectItem>
                  </SelectContent>
                </Select>
              )}
            />
            {errors.type && <p className="text-xs text-destructive mt-1">{errors.type.message}</p>}
          </div>

          <div className="space-y-2">
            <Label htmlFor="balance">Saldo Awal</Label>
            <div className="relative">
              <span className="absolute left-3 top-2.5 text-slate-500 text-sm">Rp</span>
              <Input
                id="balance"
                type="number"
                placeholder="0"
                {...register('balance', { valueAsNumber: true })}
                className={cn("pl-10", errors.balance ? "border-destructive" : "")}
              />
            </div>
            {errors.balance && <p className="text-xs text-destructive mt-1">{errors.balance.message}</p>}
          </div>

          <div className="space-y-2">
            <Label htmlFor="color">Warna Label</Label>
            <Input id="color" type="color" className="h-10 p-1" {...register('color')} />
          </div>

          <Button type="submit" className="w-full bg-emerald-600 hover:bg-emerald-700 text-white" disabled={isSubmitting}>
            {isSubmitting ? 'Menyimpan...' : 'Simpan Dompet'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}
```

---

## Ringkasan Diagnosis

| # | Masalah | Penyebab | File yang Diubah |
|---|---------|----------|-----------------|
| 1 | Dropdown muncul di pojok | `alignItemWithTrigger=true` + CSS `transform` Dialog menciptakan *containing block* baru untuk `position: fixed` | `select.tsx` |
| 2 | Item tidak bisa diklik | Base UI Dialog focus trap memblokir klik ke portal Select yang ada di luar DOM subtree Dialog | `WalletForm.tsx` |
| 3 | Saran `asChild` round 1 salah | Base UI menggunakan `render={}` bukan `asChild` — kode trigger asli sudah benar | — |

---

## Catatan untuk Tim

> **Pelajaran utama:** `@base-ui/react` dan `@radix-ui/react` memiliki API yang serupa secara visual tapi berbeda secara perilaku, terutama dalam hal focus management dan portal positioning. Jangan asumsikan pola dari satu library berlaku di library lain. Selalu cek `dialog.tsx` dan `select.tsx` untuk mengetahui underlying primitive yang digunakan sebelum melakukan debugging.

---

*Generated from code review — Finance Memo Project*
