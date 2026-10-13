"use client";

import { Autocomplete } from "@base-ui/react/autocomplete";
import { FormEvent, useState } from "react";
import {
    ArrowLeftRight,
    ChevronDown,
    LoaderCircle,
    Plus,
    Save,
    X,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { CurrencyFlag } from "@/components/ui/currency-flag";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { SelectField } from "@/components/ui/select-field";
import { useToast } from "@/components/ui/toast";
import {
    businessCurrencyConfigurationSchema,
    normalizeCurrencyConfiguration,
    type BusinessCurrency,
    type BusinessCurrencyConfiguration,
} from "@/lib/api/currency";
import {
    useGetBusinessCurrenciesQuery,
    useUpdateBusinessCurrenciesMutation,
} from "@/services/currencyApi";
import LiveExchangeRatesSection from "./LiveExchangeRatesSection";

function getApiErrorMessage(error: unknown, fallback: string) {
    if (
        typeof error === "object" &&
        error !== null &&
        "data" in error &&
        typeof error.data === "object" &&
        error.data !== null &&
        "message" in error.data &&
        typeof error.data.message === "string"
    ) {
        return error.data.message;
    }

    return fallback;
}

function getCurrencyDetails(code: string) {
    const normalizedInput = code.trim().toUpperCase();
    const normalizedCode =
        normalizedInput.match(/^[A-Z]{3}$/)?.[0] ||
        normalizedInput.match(/\(([A-Z]{3})\)$/)?.[1] ||
        "";

    if (!/^[A-Z]{3}$/.test(normalizedCode)) {
        return null;
    }

    try {
        const name =
            new Intl.DisplayNames(["en"], { type: "currency" }).of(
                normalizedCode,
            ) || normalizedCode;
        const symbol =
            new Intl.NumberFormat("en", {
                style: "currency",
                currency: normalizedCode,
                currencyDisplay: "narrowSymbol",
            })
                .formatToParts(0)
                .find((part) => part.type === "currency")?.value ||
            normalizedCode;

        return { code: normalizedCode, name, symbol };
    } catch {
        return null;
    }
}

const supportedCurrencyOptions = Intl.supportedValuesOf("currency").flatMap(
    (code) => {
        const currency = getCurrencyDetails(code);

        return currency
            ? [
                  {
                      value: currency.code,
                      label: `${currency.name} (${currency.code})`,
                  },
              ]
            : [];
    },
);

type CurrencyOption = (typeof supportedCurrencyOptions)[number];

function CurrencyAutocomplete({
    value,
    onValueChange,
    options,
}: {
    value: string;
    onValueChange: (value: string) => void;
    options: CurrencyOption[];
}) {
    return (
        <Autocomplete.Root
            items={options}
            value={value}
            onValueChange={onValueChange}
            itemToStringValue={(option) => option.label}
            openOnInputClick
            autoHighlight
        >
            <Autocomplete.InputGroup className="relative h-[38px] min-w-[240px] flex-1">
                <Autocomplete.Input
                    id="add-currency"
                    aria-label="Search or enter a three-letter currency code"
                    placeholder="Search or enter a currency..."
                    className="h-full w-full rounded-lg border-0 bg-transparent px-3 pr-10 text-base text-foreground outline-none placeholder:text-muted-foreground focus-visible:ring-0"
                />
                <Autocomplete.Trigger
                    type="button"
                    aria-label="Show currency options"
                    className="absolute inset-y-0 right-0 flex w-10 items-center justify-center rounded-lg text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-primary"
                >
                    <ChevronDown className="size-4" />
                </Autocomplete.Trigger>
            </Autocomplete.InputGroup>

            <Autocomplete.Portal>
                <Autocomplete.Positioner
                    sideOffset={4}
                    align="start"
                    className="isolate z-50 outline-none"
                >
                    <Autocomplete.Popup className="w-(--anchor-width) min-w-64 overflow-hidden rounded-xl bg-popover p-1.5 text-popover-foreground shadow-lg ring-1 ring-border">
                        <Autocomplete.Empty className="px-3 py-3 text-sm text-muted-foreground">
                            No matching currency. You can enter a custom
                            three-letter code.
                        </Autocomplete.Empty>
                        <Autocomplete.List className="max-h-72 overflow-y-auto overscroll-contain outline-none">
                            {(option: CurrencyOption) => (
                                <Autocomplete.Item
                                    key={option.value}
                                    value={option}
                                    className="flex cursor-default items-center gap-2 rounded-lg px-3 py-2.5 text-base outline-none select-none data-highlighted:bg-primary/10 data-highlighted:text-primary"
                                >
                                    <CurrencyFlag code={option.value} size="xs" />
                                    <span>{option.label}</span>
                                </Autocomplete.Item>
                            )}
                        </Autocomplete.List>
                    </Autocomplete.Popup>
                </Autocomplete.Positioner>
            </Autocomplete.Portal>
        </Autocomplete.Root>
    );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
    return (
        <div className="flex items-center gap-3">
            <span className="h-6 w-1.5 rounded-full bg-primary" />
            <h2 className="text-xl leading-5 font-semibold tracking-[0.5px] text-primary uppercase">
                {children}
            </h2>
        </div>
    );
}

function CurrencyEditor({
    initialConfiguration,
}: {
    initialConfiguration: BusinessCurrencyConfiguration;
}) {
    const normalized = normalizeCurrencyConfiguration(
        initialConfiguration,
    );
    const [currencies, setCurrencies] = useState(normalized.currencies);
    const [baseCurrency, setBaseCurrency] = useState(
        normalized.baseCurrency,
    );
    const [displayCurrency, setDisplayCurrency] = useState(
        normalized.displayCurrency || normalized.baseCurrency,
    );
    const [selectedTarget, setSelectedTarget] = useState(
        normalized.currencies.find(
            (currency) => currency.code !== normalized.baseCurrency,
        )?.code || "",
    );
    const [newCurrencyCode, setNewCurrencyCode] = useState("");
    const [updateCurrencies, updateState] =
        useUpdateBusinessCurrenciesMutation();
    const { toast } = useToast();

    const base =
        currencies.find((currency) => currency.code === baseCurrency) ||
        currencies[0];
    const target =
        currencies.find((currency) => currency.code === selectedTarget) ||
        currencies.find((currency) => currency.code !== baseCurrency);
    const configuredCurrencyCodes = new Set(
        currencies.map((currency) => currency.code),
    );
    const availableCurrencyOptions = supportedCurrencyOptions.filter(
        (option) => !configuredCurrencyCodes.has(option.value),
    );

    function updateCurrency(
        code: string,
        patch: Partial<BusinessCurrency>,
    ) {
        setCurrencies((current) =>
            current.map((currency) =>
                currency.code === code
                    ? { ...currency, ...patch }
                    : currency,
            ),
        );
    }

    function addCurrency() {
        const details = getCurrencyDetails(newCurrencyCode);

        if (!details) {
            toast({
                tone: "error",
                title: "Currency not added",
                description: "Enter a valid three-letter currency code.",
            });
            return;
        }

        if (
            currencies.some(
                (currency) => currency.code === details.code,
            )
        ) {
            toast({
                tone: "error",
                title: "Currency not added",
                description: `${details.code} is already configured.`,
            });
            return;
        }

        setCurrencies((current) => [
            ...current,
            {
                ...details,
                exchangeRate: 1,
                decimalPlaces: 2,
            },
        ]);
        setSelectedTarget(details.code);
        setNewCurrencyCode("");
    }

    function removeCurrency(code: string) {
        if (code === baseCurrency) {
            toast({
                tone: "error",
                title: "Currency not removed",
                description:
                    "Choose another base currency before removing this one.",
            });
            return;
        }

        const remaining = currencies.filter(
            (currency) => currency.code !== code,
        );
        setCurrencies(remaining);
        if (displayCurrency === code) {
            setDisplayCurrency(baseCurrency);
        }
        if (selectedTarget === code) {
            setSelectedTarget(
                remaining.find(
                    (currency) => currency.code !== baseCurrency,
                )?.code || "",
            );
        }
    }

    function changeBaseCurrency(code: string | null) {
        if (!code || code === baseCurrency) {
            return;
        }

        const nextBase = currencies.find(
            (currency) => currency.code === code,
        );

        if (!nextBase || nextBase.exchangeRate <= 0) {
            toast({
                tone: "error",
                title: "Base currency not changed",
                description:
                    "Set a positive exchange rate before using this as the base currency.",
            });
            return;
        }

        setCurrencies((current) =>
            current.map((currency) => ({
                ...currency,
                exchangeRate:
                    currency.code === code
                        ? 1
                        : currency.exchangeRate / nextBase.exchangeRate,
            })),
        );
        setBaseCurrency(code);
        if (selectedTarget === code) {
            setSelectedTarget(baseCurrency);
        }
    }

    function swapCalculatorCurrencies() {
        if (!target) {
            return;
        }

        changeBaseCurrency(target.code);
    }

    function resetForm() {
        setCurrencies(normalized.currencies);
        setBaseCurrency(normalized.baseCurrency);
        setDisplayCurrency(
            normalized.displayCurrency || normalized.baseCurrency,
        );
        setSelectedTarget(
            normalized.currencies.find(
                (currency) =>
                    currency.code !== normalized.baseCurrency,
            )?.code || "",
        );
        setNewCurrencyCode("");
    }

    function handleApplyLiveRate(code: string, liveRate: number) {
        updateCurrency(code, { exchangeRate: liveRate });
    }

    function handleSyncAllLiveRates(ratesMap: Record<string, number>) {
        setCurrencies((current) =>
            current.map((curr) => {
                if (curr.code === baseCurrency) return curr;
                const liveRate = ratesMap[curr.code];
                if (liveRate && liveRate > 0) {
                    return { ...curr, exchangeRate: liveRate };
                }
                return curr;
            })
        );
    }

    function handleAddAndConfigureCurrency(code: string, liveRate: number) {
        const details = getCurrencyDetails(code);
        const name = details?.name || code;
        const symbol = details?.symbol || code;

        if (!currencies.some((c) => c.code === code)) {
            setCurrencies((current) => [
                ...current,
                {
                    code,
                    name,
                    symbol,
                    exchangeRate: liveRate,
                    decimalPlaces: 2,
                },
            ]);
            setSelectedTarget(code);
        } else {
            updateCurrency(code, { exchangeRate: liveRate });
            setSelectedTarget(code);
        }
    }

    async function handleSubmit(event: FormEvent<HTMLFormElement>) {
        event.preventDefault();

        const result = businessCurrencyConfigurationSchema.safeParse({
            baseCurrency,
            displayCurrency,
            currencies,
        });

        if (!result.success) {
            toast({
                tone: "error",
                title: "Currency settings not saved",
                description:
                    result.error.issues[0]?.message ||
                    "Check the currency configuration.",
            });
            return;
        }

        try {
            const updated = await updateCurrencies(result.data).unwrap();
            const next = normalizeCurrencyConfiguration(updated);
            setCurrencies(next.currencies);
            setBaseCurrency(next.baseCurrency);
            setDisplayCurrency(next.displayCurrency || next.baseCurrency);
            setSelectedTarget(
                next.currencies.find(
                    (currency) => currency.code !== next.baseCurrency,
                )?.code || "",
            );
            toast({
                tone: "success",
                title: "Currency settings saved",
                description:
                    "Your business currency configuration is up to date.",
            });
        } catch (error) {
            toast({
                tone: "error",
                title: "Currency settings not saved",
                description: getApiErrorMessage(
                    error,
                    "Unable to save the currency configuration.",
                ),
            });
        }
    }

    return (
        <form
            onSubmit={handleSubmit}
            noValidate
            className="flex min-h-[795px] flex-col"
        >
            <section className="rounded-2xl bg-card border border-border p-4 sm:p-6 shadow-[0_4px_10px_rgba(26,34,43,0.04)] dark:shadow-[0_8px_30px_rgba(0,0,0,0.3)]">
                <SectionTitle>General Configuration</SectionTitle>

                <div className="mt-5 grid gap-4 md:grid-cols-2">
                    <div data-tour="currency-base">
                        <Label
                            htmlFor="base-currency"
                            className="mb-3 ml-1 block text-base font-medium text-foreground"
                        >
                            Base Currency
                        </Label>
                        <SelectField
                            id="base-currency"
                            value={baseCurrency}
                            onValueChange={changeBaseCurrency}
                            className="rounded-xl border-border bg-popover text-base shadow-none data-[size=default]:h-14"
                            options={currencies.map((currency) => ({
                                value: currency.code,
                                label: (
                                    <span className="flex items-center gap-2">
                                        <CurrencyFlag code={currency.code} size="xs" />
                                        <span>{currency.name} ({currency.code})</span>
                                    </span>
                                ),
                            }))}
                        />
                    </div>

                    <div data-tour="currency-decimals">
                        <Label
                            htmlFor="decimal-places"
                            className="mb-3 ml-1 block text-base font-medium text-foreground"
                        >
                            Decimal Places
                        </Label>
                        <SelectField
                            id="decimal-places"
                            value={String(base?.decimalPlaces ?? 2)}
                            onValueChange={(value) => {
                                if (value && base) {
                                    updateCurrency(base.code, {
                                        decimalPlaces: Number(value),
                                    });
                                }
                            }}
                            className="rounded-xl border-border bg-popover text-base shadow-none data-[size=default]:h-14"
                            options={[
                                { value: "0", label: "0 decimals" },
                                { value: "1", label: "1 decimal" },
                                {
                                    value: "2",
                                    label: "2 decimals (Standard)",
                                },
                                { value: "3", label: "3 decimals" },
                            ]}
                        />
                    </div>

                    <div data-tour="currency-display">
                        <Label
                            htmlFor="display-currency"
                            className="mb-3 ml-1 block text-base font-medium text-foreground"
                        >
                            Display Currency
                        </Label>
                        <SelectField
                            id="display-currency"
                            value={displayCurrency}
                            onValueChange={(value) =>
                                value && setDisplayCurrency(value)
                            }
                            className="rounded-xl border-border bg-popover text-base shadow-none data-[size=default]:h-14"
                            options={currencies.map((currency) => ({
                                value: currency.code,
                                label: (
                                    <span className="flex items-center gap-2">
                                        <CurrencyFlag code={currency.code} size="xs" />
                                        <span>{currency.name} ({currency.code})</span>
                                    </span>
                                ),
                            }))}
                        />
                        <p className="mt-2 ml-1 text-sm text-muted-foreground">
                            {displayCurrency === baseCurrency
                                ? "Matches the base currency, so no converted amount is shown."
                                : `Totals and receipts also show the ${displayCurrency} equivalent.`}
                        </p>
                    </div>
                </div>

                <div className="mt-7" data-tour="currency-list">
                    <Label
                        htmlFor="add-currency"
                        className="mb-3 ml-1 block text-base font-medium text-foreground"
                    >
                        Currencies
                    </Label>
                    <div className="flex min-h-[56px] flex-wrap items-center gap-2 rounded-xl border border-border bg-popover p-2">
                        {currencies.map((currency) => {
                            const isSelected =
                                currency.code === selectedTarget;
                            return (
                                <div
                                    key={currency.code}
                                    className={`flex h-[38px] items-center gap-2 rounded-full px-3 text-sm font-medium tracking-[0.14px] outline-none focus-visible:ring-2 focus-visible:ring-primary ${
                                        isSelected
                                            ? "bg-success/25 text-success ring-1 ring-success/40"
                                            : "bg-success/10 text-success"
                                    }`}
                                >
                                    <button
                                        type="button"
                                        onClick={() => {
                                            if (
                                                currency.code !==
                                                baseCurrency
                                            ) {
                                                setSelectedTarget(
                                                    currency.code,
                                                );
                                            }
                                        }}
                                        className="flex items-center gap-1.5 rounded-full outline-none focus-visible:ring-2 focus-visible:ring-primary"
                                    >
                                        <CurrencyFlag code={currency.code} size="xs" />
                                        <span>{currency.code}</span>
                                    </button>
                                    <button
                                        type="button"
                                        aria-label={`Remove ${currency.code}`}
                                        onClick={(event) => {
                                            event.stopPropagation();
                                            removeCurrency(
                                                currency.code,
                                            );
                                        }}
                                        className="rounded-full outline-none focus-visible:ring-2 focus-visible:ring-primary"
                                    >
                                        <X className="size-3" />
                                    </button>
                                </div>
                            );
                        })}
                        <CurrencyAutocomplete
                            value={newCurrencyCode}
                            onValueChange={setNewCurrencyCode}
                            options={availableCurrencyOptions}
                        />
                        <Button
                            type="button"
                            variant="ghost"
                            size="icon-sm"
                            onClick={addCurrency}
                            disabled={!newCurrencyCode.trim()}
                            aria-label="Add currency"
                        >
                            <Plus className="size-5" />
                        </Button>
                    </div>
                </div>
            </section>

            <section data-tour="currency-calculator" className="mt-4 rounded-2xl bg-card border border-border p-4 sm:p-6 shadow-[0_4px_10px_rgba(26,34,43,0.04)] dark:shadow-[0_8px_30px_rgba(0,0,0,0.3)]">
                <SectionTitle>Exchange Rate &amp; Calculator</SectionTitle>

                <div className="mt-5 max-w-[630px] rounded-2xl border border-border bg-muted/30 p-3.5 sm:p-6">
                    {base && target ? (
                        <>
                            <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] sm:items-end">
                                <div className="min-w-0">
                                    <div className="mb-3 flex items-center justify-between gap-3">
                                        <Label
                                            htmlFor="calculator-base-currency"
                                            className="text-lg font-semibold text-foreground/80"
                                        >
                                            Base
                                        </Label>
                                        <SelectField
                                            id="calculator-base-currency"
                                            value={base.code}
                                            onValueChange={(code) => {
                                                if (
                                                    code === target.code
                                                ) {
                                                    swapCalculatorCurrencies();
                                                } else {
                                                    changeBaseCurrency(code);
                                                }
                                            }}
                                            className="w-[100px] min-w-[92px] rounded-lg border-border bg-popover px-3 text-sm shadow-none data-[size=default]:h-9"
                                            options={currencies.map(
                                                (currency) => ({
                                                    value: currency.code,
                                                    label: currency.code,
                                                }),
                                            )}
                                        />
                                    </div>
                                    <div className="flex h-[54px] items-center gap-3 rounded-xl border border-border bg-popover px-4">
                                        <span className="text-base font-bold text-success">
                                            {base.symbol}
                                        </span>
                                        <span className="px-3 text-2xl font-bold text-foreground">
                                            1
                                        </span>
                                    </div>
                                </div>

                                <button
                                    type="button"
                                    onClick={swapCalculatorCurrencies}
                                    aria-label={`Switch ${base.code} and ${target.code}`}
                                    className="flex size-8 shrink-0 items-center justify-center justify-self-center rounded-full bg-warning/20 text-warning outline-none transition-colors hover:bg-warning/30 focus-visible:ring-2 focus-visible:ring-warning/40 sm:mb-[11px]"
                                >
                                    <ArrowLeftRight className="size-4" />
                                </button>

                                <div className="min-w-0">
                                    <div className="mb-3 flex items-center justify-between gap-3">
                                        <Label
                                            htmlFor="exchange-currency"
                                            className="text-lg font-semibold text-foreground/80"
                                        >
                                            Exchange
                                        </Label>
                                        <SelectField
                                            id="exchange-currency"
                                            value={target.code}
                                            onValueChange={setSelectedTarget}
                                            className="w-[100px] min-w-[92px] rounded-lg border-border bg-popover px-3 text-sm shadow-none data-[size=default]:h-9"
                                            options={currencies
                                                .filter(
                                                    (currency) =>
                                                        currency.code !==
                                                        baseCurrency,
                                                )
                                                .map((currency) => ({
                                                    value: currency.code,
                                                    label: currency.code,
                                                }))}
                                        />
                                    </div>
                                    <div className="flex h-[54px] items-center gap-1 rounded-xl border border-border bg-popover px-4">
                                        <span className="text-base font-bold text-success">
                                            {target.symbol}
                                        </span>
                                        <Input
                                            id="exchange-rate"
                                            aria-label={`Exchange rate for ${target.code}`}
                                            type="number"
                                            min="0"
                                            step="any"
                                            value={target.exchangeRate}
                                            onChange={(event) =>
                                                updateCurrency(
                                                    target.code,
                                                    {
                                                        exchangeRate: Number(
                                                            event.target
                                                                .value,
                                                        ),
                                                    },
                                                )
                                            }
                                            className="h-12 rounded-none border-0 bg-transparent px-3 text-2xl font-bold text-foreground shadow-none focus-visible:ring-0"
                                        />
                                    </div>
                                </div>
                            </div>
                        </>
                    ) : (
                        <div className="flex min-h-36 items-center justify-center rounded-xl bg-muted/40 border border-border px-6 text-center text-sm text-muted-foreground">
                            Add a second currency to configure an exchange
                            rate.
                        </div>
                    )}
                </div>
            </section>

            <LiveExchangeRatesSection
                baseCurrency={baseCurrency}
                configuredCurrencies={currencies}
                onApplyLiveRate={handleApplyLiveRate}
                onSyncAllLiveRates={handleSyncAllLiveRates}
                onAddAndConfigureCurrency={handleAddAndConfigureCurrency}
            />

            <div className="sticky -bottom-8 z-30 -mx-5 mt-auto flex flex-wrap items-center justify-end gap-2.5 border-t border-border bg-shell px-5 py-3.5 sm:gap-3 sm:py-4 lg:-mx-8 lg:px-8">
                <Button
                    type="button"
                    variant="outline"
                    onClick={resetForm}
                    disabled={updateState.isLoading}
                    className="h-10 flex-1 rounded-xl px-4 text-xs sm:h-11 sm:flex-initial sm:px-6 sm:text-sm"
                >
                    Cancel
                </Button>
                <Button
                    type="submit"
                    data-tour="currency-save"
                    disabled={updateState.isLoading}
                    className="h-10 flex-1 rounded-xl px-4 text-xs sm:h-11 sm:flex-initial sm:px-6 sm:text-sm"
                >
                    {updateState.isLoading ? (
                        <>
                            <LoaderCircle className="size-4 shrink-0 animate-spin" />
                            <span>Saving…</span>
                        </>
                    ) : (
                        <>
                            <Save className="size-4 shrink-0" />
                            <span>Save</span>
                        </>
                    )}
                </Button>
            </div>
        </form>
    );
}

