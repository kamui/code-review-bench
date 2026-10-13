# Agency Dashboard UI Overhaul — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task.

**Goal:** Transform the rough Sales CRM dashboard into a premium, Awwwards-caliber agency dashboard with polished UI/UX, animations, mobile responsiveness, and convenient features.

**Architecture:** Component-based React SPA with FastAPI backend. Focus is entirely on frontend enhancement — backend changes are minimal (add one endpoint for CSV export). Use Magic UI components + custom shadcn-style components. Dark theme remains default.

**Tech Stack:** React 18, TypeScript, Tailwind CSS 3, @base-ui/react, motion (Framer Motion), recharts, Magic UI, lucide-react

## Global Constraints

- All existing functionality must continue to work
- No breaking API changes to the backend
- Keep the dark theme as default
- All new components must be accessible (ARIA labels, keyboard nav)
- Mobile responsive: sidebar collapses, tables scroll horizontally
- No hardcoded secrets. All API URLs go through Vite proxy
- Follow existing file patterns (components at `@/components/ui/`, pages at `@/pages/`)
- Use Tailwind CSS for all styling; no CSS-in-JS
- New components must have loading states and error handling

---

## File Structure Changes

### New files to create:
- `frontend/src/components/ui/skeleton.tsx` — Loading skeleton component
- `frontend/src/components/ui/toast.tsx` — Toast notification system
- `frontend/src/components/ui/tooltip.tsx` — Tooltip component
- `frontend/src/components/ui/avatar.tsx` — Avatar component
- `frontend/src/components/ui/dropdown-menu.tsx` — Dropdown menu component
- `frontend/src/components/ui/bento-grid.tsx` — Bento grid layout
- `frontend/src/components/ui/number-ticker.tsx` — Animated number counter
- `frontend/src/components/ui/border-beam.tsx` — Animated border beam
- `frontend/src/components/ui/animated-list.tsx` — Animated list for activity feed
- `frontend/src/components/ui/particles.tsx` — Particle background
- `frontend/src/components/ui/confetti.tsx` — Confetti celebration effect
- `frontend/src/components/ui/progressive-blur.tsx` — Blur transition effect
- `frontend/src/components/layout/Sidebar.tsx` — New collapsible sidebar
- `frontend/src/components/layout/PageHeader.tsx` — Reusable page header
- `frontend/src/components/layout/AppShell.tsx` — Main app layout shell
- `frontend/src/components/leads/LeadDetailDialog.tsx` — Lead detail/edit modal
- `frontend/src/components/leads/AssignRepDialog.tsx` — Assign rep to leads modal
- `frontend/src/components/leads/DeleteConfirmDialog.tsx` — Delete confirmation modal
- `frontend/src/components/leads/BulkActions.tsx` — Bulk selection toolbar
- `frontend/src/components/dashboard/KpiCard.tsx` — Enhanced KPI card
- `frontend/src/components/dashboard/ActivityFeed.tsx` — Recent activity stream
- `frontend/src/components/dashboard/TrendChart.tsx` — Line chart for trends
- `frontend/src/components/dashboard/DateRangeFilter.tsx` — Date range picker
- `frontend/src/components/rep/CallTimer.tsx` — Call duration timer
- `frontend/src/components/rep/CallHistory.tsx` — Enhanced call history
- `frontend/src/hooks/useToast.ts` — Toast notification hook
- `frontend/src/hooks/useMediaQuery.ts` — Media query hook for responsive
- `frontend/src/lib/export-csv.ts` — CSV export utility
- `frontend/src/lib/format.ts` — Formatting utilities (dates, currency, phone)
- `backend/app/api/export.py` — CSV export endpoint

### Files to modify:
- `frontend/src/index.css` — Enhanced design tokens, animations
- `frontend/src/App.tsx` — New layout, toast provider, page transitions
- `frontend/src/pages/Login.tsx` — Complete redesign
- `frontend/src/pages/manager/Dashboard.tsx` — Enhanced with trends, activity, date filter
- `frontend/src/pages/manager/Leads.tsx` — Enhanced with dialogs, bulk actions, export
- `frontend/src/pages/rep/CallingView.tsx` — Enhanced with timer, better history
- `frontend/src/api/client.ts` — Add export API call
- `frontend/src/types.ts` — Add new type definitions
- `backend/app/main.py` — Register export router

---

## Tasks

### Task 1: Design System Enhancement & New UI Components

**Files:**
- Modify: `frontend/src/index.css`
- Create: `frontend/src/components/ui/skeleton.tsx`
- Create: `frontend/src/components/ui/toast.tsx`
- Create: `frontend/src/components/ui/tooltip.tsx`
- Create: `frontend/src/components/ui/avatar.tsx`
- Create: `frontend/src/components/ui/dropdown-menu.tsx`
- Create: `frontend/src/components/ui/number-ticker.tsx`
- Create: `frontend/src/components/ui/border-beam.tsx`
- Create: `frontend/src/components/ui/animated-list.tsx`
- Create: `frontend/src/components/ui/particles.tsx`
- Create: `frontend/src/components/ui/confetti.tsx`
- Create: `frontend/src/components/ui/progressive-blur.tsx`
- Create: `frontend/src/hooks/useToast.ts`
- Create: `frontend/src/hooks/useMediaQuery.ts`
- Create: `frontend/src/lib/format.ts`

**Interfaces:**
- Consumes: Existing design tokens in `index.css`
- Produces: Reusable component library for all subsequent tasks

- [ ] **Step 1: Enhance index.css with design tokens**

Add to `frontend/src/index.css` — new animation keyframes and refined variables:

```css
/* Add after existing animations */
@keyframes fade-in {
  from { opacity: 0; transform: translateY(4px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes slide-in-right {
  from { transform: translateX(100%); opacity: 0; }
  to { transform: translateX(0); opacity: 1; }
}

@keyframes scale-in {
  from { transform: scale(0.95); opacity: 0; }
  to { transform: scale(1); opacity: 1; }
}

@layer utilities {
  .animate-fade-in {
    animation: fade-in 0.3s ease-out;
  }
  .animate-slide-in-right {
    animation: slide-in-right 0.3s ease-out;
  }
  .animate-scale-in {
    animation: scale-in 0.2s ease-out;
  }
}
```

Also add refined card/surface colors to `:root`:
```css
:root {
  /* ... existing vars ... */
  --surface: oklch(0.17 0 0);
  --surface-elevated: oklch(0.22 0 0);
  --glass: oklch(1 0 0 / 5%);
  --glass-border: oklch(1 0 0 / 8%);
}
```

Add corresponding utilities in `tailwind.config.js`:
```js
colors: {
  surface: { DEFAULT: 'oklch(0.17 0 0)', elevated: 'oklch(0.22 0 0)' },
  glass: { DEFAULT: 'oklch(1 0 0 / 5%)', border: 'oklch(1 0 0 / 8%)' },
}
```

- [ ] **Step 2: Create skeleton.tsx**

```tsx
import { cn } from '@/lib/utils'

function Skeleton({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      data-slot="skeleton"
      className={cn('animate-pulse rounded-md bg-muted', className)}
      {...props}
    />
  )
}

export { Skeleton }
```

- [ ] **Step 3: Create toast system (toast.tsx + useToast.ts)**

`frontend/src/hooks/useToast.ts`:
```tsx
import { useState, useCallback } from 'react'

export interface Toast {
  id: string
  title: string
  description?: string
  variant?: 'default' | 'success' | 'error' | 'warning'
}

let toastListeners: ((toast: Toast) => void)[] = []
let toastId = 0

export function toast(toast: Omit<Toast, 'id'>) {
  const id = String(++toastId)
  const newToast = { ...toast, id }
  toastListeners.forEach((listener) => listener(newToast))
  return id
}

export function useToast() {
  const [toasts, setToasts] = useState<Toast[]>([])

  const addToast = useCallback((t: Toast) => {
    setToasts((prev) => [...prev, t])
    setTimeout(() => {
      setToasts((prev) => prev.filter((x) => x.id !== t.id))
    }, 4000)
  }, [])

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((x) => x.id !== id))
  }, [])

  useState(() => {
    toastListeners.push(addToast)
    return () => {
      toastListeners = toastListeners.filter((l) => l !== addToast)
    }
  })

  return { toasts, dismissToast }
}
```

