import { useState } from "react"
import { Minus, Plus, XIcon } from "lucide-react"
import { Dialog as DialogPrimitive } from "@base-ui/react/dialog"

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
import { cn } from "@/lib/utils"

import {
  DISCOUNT_TYPE_OPTIONS,
  createBillingItem,
  getUnitPriceAfterDiscount,
  type BillableStockItem,
  type BillingItem,
  type DiscountType,
} from "./CreateNewInvoiceData"
import { formatAmount } from "../stock/StockItemsData"

const getDiscountTypeLabel = (value: string | null): string =>
  DISCOUNT_TYPE_OPTIONS.find((opt) => opt.value === value)?.label ?? "Select Type"

interface SelectedItemPopupProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  item: BillableStockItem | null
  
  existingItem?: BillingItem | null
  onConfirm: (billingItem: BillingItem) => void
}


function SelectedItemPopup({
  open,
  onOpenChange,
  item,
  existingItem,
  onConfirm,
}: SelectedItemPopupProps) {
  if (!item) return null

  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Popup
          className={cn(
            "fixed top-1/2 left-1/2 z-50 grid w-full max-w-[calc(100%-2rem)] -translate-x-1/2 -translate-y-1/2",
            "max-h-[85vh] gap-4 overflow-y-auto rounded-xl bg-popover p-4 text-sm text-popover-foreground",
            "ring-1 ring-foreground/10 outline-none sm:max-w-md",
            "[scrollbar-width:thin] [scrollbar-color:var(--border)_transparent]",
            "[&::-webkit-scrollbar]:w-1.5",
            "[&::-webkit-scrollbar-track]:bg-transparent",
            "[&::-webkit-scrollbar-thumb]:rounded-full [&::-webkit-scrollbar-thumb]:bg-border",
            "duration-100 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95",
            "data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95"
          )}
        >
          {open && (
            <SelectedItemPopupForm
              key={`${item.id}-${existingItem?.lineId ?? "new"}`}
              item={item}
              existingItem={existingItem ?? null}
              onCancel={() => onOpenChange(false)}
              onConfirm={(billingItem) => {
                onConfirm(billingItem)
                onOpenChange(false)
              }}
            />
          )}

          <DialogPrimitive.Close
            render={
              <Button variant="ghost" size="icon-sm" className="absolute top-2 right-2" />
            }
          >
            <XIcon />
            <span className="sr-only">Close</span>
          </DialogPrimitive.Close>
        </DialogPrimitive.Popup>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}

interface SelectedItemPopupFormProps {
  item: BillableStockItem
  existingItem: BillingItem | null
  onCancel: () => void
  onConfirm: (billingItem: BillingItem) => void
}

