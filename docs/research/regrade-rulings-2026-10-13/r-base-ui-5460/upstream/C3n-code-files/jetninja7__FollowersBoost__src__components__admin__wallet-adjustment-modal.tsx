'use client';

import { useState, FormEvent } from 'react';
import { Dialog as DialogPrimitive } from '@base-ui/react/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
import { cn } from '@/lib/utils';
import { XIcon, Loader2Icon } from 'lucide-react';
import { toast } from 'sonner';

interface WalletAdjustmentModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  userId: string;
  currentBalance: string;
}

export function WalletAdjustmentModal({
  isOpen,
  onClose,
  onSuccess,
  userId,
  currentBalance,
}: WalletAdjustmentModalProps) {
  const [operation, setOperation] = useState<'ADD' | 'SUBTRACT'>('ADD');
  const [amount, setAmount] = useState('');
  const [reason, setReason] = useState('');
  const [isProcessing, setIsProcessing] = useState(false);

  const numericAmount = parseFloat(amount) || 0;
  const isValidAmount = numericAmount > 0;
  const canSubmit = isValidAmount && reason.trim().length > 0 && !isProcessing;

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();

    if (!canSubmit) return;

    setIsProcessing(true);

    try {
      const response = await fetch(`/api/admin/users/${userId}/wallet/adjust`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          operation,
          amount: numericAmount,
          reason: reason.trim(),
        }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.error?.message || 'Failed to adjust wallet');
      }

      toast.success('Wallet adjusted successfully', {
        description: `${operation === 'ADD' ? 'Added' : 'Subtracted'} $${numericAmount.toFixed(2)}`,
      });

      // Reset form
      setAmount('');
      setReason('');
      setOperation('ADD');

      onSuccess();
      onClose();
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'An error occurred';
      toast.error('Failed to adjust wallet', {
        description: errorMessage,
      });
    } finally {
      setIsProcessing(false);
    }
  };

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
              Adjust Wallet Balance
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
            <div className="mb-4">
              <p className="text-sm text-gray-600">
                Current Balance: <span className="font-semibold">${Number(currentBalance).toFixed(2)}</span>
              </p>
            </div>

            <form onSubmit={handleSubmit} className="space-y-4">
              {/* Operation Selection */}
              <div className="space-y-2">
                <Label>Operation</Label>
                <RadioGroup
                  value={operation}
                  onValueChange={(value: string) => setOperation(value as 'ADD' | 'SUBTRACT')}
                >
                  <div className="flex items-center space-x-4">
                    <div className="flex items-center space-x-2">
                      <RadioGroupItem value="ADD" id="add" />
                      <label
                        htmlFor="add"
                        className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70 cursor-pointer"
                      >
                        Add Funds
                      </label>
                    </div>
                    <div className="flex items-center space-x-2">
                      <RadioGroupItem value="SUBTRACT" id="subtract" />
                      <label
                        htmlFor="subtract"
                        className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70 cursor-pointer"
                      >
                        Subtract Funds
                      </label>
                    </div>
                  </div>
                </RadioGroup>
              </div>

              {/* Amount Input */}
              <div className="space-y-2">
                <Label htmlFor="amount">Amount</Label>
                <div className="relative">
                  <span className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground">
                    $
                  </span>
                  <Input
                    id="amount"
                    type="number"
                    min="0.01"
                    step="0.01"
                    value={amount}
                    onChange={(e) => setAmount(e.target.value)}
                    className="pl-6"
                    placeholder="0.00"
                    disabled={isProcessing}
                    required
                  />
                </div>
              </div>

              {/* Reason Input */}
              <div className="space-y-2">
                <Label htmlFor="reason">Reason</Label>
                <Textarea
                  id="reason"
                  value={reason}
                  onChange={(e) => setReason(e.target.value)}
                  placeholder="Enter reason for adjustment..."
                  disabled={isProcessing}
                  required
                  rows={3}
                />
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
                <Button
                  type="submit"
                  disabled={!canSubmit}
                  className="flex-1"
                >
                  {isProcessing ? (
                    <>
                      <Loader2Icon className="animate-spin" />
                      Processing...
                    </>
                  ) : (
                    'Adjust Wallet'
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