`frontend/src/components/ui/toast.tsx`:
```tsx
import * as React from 'react'
import { cn } from '@/lib/utils'
import { X, CheckCircle, AlertCircle, AlertTriangle, Info } from 'lucide-react'
import { useToast, type Toast } from '@/hooks/useToast'

const icons = {
  success: CheckCircle,
  error: AlertCircle,
  warning: AlertTriangle,
  default: Info,
}

const colors = {
  success: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-400',
  error: 'border-red-500/30 bg-red-500/10 text-red-400',
  warning: 'border-amber-500/30 bg-amber-500/10 text-amber-400',
  default: 'border-border bg-card',
}

function ToastContainer() {
  const { toasts, dismissToast } = useToast()
  return (
    <div className="fixed bottom-4 right-4 z-[100] flex flex-col gap-2 max-w-sm">
      {toasts.map((t) => {
        const Icon = icons[t.variant || 'default']
        return (
          <div
            key={t.id}
            className={cn(
              'animate-slide-in-right flex items-start gap-3 rounded-lg border p-4 text-sm shadow-lg',
              colors[t.variant || 'default']
            )}
          >
            <Icon className="mt-0.5 h-4 w-4 shrink-0" />
            <div className="flex-1">
              <p className="font-medium">{t.title}</p>
              {t.description && (
                <p className="mt-1 text-xs opacity-80">{t.description}</p>
              )}
            </div>
            <button onClick={() => dismissToast(t.id)} className="shrink-0 opacity-60 hover:opacity-100">
              <X className="h-4 w-4" />
            </button>
          </div>
        )
      })}
    </div>
  )
}

export { ToastContainer, toast, useToast }
```

- [ ] **Step 4: Create tooltip.tsx**

```tsx
import * as React from 'react'
import { cn } from '@/lib/utils'

interface TooltipProps {
  content: string
  children: React.ReactNode
  side?: 'top' | 'bottom' | 'left' | 'right'
}

function Tooltip({ content, children, side = 'top' }: TooltipProps) {
  const [visible, setVisible] = React.useState(false)
  const positionClasses = {
    top: 'bottom-full left-1/2 -translate-x-1/2 mb-2',
    bottom: 'top-full left-1/2 -translate-x-1/2 mt-2',
    left: 'right-full top-1/2 -translate-y-1/2 mr-2',
    right: 'left-full top-1/2 -translate-y-1/2 ml-2',
  }

  return (
    <div className="relative inline-flex" onMouseEnter={() => setVisible(true)} onMouseLeave={() => setVisible(false)} onFocus={() => setVisible(true)} onBlur={() => setVisible(false)}>
      {children}
      {visible && (
        <div className={cn(
          'absolute z-50 pointer-events-none animate-scale-in',
          'rounded-md bg-popover px-2.5 py-1.5 text-xs font-medium text-popover-foreground shadow-md ring-1 ring-foreground/10',
          'whitespace-nowrap',
          positionClasses[side]
        )}>
          {content}
        </div>
      )}
    </div>
  )
}

export { Tooltip }
```

- [ ] **Step 5: Create avatar.tsx**

```tsx
import * as React from 'react'
import { cn } from '@/lib/utils'
import { User } from 'lucide-react'

interface AvatarProps {
  src?: string
  name?: string
  size?: 'sm' | 'md' | 'lg'
  className?: string
}

function Avatar({ src, name, size = 'md', className }: AvatarProps) {
  const sizeClasses = { sm: 'h-7 w-7 text-xs', md: 'h-9 w-9 text-sm', lg: 'h-12 w-12 text-base' }
  const initials = name ? name.split(' ').map(n => n[0]).join('').toUpperCase().slice(0, 2) : '?'

  return (
    <div className={cn(
      'relative inline-flex items-center justify-center rounded-full bg-muted font-medium text-muted-foreground shrink-0 overflow-hidden',
      sizeClasses[size],
      className
    )}>
      {src ? (
        <img src={src} alt={name || 'Avatar'} className="h-full w-full object-cover" />
      ) : (
        name ? <span>{initials}</span> : <User className="h-4 w-4" />
      )}
    </div>
  )
}

export { Avatar }
```

- [ ] **Step 6: Create number-ticker.tsx**

```tsx
'use client'
import { useEffect, useRef } from 'react'
import { useInView } from 'motion/react'
import { cn } from '@/lib/utils'

function NumberTicker({
  value,
  direction = 'up',
  delay = 0,
  className,
  prefix = '',
  suffix = '',
}: {
  value: number
  direction?: 'up' | 'down'
  delay?: number
  className?: string
  prefix?: string
  suffix?: string
}) {
  const ref = useRef<HTMLSpanElement>(null)
  const isInView = useInView(ref, { once: true, margin: '0px' })

  useEffect(() => {
    if (!isInView) return
    const startTime = Date.now() + delay * 1000
    const duration = 1500
    const startValue = 0
    const endValue = value

    const tick = () => {
      const elapsed = Date.now() - startTime
      const progress = Math.min(elapsed / duration, 1)
      const eased = 1 - Math.pow(1 - progress, 3)
      const current = Math.round(startValue + (endValue - startValue) * eased)

      if (ref.current) {
        ref.current.textContent = `${prefix}${current.toLocaleString()}${suffix}`
      }
      if (progress < 1) requestAnimationFrame(tick)
    }
    requestAnimationFrame(tick)
  }, [isInView, value, delay, prefix, suffix])

  return <span ref={ref} className={cn('tabular-nums', className)}>{prefix}0{suffix}</span>
}

export { NumberTicker }
```

- [ ] **Step 7: Create border-beam.tsx**

```tsx
import { cn } from '@/lib/utils'

function BorderBeam({
  className,
  size = 200,
  duration = 3,
  colorFrom = '#34d399',
  colorTo = '#059669',
}: {
  className?: string
  size?: number
  duration?: number
  colorFrom?: string
  colorTo?: string
}) {
  return (
    <div
      className={cn(
        'pointer-events-none absolute inset-0 rounded-[inherit] [border-radius:inherit] overflow-hidden',
        className
      )}
    >
      <div
        className="absolute inset-0 animate-shimmer-slide"
        style={{
          background: `conic-gradient(from 90deg at 50% 50%, transparent 0%, ${colorFrom} 25%, ${colorTo} 50%, transparent 75%)`,
          filter: 'blur(4px)',
          animationDuration: `${duration}s`,
          WebkitMask: 'radial-gradient(ellipse at 50% 50%, transparent 40%, black 70%)',
          mask: 'radial-gradient(ellipse at 50% 50%, transparent 40%, black 70%)',
        }}
      />
    </div>
  )
}

export { BorderBeam }
```

- [ ] **Step 8: Create animated-list.tsx**

```tsx
'use client'
import { useRef } from 'react'
import { motion, useInView } from 'motion/react'
import { cn } from '@/lib/utils'

function AnimatedList({
  children,
  className,
  delay = 0.1,
}: {
  children: React.ReactNode
  className?: string
  delay?: number
}) {
  const ref = useRef<HTMLDivElement>(null)
  const isInView = useInView(ref, { once: true, margin: '0px' })

  return (
    <div ref={ref} className={cn('flex flex-col', className)}>
      {Array.isArray(children) ? children.map((child, i) => (
        <motion.div
          key={i}
          initial={{ opacity: 0, y: 10 }}
          animate={isInView ? { opacity: 1, y: 0 } : {}}
          transition={{ delay: i * delay, duration: 0.3, ease: 'easeOut' }}
        >
          {child}
        </motion.div>
      )) : children}
    </div>
  )
}

export { AnimatedList }
```

- [ ] **Step 9: Create particles.tsx**

```tsx
import { useEffect, useRef } from 'react'
import { cn } from '@/lib/utils'

interface Particle {
  x: number
  y: number
  vx: number
  vy: number
  size: number
  opacity: number
}

function Particles({
  className,
  quantity = 50,
  color = '#34d399',
  speed = 0.5,
}: {
  className?: string
  quantity?: number
  color?: string
  speed?: number
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    let animationId: number
    let particles: Particle[] = []

    const resize = () => {
      canvas.width = canvas.offsetWidth * window.devicePixelRatio
      canvas.height = canvas.offsetHeight * window.devicePixelRatio
      ctx.scale(window.devicePixelRatio, window.devicePixelRatio)
    }

    const init = () => {
      particles = Array.from({ length: quantity }, () => ({
        x: Math.random() * canvas.offsetWidth,
        y: Math.random() * canvas.offsetHeight,
        vx: (Math.random() - 0.5) * speed,
        vy: (Math.random() - 0.5) * speed,
        size: Math.random() * 2 + 0.5,
        opacity: Math.random() * 0.5 + 0.1,
      }))
    }

    const animate = () => {
      ctx.clearRect(0, 0, canvas.offsetWidth, canvas.offsetHeight)
      for (const p of particles) {
        p.x += p.vx
        p.y += p.vy
        if (p.x < 0 || p.x > canvas.offsetWidth) p.vx *= -1
        if (p.y < 0 || p.y > canvas.offsetHeight) p.vy *= -1

        ctx.beginPath()
        ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2)
        ctx.fillStyle = color
        ctx.globalAlpha = p.opacity
        ctx.fill()
      }

      // Draw connections
      for (let i = 0; i < particles.length; i++) {
        for (let j = i + 1; j < particles.length; j++) {
          const dx = particles[i].x - particles[j].x
          const dy = particles[i].y - particles[j].y
          const dist = Math.sqrt(dx * dx + dy * dy)
          if (dist < 120) {
            ctx.beginPath()
            ctx.moveTo(particles[i].x, particles[i].y)
            ctx.lineTo(particles[j].x, particles[j].y)
            ctx.strokeStyle = color
            ctx.globalAlpha = 0.05 * (1 - dist / 120)
            ctx.stroke()
          }
        }
      }

      animationId = requestAnimationFrame(animate)
    }

    resize()
    init()
    animate()

    window.addEventListener('resize', resize)
    return () => {
      cancelAnimationFrame(animationId)
      window.removeEventListener('resize', resize)
    }
  }, [quantity, color, speed])

  return (
    <canvas
      ref={canvasRef}
      className={cn('absolute inset-0 pointer-events-none', className)}
    />
  )
}

export { Particles }
```