function SelectedItemPopupForm({
  item,
  existingItem,
  onCancel,
  onConfirm,
}: SelectedItemPopupFormProps) {
  const [quantity, setQuantity] = useState(() => existingItem?.quantity ?? 1)
  const [price, setPrice] = useState(() => existingItem?.price ?? item.sellingPrice)
  const [discountType, setDiscountType] = useState<DiscountType | null>(
    () => existingItem?.discountType ?? null
  )
  const [discountAmount, setDiscountAmount] = useState(
    () => existingItem?.discountAmount ?? 0
  )

  const maxQuantity = Math.max(item.quantity, 1)
  const isQuantityValid = quantity >= 1 && quantity <= item.quantity && item.quantity > 0

  const previewItem: BillingItem = createBillingItem(item, {
    quantity: Math.max(quantity, 0),
    price,
    discountType,
    discountAmount,
  })
  const unitPriceAfterDiscount = getUnitPriceAfterDiscount(previewItem)
  const discountPerUnit = price - unitPriceAfterDiscount
  const lineTotal = unitPriceAfterDiscount * Math.max(quantity, 0)

  const handleQuantityChange = (raw: string) => {
    const parsed = Number(raw)
    if (Number.isNaN(parsed)) {
      setQuantity(0)
      return
    }
    setQuantity(Math.min(Math.max(parsed, 0), maxQuantity))
  }

  const handleDiscountTypeChange = (value: string | null) => {
    const nextType = (value as DiscountType) ?? null
    setDiscountType(nextType)
    if (!nextType) setDiscountAmount(0)
  }

  const handleAddToInvoice = () => {
    if (!isQuantityValid) return
    onConfirm(
      createBillingItem(item, {
        quantity,
        price,
        discountType,
        discountAmount,
      })
    )
  }

  return (
    <>
      {/* ---- Header ---------------------------------------------------------- */}
      <div className="flex flex-col gap-2">
        <h2 className="font-heading text-base leading-none font-medium">Enter Quantity</h2>
        <div className="space-y-0.5">
          <p className="font-medium text-foreground">{item.itemName}</p>
          <p className="font-mono text-xs text-muted-foreground italic">
            Item Code: {item.itemCode} 
          </p>
        </div>
      </div>

      <div className="space-y-4">
        {/* Available stock ------------------------------------------------- */}
        <div className="flex items-center justify-between text-sm">
          <span className="text-muted-foreground">Available Stock</span>
          <span className="font-semibold text-emerald-600">{item.quantity} units</span>
        </div>

        {/* Quantity stepper -------------------------------------------------- */}
        <div className="space-y-1.5">
          <Label htmlFor="popup-quantity">Quantity to add</Label>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="icon"
              onClick={() => handleQuantityChange(String(quantity - 1))}
              disabled={quantity <= 1}
            >
              <Minus className="size-4" />
            </Button>
            <Input
              id="popup-quantity"
              type="number"
              min={1}
              max={maxQuantity}
              value={quantity}
              onChange={(e) => handleQuantityChange(e.target.value)}
              className="text-center"
            />
            <Button
              type="button"
              variant="outline"
              size="icon"
              onClick={() => handleQuantityChange(String(quantity + 1))}
              disabled={quantity >= maxQuantity}
            >
              <Plus className="size-4" />
            </Button>
          </div>
          {!isQuantityValid && (
            <p className="text-xs text-destructive">
              {item.quantity === 0
                ? "This item is out of stock."
                : `Enter a quantity between 1 and ${item.quantity}.`}
            </p>
          )}
        </div>

        {/* Cost / default selling price cards --------------------------------- */}
        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-lg border border-destructive/20 bg-destructive/5 p-3">
            <p className="text-xs font-medium text-destructive">Cost Price</p>
            <p className="mt-1 text-base font-semibold">{formatAmount(item.costPrice)}</p>
          </div>
          <div className="rounded-lg border border-primary/20 bg-primary/5 p-3">
            <p className="text-xs font-medium text-primary">Default Selling Price</p>
            <p className="mt-1 text-base font-semibold">{formatAmount(item.sellingPrice)}</p>
          </div>
        </div>

        {/* New selling price --------------------------------------------------- */}
        <div className="space-y-1.5">
          <Label htmlFor="popup-price" className="text-emerald-600">
            New Selling Price
          </Label>
          <Input
            id="popup-price"
            type="number"
            min={0}
            value={price}
            onChange={(e) => setPrice(Math.max(0, Number(e.target.value) || 0))}
          />
        </div>

        {/* Discount --------------------------------------------------------------- */}
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1.5">
            <Label>Discount Type</Label>
            <Select
              value={discountType}
              onValueChange={(value) => handleDiscountTypeChange(value as string | null)}
            >
              <SelectTrigger className="w-full min-w-0">
                <SelectValue placeholder="Select Type">
                  {(value: string | null) => getDiscountTypeLabel(value)}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {DISCOUNT_TYPE_OPTIONS.map((opt) => (
                  <SelectItem key={opt.value} value={opt.value}>
                    {opt.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="popup-discount-amount">Discount Amount</Label>
            <Input
              id="popup-discount-amount"
              type="number"
              min={0}
              disabled={!discountType}
              value={discountAmount}
              onChange={(e) => setDiscountAmount(Math.max(0, Number(e.target.value) || 0))}
            />
          </div>
        </div>

        {/* Breakdown ------------------------------------------------------------- */}
        <div className="space-y-1.5 rounded-lg border border-border/60 bg-muted/30 p-3 text-sm">
          <div className="flex justify-between">
            <span className="text-muted-foreground">Selling Price</span>
            <span>{formatAmount(price)}</span>
          </div>
          <div className="flex justify-between text-destructive">
            <span>Discount ({discountType === "percentage" ? "%" : "LKR"})</span>
            <span>- {formatAmount(discountPerUnit)}</span>
          </div>
          <div className="flex justify-between font-medium">
            <span>Unit Price After Discount</span>
            <span>{formatAmount(unitPriceAfterDiscount)}</span>
          </div>
          <div className="flex justify-between border-t border-border/60 pt-1.5 font-semibold">
            <span>
              Line Total ({Math.max(quantity, 0)} × {formatAmount(unitPriceAfterDiscount)})
            </span>
            <span>{formatAmount(lineTotal)}</span>
          </div>
        </div>
      </div>

      {/* ---- Footer ---------------------------------------------------------- */}
      <div className="-mx-4 -mb-4 flex flex-col-reverse gap-2 rounded-b-xl border-t bg-muted/50 p-4 sm:flex-row sm:justify-end">
        <Button type="button" variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="button" onClick={handleAddToInvoice} disabled={!isQuantityValid}>
          Add to Invoice
        </Button>
      </div>
    </>
  )
}

export default SelectedItemPopup
