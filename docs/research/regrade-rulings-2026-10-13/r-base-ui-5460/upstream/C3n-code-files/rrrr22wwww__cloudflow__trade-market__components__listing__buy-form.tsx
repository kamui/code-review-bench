"use client";

import { NumberField } from "@base-ui-components/react/number-field";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { cn, formatPrice } from "@/lib/utils";

const modes = ["Market", "Limit", "Stop"] as const;

export function BuyForm({ price }: { price: number }) {
  const [mode, setMode] = useState<(typeof modes)[number]>("Market");
  const [quantity, setQuantity] = useState(1);
  const [limitPrice, setLimitPrice] = useState(price);
  const [balancePercent, setBalancePercent] = useState(30);

  const effectivePrice = mode === "Market" ? price : limitPrice;
  const total = useMemo(() => effectivePrice * quantity, [effectivePrice, quantity]);

  return (
    <form className="space-y-4 rounded-lg border border-border bg-surface-raised p-4" aria-label="Buy asset form">
      <div className="grid grid-cols-3 gap-2">
        {modes.map((tab) => (
          <button
            type="button"
            key={tab}
            onClick={() => setMode(tab)}
            className={cn(
              "rounded-md border px-2 py-1.5 text-xs font-mono uppercase tracking-wide transition-all duration-200 ease-spring",
              mode === tab
                ? "border-brand/60 bg-brand-muted text-brand"
                : "border-border bg-surface text-muted-foreground hover:border-brand/30",
            )}
            aria-pressed={mode === tab}
          >
            {tab}
          </button>
        ))}
      </div>

      <div className="space-y-2">
        <label className="text-xs font-mono uppercase tracking-widest text-muted-foreground">Quantity</label>
        <NumberField.Root
          value={quantity}
          min={0.1}
          step={0.1}
          onValueChange={(value) => setQuantity(value ?? 0)}
          className="rounded-md border border-border bg-surface px-2 py-2"
        >
          <NumberField.Group className="flex items-center gap-2">
            <NumberField.Decrement className="rounded border border-border px-2 py-1 text-sm text-muted-foreground hover:text-foreground">
              -
            </NumberField.Decrement>
            <NumberField.Input
              className="h-8 flex-1 bg-transparent text-center font-mono text-sm text-foreground outline-none"
              aria-label="Buy quantity"
            />
            <NumberField.Increment className="rounded border border-border px-2 py-1 text-sm text-muted-foreground hover:text-foreground">
              +
            </NumberField.Increment>
          </NumberField.Group>
        </NumberField.Root>
      </div>

      {mode !== "Market" && (
        <div className="space-y-2">
          <label className="text-xs font-mono uppercase tracking-widest text-muted-foreground">
            {mode} Price
          </label>
          <input
            type="number"
            className="h-10 w-full rounded-md border border-border bg-surface px-3 font-mono text-sm"
            value={limitPrice}
            min={0}
            onChange={(event) => setLimitPrice(Number(event.target.value))}
            aria-label={`${mode} price`}
          />
        </div>
      )}

      <div className="space-y-2">
        <label className="text-xs font-mono uppercase tracking-widest text-muted-foreground">Balance Allocation</label>
        <input
          type="range"
          min={0}
          max={100}
          value={balancePercent}
          onChange={(event) => setBalancePercent(Number(event.target.value))}
          className="w-full"
          aria-label="Balance percent"
        />
        <p className="text-xs text-muted-foreground">{balancePercent}% of available balance</p>
      </div>

      <div className="rounded-md border border-brand/30 bg-brand-muted p-3 text-sm">
        <div className="mb-1 flex justify-between text-muted-foreground">
          <span>Price</span>
          <span className="font-mono text-foreground">{formatPrice(effectivePrice)}</span>
        </div>
        <div className="flex justify-between text-muted-foreground">
          <span>Total</span>
          <span className="font-mono text-brand">{formatPrice(total)}</span>
        </div>
      </div>

      <Button type="submit" variant="primary" className="w-full">
        Execute Buy Order
      </Button>
    </form>
  );
}