- [ ] **Step 10: Create confetti.tsx**

```tsx
import { useEffect, useRef } from 'react'

function Confetti({ active }: { active: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    if (!active) return
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    canvas.width = window.innerWidth
    canvas.height = window.innerHeight

    const colors = ['#34d399', '#60a5fa', '#fbbf24', '#a78bfa', '#f87171']
    const pieces = Array.from({ length: 100 }, () => ({
      x: Math.random() * canvas.width,
      y: -20 - Math.random() * 100,
      w: Math.random() * 8 + 4,
      h: Math.random() * 6 + 3,
      color: colors[Math.floor(Math.random() * colors.length)],
      vx: (Math.random() - 0.5) * 4,
      vy: Math.random() * 3 + 2,
      rotation: Math.random() * 360,
      rotSpeed: (Math.random() - 0.5) * 10,
    }))

    let frame = 0
    const animate = () => {
      frame++
      if (frame > 180) return // stop after ~3 seconds
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      for (const p of pieces) {
        p.x += p.vx
        p.y += p.vy
        p.vy += 0.05
        p.rotation += p.rotSpeed
        ctx.save()
        ctx.translate(p.x, p.y)
        ctx.rotate((p.rotation * Math.PI) / 180)
        ctx.fillStyle = p.color
        ctx.fillRect(-p.w / 2, -p.h / 2, p.w, p.h)
        ctx.restore()
      }
      requestAnimationFrame(animate)
    }
    animate()
  }, [active])

  return <canvas ref={canvasRef} className="fixed inset-0 pointer-events-none z-[200]" />
}

export { Confetti }
```

- [ ] **Step 11: Create useMediaQuery.ts**

```tsx
import { useState, useEffect } from 'react'

export function useMediaQuery(query: string) {
  const [matches, setMatches] = useState(false)

  useEffect(() => {
    const mql = window.matchMedia(query)
    setMatches(mql.matches)
    const handler = (e: MediaQueryListEvent) => setMatches(e.matches)
    mql.addEventListener('change', handler)
    return () => mql.removeEventListener('change', handler)
  }, [query])

  return matches
}

export function useIsMobile() {
  return useMediaQuery('(max-width: 768px)')
}
```

- [ ] **Step 12: Create format.ts**

```ts
export function formatCurrency(value: number | null | undefined): string {
  if (value == null) return '—'
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 0 }).format(value)
}

export function formatPhone(phone: string): string {
  const cleaned = phone.replace(/\D/g, '')
  if (cleaned.length === 10) {
    return `(${cleaned.slice(0, 3)}) ${cleaned.slice(3, 6)}-${cleaned.slice(6)}`
  }
  return phone
}

export function formatDate(date: string | null | undefined): string {
  if (!date) return '—'
  return new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit' }).format(new Date(date))
}

export function formatRelativeTime(date: string | null | undefined): string {
  if (!date) return ''
  const now = Date.now()
  const diff = now - new Date(date).getTime()
  const minutes = Math.floor(diff / 60000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  if (days < 7) return `${days}d ago`
  return formatDate(date)
}
```

- [ ] **Step 13: Create dropdown-menu.tsx**

```tsx
import * as React from 'react'
import { cn } from '@/lib/utils'

interface DropdownMenuProps {
  trigger: React.ReactNode
  children: React.ReactNode
  align?: 'start' | 'end'
  className?: string
}

function DropdownMenu({ trigger, children, align = 'start', className }: DropdownMenuProps) {
  const [open, setOpen] = React.useState(false)
  const ref = React.useRef<HTMLDivElement>(null)

  React.useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  return (
    <div ref={ref} className="relative inline-block">
      <div onClick={() => setOpen(!open)}>{trigger}</div>
      {open && (
        <div className={cn(
          'absolute z-50 mt-1 min-w-[10rem] animate-scale-in rounded-lg border bg-popover p-1 text-popover-foreground shadow-md',
          align === 'end' ? 'right-0' : 'left-0',
          className
        )}>
          {children}
        </div>
      )}
    </div>
  )
}

function DropdownMenuItem({
  children,
  onClick,
  className,
  destructive,
}: {
  children: React.ReactNode
  onClick?: () => void
  className?: string
  destructive?: boolean
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm outline-none transition-colors hover:bg-accent hover:text-accent-foreground',
        destructive && 'text-destructive hover:bg-destructive/10',
        className
      )}
    >
      {children}
    </button>
  )
}

export { DropdownMenu, DropdownMenuItem }
```

- [ ] **Step 14: Install the new animate utilities in tailwind.config.js**

Update `tailwind.config.js` to add the utility classes for glass, surface, and new animations.

- [ ] **Step 15: Commit**

```
git add frontend/src/components/ui/skeleton.tsx frontend/src/components/ui/toast.tsx frontend/src/components/ui/tooltip.tsx frontend/src/components/ui/avatar.tsx frontend/src/components/ui/dropdown-menu.tsx frontend/src/components/ui/number-ticker.tsx frontend/src/components/ui/border-beam.tsx frontend/src/components/ui/animated-list.tsx frontend/src/components/ui/particles.tsx frontend/src/components/ui/confetti.tsx frontend/src/components/ui/progressive-blur.tsx frontend/src/hooks/useToast.ts frontend/src/hooks/useMediaQuery.ts frontend/src/lib/format.ts frontend/src/index.css frontend/tailwind.config.js
git commit -m "feat: add design system enhancements and new UI components"
```

---

### Task 2: Responsive Sidebar & App Shell

**Files:**
- Create: `frontend/src/components/layout/Sidebar.tsx`
- Create: `frontend/src/components/layout/PageHeader.tsx`
- Create: `frontend/src/components/layout/AppShell.tsx`
- Modify: `frontend/src/App.tsx`

**Interfaces:**
- Consumes: `useIsMobile` from hooks, lucide icons, `useAuth` context
- Produces: New app layout used by all page routes

- [ ] **Step 1: Create Sidebar.tsx**

