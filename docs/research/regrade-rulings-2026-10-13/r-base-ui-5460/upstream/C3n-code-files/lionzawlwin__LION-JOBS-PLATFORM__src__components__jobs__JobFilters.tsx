'use client';

import { X, SlidersHorizontal, ChevronDown } from 'lucide-react';
import { useState } from 'react';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
} from '@/components/ui/select';
import { cn } from '@/lib/utils';
import { useLanguage } from '@/contexts/LanguageContext';
import type { JobCategory, JobFilters, JobType } from '@/types';

const CATEGORIES: JobCategory[] = [
  'Engineering', 'Design', 'Marketing', 'Sales',
  'Finance', 'Operations', 'Customer Service', 'Healthcare', 'Education',
  'Logistics & Distribution', 'Other',
];

const TYPES: JobType[] = ['Full-time', 'Part-time', 'Contract', 'Remote', 'Hybrid'];

const LOCATIONS = ['Yangon, Myanmar', 'Mandalay, Myanmar', 'Remote'];

// @base-ui/react Select can't resolve item labels before SelectContent mounts,
// so "value" renders as raw text (the __all__ bug). Fix: render the trigger
// label explicitly ourselves and use standard "all" / "any" sentinel values.

interface Props {
  filters: JobFilters;
  onChange: (patch: Partial<JobFilters>) => void;
  total: number;
}

