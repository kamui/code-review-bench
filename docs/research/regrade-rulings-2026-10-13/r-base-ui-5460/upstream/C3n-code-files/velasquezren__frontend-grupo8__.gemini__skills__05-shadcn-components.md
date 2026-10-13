# 🧩 Componentes shadcn/ui — Guía de Uso

> Componentes de shadcn/ui copiados al proyecto. Usa **Base UI** (`@base-ui/react`) como primitivos. Directorio: `components/ui/`. Siempre importar desde `@/components/ui/`.

## Inicialización

```bash
npx shadcn@latest init
# Opciones recomendadas:
# Style: Default
# Base color: Slate (o Zinc para banking)
# CSS variables: Yes
# Alias: @/components, @/lib
```

## Componentes Esenciales para Banca

### Instalar todos de una vez
```bash
npx shadcn@latest add button card input label dialog sheet dropdown-menu \
  avatar badge separator skeleton table tabs toast select textarea \
  form popover command scroll-area tooltip alert alert-dialog switch \
  navigation-menu sidebar breadcrumb chart
```

## Catálogo por Uso

### 📋 Layout & Navegación
| Componente | Uso |
|-----------|-----|
| `sidebar` | Navegación lateral principal |
| `sheet` | Nav móvil (slide-in) |
| `navigation-menu` | Nav horizontal |
| `breadcrumb` | Indicar ubicación actual |
| `tabs` | Cambiar entre vistas |
| `separator` | Dividir secciones |
| `scroll-area` | Scroll personalizado |

### 📝 Formularios & Inputs
| Componente | Uso |
|-----------|-----|
| `form` | Wrapper con react-hook-form |
| `input` | Campos de texto |
| `select` | Dropdowns |
| `textarea` | Texto multilínea |
| `label` | Labels accesibles |
| `switch` | Toggles on/off |
| `button` | Acciones |

### 📊 Datos & Display
| Componente | Uso |
|-----------|-----|
| `card` | Contenedores de contenido |
| `table` | Tablas de datos |
| `badge` | Etiquetas de estado |
| `avatar` | Foto de usuario |
| `skeleton` | Loading states |
| `chart` | Gráficas (Recharts) |
| `tooltip` | Info adicional al hover |

### 🔔 Feedback & Overlays
| Componente | Uso |
|-----------|-----|
| `dialog` | Modales de confirmación |
| `alert-dialog` | Confirmaciones destructivas |
| `alert` | Mensajes en línea |
| `toast` / `sonner` | Notificaciones temporales |
| `popover` | Contenido flotante |
| `command` | Paleta de comandos (Ctrl+K) |
| `dropdown-menu` | Menú contextual |

## Patterns de Uso

### Button Variants
```tsx
import { Button } from '@/components/ui/button'

<Button>Default</Button>
<Button variant="secondary">Secondary</Button>
<Button variant="outline">Outline</Button>
<Button variant="ghost">Ghost</Button>
<Button variant="destructive">Eliminar</Button>
<Button variant="link">Link</Button>
<Button size="sm">Small</Button>
<Button size="lg">Large</Button>
<Button size="icon"><Icon className="h-4 w-4" /></Button>
```

### Card Pattern
```tsx
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card'

<Card>
  <CardHeader>
    <CardTitle>Cuenta de Ahorro</CardTitle>
    <CardDescription>****4532</CardDescription>
  </CardHeader>
  <CardContent>
    <p className="text-2xl font-bold font-mono tabular-nums">$12,450.00</p>
  </CardContent>
  <CardFooter>
    <Button variant="outline" size="sm">Ver detalles</Button>
  </CardFooter>
</Card>
```

### Form con react-hook-form + zod
```tsx
'use client'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from '@/components/ui/form'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'

const schema = z.object({
  amount: z.coerce.number().positive('Debe ser positivo'),
  destinatario: z.string().min(1, 'Requerido'),
})

export function TransferForm() {
  const form = useForm<z.infer<typeof schema>>({
    resolver: zodResolver(schema),
    defaultValues: { amount: 0, destinatario: '' },
  })

  function onSubmit(values: z.infer<typeof schema>) {
    // Server action o mutation
  }

  return (
    <Form {...form}>
      <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
        <FormField control={form.control} name="destinatario" render={({ field }) => (
          <FormItem>
            <FormLabel>Destinatario</FormLabel>
            <FormControl><Input placeholder="Nombre o cuenta" {...field} /></FormControl>
            <FormMessage />
          </FormItem>
        )} />
        <FormField control={form.control} name="amount" render={({ field }) => (
          <FormItem>
            <FormLabel>Monto</FormLabel>
            <FormControl><Input type="number" step="0.01" {...field} /></FormControl>
            <FormMessage />
          </FormItem>
        )} />
        <Button type="submit" className="w-full">Transferir</Button>
      </form>
    </Form>
  )
}
```

### Badge de Estado
```tsx
import { Badge } from '@/components/ui/badge'

// Transacciones
<Badge variant="default">Completada</Badge>
<Badge variant="secondary">Pendiente</Badge>
<Badge variant="destructive">Rechazada</Badge>
<Badge variant="outline">En proceso</Badge>
```

### Table
```tsx
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'

<Table>
  <TableHeader>
    <TableRow>
      <TableHead>Fecha</TableHead>
      <TableHead>Descripción</TableHead>
      <TableHead className="text-right">Monto</TableHead>
    </TableRow>
  </TableHeader>
  <TableBody>
    {transactions.map((tx) => (
      <TableRow key={tx.id}>
        <TableCell>{tx.date}</TableCell>
        <TableCell>{tx.description}</TableCell>
        <TableCell className="text-right font-mono tabular-nums">
          {formatCurrency(tx.amount)}
        </TableCell>
      </TableRow>
    ))}
  </TableBody>
</Table>
```

### Toast (Sonner)
```tsx
// app/layout.tsx — montar el Toaster
import { Toaster } from '@/components/ui/sonner'
<body><Toaster />{children}</body>

// En cualquier Client Component
import { toast } from 'sonner'
toast.success('Transferencia exitosa')
toast.error('Error al procesar')
toast.loading('Procesando...')
```

## Reglas

1. **NUNCA modificar** la API pública de un componente shadcn sin documentarlo
2. **SIEMPRE** usar `FormField` para conectar react-hook-form con shadcn inputs
3. **Personalizar** vía las CSS variables, NO hardcodeando colores
4. **Skeleton** matching — Cada componente debe tener un skeleton equivalente
5. **NUNCA usar `asChild`** — Usar la prop `render` para composición de componentes:
   ```tsx
   // ✅ Correcto — render prop (Base UI)
   <Button render={<Link href="/dashboard">Dashboard</Link>} />
   <DropdownMenuTrigger render={<Button variant="ghost">Open</Button>} />
   <SidebarMenuButton render={<Link href="/cuentas">Cuentas</Link>} />
   
   // ❌ Incorrecto — asChild NO existe en Base UI
   <Button asChild><Link href="/">Home</Link></Button>
   ```
6. **Select `onValueChange`** retorna `string | null`, no solo `string`:
   ```tsx
   // ✅ Correcto
   <Select value={val} onValueChange={(v) => setVal(v || 'default')}>
   
   // ❌ Incorrecto — TypeScript error
   <Select value={val} onValueChange={setVal}>
   ```