```tsx
import { useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { useAuth } from '@/context/AuthContext'
import { useIsMobile } from '@/hooks/useMediaQuery'
import { cn } from '@/lib/utils'
import {
  LayoutDashboard, Users, PhoneCall, LogOut, UserCircle,
  ChevronLeft, Menu, X,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Avatar } from '@/components/ui/avatar'

interface SidebarProps {
  collapsed: boolean
  onToggle: () => void
}

export function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const { user, logout } = useAuth()
  const location = useLocation()
  const isMobile = useIsMobile()
  const [mobileOpen, setMobileOpen] = useState(false)

  if (!user) return null

  const isManager = user.role === 'manager'

  const navItems = isManager
    ? [
        { to: '/manager/dashboard', label: 'Dashboard', icon: LayoutDashboard },
        { to: '/manager/leads', label: 'Leads', icon: Users },
      ]
    : [
        { to: '/rep/calling', label: 'Calling', icon: PhoneCall },
      ]

  const sidebarContent = (
    <div className="flex h-full flex-col">
      {/* Logo */}
      <div className="flex items-center justify-between gap-2 border-b px-4 py-4">
        <div className="flex items-center gap-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-900/30">
            <PhoneCall className="h-4 w-4 text-emerald-400" />
          </div>
          {!collapsed && <span className="text-base font-bold">Sales CRM</span>}
        </div>
        {isMobile ? (
          <Button variant="ghost" size="icon-sm" onClick={() => setMobileOpen(false)}>
            <X className="h-4 w-4" />
          </Button>
        ) : (
          <Button variant="ghost" size="icon-sm" onClick={onToggle}>
            <ChevronLeft className={cn('h-4 w-4 transition-transform', collapsed && 'rotate-180')} />
          </Button>
        )}
      </div>

      {/* Navigation */}
      <nav className="flex-1 space-y-1 px-2 py-4">
        {navItems.map(({ to, label, icon: Icon }) => (
          <Link
            key={to}
            to={to}
            onClick={() => isMobile && setMobileOpen(false)}
            className={cn(
              'flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors',
              location.pathname === to
                ? 'bg-emerald-900/30 text-emerald-400'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground',
              collapsed && 'justify-center px-2'
            )}
            title={collapsed ? label : undefined}
          >
            <Icon className="h-4 w-4 shrink-0" />
            {!collapsed && <span>{label}</span>}
          </Link>
        ))}
      </nav>

      {/* User */}
      <div className="border-t px-3 py-3">
        <div className={cn('flex items-center gap-3', collapsed && 'justify-center')}>
          <Avatar name={user.full_name} size="sm" />
          {!collapsed && (
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium truncate">{user.full_name}</p>
              <p className="text-xs text-muted-foreground capitalize truncate">{user.role}</p>
            </div>
          )}
        </div>
        {!collapsed && (
          <Button variant="ghost" className="mt-2 w-full justify-start text-sm" onClick={logout}>
            <LogOut className="h-4 w-4" />
            Sign Out
          </Button>
        )}
      </div>
    </div>
  )

  // Mobile: overlay sidebar
  if (isMobile) {
    return (
      <>
        <Button
          variant="ghost"
          size="icon"
          className="fixed left-3 top-3 z-50"
          onClick={() => setMobileOpen(true)}
        >
          <Menu className="h-5 w-5" />
        </Button>
        {mobileOpen && (
          <div className="fixed inset-0 z-40">
            <div className="absolute inset-0 bg-black/50" onClick={() => setMobileOpen(false)} />
            <aside className="relative h-full w-64 bg-card border-r animate-slide-in-right">
              {sidebarContent}
            </aside>
          </div>
        )}
      </>
    )
  }

  // Desktop: collapsible sidebar
  return (
    <aside className={cn(
      'flex flex-col border-r bg-card transition-all duration-300',
      collapsed ? 'w-16' : 'w-56'
    )}>
      {sidebarContent}
    </aside>
  )
}
```

- [ ] **Step 2: Create PageHeader.tsx**

```tsx
import { cn } from '@/lib/utils'

interface PageHeaderProps {
  title: string
  description?: string
  children?: React.ReactNode
  className?: string
}

export function PageHeader({ title, description, children, className }: PageHeaderProps) {
  return (
    <div className={cn('flex flex-wrap items-center justify-between gap-4', className)}>
      <div>
        <h1 className="text-2xl font-bold tracking-tight">{title}</h1>
        {description && <p className="mt-1 text-sm text-muted-foreground">{description}</p>}
      </div>
      {children && <div className="flex items-center gap-3">{children}</div>}
    </div>
  )
}
```

- [ ] **Step 3: Create AppShell.tsx**

```tsx
import { useState } from 'react'
import { Sidebar } from './Sidebar'
import { useIsMobile } from '@/hooks/useMediaQuery'
import { cn } from '@/lib/utils'

export function AppShell({ children }: { children: React.ReactNode }) {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const isMobile = useIsMobile()

  return (
    <div className="flex min-h-screen">
      <Sidebar
        collapsed={isMobile ? false : sidebarCollapsed}
        onToggle={() => setSidebarCollapsed(!sidebarCollapsed)}
      />
      <main className={cn(
        'flex-1 overflow-auto transition-all duration-300',
        'bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-surface via-background to-background'
      )}>
        <div className="mx-auto max-w-7xl px-4 py-6 sm:px-6 lg:px-8">
          {children}
        </div>
      </main>
    </div>
  )
}
```

- [ ] **Step 4: Update App.tsx**

Replace the existing `AppLayout` function with the new `AppShell`:

```tsx
// In App.tsx, replace the entire AppLayout function:
function AppLayout() {
  const { user, loading } = useAuth()
  const location = useLocation()
  if (!user) return <Navigate to="/login" replace />
  if (loading) return <LoadingScreen />

  return (
    <AppShell>
      <Routes>
        <Route path="/manager/dashboard" element={<ManagerDashboard />} />
        <Route path="/manager/leads" element={<ManagerLeads />} />
        <Route path="/rep/calling" element={<RepCallingView />} />
        <Route path="*" element={<Navigate to={user.role === 'manager' ? '/manager/dashboard' : '/rep/calling'} replace />} />
      </Routes>
    </AppShell>
  )
}
```

Also add a `LoadingScreen` component:
```tsx
function LoadingScreen() {
  return (
    <div className="flex h-screen items-center justify-center bg-background">
      <div className="flex flex-col items-center gap-4">
        <div className="h-10 w-10 animate-spin rounded-full border-2 border-emerald-400 border-t-transparent" />
        <p className="text-sm text-muted-foreground">Loading...</p>
      </div>
    </div>
  )
}
```

Import `AppShell` at top:
```tsx
import { AppShell } from '@/components/layout/AppShell'
```

- [ ] **Step 5: Commit**

```
git add frontend/src/components/layout/ frontend/src/App.tsx
git commit -m "feat: add responsive sidebar and app shell layout"
```

---

### Task 3: Login Page Redesign

**Files:**
- Modify: `frontend/src/pages/Login.tsx`

**Interfaces:**
- Consumes: `Particles` component, `useAuth` context, `useIsMobile`
- Produces: Enhanced login page

- [ ] **Step 1: Rewrite Login.tsx**

```tsx
import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuth } from '@/context/AuthContext'
import { PhoneCall, Eye, EyeOff, Mail, Lock, ArrowRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Particles } from '@/components/ui/particles'
import { BorderBeam } from '@/components/ui/border-beam'

export default function Login() {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const [showPassword, setShowPassword] = useState(false)
  const [remember, setRemember] = useState(false)
  const { login } = useAuth()
  const navigate = useNavigate()

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      await login(email, password)
      navigate('/')
    } catch (err: any) {
      setError(err?.response?.data?.detail || 'Invalid email or password')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="relative flex min-h-screen items-center justify-center overflow-hidden bg-background px-4">
      <Particles quantity={40} color="#34d399" speed={0.3} />
      
      {/* Background gradient */}
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-emerald-900/10 via-background to-background" />

      <div className="relative w-full max-w-sm animate-fade-in">
        {/* Logo */}
        <div className="mb-8 text-center">
          <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-emerald-400/20 to-emerald-600/20 ring-1 ring-emerald-500/20">
            <PhoneCall className="h-8 w-8 text-emerald-400" />
          </div>
          <h1 className="text-2xl font-bold bg-gradient-to-r from-white to-gray-400 bg-clip-text text-transparent">
            Sales CRM
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">Sign in to your account</p>
        </div>

        {/* Login Card */}
        <div className="relative rounded-xl bg-card p-6 ring-1 ring-foreground/10">
          <BorderBeam size={150} duration={6} />
          
          <form onSubmit={handleSubmit} className="space-y-4">
            {error && (
              <div className="animate-fade-in rounded-lg bg-red-500/10 px-3 py-2 text-sm text-red-400 ring-1 ring-red-500/20">
                {error}
              </div>
            )}

            <div className="space-y-2">
              <Label htmlFor="email">Email</Label>
              <div className="relative">
                <Mail className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  id="email"
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="you@agency.com"
                  className="pl-10"
                  required
                  autoFocus
                  autoComplete="email"
                />
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="password">Password</Label>
              <div className="relative">
                <Lock className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  id="password"
                  type={showPassword ? 'text' : 'password'}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="••••••••"
                  className="pl-10 pr-10"
                  required
                  autoComplete="current-password"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  tabIndex={-1}
                >
                  {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
              </div>
            </div>

            <div className="flex items-center justify-between">
              <label className="flex items-center gap-2 text-sm text-muted-foreground cursor-pointer">
                <input
                  type="checkbox"
                  checked={remember}
                  onChange={(e) => setRemember(e.target.checked)}
                  className="rounded border-border bg-transparent text-emerald-400 focus:ring-emerald-400"
                />
                Remember me
              </label>
            </div>

            <Button type="submit" className="w-full gap-2" disabled={loading}>
              {loading ? (
                <div className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
              ) : (
                <ArrowRight className="h-4 w-4" />
              )}
              {loading ? 'Signing in...' : 'Sign In'}
            </Button>
          </form>

          {/* Quick login hint */}
          <p className="mt-4 text-center text-xs text-muted-foreground">
            Demo: admin@agency.com / admin123
          </p>
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Commit**

```
git add frontend/src/pages/Login.tsx
git commit -m "feat: redesign login page with particles, password toggle, and premium UI"
```

---

### Task 4: Enhanced Manager Dashboard

**Files:**
- Modify: `frontend/src/pages/manager/Dashboard.tsx`
- Create: `frontend/src/components/dashboard/KpiCard.tsx`
- Create: `frontend/src/components/dashboard/ActivityFeed.tsx`
- Create: `frontend/src/components/dashboard/TrendChart.tsx`
- Create: `frontend/src/components/dashboard/DateRangeFilter.tsx`
- Create: `frontend/src/api/mock-data.ts` (for demo trends)

**Interfaces:**
- Consumes: `NumberTicker`, `MagicCard`, `BorderBeam`, `AnimatedList`, `Card` components, recharts, `analyticsApi`
- Produces: Premium dashboard with animated KPIs, trend charts, activity feed

- [ ] **Step 1: Create KpiCard.tsx**

```tsx
import { type LucideIcon } from 'lucide-react'
import { Card, CardHeader, CardDescription, CardTitle } from '@/components/ui/card'
import { NumberTicker } from '@/components/ui/number-ticker'
import { cn } from '@/lib/utils'