export function JobFilters({ filters, onChange, total }: Props) {
  const { t } = useLanguage();
  const [showSalary, setShowSalary] = useState(false);
  const hasSalary = (filters.salaryMin ?? 0) > 0 || (filters.salaryMax ?? 0) > 0;
  const hasActiveFilters = Boolean(filters.category || filters.type || filters.location || hasSalary);

  function clearAll() {
    onChange({ category: '', type: '', location: '', salaryMin: 0, salaryMax: 0 });
    setShowSalary(false);
  }

  return (
    <div className="flex flex-col gap-3">
      {/* ── Top row: filter icon + selects + clear ── */}
      <div className="flex flex-wrap items-center gap-2">
        <div className="flex items-center gap-1.5 text-xs font-semibold text-muted-foreground uppercase tracking-wide mr-1">
          <SlidersHorizontal size={13} />
          {t('jf_filter_label')}
        </div>

        {/* Category */}
        <Select
          value={filters.category || 'all'}
          onValueChange={(v) => onChange({ category: v === 'all' ? '' : (v as JobCategory) })}
        >
          <SelectTrigger className={cn(
            'h-9 w-auto min-w-[130px] rounded-xl text-sm border-border/70 transition-colors',
            filters.category
              ? 'border-brand-600/50 bg-brand-50 text-brand-700 dark:bg-brand-600/10 dark:text-brand-300'
              : 'bg-background',
          )}>
            {/* Render text explicitly — avoids @base-ui label-resolution race */}
            <span className={cn('text-sm', !filters.category && 'text-muted-foreground')}>
              {filters.category || t('jf_all_categories')}
            </span>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('jf_all_categories')}</SelectItem>
            {CATEGORIES.map((c) => <SelectItem key={c} value={c}>{c}</SelectItem>)}
          </SelectContent>
        </Select>

        {/* Type */}
        <Select
          value={filters.type || 'all'}
          onValueChange={(v) => onChange({ type: v === 'all' ? '' : (v as JobType) })}
        >
          <SelectTrigger className={cn(
            'h-9 w-auto min-w-[110px] rounded-xl text-sm border-border/70 transition-colors',
            filters.type
              ? 'border-brand-600/50 bg-brand-50 text-brand-700 dark:bg-brand-600/10 dark:text-brand-300'
              : 'bg-background',
          )}>
            <span className={cn('text-sm', !filters.type && 'text-muted-foreground')}>
              {filters.type || t('jf_all_types')}
            </span>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('jf_all_types')}</SelectItem>
            {TYPES.map((ty) => <SelectItem key={ty} value={ty}>{ty}</SelectItem>)}
          </SelectContent>
        </Select>

        {/* Location */}
        <Select
          value={filters.location || 'any'}
          onValueChange={(v) => onChange({ location: v === 'any' ? '' : (v ?? '') })}
        >
          <SelectTrigger className={cn(
            'h-9 w-auto min-w-[130px] rounded-xl text-sm border-border/70 transition-colors',
            filters.location
              ? 'border-brand-600/50 bg-brand-50 text-brand-700 dark:bg-brand-600/10 dark:text-brand-300'
              : 'bg-background',
          )}>
            <span className={cn('text-sm', !filters.location && 'text-muted-foreground')}>
              {filters.location || t('jf_any_location')}
            </span>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="any">{t('jf_any_location')}</SelectItem>
            {LOCATIONS.map((l) => <SelectItem key={l} value={l}>{l}</SelectItem>)}
          </SelectContent>
        </Select>

        {/* Salary toggle button */}
        <button
          type="button"
          onClick={() => setShowSalary((v) => !v)}
          className={cn(
            'flex items-center gap-1.5 rounded-xl border px-3 py-2 text-sm transition-colors',
            hasSalary
              ? 'border-brand-600/50 bg-brand-50 text-brand-700 dark:bg-brand-600/10 dark:text-brand-300'
              : 'border-border/70 bg-background text-muted-foreground hover:text-foreground',
          )}
        >
          {t('jf_salary_label')}
          {hasSalary && <span className="text-[10px] font-bold">✓</span>}
          <ChevronDown size={12} className={cn('transition-transform', showSalary && 'rotate-180')} />
        </button>

        {/* Clear button */}
        {hasActiveFilters && (
          <button
            onClick={clearAll}
            className="flex items-center gap-1 rounded-full border border-border/60 bg-muted/50 px-3 py-1.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
          >
            <X size={11} /> {t('jf_clear_all')}
          </button>
        )}

        {/* Active filter pills */}
        <div className="ml-auto flex items-center gap-1.5">
          {[
            filters.category && { label: filters.category, clear: () => onChange({ category: '' }) },
            filters.type     && { label: filters.type,     clear: () => onChange({ type: '' }) },
            filters.location && { label: filters.location, clear: () => onChange({ location: '' }) },
            hasSalary && {
              label: `${filters.salaryMin ? filters.salaryMin + 'K' : ''}${filters.salaryMin && filters.salaryMax ? '–' : ''}${filters.salaryMax ? filters.salaryMax + 'K' : ''} MMK`,
              clear: () => onChange({ salaryMin: 0, salaryMax: 0 }),
            },
          ].filter(Boolean).map((pill) => {
            if (!pill) return null;
            return (
              <span
                key={pill.label}
                className="flex items-center gap-1 rounded-full bg-brand-600 px-2.5 py-1 text-xs font-medium text-white"
              >
                {pill.label}
                <button onClick={pill.clear} className="ml-0.5 hover:opacity-70 transition-opacity">
                  <X size={9} />
                </button>
              </span>
            );
          })}
        </div>
      </div>

      {/* ── Salary Range Row ── */}
      {showSalary && (
        <div className="flex flex-wrap items-center gap-3 rounded-xl border border-border/60 bg-muted/30 px-4 py-3">
          <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">{t('jf_salary_range_label')}</span>
          <div className="flex items-center gap-2">
            <input
              type="number"
              min={0}
              placeholder={t('jf_min')}
              value={filters.salaryMin || ''}
              onChange={(e) => onChange({ salaryMin: Number(e.target.value) || 0 })}
              className="h-9 w-24 rounded-lg border border-border/70 bg-background px-3 text-sm focus:outline-none focus:ring-2 focus:ring-brand-600"
            />
            <span className="text-xs text-muted-foreground">{t('jf_to')}</span>
            <input
              type="number"
              min={0}
              placeholder={t('jf_max')}
              value={filters.salaryMax || ''}
              onChange={(e) => onChange({ salaryMax: Number(e.target.value) || 0 })}
              className="h-9 w-24 rounded-lg border border-border/70 bg-background px-3 text-sm focus:outline-none focus:ring-2 focus:ring-brand-600"
            />
          </div>
          <div className="flex gap-2 ml-auto">
            {[
              { label: t('jf_under_500k'), min: 0, max: 500 },
              { label: t('jf_500k_1m'), min: 500, max: 1000 },
              { label: t('jf_1m_plus'), min: 1000, max: 0 },
            ].map((preset) => (
              <button
                key={preset.label}
                type="button"
                onClick={() => onChange({ salaryMin: preset.min, salaryMax: preset.max })}
                className={cn(
                  'rounded-full border px-2.5 py-1 text-xs font-medium transition-colors',
                  filters.salaryMin === preset.min && filters.salaryMax === preset.max
                    ? 'border-brand-600 bg-brand-600 text-white'
                    : 'border-border/60 text-muted-foreground hover:border-brand-400 hover:text-brand-600',
                )}
              >
                {preset.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* ── Results count ── */}
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>
          {t('jf_showing')} <span className="font-bold text-foreground">{total}</span>{' '}
          {total === 1 ? t('jf_role_singular') : t('jf_role_plural')}
          {hasActiveFilters ? ` ${t('jf_matching_filters')}` : ''}
        </span>
      </div>
    </div>
  );
}
