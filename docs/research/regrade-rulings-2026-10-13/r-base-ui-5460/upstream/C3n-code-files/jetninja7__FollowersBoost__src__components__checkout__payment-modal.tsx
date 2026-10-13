'use client';

import { useState, useEffect, FormEvent } from 'react';
import { loadStripe, StripeElementsOptions } from '@stripe/stripe-js';
import {
  Elements,
  CardElement,
  useStripe,
  useElements,
} from '@stripe/react-stripe-js';
import {
  PayPalScriptProvider,
  PayPalButtons,
  FUNDING,
} from '@paypal/react-paypal-js';
import { Dialog as DialogPrimitive } from '@base-ui/react/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import { CARD_ELEMENT_OPTIONS } from '@/lib/stripe-ui';
import { XIcon, Loader2Icon } from 'lucide-react';
import { toast } from 'sonner';
import { SavedCards } from '@/components/checkout/saved-cards';
import { Checkbox } from '@/components/ui/checkbox';

// Initialize Stripe outside of component to avoid recreating on every render
const stripePromise = loadStripe(
  process.env.NEXT_PUBLIC_STRIPE_PUBLISHABLE_KEY!
);

interface PaymentModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: (amount: number) => void;
  minimumAmount?: number;
}

/**
 * Inner payment form component that uses Stripe hooks
 * Must be wrapped in Elements provider
 */
function PaymentForm({
  amount,
  onSuccess,
  onClose,
}: {
  amount: number;
  onSuccess: (amount: number) => void;
  onClose: () => void;
}) {
  const stripe = useStripe();
  const elements = useElements();

  const [cardholderName, setCardholderName] = useState('');
  const [isProcessing, setIsProcessing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cardComplete, setCardComplete] = useState(false);
  const [selectedCardId, setSelectedCardId] = useState<string | null>(null);
  const [saveCard, setSaveCard] = useState(false);

  // Can submit if using saved card OR if new card is complete
  const canSubmit =
    stripe &&
    !isProcessing &&
    amount >= 1 &&
    (selectedCardId
      ? true // Using saved card
      : elements && cardholderName.trim() !== '' && cardComplete); // New card

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();

    if (!canSubmit) return;

    setIsProcessing(true);
    setError(null);

    try {
      // Step 1: Create payment intent on server
      const response = await fetch('/api/wallet/add-funds', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          amount,
          paymentMethod: 'stripe',
        }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.error || 'Failed to create payment intent');
      }

      const { data } = await response.json();

      if (!data?.clientSecret) {
        throw new Error('No client secret received from server');
      }

      // Step 2: Confirm card payment with Stripe
      let result;

      if (selectedCardId) {
        // Use saved payment method
        result = await stripe.confirmCardPayment(data.clientSecret, {
          payment_method: selectedCardId,
        });
      } else {
        // Use new card
        const cardElement = elements!.getElement(CardElement);
        if (!cardElement) {
          throw new Error('Card element not found');
        }

        result = await stripe.confirmCardPayment(data.clientSecret, {
          payment_method: {
            card: cardElement,
            billing_details: {
              name: cardholderName,
            },
          },
          setup_future_usage: saveCard ? 'off_session' : undefined,
        });
      }

      if (result.error) {
        throw new Error(result.error.message || 'Payment failed');
      }

      // Step 3: Success!
      toast.success('Funds added successfully', {
        description: `$${amount.toFixed(2)} has been added to your wallet`,
      });

      onSuccess(amount);
      onClose();
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : 'An error occurred';
      setError(errorMessage);
      toast.error('Payment failed', {
        description: errorMessage,
      });
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      {/* Saved Cards */}
      <SavedCards
        onSelectCard={(cardId) => {
          setSelectedCardId(cardId || null);
          if (cardId) {
            setError(null);
          }
        }}
        selectedCardId={selectedCardId}
      />

      {/* New Card Section - Show only if no saved card selected */}
      {!selectedCardId && (
        <>
          {/* Cardholder Name */}
          <div className="space-y-2">
            <Label htmlFor="cardholder-name">Cardholder Name</Label>
            <Input
              id="cardholder-name"
              type="text"
              placeholder="John Doe"
              value={cardholderName}
              onChange={(e) => setCardholderName(e.target.value)}
              disabled={isProcessing}
              required
            />
          </div>

          {/* Card Element */}
          <div className="space-y-2">
            <Label>Card Information</Label>
            <div
              className={cn(
                'rounded-lg border border-input bg-transparent px-2.5 py-3 transition-colors',
                'focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50',
                isProcessing && 'opacity-50 pointer-events-none',
                error && 'border-destructive'
              )}
            >
              <CardElement
                options={CARD_ELEMENT_OPTIONS}
                onChange={(e) => {
                  setCardComplete(e.complete);
                  if (e.error) {
                    setError(e.error.message);
                  } else {
                    setError(null);
                  }
                }}
              />
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
          </div>

          {/* Save Card Checkbox */}
          <div className="flex items-center space-x-2">
            <Checkbox
              id="save-card"
              checked={saveCard}
              onCheckedChange={(checked) => setSaveCard(checked === true)}
              disabled={isProcessing}
            />
            <label
              htmlFor="save-card"
              className="text-sm text-muted-foreground cursor-pointer"
            >
              Save card for future payments
            </label>
          </div>
        </>
      )}

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
            'Add Funds'
          )}
        </Button>
      </div>
    </form>
  );
}