function CurrencyQueryError({
    message,
    onRetry,
}: {
    message: string;
    onRetry: () => void;
}) {
    return (
        <div
            role="alert"
            className="rounded-2xl border border-danger/20 bg-card p-6 shadow-[0_4px_10px_rgba(26,34,43,0.04)]"
        >
            <h2 className="text-lg font-bold text-foreground">
                Unable to load currencies
            </h2>
            <p className="mt-2 text-sm text-muted-foreground">{message}</p>
            <Button
                type="button"
                onClick={onRetry}
                className="mt-5"
            >
                Try again
            </Button>
        </div>
    );
}

import { FormSkeleton } from "@/components/ui/skeleton";

export default function BusinessCurrencyForm() {
    const query = useGetBusinessCurrenciesQuery();

    if (query.isLoading) {
        return <FormSkeleton rows={5} />;
    }

    if (query.error || !query.data) {
        return (
            <CurrencyQueryError
                message={getApiErrorMessage(
                    query.error,
                    "Your currencies could not be loaded. Check the connection and try again.",
                )}
                onRetry={() => void query.refetch()}
            />
        );
    }

    const configuration = normalizeCurrencyConfiguration(query.data);
    const editorKey = [
        configuration.baseCurrency,
        configuration.displayCurrency,
        ...configuration.currencies.flatMap((currency) => [
            currency.code,
            currency.name,
            currency.symbol,
            currency.exchangeRate,
            currency.decimalPlaces,
        ]),
    ].join("|");

    return (
        <CurrencyEditor
            key={editorKey}
            initialConfiguration={configuration}
        />
    );
}
