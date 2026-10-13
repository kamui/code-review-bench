import { Button } from '@base-ui/react/button'
import { Checkbox } from '@base-ui/react/checkbox'
import { Radio } from '@base-ui/react/radio'
import { RadioGroup } from '@base-ui/react/radio-group'
import { Select } from '@base-ui/react/select'
import { Check, ChevronDown, Search, X } from 'lucide-react'
import { useEffect, useState } from 'react'

import {
  categoryLabels,
  productCategories,
  productPriceRange,
  productSizes,
} from '../../data/products'
import type { ProductCategoryCounts, ProductSort } from '../../data/product-search'
import { joinClasses, paiseToRupees, rupeesToPaise } from '../../lib/format'
import type { ProductCategoryFilter, ProductSearchState } from '../../lib/product-search-url'

type ProductFiltersProps = {
  search: Required<ProductSearchState>
  resultCount: number
  categoryCounts: ProductCategoryCounts
  onSearchChange: (search: Partial<ProductSearchState>) => void
  idPrefix?: string
  onDone?: () => void
  showHeader?: boolean
  variant?: 'panel' | 'sheet'
}

const categories: ProductCategoryFilter[] = ['all', ...productCategories]

const sortLabels: Record<ProductSort, string> = {
  recommended: 'Recommended',
  newest: 'Newest arrivals',
  'price-asc': 'Price: low to high',
  'price-desc': 'Price: high to low',
  'discount-desc': 'Best discount',
}