/**
 * Payment Modal Component
 * Allows users to add funds to their wallet via Stripe
 */
export function PaymentModal({
  isOpen,
  onClose,
  onSuccess,
  minimumAmount = 0,
}: PaymentModalProps) {
  const [amount, setAmount] = useState<string>(
    minimumAmount > 0 ? minimumAmount.toFixed(2) : ''
  );
  const [activeTab, setActiveTab] = useState('stripe');

  // Update amount when minimumAmount prop changes
  useEffect(() => {
    if (minimumAmount > 0 && (!amount || parseFloat(amount) < minimumAmount)) {
      setAmount(minimumAmount.toFixed(2));
    }
  }, [minimumAmount, amount]);

  // Parse amount as number
  const numericAmount = parseFloat(amount) || 0;
  const isValidAmount = numericAmount >= 1 && numericAmount <= 10000;

  // Quick amount suggestions
  const suggestAmount = (increment: number) => {
    const newAmount = numericAmount + increment;
    if (newAmount <= 10000) {
      setAmount(newAmount.toFixed(2));
    }
  };

  // Elements options
  const elementsOptions: StripeElementsOptions = {
    appearance: {
      theme: 'stripe',
    },
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
              Add Funds
            </DialogPrimitive.Title>
            <DialogPrimitive.Close
              render={
                <Button variant="ghost" size="icon-sm" />
              }
            >
              <XIcon />
              <span className="sr-only">Close</span>
            </DialogPrimitive.Close>
          </div>

          {/* Body */}
          <div className="p-4 space-y-4">
            {/* Amount Section */}
            <div className="space-y-2">
              <Label htmlFor="amount">Amount to add</Label>
              <div className="flex items-center gap-2">
                <div className="relative flex-1">
                  <span className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground">
                    $
                  </span>
                  <Input
                    id="amount"
                    type="number"
                    min="1"
                    max="10000"
                    step="0.01"
                    value={amount}
                    onChange={(e) => setAmount(e.target.value)}
                    className="pl-6"
                    placeholder="0.00"
                  />
                </div>
              </div>

              {/* Quick Add Buttons */}
              <div className="flex gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => suggestAmount(10)}
                  disabled={numericAmount + 10 > 10000}
                >
                  +$10
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => suggestAmount(25)}
                  disabled={numericAmount + 25 > 10000}
                >
                  +$25
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => suggestAmount(50)}
                  disabled={numericAmount + 50 > 10000}
                >
                  +$50
                </Button>
              </div>

              {/* Amount validation messages */}
              {amount && !isValidAmount && (
                <p className="text-sm text-destructive">
                  {numericAmount < 1
                    ? 'Minimum amount is $1.00'
                    : 'Maximum amount is $10,000.00'}
                </p>
              )}
              {minimumAmount > 0 && numericAmount < minimumAmount && (
                <p className="text-sm text-amber-600">
                  You need at least ${minimumAmount.toFixed(2)} to complete your order
                </p>
              )}
            </div>

            {/* Payment Method Tabs */}
            <Tabs value={activeTab} onValueChange={setActiveTab}>
              <TabsList className="w-full">
                <TabsTrigger value="stripe" className="flex-1">
                  Credit/Debit Card
                </TabsTrigger>
                <TabsTrigger
                  value="paypal"
                  className="flex-1 flex items-center gap-2"
                >
                  PayPal
                </TabsTrigger>
              </TabsList>

              {/* Stripe Payment Tab */}
              <TabsContent value="stripe" className="mt-4">
                {isValidAmount ? (
                  <Elements stripe={stripePromise} options={elementsOptions}>
                    <PaymentForm
                      amount={numericAmount}
                      onSuccess={onSuccess}
                      onClose={onClose}
                    />
                  </Elements>
                ) : (
                  <div className="text-center py-8 text-muted-foreground">
                    Please enter a valid amount between $1.00 and $10,000.00
                  </div>
                )}
              </TabsContent>

              {/* PayPal Tab */}
              <TabsContent value="paypal" className="mt-4">
                {isValidAmount ? (
                  <PayPalScriptProvider
                    options={{
                      clientId: process.env.NEXT_PUBLIC_PAYPAL_CLIENT_ID || '',
                      currency: 'USD',
                      intent: 'capture',
                    }}
                  >
                    <div className="space-y-4">
                      <div className="text-sm text-muted-foreground text-center">
                        You will be charged ${numericAmount.toFixed(2)} USD
                      </div>
                      <PayPalButtons
                        fundingSource={FUNDING.PAYPAL}
                        style={{
                          layout: 'vertical',
                          color: 'gold',
                          shape: 'rect',
                          label: 'paypal',
                        }}
                        createOrder={async () => {
                          try {
                            const response = await fetch(
                              '/api/wallet/paypal/create-order',
                              {
                                method: 'POST',
                                headers: { 'Content-Type': 'application/json' },
                                body: JSON.stringify({ amount: numericAmount }),
                              }
                            );

                            if (!response.ok) {
                              throw new Error('Failed to create PayPal order');
                            }

                            const { data } = await response.json();
                            return data.orderId;
                          } catch (error) {
                            console.error('PayPal create order error:', error);
                            toast.error('Failed to create PayPal order');
                            throw error;
                          }
                        }}
                        onApprove={async (data) => {
                          try {
                            const response = await fetch(
                              '/api/wallet/paypal/capture',
                              {
                                method: 'POST',
                                headers: { 'Content-Type': 'application/json' },
                                body: JSON.stringify({
                                  orderId: data.orderID,
                                  amount: numericAmount,
                                }),
                              }
                            );

                            if (!response.ok) {
                              throw new Error('Failed to capture PayPal payment');
                            }

                            const result = await response.json();
                            toast.success(
                              `Successfully added $${numericAmount.toFixed(
                                2
                              )} to your wallet!`
                            );
                            onSuccess(numericAmount);
                            onClose();
                          } catch (error) {
                            console.error('PayPal capture error:', error);
                            toast.error('Payment failed. Please try again.');
                          }
                        }}
                        onError={(err) => {
                          console.error('PayPal error:', err);
                          toast.error('PayPal payment failed');
                        }}
                        onCancel={() => {
                          toast.info('Payment cancelled');
                        }}
                      />
                    </div>
                  </PayPalScriptProvider>
                ) : (
                  <div className="text-center py-8 text-muted-foreground">
                    Please enter a valid amount between $1.00 and $10,000.00
                  </div>
                )}
              </TabsContent>
            </Tabs>
          </div>
        </DialogPrimitive.Popup>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