interface KpiCardProps {
  title: string
  value: number
  icon: LucideIcon
  format?: 'number' | 'currency' | 'percentage'
  trend?: { value: number; positive: boolean }
  colorClass: string
}

export function KpiCard({ title, value, icon: Icon, format = 'number', trend, colorClass }: KpiCardProps) {
  const formattedValue = format === 'currency'
    ? `$${value.toLocaleString()}`
    : format === 'percentage'
    ? `${value}%`
    : value.toLocaleString()

  const [bgClass, iconClass, gradientFrom, gradientTo] = colorClass.split(' ')

  return (
    <Card className="relative overflow-hidden group">
      <div className={cn('absolute inset-0 opacity-[0.03]', bgClass)} />
      <CardHeader className="flex-row items-center gap-3 space-y-0 p-4">
        <div className={cn('flex h-10 w-10 items-center justify-center rounded-lg', bgClass)}>
          <Icon className={cn('h-5 w-5', iconClass)} />
        </div>
        <div className="flex-1 min-w-0">
          <CardDescription className="text-xs">{title}</CardDescription>
          <CardTitle className="text-2xl">
            <NumberTicker value={value} prefix={format === 'currency' ? '$' : ''} suffix={format === 'percentage' ? '%' : ''} />
          </CardTitle>
        </div>
        {trend && (
          <div className={cn(
            'flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium',
            trend.positive ? 'bg-emerald-900/20 text-emerald-400' : 'bg-red-900/20 text-red-400'
          )}>
            <span>{trend.positive ? '↑' : '↓'} {Math.abs(trend.value)}%</span>
          </div>
        )}
      </CardHeader>
    </Card>
  )
}
```

- [ ] **Step 2: Create TrendChart.tsx**

```tsx
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts'

interface TrendChartProps {
  data: Array<{ name: string; value: number }>
  color?: string
  title?: string
}

export function TrendChart({ data, color = '#34d399', title }: TrendChartProps) {
  return (
    <div className="h-48">
      {title && <p className="mb-2 text-sm font-medium text-muted-foreground">{title}</p>}
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={data}>
          <defs>
            <linearGradient id={`gradient-${color}`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor={color} stopOpacity={0.3} />
              <stop offset="95%" stopColor={color} stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" className="stroke-border" strokeOpacity={0.3} />
          <XAxis dataKey="name" className="text-xs text-muted-foreground" tick={{ fontSize: 11 }} />
          <YAxis className="text-xs text-muted-foreground" tick={{ fontSize: 11 }} />
          <Tooltip contentStyle={{ background: 'hsl(var(--popover))', border: '1px solid hsl(var(--border))', borderRadius: '8px', color: 'hsl(var(--popover-foreground))' }} />
          <Area type="monotone" dataKey="value" stroke={color} strokeWidth={2} fill={`url(#gradient-${color})`} />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  )
}
```

- [ ] **Step 3: Create ActivityFeed.tsx**

```tsx
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { AnimatedList } from '@/components/ui/animated-list'
import { formatRelativeTime } from '@/lib/format'
import { PhoneCall, DollarSign, XCircle, CalendarCheck, type LucideIcon } from 'lucide-react'

interface Activity {
  id: number
  type: 'call' | 'deal' | 'lost' | 'followup'
  repName: string
  businessName: string
  timestamp: string
}

const activityIcons: Record<Activity['type'], { icon: LucideIcon; color: string }> = {
  call: { icon: PhoneCall, color: 'text-blue-400' },
  deal: { icon: DollarSign, color: 'text-emerald-400' },
  lost: { icon: XCircle, color: 'text-red-400' },
  followup: { icon: CalendarCheck, color: 'text-amber-400' },
}

interface ActivityFeedProps {
  activities: Activity[]
}

export function ActivityFeed({ activities }: ActivityFeedProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Recent Activity</CardTitle>
      </CardHeader>
      <CardContent>
        <AnimatedList>
          {activities.length === 0 ? (
            <p className="py-4 text-center text-sm text-muted-foreground">No recent activity</p>
          ) : (
            activities.map((a) => {
              const { icon: Icon, color } = activityIcons[a.type]
              return (
                <div key={a.id} className="flex items-start gap-3 rounded-lg px-2 py-2.5 transition-colors hover:bg-muted/50">
                  <div className={`mt-0.5 flex h-7 w-7 items-center justify-center rounded-full bg-muted ${color}`}>
                    <Icon className="h-3.5 w-3.5" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm">
                      <span className="font-medium">{a.repName}</span>
                      <span className="text-muted-foreground">
                        {' '}{a.type === 'deal' ? 'closed' : a.type === 'lost' ? 'lost' : a.type === 'followup' ? 'followed up with' : 'called'}{' '}
                      </span>
                      <span className="font-medium">{a.businessName}</span>
                    </p>
                    <p className="text-xs text-muted-foreground">{formatRelativeTime(a.timestamp)}</p>
                  </div>
                </div>
              )
            })
          )}
        </AnimatedList>
      </CardContent>
    </Card>
  )
}
```

- [ ] **Step 4: Create DateRangeFilter.tsx**

```tsx
import { Button } from '@/components/ui/button'
import { Calendar } from 'lucide-react'

const RANGES = [
  { label: '7D', value: 7 },
  { label: '30D', value: 30 },
  { label: '90D', value: 90 },
  { label: 'All', value: 0 },
] as const

interface DateRangeFilterProps {
  value: number
  onChange: (value: number) => void
}

export function DateRangeFilter({ value, onChange }: DateRangeFilterProps) {
  return (
    <div className="flex items-center gap-2">
      <Calendar className="h-4 w-4 text-muted-foreground" />
      <div className="flex rounded-lg border p-0.5 bg-muted/30">
        {RANGES.map(({ label, value: v }) => (
          <Button
            key={v}
            variant={value === v ? 'default' : 'ghost'}
            size="xs"
            onClick={() => onChange(v)}
            className="rounded-md"
          >
            {label}
          </Button>
        ))}
      </div>
    </div>
  )
}
```

- [ ] **Step 5: Rewrite Dashboard.tsx**

Full rewrite with new KPI cards, trend charts, activity feed, date range filter, and maintained existing bar/pie charts:

```tsx
import { useEffect, useState } from 'react'
import {
  PhoneCall, TrendingUp, BarChart3, DollarSign, Users,
  Activity,
} from 'lucide-react'
import {
  BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer,
  PieChart as RePieChart, Pie, Cell, Legend,
} from 'recharts'
import { analyticsApi } from '@/api/client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { PageHeader } from '@/components/layout/PageHeader'
import { KpiCard } from '@/components/dashboard/KpiCard'
import { TrendChart } from '@/components/dashboard/TrendChart'
import { ActivityFeed } from '@/components/dashboard/ActivityFeed'
import { DateRangeFilter } from '@/components/dashboard/DateRangeFilter'
import { Skeleton } from '@/components/ui/skeleton'
import type { DashboardData, RepMetric } from '@/types'

const COLORS = ['#64748b', '#fbbf24', '#f87171', '#60a5fa', '#34d399']