export function ProductFilters({
  search,
  resultCount,
  categoryCounts,
  onSearchChange,
  idPrefix = 'products',
  onDone,
  showHeader = false,
  variant = 'panel',
}: ProductFiltersProps) {
  const [query, setQuery] = useState(search.q)
  const searchInputId = `${idPrefix}-product-search`

  useEffect(() => {
    setQuery(search.q)
  }, [search.q])

  useEffect(() => {
    const nextQuery = query.trim()
    const currentQuery = search.q.trim()

    if (nextQuery === currentQuery) return

    const timeoutId = window.setTimeout(() => {
      onSearchChange({ q: nextQuery })
    }, 180)

    return () => window.clearTimeout(timeoutId)
  }, [onSearchChange, query, search.q])

  const activeFilterCount =
    (search.category === 'all' ? 0 : 1) +
    search.sizes.length +
    (search.minPrice > productPriceRange.min ? 1 : 0) +
    (search.maxPrice < productPriceRange.max ? 1 : 0) +
    Number(search.inStockOnly) +
    Number(search.saleOnly)

  return (
    <aside
      aria-label="Product filters"
      className={joinClasses(
        'w-full min-w-0 self-start overflow-x-hidden',
        variant === 'panel'
          ? 'space-y-7 bg-[var(--color-paper)] lg:sticky lg:top-[calc(var(--site-header-height)+var(--sticky-panel-gap))] lg:max-h-[calc(100svh-var(--site-header-height)-(var(--sticky-panel-gap)*2))] lg:overflow-y-auto lg:overscroll-contain'
          : 'space-y-6',
      )}
    >
      {showHeader ? (
        <div className="flex items-start justify-between gap-3 border-b border-[var(--color-line)] pb-5">
          <div>
            <p className="font-serif text-3xl font-normal leading-none text-[var(--color-ink)]">Filters</p>
            <p className="mt-1 text-xs leading-5 text-[var(--color-muted)]">
              Narrow by category, size, price, and availability.
            </p>
          </div>
          {activeFilterCount > 0 ? (
            <button
              type="button"
              onClick={() =>
                onSearchChange({
                  category: 'all',
                  sizes: [],
                  minPrice: productPriceRange.min,
                  maxPrice: productPriceRange.max,
                  inStockOnly: false,
                  saleOnly: false,
                })
              }
              className="shrink-0 text-sm font-medium text-[var(--color-primary)] transition duration-150 ease-out hover:text-[var(--color-primary-dark)] focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2 active:scale-[0.99]"
            >
              Reset
            </button>
          ) : null}
        </div>
      ) : null}

      <div className="space-y-2">
        <label htmlFor={searchInputId} className="text-sm font-medium text-[var(--color-ink)]">
          Search
        </label>
        <div className="relative">
          <Search
            className="pointer-events-none absolute left-4 top-1/2 size-4 -translate-y-1/2 text-[var(--color-muted)]"
            aria-hidden="true"
          />
          <input
            id={searchInputId}
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search pieces"
            className="h-11 w-full border border-[var(--color-line)] bg-[var(--color-paper)] pl-10 pr-11 text-sm text-[var(--color-ink)] outline-none transition placeholder:text-[var(--color-muted)]/70 focus:border-[var(--color-primary)] focus:bg-[var(--color-surface-soft)]"
          />
          {query ? (
            <Button
              type="button"
              aria-label="Clear search"
              onClick={() => setQuery('')}
              className="absolute right-2 top-1/2 flex size-7 -translate-y-1/2 items-center justify-center rounded-full text-[var(--color-muted)] transition duration-150 ease-out hover:bg-[var(--color-line)] hover:text-[var(--color-ink)] focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] active:scale-95"
            >
              <X className="size-4" aria-hidden="true" />
            </Button>
          ) : null}
        </div>
      </div>

      <div className="space-y-3">
        <RadioGroup
          value={search.category}
          onValueChange={(category) =>
            onSearchChange({ category: category as ProductCategoryFilter })
          }
          className="space-y-3"
        >
          <p className="text-sm font-medium text-[var(--color-ink)]">Category</p>
          {categories.map((category) => (
            <label
              key={category}
              className="flex cursor-pointer items-center gap-3 text-sm text-[var(--color-muted)] transition hover:text-[var(--color-ink)]"
            >
              <Radio.Root
                value={category}
                className="flex size-4 items-center justify-center rounded-full border border-[var(--color-line)] bg-[var(--color-paper)] text-transparent outline-none transition duration-150 ease-out data-[checked]:border-[var(--color-primary)] data-[checked]:bg-[var(--color-primary)] data-[checked]:text-[var(--color-paper)] focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2"
              >
                <Radio.Indicator className="size-1.5 rounded-full bg-current" />
              </Radio.Root>
              <span className="flex min-w-0 flex-1 items-center justify-between gap-3">
                <span>{category === 'all' ? 'All products' : categoryLabels[category]}</span>
                <span className="text-xs text-[var(--color-muted)]/70">
                  {category === 'all'
                    ? Object.values(categoryCounts).reduce((total, count) => total + count, 0)
                    : categoryCounts[category] ?? 0}
                </span>
              </span>
            </label>
          ))}
        </RadioGroup>
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <p className="text-sm font-medium text-[var(--color-ink)]">Size</p>
          {search.sizes.length > 0 ? (
            <button
              type="button"
              onClick={() => onSearchChange({ sizes: [] })}
              className="text-xs font-medium text-[var(--color-muted)] transition duration-150 ease-out hover:text-[var(--color-primary)] focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2 active:scale-[0.99]"
            >
              Clear
            </button>
          ) : null}
        </div>
        <div className="grid grid-cols-3 gap-2">
          {productSizes.map((size) => {
            const isSelected = search.sizes.includes(size)

            return (
              <button
                key={size}
                type="button"
                aria-pressed={isSelected}
                onClick={() =>
                  onSearchChange({
                    sizes: toggleValue(search.sizes, size),
                  })
                }
                className={joinClasses(
                  'h-9 border text-sm font-medium transition duration-150 ease-out focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2',
                  isSelected
                    ? 'border-[var(--color-primary)] bg-[var(--color-primary)] text-[var(--color-paper)]'
                    : 'border-[var(--color-line)] bg-[var(--color-paper)] text-[var(--color-ink)] hover:border-[var(--color-primary)]',
                )}
              >
                {size}
              </button>
            )
          })}
        </div>
      </div>

      <div className="space-y-3">
        <p className="text-sm font-medium text-[var(--color-ink)]">Price</p>
        <div className="grid min-w-0 grid-cols-2 gap-2">
          <label className="min-w-0 space-y-1">
            <span className="text-xs text-[var(--color-muted)]">Min</span>
            <input
              type="number"
              min={paiseToRupees(productPriceRange.min)}
              max={paiseToRupees(productPriceRange.max)}
              value={paiseToRupees(search.minPrice)}
              onChange={(event) =>
                onSearchChange({ minPrice: rupeesToPaise(Number(event.currentTarget.value)) })
              }
              className="h-10 w-full min-w-0 border border-[var(--color-line)] bg-[var(--color-paper)] px-3 text-sm text-[var(--color-ink)] outline-none transition duration-150 ease-out focus:border-[var(--color-primary)] focus:bg-[var(--color-surface-soft)]"
            />
          </label>
          <label className="min-w-0 space-y-1">
            <span className="text-xs text-[var(--color-muted)]">Max</span>
            <input
              type="number"
              min={paiseToRupees(productPriceRange.min)}
              max={paiseToRupees(productPriceRange.max)}
              value={paiseToRupees(search.maxPrice)}
              onChange={(event) =>
                onSearchChange({ maxPrice: rupeesToPaise(Number(event.currentTarget.value)) })
              }
              className="h-10 w-full min-w-0 border border-[var(--color-line)] bg-[var(--color-paper)] px-3 text-sm text-[var(--color-ink)] outline-none transition duration-150 ease-out focus:border-[var(--color-primary)] focus:bg-[var(--color-surface-soft)]"
            />
          </label>
        </div>
      </div>

      <div className="space-y-3">
        <p className="text-sm font-medium text-[var(--color-ink)]">Availability</p>
        <div className="space-y-2">
          <FilterCheckbox
            checked={search.inStockOnly}
            label="In stock only"
            onChange={(checked) => onSearchChange({ inStockOnly: checked })}
          />
          <FilterCheckbox
            checked={search.saleOnly}
            label="On sale"
            onChange={(checked) => onSearchChange({ saleOnly: checked })}
          />
        </div>
      </div>

      <div className="space-y-3">
        <Select.Root
          value={search.sort}
          onValueChange={(sort) => onSearchChange({ sort: sort as ProductSort })}
        >
          <Select.Label className="text-sm font-medium text-[var(--color-ink)]">Sort</Select.Label>
          <Select.Trigger className="mt-3 flex h-10 w-full items-center justify-between border border-[var(--color-line)] bg-[var(--color-paper)] px-4 text-left text-sm text-[var(--color-ink)] outline-none transition duration-150 ease-out hover:border-[var(--color-primary)] hover:bg-[var(--color-surface-soft)] focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2 data-[popup-open]:border-[var(--color-primary)]">
            <Select.Value>
              {(value: ProductSort | null) => (value ? sortLabels[value] : sortLabels.recommended)}
            </Select.Value>
            <Select.Icon className="text-[var(--color-muted)]">
              <ChevronDown className="size-4" aria-hidden="true" />
            </Select.Icon>
          </Select.Trigger>
          <Select.Portal>
            <Select.Positioner sideOffset={6}>
              <Select.Popup className="min-w-[var(--anchor-width)] border border-[var(--color-line)] bg-[var(--color-paper)] p-1 outline-none transition duration-150 ease-out data-[ending-style]:scale-95 data-[ending-style]:opacity-0 data-[starting-style]:scale-95 data-[starting-style]:opacity-0">
                {Object.entries(sortLabels).map(([value, label]) => (
                  <Select.Item
                    key={value}
                    value={value}
                    className={joinClasses(
                      'flex cursor-default items-center justify-between px-3 py-2 text-sm text-[var(--color-muted)] outline-none transition duration-150 ease-out',
                      'data-[highlighted]:bg-[var(--color-canvas)] data-[selected]:text-[var(--color-ink)]',
                    )}
                  >
                    <Select.ItemText>{label}</Select.ItemText>
                    <Select.ItemIndicator className="text-[var(--color-primary)]">
                      <Check className="size-4" aria-hidden="true" />
                    </Select.ItemIndicator>
                  </Select.Item>
                ))}
              </Select.Popup>
            </Select.Positioner>
          </Select.Portal>
        </Select.Root>
      </div>

      <div className="border-t border-[var(--color-line)] pt-5">
        <div className="flex items-center justify-between gap-3 text-sm">
          <p className="text-[var(--color-muted)]">
            {resultCount} {resultCount === 1 ? 'piece' : 'pieces'}
          </p>
          {activeFilterCount > 0 && !onDone ? (
            <button
              type="button"
              onClick={() =>
                onSearchChange({
                  category: 'all',
                  sizes: [],
                  minPrice: productPriceRange.min,
                  maxPrice: productPriceRange.max,
                  inStockOnly: false,
                  saleOnly: false,
                })
              }
              className="text-sm font-medium text-[var(--color-ink)] underline-offset-4 transition duration-150 ease-out hover:text-[var(--color-primary)] hover:underline focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2 active:scale-[0.99]"
            >
              Reset filters
            </button>
          ) : null}
        </div>
        {onDone ? (
          <Button
            type="button"
            onClick={onDone}
            className="mt-4 inline-flex h-12 w-full items-center justify-center bg-[var(--color-primary)] px-5 text-sm font-medium text-[var(--color-paper)] transition duration-150 ease-out hover:bg-[var(--color-primary-dark)] focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2 active:scale-[0.99]"
          >
            Show {resultCount} {resultCount === 1 ? 'piece' : 'pieces'}
          </Button>
        ) : null}
      </div>
    </aside>
  )
}

function FilterCheckbox({
  checked,
  label,
  onChange,
}: {
  checked: boolean
  label: string
  onChange: (checked: boolean) => void
}) {
  return (
    <label className="flex cursor-pointer items-center gap-3 text-sm text-[var(--color-muted)] transition hover:text-[var(--color-ink)]">
      <Checkbox.Root
        checked={checked}
        onCheckedChange={onChange}
        className="flex size-4 items-center justify-center rounded border border-[var(--color-line)] bg-[var(--color-paper)] text-transparent outline-none transition duration-150 ease-out data-[checked]:border-[var(--color-primary)] data-[checked]:bg-[var(--color-primary)] data-[checked]:text-[var(--color-paper)] focus-visible:ring-2 focus-visible:ring-[var(--color-primary)] focus-visible:ring-offset-2"
      >
        <Checkbox.Indicator>
          <Check className="size-3" aria-hidden="true" />
        </Checkbox.Indicator>
      </Checkbox.Root>
      <span>{label}</span>
    </label>
  )
}

function toggleValue(values: string[], value: string) {
  return values.includes(value)
    ? values.filter((item) => item !== value)
    : [...values, value]
}
