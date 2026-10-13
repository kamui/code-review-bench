'use client';

import { useState, FormEvent, useEffect } from 'react';
import { Dialog as DialogPrimitive } from '@base-ui/react/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Checkbox } from '@/components/ui/checkbox';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { cn } from '@/lib/utils';
import { XIcon, Loader2Icon } from 'lucide-react';
import { toast } from 'sonner';

interface ServiceFormProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  categories: Array<{
    id: string;
    name: string;
    platform?: { name: string };
  }>;
  service?: {
    id: string;
    categoryId: string;
    name: string;
    slug: string;
    description: string;
    price: string;
    minQuantity: number;
    maxQuantity: number;
    estimatedDeliveryTime: string | null;
    isActive: boolean;
  } | null;
}

export function ServiceForm({
  isOpen,
  onClose,
  onSuccess,
  categories,
  service = null,
}: ServiceFormProps) {
  const [categoryId, setCategoryId] = useState('');
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [description, setDescription] = useState('');
  const [price, setPrice] = useState('');
  const [minQuantity, setMinQuantity] = useState('1');
  const [maxQuantity, setMaxQuantity] = useState('1000');
  const [estimatedDeliveryTime, setEstimatedDeliveryTime] = useState('');
  const [isActive, setIsActive] = useState(true);
  const [isProcessing, setIsProcessing] = useState(false);

  const isEditMode = !!service;

  // Reset form when service changes or modal opens/closes
  useEffect(() => {
    if (service) {
      setCategoryId(service.categoryId);
      setName(service.name);
      setSlug(service.slug);
      setDescription(service.description);
      setPrice(service.price);
      setMinQuantity(service.minQuantity.toString());
      setMaxQuantity(service.maxQuantity.toString());
      setEstimatedDeliveryTime(service.estimatedDeliveryTime || '');
      setIsActive(service.isActive);
    } else {
      setCategoryId('');
      setName('');
      setSlug('');
      setDescription('');
      setPrice('');
      setMinQuantity('1');
      setMaxQuantity('1000');
      setEstimatedDeliveryTime('');
      setIsActive(true);
    }
  }, [service, isOpen]);

  const isValidSlug = /^[a-z0-9-]*$/.test(slug);
  const priceNum = parseFloat(price);
  const minQty = parseInt(minQuantity, 10);
  const maxQty = parseInt(maxQuantity, 10);

  const canSubmit =
    categoryId.trim().length > 0 &&
    name.trim().length > 0 &&
    name.length <= 100 &&
    slug.trim().length > 0 &&
    isValidSlug &&
    description.length <= 500 &&
    !isNaN(priceNum) &&
    priceNum >= 0.01 &&
    priceNum <= 10000 &&
    !isNaN(minQty) &&
    minQty >= 1 &&
    !isNaN(maxQty) &&
    maxQty > minQty &&
    !isProcessing;

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();

    if (!canSubmit) return;

    setIsProcessing(true);

    try {
      const endpoint = isEditMode
        ? `/api/admin/services/${service.id}`
        : '/api/admin/services';

      const method = isEditMode ? 'PATCH' : 'POST';

      const response = await fetch(endpoint, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          categoryId,
          name: name.trim(),
          slug: slug.trim(),
          description: description.trim(),
          price: priceNum,
          minQuantity: minQty,
          maxQuantity: maxQty,
          estimatedDeliveryTime: estimatedDeliveryTime.trim() || null,
          isActive,
        }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.error?.message || 'Failed to save service');
      }

      toast.success(
        `Service ${isEditMode ? 'updated' : 'created'} successfully`
      );

      onSuccess();
      onClose();
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'An error occurred';
      toast.error(`Failed to ${isEditMode ? 'update' : 'create'} service`, {
        description: errorMessage,
      });
    } finally {
      setIsProcessing(false);
    }
  };

  // Group categories by platform
  const groupedCategories = categories.reduce((acc, cat) => {
    const platformName = cat.platform?.name || 'Unknown';
    if (!acc[platformName]) {
      acc[platformName] = [];
    }
    acc[platformName].push(cat);
    return acc;
  }, {} as Record<string, typeof categories>);

  return (
    <DialogPrimitive.Root open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogPrimitive.Portal>
        {/* Backdrop */}
        <DialogPrimitive.Backdrop
          className={cn(
            'fixed inset-0 z-50 bg-black/50 transition-opacity duration-200',
            'data-ending-style:opacity-0 data-starting-style:opacity-0',
            'supports-backdrop-filter:backdrop-blur-sm'
          )}
        />

        {/* Modal Content */}
        <DialogPrimitive.Popup
          className={cn(
            'fixed left-1/2 top-1/2 z-50 w-full max-w-lg -translate-x-1/2 -translate-y-1/2',
            'rounded-lg border border-border bg-popover shadow-lg',
            'transition duration-200 ease-out',
            'data-ending-style:opacity-0 data-ending-style:scale-95',
            'data-starting-style:opacity-0 data-starting-style:scale-95',
            'max-h-[90vh] overflow-y-auto'
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-border p-4">
            <DialogPrimitive.Title className="text-lg font-semibold">
              {isEditMode ? 'Edit Service' : 'Create Service'}
            </DialogPrimitive.Title>
            <DialogPrimitive.Close
              render={<Button variant="ghost" size="icon-sm" />}
            >
              <XIcon />
              <span className="sr-only">Close</span>
            </DialogPrimitive.Close>
          </div>

          {/* Body */}
          <div className="p-4">
            <form onSubmit={handleSubmit} className="space-y-4">
              {/* Category */}
              <div className="space-y-2">
                <Label htmlFor="categoryId">
                  Category <span className="text-destructive">*</span>
                </Label>
                <Select
                  value={categoryId}
                  onValueChange={(value) => setCategoryId(value || '')}
                  disabled={isProcessing}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Select a category" />
                  </SelectTrigger>
                  <SelectContent>
                    {Object.entries(groupedCategories).map(([platform, cats]) => (
                      <div key={platform}>
                        <div className="px-2 py-1.5 text-sm font-semibold text-muted-foreground">
                          {platform}
                        </div>
                        {cats.map((cat) => (
                          <SelectItem key={cat.id} value={cat.id}>
                            {cat.name}
                          </SelectItem>
                        ))}
                      </div>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* Name */}
              <div className="space-y-2">
                <Label htmlFor="name">
                  Name <span className="text-destructive">*</span>
                </Label>
                <Input
                  id="name"
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="1000 Followers"
                  disabled={isProcessing}
                  maxLength={100}
                  required
                />
                <p className="text-xs text-muted-foreground">
                  {name.length}/100 characters
                </p>
              </div>

              {/* Slug */}
              <div className="space-y-2">
                <Label htmlFor="slug">
                  Slug <span className="text-destructive">*</span>
                </Label>
                <Input
                  id="slug"
                  type="text"
                  value={slug}
                  onChange={(e) => setSlug(e.target.value.toLowerCase())}
                  placeholder="1000-followers"
                  disabled={isProcessing}
                  required
                />
                {!isValidSlug && slug.length > 0 && (
                  <p className="text-xs text-destructive">
                    Only lowercase letters, numbers, and hyphens allowed
                  </p>
                )}
              </div>

              {/* Description */}
              <div className="space-y-2">
                <Label htmlFor="description">
                  Description <span className="text-destructive">*</span>
                </Label>
                <Textarea
                  id="description"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="Service description..."
                  disabled={isProcessing}
                  maxLength={500}
                  rows={3}
                  required
                />
                <p className="text-xs text-muted-foreground">
                  {description.length}/500 characters
                </p>
              </div>

              {/* Price */}
              <div className="space-y-2">
                <Label htmlFor="price">
                  Price <span className="text-destructive">*</span>
                </Label>
                <div className="relative">
                  <span className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground">
                    $
                  </span>
                  <Input
                    id="price"
                    type="number"
                    min="0.01"
                    max="10000"
                    step="0.01"
                    value={price}
                    onChange={(e) => setPrice(e.target.value)}
                    className="pl-6"
                    placeholder="0.00"
                    disabled={isProcessing}
                    required
                  />
                </div>
                <p className="text-xs text-muted-foreground">
                  Price between $0.01 and $10,000
                </p>
              </div>

              {/* Min & Max Quantity */}
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label htmlFor="minQuantity">
                    Min Quantity <span className="text-destructive">*</span>
                  </Label>
                  <Input
                    id="minQuantity"
                    type="number"
                    min="1"
                    value={minQuantity}
                    onChange={(e) => setMinQuantity(e.target.value)}
                    disabled={isProcessing}
                    required
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="maxQuantity">
                    Max Quantity <span className="text-destructive">*</span>
                  </Label>
                  <Input
                    id="maxQuantity"
                    type="number"
                    min="1"
                    value={maxQuantity}
                    onChange={(e) => setMaxQuantity(e.target.value)}
                    disabled={isProcessing}
                    required
                  />
                </div>
              </div>
              {!isNaN(minQty) && !isNaN(maxQty) && maxQty <= minQty && (
                <p className="text-xs text-destructive">
                  Max quantity must be greater than min quantity
                </p>
              )}

              {/* Estimated Delivery Time */}
              <div className="space-y-2">
                <Label htmlFor="estimatedDeliveryTime">
                  Estimated Delivery Time
                </Label>
                <Input
                  id="estimatedDeliveryTime"
                  type="text"
                  value={estimatedDeliveryTime}
                  onChange={(e) => setEstimatedDeliveryTime(e.target.value)}
                  placeholder="1-3 days"
                  disabled={isProcessing}
                />
              </div>

              {/* Is Active */}
              <div className="flex items-center space-x-2">
                <Checkbox
                  id="isActive"
                  checked={isActive}
                  onCheckedChange={(checked) =>
                    setIsActive(checked === true)
                  }
                  disabled={isProcessing}
                />
                <label
                  htmlFor="isActive"
                  className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70 cursor-pointer"
                >
                  Active
                </label>
              </div>

              {/* Action Buttons */}
              <div className="flex gap-3 pt-2">
                <Button
                  type="button"
                  variant="outline"
                  onClick={onClose}
                  disabled={isProcessing}
                  className="flex-1"
                >
                  Cancel
                </Button>
                <Button type="submit" disabled={!canSubmit} className="flex-1">
                  {isProcessing ? (
                    <>
                      <Loader2Icon className="animate-spin" />
                      {isEditMode ? 'Updating...' : 'Creating...'}
                    </>
                  ) : isEditMode ? (
                    'Update'
                  ) : (
                    'Create'
                  )}
                </Button>
              </div>
            </form>
          </div>
        </DialogPrimitive.Popup>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