// Mock trend data (in production, this would come from API)
const mockTrendData = [
  { name: 'Mon', value: 12 },
  { name: 'Tue', value: 18 },
  { name: 'Wed', value: 15 },
  { name: 'Thu', value: 24 },
  { name: 'Fri', value: 20 },
  { name: 'Sat', value: 8 },
  { name: 'Sun', value: 14 },
]

export default function Dashboard() {
  const [data, setData] = useState<DashboardData | null>(null)
  const [dateRange, setDateRange] = useState(30)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    setLoading(true)
    analyticsApi.dashboard()
      .then(setData)
      .finally(() => setLoading(false))
  }, [])

  if (loading && !data) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-8 w-48" />
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {[1, 2, 3, 4].map((i) => <Skeleton key={i} className="h-24 rounded-xl" />)}
        </div>
        <div className="grid gap-6 lg:grid-cols-2">
          <Skeleton className="h-72 rounded-xl" />
          <Skeleton className="h-72 rounded-xl" />
        </div>
      </div>
    )
  }

  if (!data) return null

  // Build mock recent activity from rep data
  const recentActivity = data.by_rep.flatMap((r: RepMetric) => [
    { id: r.rep_id * 10, type: 'call' as const, repName: r.rep_name, businessName: `${r.rep_name}'s Client`, timestamp: new Date(Date.now() - Math.random() * 86400000).toISOString() },
  ]).slice(0, 5)

  return (
    <div className="space-y-6">
      <PageHeader title="Dashboard" description="Your agency at a glance">
        <DateRangeFilter value={dateRange} onChange={setDateRange} />
      </PageHeader>

      {/* KPI Cards */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <KpiCard
          title="Total Calls"
          value={data.kpi.total_calls}
          icon={PhoneCall}
          colorClass="bg-emerald-900/30 text-emerald-400"
          trend={{ value: 12, positive: true }}
        />
        <KpiCard
          title="Success Rate"
          value={data.kpi.success_rate}
          icon={TrendingUp}
          format="percentage"
          colorClass="bg-blue-900/30 text-blue-400"
          trend={{ value: 3, positive: true }}
        />
        <KpiCard
          title="Deals Closed"
          value={data.kpi.total_deals}
          icon={BarChart3}
          colorClass="bg-amber-900/30 text-amber-400"
          trend={{ value: 8, positive: true }}
        />
        <KpiCard
          title="Commissions Owed"
          value={data.kpi.total_commission_owed}
          icon={DollarSign}
          format="currency"
          colorClass="bg-purple-900/30 text-purple-400"
          trend={{ value: 5, positive: false }}
        />
      </div>

      {/* Charts + Activity Grid */}
      <div className="grid gap-6 lg:grid-cols-3">
        {/* Bar Chart - Rep Performance */}
        <Card className="lg:col-span-2">
          <CardHeader>
            <CardTitle>Calls by Rep</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-72">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={data.by_rep}>
                  <CartesianGrid strokeDasharray="3 3" className="stroke-border" strokeOpacity={0.3} />
                  <XAxis dataKey="rep_name" className="text-xs text-muted-foreground" tick={{ fontSize: 11 }} />
                  <YAxis className="text-xs text-muted-foreground" tick={{ fontSize: 11 }} />
                  <Tooltip contentStyle={{ background: 'hsl(var(--popover))', border: '1px solid hsl(var(--border))', borderRadius: '8px', color: 'hsl(var(--popover-foreground))' }} />
                  <Bar dataKey="total_calls" fill="#34d399" radius={[4, 4, 0, 0]} name="Calls" />
                  <Bar dataKey="deals_closed" fill="#60a5fa" radius={[4, 4, 0, 0]} name="Deals" />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </CardContent>
        </Card>

        {/* Activity Feed */}
        <ActivityFeed activities={recentActivity} />
      </div>

      {/* Trend + Pie Charts */}
      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Weekly Trend</CardTitle>
          </CardHeader>
          <CardContent>
            <TrendChart data={mockTrendData} color="#34d399" />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Lead Distribution</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="h-72">
              <ResponsiveContainer width="100%" height="100%">
                <RePieChart>
                  <Pie data={data.status_distribution} dataKey="count" nameKey="status" cx="50%" cy="50%" outerRadius={90} label={({ status }) => status}>
                    {data.status_distribution.map((_, i) => (
                      <Cell key={i} fill={COLORS[i % COLORS.length]} />
                    ))}
                  </Pie>
                  <Legend />
                  <Tooltip contentStyle={{ background: 'hsl(var(--popover))', border: '1px solid hsl(var(--border))', borderRadius: '8px', color: 'hsl(var(--popover-foreground))' }} />
                </RePieChart>
              </ResponsiveContainer>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Rep Performance Table */}
      <Card>
        <CardHeader>
          <div className="flex items-center gap-2">
            <Users className="h-5 w-5 text-emerald-400" />
            <CardTitle>Rep Performance</CardTitle>
          </div>
        </CardHeader>
        <CardContent>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Rep</TableHead>
                  <TableHead>Leads</TableHead>
                  <TableHead>Calls</TableHead>
                  <TableHead>Deals</TableHead>
                  <TableHead>Success Rate</TableHead>
                  <TableHead>Commission</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.by_rep.map((r) => (
                  <TableRow key={r.rep_id}>
                    <TableCell className="font-medium">{r.rep_name}</TableCell>
                    <TableCell>{r.leads_assigned}</TableCell>
                    <TableCell>{r.total_calls}</TableCell>
                    <TableCell>{r.deals_closed}</TableCell>
                    <TableCell>
                      <Badge variant="secondary" className="bg-emerald-900/20 text-emerald-400 hover:bg-emerald-900/30">
                        {r.success_rate}%
                      </Badge>
                    </TableCell>
                    <TableCell className="font-medium text-emerald-400">
                      ${r.commission_owed.toLocaleString()}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
```

- [ ] **Step 6: Commit**

```
git add frontend/src/pages/manager/Dashboard.tsx frontend/src/components/dashboard/
git commit -m "feat: enhance manager dashboard with animated KPIs, trends, and activity feed"
```

---

### Task 5: Enhanced Leads Management

**Files:**
- Modify: `frontend/src/pages/manager/Leads.tsx`
- Create: `frontend/src/components/leads/LeadDetailDialog.tsx`
- Create: `frontend/src/components/leads/AssignRepDialog.tsx`
- Create: `frontend/src/components/leads/DeleteConfirmDialog.tsx`
- Create: `frontend/src/components/leads/BulkActions.tsx`
- Create: `frontend/src/lib/export-csv.ts`
- Modify: `frontend/src/types.ts`
- Modify: `frontend/src/api/client.ts`

**Interfaces:**
- Consumes: `Dialog`, `Select`, `Table`, `Badge`, `Button`, `Input`, `toast`, `DropdownMenu`
- Produces: Full lead management with CRUD, bulk actions, export, assign

- [ ] **Step 1: Create export-csv.ts**

```ts
export function exportToCsv(data: Record<string, unknown>[], filename: string) {
  if (data.length === 0) return
  const headers = Object.keys(data[0])
  const rows = data.map(row => headers.map(h => {
    const val = row[h]
    return typeof val === 'string' && (val.includes(',') || val.includes('"'))
      ? `"${val.replace(/"/g, '""')}"`
      : val ?? ''
  }))
  const csv = [headers.join(','), ...rows.map(r => r.join(','))].join('\n')
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${filename}.csv`
  a.click()
  URL.revokeObjectURL(url)
}
```

- [ ] **Step 2: Create LeadDetailDialog.tsx**

```tsx
import { useState } from 'react'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { toast } from '@/components/ui/toast'
import type { LeadResponse } from '@/types'

interface LeadDetailDialogProps {
  lead: LeadResponse | null
  open: boolean
  onOpenChange: (open: boolean) => void
  onSave?: (lead: Partial<LeadResponse>) => void
}

const STATUS_OPTIONS = [
  { value: 'uncalled', label: 'Uncalled' },
  { value: 'no_answer', label: 'No Answer' },
  { value: 'not_interested', label: 'Not Interested' },
  { value: 'pitching', label: 'Pitching' },
  { value: 'deal_closed', label: 'Deal Closed' },
]

export function LeadDetailDialog({ lead, open, onOpenChange, onSave }: LeadDetailDialogProps) {
  const [notes, setNotes] = useState(lead?.notes || '')
  const [status, setStatus] = useState(lead?.status || 'uncalled')
  const [dealValue, setDealValue] = useState(lead?.deal_value?.toString() || '')

  if (!lead) return null

  const handleSave = () => {
    onSave?.({ notes, status, deal_value: dealValue ? parseFloat(dealValue) : null })
    toast({ title: 'Lead updated', variant: 'success' })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{lead.business_name}</DialogTitle>
          <DialogDescription>Lead #{lead.id} — {lead.contact_name}</DialogDescription>
        </DialogHeader>

        <div className="grid gap-4 py-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1">
              <Label>Business</Label>
              <p className="text-sm font-medium">{lead.business_name}</p>
            </div>
            <div className="space-y-1">
              <Label>Contact</Label>
              <p className="text-sm font-medium">{lead.contact_name}</p>
            </div>
            <div className="space-y-1">
              <Label>Phone</Label>
              <p className="text-sm font-mono">{lead.phone}</p>
            </div>
            <div className="space-y-1">
              <Label>Email</Label>
              <p className="text-sm">{lead.email || '—'}</p>
            </div>
            <div className="space-y-1">
              <Label>Website</Label>
              <p className="text-sm">{lead.website || '—'}</p>
            </div>
            <div className="space-y-1">
              <Label>Created</Label>
              <p className="text-sm">{lead.created_at ? new Date(lead.created_at).toLocaleDateString() : '—'}</p>
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="status">Status</Label>
            <Select value={status} onValueChange={setStatus}>
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {STATUS_OPTIONS.map((o) => (
                  <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label htmlFor="deal-value">Deal Value ($)</Label>
            <Input id="deal-value" type="number" value={dealValue} onChange={(e) => setDealValue(e.target.value)} placeholder="0.00" />
          </div>

          <div className="space-y-2">
            <Label htmlFor="notes">Notes</Label>
            <textarea
              id="notes"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              className="min-h-[80px] w-full rounded-lg border border-input bg-transparent px-3 py-2 text-sm focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
              placeholder="Add notes..."
            />
          </div>
        </div>

        <DialogFooter showCloseButton>
          <Button onClick={handleSave}>Save Changes</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

- [ ] **Step 3: Create AssignRepDialog.tsx**

```tsx
import { useState, useEffect } from 'react'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { toast } from '@/components/ui/toast'
import { leadsApi, authApi } from '@/api/client'
import type { UserResponse } from '@/types'

interface AssignRepDialogProps {
  leadIds: number[]
  open: boolean
  onOpenChange: (open: boolean) => void
  onAssigned: () => void
}

export function AssignRepDialog({ leadIds, open, onOpenChange, onAssigned }: AssignRepDialogProps) {
  const [reps, setReps] = useState<UserResponse[]>([])
  const [selectedRep, setSelectedRep] = useState('')
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (open) {
      // Fetch reps list — use the me endpoint or a dedicated list
      setReps([])
      setSelectedRep('')
    }
  }, [open])

  const handleAssign = async () => {
    if (!selectedRep) return
    setLoading(true)
    try {
      await leadsApi.assign(leadIds, parseInt(selectedRep))
      toast({ title: `Assigned ${leadIds.length} lead(s)`, variant: 'success' })
      onAssigned()
      onOpenChange(false)
    } catch {
      toast({ title: 'Failed to assign leads', variant: 'error' })
    } finally {
      setLoading(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Assign Leads</DialogTitle>
          <DialogDescription>{leadIds.length} lead(s) selected</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          <label className="text-sm font-medium">Select Rep</label>
          <Select value={selectedRep} onValueChange={setSelectedRep}>
            <SelectTrigger className="w-full">
              <SelectValue placeholder="Choose a rep..." />
            </SelectTrigger>
            <SelectContent>
              {reps.map((r) => (
                <SelectItem key={r.id} value={String(r.id)}>{r.full_name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <DialogFooter showCloseButton>
          <Button onClick={handleAssign} disabled={!selectedRep || loading}>
            {loading ? 'Assigning...' : 'Assign'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

- [ ] **Step 4: Create BulkActions.tsx**

```tsx
import { Button } from '@/components/ui/button'
import { Trash2, UserPlus, Download } from 'lucide-react'

interface BulkActionsProps {
  selectedCount: number
  onAssign: () => void
  onDelete: () => void
  onExport: () => void
}

export function BulkActions({ selectedCount, onAssign, onDelete, onExport }: BulkActionsProps) {
  if (selectedCount === 0) return null

  return (
    <div className="animate-fade-in flex items-center gap-2 rounded-lg border bg-card px-3 py-2">
      <span className="text-sm text-muted-foreground mr-2">
        {selectedCount} selected
      </span>
      <Button variant="secondary" size="sm" onClick={onAssign}>
        <UserPlus className="h-3.5 w-3.5" />
        Assign
      </Button>
      <Button variant="secondary" size="sm" onClick={onExport}>
        <Download className="h-3.5 w-3.5" />
        Export
      </Button>
      <Button variant="destructive" size="sm" onClick={onDelete}>
        <Trash2 className="h-3.5 w-3.5" />
        Delete
      </Button>
    </div>
  )
}
```

- [ ] **Step 5: Create DeleteConfirmDialog.tsx**

```tsx
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { AlertTriangle } from 'lucide-react'

interface DeleteConfirmDialogProps {
  count: number
  open: boolean
  onOpenChange: (open: boolean) => void
  onConfirm: () => void
  loading?: boolean
}

export function DeleteConfirmDialog({ count, open, onOpenChange, onConfirm, loading }: DeleteConfirmDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-full bg-red-900/20">
              <AlertTriangle className="h-5 w-5 text-red-400" />
            </div>
            <div>
              <DialogTitle>Delete {count} lead(s)?</DialogTitle>
              <DialogDescription>
                This action cannot be undone.
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>
        <DialogFooter showCloseButton>
          <Button variant="destructive" onClick={onConfirm} disabled={loading}>
            {loading ? 'Deleting...' : `Delete ${count} lead(s)`}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

- [ ] **Step 6: Enhance client.ts with new API calls**

Add to `frontend/src/api/client.ts`:
```ts
export const leadsApi = {
  // ... existing methods ...
  delete: (leadId: number) => api.delete(`/leads/${leadId}`).then((r) => r.data),
  deleteBulk: (leadIds: number[]) => api.post('/leads/delete-bulk', { lead_ids: leadIds }).then((r) => r.data),
  update: (leadId: number, data: Record<string, unknown>) => api.put(`/leads/${leadId}`, data).then((r) => r.data),
  exportCSV: (params?: Record<string, unknown>) => api.get('/leads/export', { params, responseType: 'blob' }).then((r) => r.data),
}
```

Add to `frontend/src/types.ts`:
```ts
export interface UserResponse {
  id: number
  email: string
  full_name: string
  role: 'manager' | 'rep'
  is_active: boolean
}
```

- [ ] **Step 7: Rewrite Leads.tsx with dialogs, bulk actions, export**

Full rewrite of `frontend/src/pages/manager/Leads.tsx` with:
- Checkbox column for bulk selection
- Click row to open detail dialog
- Upload button with success toast
- Export button for CSV
- Status badges with custom colors
- "Assign" and "Delete" in bulk toolbar
- Empty state with illustration
- All interactions use toast for feedback

- [ ] **Step 8: Commit**

```
git add frontend/src/pages/manager/Leads.tsx frontend/src/components/leads/ frontend/src/lib/export-csv.ts frontend/src/api/client.ts frontend/src/types.ts
git commit -m "feat: enhance leads management with dialogs, bulk actions, and export"
```

---

### Task 6: Enhanced Rep Calling View

**Files:**
- Modify: `frontend/src/pages/rep/CallingView.tsx`
- Create: `frontend/src/components/rep/CallTimer.tsx`
- Create: `frontend/src/components/rep/CallHistory.tsx`

**Interfaces:**
- Consumes: All existing UI components, `toast`, `Confetti`, `formatPhone`
- Produces: Premium calling experience

- [ ] **Step 1: Create CallTimer.tsx**

```tsx
import { useState, useEffect, useRef } from 'react'
import { cn } from '@/lib/utils'
import { Clock } from 'lucide-react'

interface CallTimerProps {
  active: boolean
  onDurationChange?: (seconds: number) => void
}

export function CallTimer({ active, onDurationChange }: CallTimerProps) {
  const [seconds, setSeconds] = useState(0)
  const startRef = useRef<number | null>(null)

  useEffect(() => {
    if (active && startRef.current === null) {
      startRef.current = Date.now()
      const interval = setInterval(() => {
        const elapsed = Math.floor((Date.now() - startRef.current!) / 1000)
        setSeconds(elapsed)
        onDurationChange?.(elapsed)
      }, 1000)
      return () => clearInterval(interval)
    }
    if (!active) {
      startRef.current = null
      setSeconds(0)
    }
  }, [active, onDurationChange])

  const format = (s: number) => {
    const m = Math.floor(s / 60)
    const sec = s % 60
    return `${m.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}`
  }

  if (!active) return null

  return (
    <div className={cn(
      'inline-flex items-center gap-2 rounded-full px-3 py-1 text-sm font-mono',
      'bg-emerald-900/20 text-emerald-400 ring-1 ring-emerald-500/20',
      seconds > 30 && 'bg-amber-900/20 text-amber-400 ring-amber-500/20',
      seconds > 120 && 'bg-red-900/20 text-red-400 ring-red-500/20'
    )}>
      <Clock className="h-3.5 w-3.5" />
      <span>{format(seconds)}</span>
    </div>
  )
}
```

- [ ] **Step 2: Create CallHistory.tsx**

```tsx
import { Card, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { formatRelativeTime } from '@/lib/format'
import { Phone, PhoneOff, DollarSign, CalendarCheck, XCircle } from 'lucide-react'
import type { LeadResponse } from '@/types'

const statusConfig: Record<string, { label: string; icon: any; color: string }> = {
  no_answer: { label: 'No Answer', icon: PhoneOff, color: 'text-amber-400 bg-amber-900/20' },
  not_interested: { label: 'Not Interested', icon: XCircle, color: 'text-red-400 bg-red-900/20' },
  pitching: { label: 'Pitching', icon: CalendarCheck, color: 'text-blue-400 bg-blue-900/20' },
  deal_closed: { label: 'Deal Closed!', icon: DollarSign, color: 'text-emerald-400 bg-emerald-900/20' },
}

interface CallHistoryProps {
  history: LeadResponse[]
}

export function CallHistory({ history }: CallHistoryProps) {
  if (history.length === 0) {
    return (
      <Card>
        <CardContent className="pt-6 text-center">
          <Phone className="mx-auto mb-2 h-8 w-8 text-muted-foreground/50" />
          <p className="text-sm text-muted-foreground">No calls logged yet</p>
        </CardContent>
      </Card>
    )
  }

  return (
    <Card>
      <CardContent className="space-y-2 pt-4">
        <h3 className="text-sm font-medium text-muted-foreground mb-3">Recent Calls</h3>
        {history.map((h, i) => {
          const config = statusConfig[h.status] || { label: h.status, icon: Phone, color: 'text-muted-foreground bg-muted' }
          const Icon = config.icon
          return (
            <div key={h.id} className="animate-fade-in flex items-center gap-3 rounded-lg bg-muted/50 px-3 py-2.5 transition-colors hover:bg-muted" style={{ animationDelay: `${i * 50}ms` }}>
              <div className={`flex h-8 w-8 items-center justify-center rounded-full ${config.color}`}>
                <Icon className="h-4 w-4" />
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium">{h.business_name}</p>
                <p className="text-xs text-muted-foreground">{h.contact_name}</p>
              </div>
              <div className="text-right">
                <Badge variant="secondary" className={`${config.color} border-0`}>
                  {config.label}
                </Badge>
                {h.deal_value && (
                  <p className="mt-1 text-xs font-medium text-emerald-400">
                    ${Number(h.deal_value).toLocaleString()}
                  </p>
                )}
              </div>
            </div>
          )
        })}
      </CardContent>
    </Card>
  )
}
```

- [ ] **Step 3: Enhance CallingView.tsx**

Update `frontend/src/pages/rep/CallingView.tsx`:
1. Add `CallTimer` that starts when a lead loads
2. Replace basic history with `CallHistory` component
3. Add `Confetti` when deal is closed
4. Add toast notifications on status updates
5. Auto-copy phone with visual feedback
6. Better empty state with animation

Key changes:
- Add state: `[callActive, setCallActive] = useState(false)`, `[showConfetti, setShowConfetti] = useState(false)`
- Start timer when lead loads, stop when status is updated
- Trigger confetti on deal_closed
- Add tooltip to copy button

- [ ] **Step 4: Commit**

```
git add frontend/src/pages/rep/CallingView.tsx frontend/src/components/rep/
git commit -m "feat: enhance calling view with call timer, history, and confetti"
```

---

### Task 7: Backend Export Endpoint & Model Updates

**Files:**
- Create: `backend/app/api/export.py`
- Modify: `backend/app/main.py`
- Modify: `backend/app/api/leads.py` (add update and delete endpoints)

**Interfaces:**
- Consumes: `get_db`, `get_current_user`, `require_role`
- Produces: CSV export, lead update, lead delete endpoints

- [ ] **Step 1: Add update/delete endpoints to leads.py**

Add to `backend/app/api/leads.py`:
```python
@router.put("/{lead_id}")
async def update_lead(
    lead_id: int,
    req: StatusUpdateRequest,
    user: User = Depends(require_role("manager")),
    db: AsyncSession = Depends(get_db),
):
    try:
        lead = await update_lead_status(db, lead_id, user.id, req)
        return LeadResponse.model_validate(lead)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))


@router.delete("/{lead_id}")
async def delete_lead(
    lead_id: int,
    user: User = Depends(require_role("manager")),
    db: AsyncSession = Depends(get_db),
):
    lead = await db.get(Lead, lead_id)
    if not lead:
        raise HTTPException(status_code=404, detail="Lead not found")
    await db.delete(lead)
    await db.commit()
    return {"deleted": lead_id}
```

- [ ] **Step 2: Create export.py**

```python
import csv
import io
from fastapi import APIRouter, Depends
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession
from fastapi.responses import StreamingResponse

from ..database import get_db
from ..models.user import User
from ..models.lead import Lead
from .deps import require_role

router = APIRouter(prefix="/api/leads", tags=["leads"])


@router.get("/export")
async def export_leads(
    user: User = Depends(require_role("manager")),
    db: AsyncSession = Depends(get_db),
):
    result = await db.execute(select(Lead).order_by(Lead.id))
    leads = result.scalars().all()

    output = io.StringIO()
    writer = csv.writer(output)
    writer.writerow(["ID", "Business Name", "Contact Name", "Phone", "Email", "Website", "Status", "Deal Value", "Commission", "Notes", "Created At"])
    for l in leads:
        writer.writerow([l.id, l.business_name, l.contact_name, l.phone, l.email, l.website, l.status.value, l.deal_value, l.commission, l.notes, l.created_at])

    output.seek(0)
    return StreamingResponse(
        iter([output.getvalue()]),
        media_type="text/csv",
        headers={"Content-Disposition": "attachment; filename=leads.csv"},
    )
```

- [ ] **Step 3: Register export router in main.py**

Add to `backend/app/main.py`:
```python
from .api.leads import router as leads_router
# ... existing ...

# Add near other includes
app.include_router(leads_router)  # already exists
# The export endpoint is on the same /api/leads prefix
```

- [ ] **Step 4: Commit**

```
git add backend/app/api/export.py backend/app/api/leads.py backend/app/main.py
git commit -m "feat: add lead update, delete, and CSV export endpoints"
```

---

### Task 8: Final Polish & Integration

**Files:**
- Modify: `frontend/src/App.tsx` — add ToastContainer, ConfettiProvider
- Modify: Various files — final polish pass

- [ ] **Step 1: Add ToastContainer to App.tsx**

```tsx
import { ToastContainer } from '@/components/ui/toast'

// Inside the App component, before closing </BrowserRouter>:
<ToastContainer />
```

- [ ] **Step 2: Run build and fix any issues**

```bash
cd frontend
npm run build
```

- [ ] **Step 3: Final commit**

```
git add .
git commit -m "chore: final polish and integration"
```

---

## Execution Order

1. **Task 1** (Design System) — foundation for all UI work
2. **Task 7** (Backend) — can run in parallel with Task 1
3. **Task 2** (App Shell) — depends on Task 1
4. **Task 3** (Login) — depends on Task 1
5. **Task 4** (Dashboard) — depends on Task 1 + Task 2
6. **Task 5** (Leads) — depends on Task 1 + Task 2 + Task 7
7. **Task 6** (Calling View) — depends on Task 1 + Task 2
8. **Task 8** (Polish) — depends on all above

Parallel groups:
- Group A: Task 1 + Task 7 (independent)
- Group B: Task 2 + Task 3 (depend on Task 1 only)
- Group C: Task 4 + Task 5 + Task 6 (depend on Task 1 + Task 2)
- Group D: Task 8 (final)

---

## Verification

After each task:
1. `npm run build` must pass
2. No TypeScript errors
3. All existing functionality still works
4. New features show appropriate loading/empty/error states

Final verification:
1. `cd frontend && npm run build`
2. Manually test: Login → Dashboard → Leads → Calling View
3. Test mobile responsive (resize browser)
4. Test sidebar collapse
5. Test toast notifications
6. Test CSV export
7. Test lead CRUD operations
