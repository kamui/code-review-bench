"use client";

import Image from "next/image";
import { useState, useEffect, useCallback, useMemo } from "react";
import { useSearchParams } from "next/navigation";
import { useForm, useFieldArray, type Resolver } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import * as z from "zod";
import axios from "axios";
import { Popover } from "@base-ui/react/popover";
import { format } from "date-fns";
import { DayPicker } from "react-day-picker";
import { Button } from "./ui/button";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "./ui/form";
import { Input } from "./ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import { Card, CardContent } from "./ui/card";
import { useToast } from "@/hooks/use-toast";
import { getAllStates, getDistricts } from "india-state-district";
import {
  BERTH_TYPES,
  SUPPORT_CONTACT_NUMBER,
  TRAIN_CONFIG,
} from "@/lib/constants";
import { normalizeAadhaar } from "@/lib/utils";
import { CalendarDays, FileText, X } from "lucide-react";
import { BookingSuccess } from "./BookingSuccess";
import { submitBooking, fetchReferenceMembers } from "@/lib/api";

const PAYMENT_MODE_OPTIONS = [
  "UPI",
  "Bank Transfer",
  "Net Banking",
  "Credit Card",
  "Debit Card",
  "Cash",
  "Other",
] as const;

const PAYMENT_MODES_REQUIRING_TRANSACTION = [
  "UPI",
  "Bank Transfer",
  "Net Banking",
  "Credit Card",
  "Debit Card",
] as const;

const PAYMENT_MODES_REQUIRING_PROOF = PAYMENT_MODES_REQUIRING_TRANSACTION;

const PENDING_PAYMENT_OPTIONS = ["FULL_PAID", "BALANCE_5000"] as const;

const GENDER_OPTIONS = ["Male", "Female", "Other"] as const;
const INDIAN_STATES = getAllStates();
const STATE_CODE_BY_NAME = Object.fromEntries(
  INDIAN_STATES.map((state) => [state.name.toLowerCase(), state.code]),
);
const INDIAN_STATE_NAMES = INDIAN_STATES.map((state) => state.name).sort();

const NATION_OPTIONS = ["India"] as const;

const RELATIONSHIP_OPTIONS_BY_GENDER: Record<string, string[]> = {
  Male: [
    "Brother",
    "Son",
    "Father",
    "Husband",
    "Uncle",
    "Cousin Brother",
    "Friend",
    "Other",
  ],
  Female: [
    "Sister",
    "Daughter",
    "Mother",
    "Wife",
    "Aunt",
    "Cousin Sister",
    "Friend",
    "Other",
  ],
  Other: ["Sibling", "Child", "Parent", "Spouse", "Cousin", "Friend", "Other"],
};

function calculateAgeFromDob(dob: string): number {
  if (!dob) {
    return 0;
  }

  const [dayString, monthString, yearString] = dob.split("/");
  const day = Number(dayString);
  const month = Number(monthString);
  const year = Number(yearString);

  if (
    !Number.isInteger(day) ||
    !Number.isInteger(month) ||
    !Number.isInteger(year)
  ) {
    return 0;
  }

  const birthDate = new Date(year, month - 1, day);
  if (
    birthDate.getFullYear() !== year ||
    birthDate.getMonth() !== month - 1 ||
    birthDate.getDate() !== day
  ) {
    return 0;
  }

  const today = new Date();
  let age = today.getFullYear() - year;
  const hasBirthdayOccurred =
    today.getMonth() > month - 1 ||
    (today.getMonth() === month - 1 && today.getDate() >= day);

  if (!hasBirthdayOccurred) {
    age -= 1;
  }

  return age >= 0 ? age : 0;
}

function parseDobToDate(dob: string): Date | undefined {
  if (!dob) {
    return undefined;
  }

  const [dayString, monthString, yearString] = dob.split("/");
  const day = Number(dayString);
  const month = Number(monthString);
  const year = Number(yearString);

  if (
    !Number.isInteger(day) ||
    !Number.isInteger(month) ||
    !Number.isInteger(year)
  ) {
    return undefined;
  }

  const parsedDate = new Date(year, month - 1, day);
  if (
    parsedDate.getFullYear() !== year ||
    parsedDate.getMonth() !== month - 1 ||
    parsedDate.getDate() !== day
  ) {
    return undefined;
  }

  return parsedDate;
}

function formatDateToDob(date: Date): string {
  return format(date, "dd/MM/yyyy");
}

function DobCalendarField({
  value,
  onSelect,
}: {
  value: string;
  onSelect: (nextDob: string) => void;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const selectedDate = parseDobToDate(value);
  const maxDate = new Date();
  const minDate = new Date(1900, 0, 1);

  return (
    <Popover.Root open={isOpen} onOpenChange={setIsOpen}>
      <Popover.Trigger className="flex h-11 w-full items-center justify-between rounded-lg border border-slate-300 bg-white px-3 text-left text-sm shadow-sm transition-colors hover:border-slate-400 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50">
        <span
          className={selectedDate ? "text-foreground" : "text-muted-foreground"}
        >
          {selectedDate
            ? format(selectedDate, "dd/MM/yyyy")
            : "Pick date of birth"}
        </span>
        <CalendarDays className="size-4 text-slate-500" />
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Positioner side="bottom" align="start" sideOffset={8}>
          <Popover.Popup className="z-50 rounded-xl border bg-white p-3 shadow-xl ring-1 ring-slate-200">
            <DayPicker
              mode="single"
              selected={selectedDate}
              onSelect={(nextDate) => {
                if (!nextDate) {
                  return;
                }
                onSelect(formatDateToDob(nextDate));
                setIsOpen(false);
              }}
              captionLayout="dropdown"
              defaultMonth={selectedDate || new Date(2000, 0, 1)}
              startMonth={minDate}
              endMonth={maxDate}
              disabled={{ before: minDate, after: maxDate }}
              className="text-sm"
              classNames={{
                month_caption: "text-sm font-medium",
                caption_label: "hidden",
                nav: "hidden",
                dropdown_root:
                  "rounded-md border border-slate-300 bg-white px-2 py-1 text-sm",
                weekday: "w-9 text-center text-xs font-medium text-slate-500",
                day: "size-9 rounded-md p-0 font-normal hover:bg-slate-100",
                selected:
                  "bg-blue-600 text-white hover:bg-blue-600 hover:text-white",
                today: "ring-1 ring-blue-300",
              }}
            />
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}

const accessibilitySupportFieldSchema = z
  .string()
  .min(1, "Please select accessibility support requirement")
  .refine((value) => ["no", "yes"].includes(value), {
    message: "Please select accessibility support requirement",
  });

const passengerSchema = z.object({
  name: z.string().min(2, "Name is too short").max(50),
  dob: z
    .string()
    .regex(
      /^(0[1-9]|[12][0-9]|3[01])\/(0[1-9]|1[0-2])\/\d{4}$/,
      "Date of Birth must be in DD/MM/YYYY format",
    ),
  age: z.coerce.number().min(1, "Invalid age").max(120),
  gender: z.enum(["Male", "Female", "Other"]),
  seatPreference: z.enum(["LB", "MB", "UB", "SL", "SU", "No Preference"]),
  requiresAccessibilitySupport: accessibilitySupportFieldSchema,
  accessibilityNote: z.string().optional(),
});

const aadhaarFieldSchema = z
  .string()
  .refine(
    (value) => normalizeAadhaar(value).length === 12,
    "Aadhaar must be 12 digits",
  );

const genderFieldSchema = z
  .string()
  .min(1, "Please select gender")
  .refine(
    (value) =>
      GENDER_OPTIONS.includes(value as (typeof GENDER_OPTIONS)[number]),
    {
      message: "Please select a valid gender",
    },
  );

const formSchema = z
  .object({
    primaryPassenger: passengerSchema.extend({
      gender: genderFieldSchema,
      phone: z.string().regex(/^\d{10}$/, "Phone must be 10 digits"),
      emergencyContactNumber: z
        .string()
        .regex(/^\d{10}$/, "Emergency contact must be 10 digits"),
      aadhaar: aadhaarFieldSchema,
      street: z.string().min(3, "Please enter street / address"),
      nation: z.string().min(2, "Please select nation"),
      state: z.string().min(2, "Please select state"),
      district: z.string().min(2, "Please select district"),
      roomPreference: z
        .string()
        .refine((value) => ["single", "group"].includes(value), {
          message: "Please select room preference.",
        }),
      referenceMember: z.string().optional(),
    }),
    groupMembers: z
      .array(
        passengerSchema.extend({
          gender: genderFieldSchema,
          aadhaar: aadhaarFieldSchema,
          relationship: z.string().min(1, "Please select relationship"),
        }),
      )
      .max(
        TRAIN_CONFIG.MAX_GROUP_SIZE - 1,
        `Max group size is ${TRAIN_CONFIG.MAX_GROUP_SIZE}`,
      ),
    paymentMode: z.enum(PAYMENT_MODE_OPTIONS),
    transactionIdUtr: z.string().optional(),
    paymentPendingStatus: z.enum(PENDING_PAYMENT_OPTIONS).optional(),
    paymentProof: z.any().optional(),
  })
  .superRefine((values, ctx) => {
    const seenAadhaars = new Set<string>();
    const primaryAadhaar = normalizeAadhaar(values.primaryPassenger.aadhaar);

    seenAadhaars.add(primaryAadhaar);

    values.groupMembers.forEach((member, index) => {
      const memberAadhaar = normalizeAadhaar(member.aadhaar);

      if (seenAadhaars.has(memberAadhaar)) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["groupMembers", index, "aadhaar"],
          message: "This Aadhaar number is already used in this booking.",
        });
        return;
      }

      seenAadhaars.add(memberAadhaar);

      if (
        member.requiresAccessibilitySupport === "yes" &&
        (!member.accessibilityNote ||
          member.accessibilityNote.trim().length < 4)
      ) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["groupMembers", index, "accessibilityNote"],
          message:
            "Please add a medical/accessibility support note for this passenger.",
        });
      }
    });

    if (
      values.primaryPassenger.requiresAccessibilitySupport === "yes" &&
      (!values.primaryPassenger.accessibilityNote ||
        values.primaryPassenger.accessibilityNote.trim().length < 4)
    ) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["primaryPassenger", "accessibilityNote"],
        message:
          "Please add a medical/accessibility support note for this passenger.",
      });
    }

    if (!values.paymentPendingStatus) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["paymentPendingStatus"],
        message: "Please choose pending amount status.",
      });
    }

    if (
      PAYMENT_MODES_REQUIRING_TRANSACTION.includes(
        values.paymentMode as (typeof PAYMENT_MODES_REQUIRING_TRANSACTION)[number],
      )
    ) {
      if (
        !values.transactionIdUtr ||
        values.transactionIdUtr.trim().length < 6
      ) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["transactionIdUtr"],
          message: "Please enter a valid transaction reference.",
        });
      }
    }

    if (
      PAYMENT_MODES_REQUIRING_PROOF.includes(
        values.paymentMode as (typeof PAYMENT_MODES_REQUIRING_PROOF)[number],
      )
    ) {
      if (
        !values.paymentProof ||
        !(values.paymentProof instanceof FileList) ||
        values.paymentProof.length === 0
      ) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["paymentProof"],
          message: "Please upload payment proof.",
        });
      }
    }
  });

export type BookingFormValues = z.infer<typeof formSchema>;

export function BookingForm() {
  const searchParams = useSearchParams();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isSuccess, setIsSuccess] = useState(false);
  const [referenceMembers, setReferenceMembers] = useState<
    Array<{ id: number; name: string }>
  >([]);
  const [loadingMembers, setLoadingMembers] = useState(true);
  const [isReferenceLocked, setIsReferenceLocked] = useState(false);
  const [paymentProofPreview, setPaymentProofPreview] = useState<string | null>(
    null,
  );
  const [paymentUploadKey, setPaymentUploadKey] = useState(0);
  const { toast } = useToast();

  // Load reference members on component mount
  useEffect(() => {
    const loadReferenceMembers = async () => {
      try {
        const members = await fetchReferenceMembers();
        setReferenceMembers(members);
      } catch (error) {
        console.error("Failed to load reference members:", error);
        // Fallback to empty list on error
        setReferenceMembers([]);
        toast({
          title: "Warning",
          description: "Could not load reference members list",
          variant: "destructive",
        });
      } finally {
        setLoadingMembers(false);
      }
    };
    loadReferenceMembers();
  }, [toast]);

  const form = useForm<BookingFormValues>({
    resolver: zodResolver(formSchema) as Resolver<BookingFormValues>,
    defaultValues: {
      primaryPassenger: {
        name: "",
        dob: "",
        age: 0,
        gender: "",
        seatPreference: "No Preference",
        requiresAccessibilitySupport: "",
        accessibilityNote: "",
        phone: "",
        emergencyContactNumber: "",
        aadhaar: "",
        street: "",
        nation: "India",
        state: "",
        district: "",
        roomPreference: "",
        referenceMember: "", // default
      },
      groupMembers: [],
      paymentMode: undefined,
      transactionIdUtr: "",
      paymentPendingStatus: undefined,
    },
  });

  const {
    fields: memberFields,
    append: appendMember,
    remove: removeMember,
  } = useFieldArray({
    control: form.control,
    name: "groupMembers",
  });

  const paymentMode = form.watch("paymentMode");
  const isCashPayment = paymentMode === "Cash";
  const shouldShowTransactionField = paymentMode !== "Cash";
  const shouldShowPaymentProofField = paymentMode !== "Cash";
  const isTransactionRequired = PAYMENT_MODES_REQUIRING_TRANSACTION.includes(
    paymentMode as (typeof PAYMENT_MODES_REQUIRING_TRANSACTION)[number],
  );
  const isPaymentProofRequired = PAYMENT_MODES_REQUIRING_PROOF.includes(
    paymentMode as (typeof PAYMENT_MODES_REQUIRING_PROOF)[number],
  );

  const transactionFieldLabel = useMemo(() => {
    if (paymentMode === "UPI") {
      return "UPI Transaction ID";
    }

    if (["Bank Transfer", "Net Banking"].includes(paymentMode || "")) {
      return "UTR / Reference Number";
    }

    if (["Credit Card", "Debit Card"].includes(paymentMode || "")) {
      return "Transaction Reference Number";
    }

    return "Transaction ID / Reference Number";
  }, [paymentMode]);
  const selectedNation = form.watch("primaryPassenger.nation");
  const selectedState = form.watch("primaryPassenger.state");
  const totalPassengers = 1 + memberFields.length;

  const normalizedNation = (selectedNation || "").trim().toLowerCase();
  const isIndiaSelected = normalizedNation === "india";
  const availableStates = useMemo(
    () => (isIndiaSelected ? [...INDIAN_STATE_NAMES] : []),
    [isIndiaSelected],
  );

  const matchedStateName = useMemo(
    () =>
      availableStates.find(
        (stateName) =>
          stateName.toLowerCase() ===
          (selectedState || "").trim().toLowerCase(),
      ) || "",
    [availableStates, selectedState],
  );

  const availableDistricts = useMemo(() => {
    if (!isIndiaSelected || !matchedStateName) {
      return [];
    }

    const stateCode = STATE_CODE_BY_NAME[matchedStateName.toLowerCase()];
    if (!stateCode) {
      return [];
    }

    return getDistricts(stateCode).sort((left, right) =>
      left.localeCompare(right),
    );
  }, [isIndiaSelected, matchedStateName]);

  const referenceQuery = (
    form.watch("primaryPassenger.referenceMember") || ""
  ).trim();
  const hasExactReferenceMatch = referenceMembers.some(
    (member) => member.name.toLowerCase() === referenceQuery.toLowerCase(),
  );
  const filteredReferenceMembers = referenceMembers
    .filter((member) =>
      member.name.toLowerCase().includes(referenceQuery.toLowerCase()),
    )
    .slice(0, 12);

  const resetBookingForm = useCallback(() => {
    form.reset({
      primaryPassenger: {
        name: "",
        dob: "",
        age: 0,
        gender: "",
        seatPreference: "No Preference",
        requiresAccessibilitySupport: "",
        accessibilityNote: "",
        phone: "",
        emergencyContactNumber: "",
        aadhaar: "",
        street: "",
        nation: "India",
        state: "",
        district: "",
        roomPreference: "",
        referenceMember: "",
      },
      groupMembers: [],
      paymentMode: undefined,
      transactionIdUtr: "",
      paymentPendingStatus: undefined,
      paymentProof: undefined,
    });
    setPaymentProofPreview(null);
    setPaymentUploadKey((prev) => prev + 1);
    setIsReferenceLocked(false);
  }, [form]);

  useEffect(() => {
    const shouldReset = searchParams.get("reset");
    if (shouldReset) {
      resetBookingForm();
      setIsSuccess(false);
    }
  }, [searchParams, resetBookingForm]);

  useEffect(() => {
    if (!isIndiaSelected) {
      if (form.getValues("primaryPassenger.state")) {
        form.setValue("primaryPassenger.state", "", { shouldValidate: false });
      }
      if (form.getValues("primaryPassenger.district")) {
        form.setValue("primaryPassenger.district", "", {
          shouldValidate: false,
        });
      }
      return;
    }

    if (!matchedStateName && form.getValues("primaryPassenger.district")) {
      form.setValue("primaryPassenger.district", "", { shouldValidate: false });
    }
  }, [form, isIndiaSelected, matchedStateName]);

  useEffect(() => {
    if (
      selectedState &&
      matchedStateName &&
      form.getValues("primaryPassenger.district") &&
      !availableDistricts.some(
        (districtName) =>
          districtName.toLowerCase() ===
          form.getValues("primaryPassenger.district").toLowerCase(),
      )
    ) {
      form.setValue("primaryPassenger.district", "", { shouldValidate: false });
    }
  }, [availableDistricts, form, matchedStateName, selectedState]);

  useEffect(() => {
    if (isCashPayment) {
      form.setValue("transactionIdUtr", "", { shouldValidate: false });
      form.setValue("paymentProof", undefined, { shouldValidate: false });
      setPaymentProofPreview(null);
      setPaymentUploadKey((prev) => prev + 1);
    }
  }, [form, isCashPayment]);

  const onSubmit = async (data: BookingFormValues) => {
    try {
      setIsSubmitting(true);
      const response = await submitBooking(data);
      setIsSuccess(true);
      toast({
        title: "Booking Submitted Successfully! ✓",
        description:
          "Your seat request has been placed. You will receive a confirmation call shortly.",
        variant: "default",
      });
      console.log("Booking response:", response);
    } catch (error: unknown) {
      let errorMessage =
        "There was an error submitting your request. Please try again.";

      if (axios.isAxiosError(error)) {
        if (error.response?.data && typeof error.response.data === "object") {
          const responseData = error.response.data as { error?: string };
          if (responseData.error) {
            errorMessage = responseData.error;
          }
        }

        if (!errorMessage && error.message) {
          errorMessage = error.message;
        }

        if (error.response?.status === 409) {
          errorMessage =
            (error.response?.data as { error?: string } | undefined)?.error ||
            "Duplicate value found. Please use a unique Aadhaar or transaction reference.";
        } else {
          console.error("Booking submission error:", error);
        }
      } else {
        console.error("Booking submission error:", error);
        if (error instanceof Error && error.message) {
          errorMessage = error.message;
        }
      }

      toast({
        title: "Booking Failed ✗",
        description: errorMessage,
        variant: "destructive",
      });
    } finally {
      setIsSubmitting(false);
    }
  };

  if (isSuccess) {
    return (
      <BookingSuccess
        onReset={() => {
          resetBookingForm();
          setIsSuccess(false);
        }}
      />
    );
  }

  return (
    <Card className="w-full max-w-4xl mx-auto shadow-lg border-0 rounded-xl overflow-hidden">
      <div className="bg-blue-600 px-6 py-8 text-white">
        <div className="flex items-center space-x-3 mb-2">
          <Image
            src="/logo.jpeg"
            alt="J Tourism logo"
            width={52}
            height={52}
            className="w-12 h-12 rounded-md object-cover bg-white p-1"
          />
          <h2 className="text-2xl font-bold">Book Train Seats</h2>
        </div>
        <p className="text-blue-100 opacity-90">
          Complete the form below to request seats for your tourism travel.
        </p>
      </div>

      <CardContent className="p-6 sm:p-8 bg-slate-50">
        <Form {...form}>
          <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-10">
            {/* 1. Primary Passenger Details */}
            <div className="space-y-6">
              <div className="flex items-center space-x-2 border-b pb-2">
                <span className="bg-blue-100 text-blue-700 w-8 h-8 rounded-full flex items-center justify-center font-bold text-sm">
                  1
                </span>
                <h3 className="text-lg font-semibold text-slate-800">
                  Primary Passenger Details
                </h3>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <FormField
                  control={form.control}
                  name="primaryPassenger.name"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Full name (as per Aadhaar)</FormLabel>
                      <FormControl>
                        <Input
                          placeholder="Enter full name as per Aadhaar"
                          {...field}
                        />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <div className="grid grid-cols-2 gap-4">
                  <FormField
                    control={form.control}
                    name="primaryPassenger.dob"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Date of Birth (DD/MM/YYYY)</FormLabel>
                        <FormControl>
                          <DobCalendarField
                            value={field.value}
                            onSelect={(nextDob) => {
                              field.onChange(nextDob);
                              form.setValue(
                                "primaryPassenger.age",
                                calculateAgeFromDob(nextDob),
                                { shouldValidate: true },
                              );
                            }}
                          />
                        </FormControl>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                  <FormField
                    control={form.control}
                    name="primaryPassenger.age"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Age</FormLabel>
                        <FormControl>
                          <Input
                            type="number"
                            placeholder="Auto-calculated from DOB"
                            readOnly
                            {...field}
                          />
                        </FormControl>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                  <FormField
                    control={form.control}
                    name="primaryPassenger.gender"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Gender</FormLabel>
                        <Select
                          onValueChange={field.onChange}
                          value={field.value}
                        >
                          <FormControl>
                            <SelectTrigger>
                              <SelectValue placeholder="Select gender" />
                            </SelectTrigger>
                          </FormControl>
                          <SelectContent>
                            {GENDER_OPTIONS.map((option) => (
                              <SelectItem key={option} value={option}>
                                {option}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                </div>

                <FormField
                  control={form.control}
                  name="primaryPassenger.phone"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Phone Number</FormLabel>
                      <FormControl>
                        <Input
                          placeholder="Enter 10-digit mobile number"
                          {...field}
                        />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="primaryPassenger.emergencyContactNumber"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Emergency Contact Number</FormLabel>
                      <FormControl>
                        <Input
                          placeholder="Enter emergency 10-digit contact number"
                          inputMode="numeric"
                          {...field}
                        />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="primaryPassenger.aadhaar"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Aadhaar Number</FormLabel>
                      <FormControl>
                        <Input
                          placeholder="Enter 12-digit Aadhaar number"
                          inputMode="numeric"
                          {...field}
                        />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="primaryPassenger.street"
                  render={({ field }) => (
                    <FormItem className="md:col-span-2">
                      <FormLabel>Street / Address</FormLabel>
                      <FormControl>
                        <Input placeholder="Enter street and area" {...field} />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="primaryPassenger.nation"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Nation</FormLabel>
                      <FormControl>
                        <Input
                          list="nation-options"
                          placeholder="Search or select nation"
                          autoComplete="off"
                          className="h-11 rounded-lg border-slate-300 bg-white shadow-sm"
                          {...field}
                        />
                      </FormControl>
                      <datalist id="nation-options">
                        {NATION_OPTIONS.map((nation) => (
                          <option key={nation} value={nation} />
                        ))}
                      </datalist>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="primaryPassenger.state"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>
                        State {isIndiaSelected ? "(India)" : ""}
                      </FormLabel>
                      <FormControl>
                        <Input
                          list="india-state-options"
                          placeholder={
                            isIndiaSelected
                              ? "Search or select state"
                              : "Select India as nation first"
                          }
                          autoComplete="off"
                          disabled={!isIndiaSelected}
                          className="h-11 rounded-lg border-slate-300 bg-white shadow-sm"
                          {...field}
                        />
                      </FormControl>
                      <datalist id="india-state-options">
                        {availableStates.map((stateName) => (
                          <option key={stateName} value={stateName} />
                        ))}
                      </datalist>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="primaryPassenger.district"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>District</FormLabel>
                      <FormControl>
                        <Input
                          list="district-options"
                          placeholder={
                            isIndiaSelected
                              ? "Search or select district"
                              : "Select India and state first"
                          }
                          autoComplete="off"
                          disabled={!isIndiaSelected || !matchedStateName}
                          className="h-11 rounded-lg border-slate-300 bg-white shadow-sm"
                          {...field}
                        />
                      </FormControl>
                      <datalist id="district-options">
                        {availableDistricts.map((districtName) => (
                          <option key={districtName} value={districtName} />
                        ))}
                      </datalist>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="primaryPassenger.seatPreference"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Seat Preference</FormLabel>
                      <Select
                        onValueChange={field.onChange}
                        value={field.value}
                      >
                        <FormControl>
                          <SelectTrigger>
                            <SelectValue placeholder="Preference" />
                          </SelectTrigger>
                        </FormControl>
                        <SelectContent>
                          <SelectItem value="No Preference">
                            No Preference
                          </SelectItem>
                          {Object.entries(BERTH_TYPES).map(([key, value]) => (
                            <SelectItem key={key} value={key}>
                              {value} ({key})
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="primaryPassenger.roomPreference"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Room Preference</FormLabel>
                      <Select
                        onValueChange={field.onChange}
                        value={field.value}
                      >
                        <FormControl>
                          <SelectTrigger>
                            <SelectValue placeholder="Select room preference" />
                          </SelectTrigger>
                        </FormControl>
                        <SelectContent>
                          <SelectItem value="single">Single</SelectItem>
                          <SelectItem value="group">Group</SelectItem>
                        </SelectContent>
                      </Select>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                <FormField
                  control={form.control}
                  name="primaryPassenger.requiresAccessibilitySupport"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Requires accessibility support?</FormLabel>
                      <Select
                        onValueChange={field.onChange}
                        value={field.value}
                      >
                        <FormControl>
                          <SelectTrigger>
                            <SelectValue placeholder="Select option" />
                          </SelectTrigger>
                        </FormControl>
                        <SelectContent>
                          <SelectItem value="no">No</SelectItem>
                          <SelectItem value="yes">Yes</SelectItem>
                        </SelectContent>
                      </Select>
                      <FormMessage />
                    </FormItem>
                  )}
                />

                {form.watch("primaryPassenger.requiresAccessibilitySupport") ===
                  "yes" && (
                  <FormField
                    control={form.control}
                    name="primaryPassenger.accessibilityNote"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>
                          Medical / Accessibility Support Note
                        </FormLabel>
                        <FormControl>
                          <Input
                            placeholder="Enter assistance details (optional equipment, mobility support, etc.)"
                            {...field}
                          />
                        </FormControl>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                )}

                <FormField
                  control={form.control}
                  name="primaryPassenger.referenceMember"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Reference Member (Optional)</FormLabel>
                      <FormControl>
                        <div className="space-y-2">
                          <Input
                            placeholder={
                              loadingMembers
                                ? "Loading reference members..."
                                : "Type to search reference members"
                            }
                            value={field.value || ""}
                            onChange={(e) => {
                              setIsReferenceLocked(false);
                              field.onChange(e.target.value);
                            }}
                            disabled={loadingMembers || isReferenceLocked}
                          />

                          {isReferenceLocked && (
                            <div className="flex items-center justify-between rounded-md border bg-slate-50 px-3 py-2 text-sm">
                              <span className="text-slate-600">
                                Selected reference member
                              </span>
                              <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={() => setIsReferenceLocked(false)}
                              >
                                Change
                              </Button>
                            </div>
                          )}

                          {!loadingMembers &&
                            !isReferenceLocked &&
                            referenceQuery.length > 0 && (
                              <div className="max-h-40 overflow-y-auto rounded-md border bg-white p-1">
                                {filteredReferenceMembers.length > 0 ? (
                                  filteredReferenceMembers.map((member) => (
                                    <button
                                      key={member.id}
                                      type="button"
                                      className="w-full rounded px-3 py-2 text-left text-sm hover:bg-slate-100"
                                      onClick={() => {
                                        field.onChange(member.name);
                                        setIsReferenceLocked(true);
                                      }}
                                    >
                                      {member.name}
                                    </button>
                                  ))
                                ) : (
                                  <p className="px-3 py-2 text-sm text-slate-500">
                                    {hasExactReferenceMatch
                                      ? "Reference selected"
                                      : "No matches found. New name will be added on submit."}
                                  </p>
                                )}
                              </div>
                            )}
                        </div>
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />
              </div>
            </div>

            {/* 2. Group Members */}
            <div className="space-y-6">
              <div className="flex items-center justify-between border-b pb-2">
                <div className="flex items-center space-x-2">
                  <span className="bg-blue-100 text-blue-700 w-8 h-8 rounded-full flex items-center justify-center font-bold text-sm">
                    2
                  </span>
                  <h3 className="text-lg font-semibold text-slate-800">
                    Group Members
                  </h3>
                </div>
                <div className="text-sm text-slate-500 font-medium">
                  Total: {totalPassengers} / {TRAIN_CONFIG.MAX_GROUP_SIZE}
                </div>
              </div>

              {memberFields.length === 0 && (
                <div className="text-center py-6 bg-white border border-dashed rounded-lg">
                  <p className="text-slate-500 mb-4">
                    Are you traveling with family or friends?
                  </p>
                </div>
              )}

              {memberFields.map((field, index) => (
                <div
                  key={field.id}
                  className="relative p-6 bg-white border rounded-xl shadow-sm mb-4"
                >
                  <button
                    type="button"
                    onClick={() => removeMember(index)}
                    className="absolute top-4 right-4 text-slate-400 hover:text-red-500 text-sm font-semibold transition"
                  >
                    Remove
                  </button>
                  <h4 className="font-semibold text-slate-700 mb-4 text-sm uppercase tracking-wider">
                    Member {index + 1}
                  </h4>

                  <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                    <FormField
                      control={form.control}
                      name={`groupMembers.${index}.name`}
                      render={({ field }) => (
                        <FormItem className="lg:col-span-2">
                          <FormLabel className="text-xs">
                            Full name (as per Aadhaar)
                          </FormLabel>
                          <FormControl>
                            <Input
                              placeholder="Enter member full name"
                              {...field}
                            />
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                    <FormField
                      control={form.control}
                      name={`groupMembers.${index}.aadhaar`}
                      render={({ field }) => (
                        <FormItem className="lg:col-span-2">
                          <FormLabel className="text-xs">
                            Aadhaar Number
                          </FormLabel>
                          <FormControl>
                            <Input
                              placeholder="Enter 12-digit Aadhaar number"
                              inputMode="numeric"
                              {...field}
                            />
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                    <FormField
                      control={form.control}
                      name={`groupMembers.${index}.dob`}
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel className="text-xs">
                            Date of Birth (DD/MM/YYYY)
                          </FormLabel>
                          <FormControl>
                            <DobCalendarField
                              value={field.value}
                              onSelect={(nextDob) => {
                                field.onChange(nextDob);
                                form.setValue(
                                  `groupMembers.${index}.age`,
                                  calculateAgeFromDob(nextDob),
                                  { shouldValidate: true },
                                );
                              }}
                            />
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                    <FormField
                      control={form.control}
                      name={`groupMembers.${index}.age`}
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel className="text-xs">Age</FormLabel>
                          <FormControl>
                            <Input
                              type="number"
                              placeholder="Auto-calculated"
                              readOnly
                              {...field}
                            />
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                    <FormField
                      control={form.control}
                      name={`groupMembers.${index}.gender`}
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel className="text-xs">Gender</FormLabel>
                          <Select
                            onValueChange={(value) => {
                              field.onChange(value);
                              form.setValue(
                                `groupMembers.${index}.relationship`,
                                "",
                                { shouldValidate: false, shouldDirty: true },
                              );
                              form.clearErrors(
                                `groupMembers.${index}.relationship`,
                              );
                            }}
                            value={field.value}
                          >
                            <FormControl>
                              <SelectTrigger>
                                <SelectValue placeholder="Select gender" />
                              </SelectTrigger>
                            </FormControl>
                            <SelectContent>
                              {GENDER_OPTIONS.map((option) => (
                                <SelectItem key={option} value={option}>
                                  {option}
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                    <FormField
                      control={form.control}
                      name={`groupMembers.${index}.seatPreference`}
                      render={({ field }) => (
                        <FormItem className="lg:col-span-2">
                          <FormLabel className="text-xs">Preference</FormLabel>
                          <Select
                            onValueChange={field.onChange}
                            value={field.value}
                          >
                            <FormControl>
                              <SelectTrigger>
                                <SelectValue placeholder="Preference" />
                              </SelectTrigger>
                            </FormControl>
                            <SelectContent>
                              <SelectItem value="No Preference">No</SelectItem>
                              {Object.entries(BERTH_TYPES).map(([key]) => (
                                <SelectItem key={key} value={key}>
                                  {key}
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                          <FormMessage />
                        </FormItem>
                      )}
                    />

                    <FormField
                      control={form.control}
                      name={`groupMembers.${index}.relationship`}
                      render={({ field }) => {
                        const selectedGender =
                          form.watch(`groupMembers.${index}.gender`) || "Other";
                        const relationshipOptions =
                          RELATIONSHIP_OPTIONS_BY_GENDER[selectedGender] ||
                          RELATIONSHIP_OPTIONS_BY_GENDER.Other;

                        return (
                          <FormItem>
                            <FormLabel className="text-xs">
                              Relationship
                            </FormLabel>
                            <Select
                              key={`${index}-${selectedGender}`}
                              onValueChange={field.onChange}
                              value={field.value}
                            >
                              <FormControl>
                                <SelectTrigger>
                                  <SelectValue placeholder="Select relationship" />
                                </SelectTrigger>
                              </FormControl>
                              <SelectContent>
                                {relationshipOptions.map((option) => (
                                  <SelectItem key={option} value={option}>
                                    {option}
                                  </SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                            <FormMessage />
                          </FormItem>
                        );
                      }}
                    />

                    <FormField
                      control={form.control}
                      name={`groupMembers.${index}.requiresAccessibilitySupport`}
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel className="text-xs">
                            Requires accessibility support?
                          </FormLabel>
                          <Select
                            onValueChange={field.onChange}
                            value={field.value}
                          >
                            <FormControl>
                              <SelectTrigger>
                                <SelectValue placeholder="Select option" />
                              </SelectTrigger>
                            </FormControl>
                            <SelectContent>
                              <SelectItem value="no">No</SelectItem>
                              <SelectItem value="yes">Yes</SelectItem>
                            </SelectContent>
                          </Select>
                          <FormMessage />
                        </FormItem>
                      )}
                    />

                    {form.watch(
                      `groupMembers.${index}.requiresAccessibilitySupport`,
                    ) === "yes" && (
                      <FormField
                        control={form.control}
                        name={`groupMembers.${index}.accessibilityNote`}
                        render={({ field }) => (
                          <FormItem className="lg:col-span-2">
                            <FormLabel className="text-xs">
                              Medical / Accessibility Support Note
                            </FormLabel>
                            <FormControl>
                              <Input
                                placeholder="Enter assistance details for this member"
                                {...field}
                              />
                            </FormControl>
                            <FormMessage />
                          </FormItem>
                        )}
                      />
                    )}
                  </div>
                </div>
              ))}

              <Button
                type="button"
                variant="outline"
                className="w-full border-dashed border-2 hover:bg-blue-50 hover:border-blue-200 hover:text-blue-700 transition-colors"
                onClick={() => {
                  if (totalPassengers < TRAIN_CONFIG.MAX_GROUP_SIZE) {
                    appendMember({
                      name: "",
                      dob: "",
                      age: 0,
                      gender: "",
                      relationship: "",
                      aadhaar: "",
                      seatPreference: "No Preference",
                      requiresAccessibilitySupport: "",
                      accessibilityNote: "",
                    });
                  } else {
                    toast({
                      title: "Group Full",
                      description: `Available seats are less than the requested group size. Please contact our coordinator at ${SUPPORT_CONTACT_NUMBER}`,
                      variant: "destructive",
                    });
                  }
                }}
              >
                + Add Member
              </Button>
            </div>

            {/* 3. Payment Details */}
            <div className="space-y-6">
              <div className="flex items-center space-x-2 border-b pb-2">
                <span className="bg-blue-100 text-blue-700 w-8 h-8 rounded-full flex items-center justify-center font-bold text-sm">
                  3
                </span>
                <h3 className="text-lg font-semibold text-slate-800">
                  Payment
                </h3>
              </div>

              {/* Payment Details */}
              <div className="bg-white p-6 rounded-xl border shadow-sm space-y-6">
                <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                  <FormField
                    control={form.control}
                    name="paymentPendingStatus"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Amount Pending Status</FormLabel>
                        <Select
                          onValueChange={field.onChange}
                          value={field.value || ""}
                        >
                          <FormControl>
                            <SelectTrigger>
                              <SelectValue placeholder="Select pending status" />
                            </SelectTrigger>
                          </FormControl>
                          <SelectContent>
                            <SelectItem value="FULL_PAID">
                              Full Amount Paid
                            </SelectItem>
                            <SelectItem value="BALANCE_5000">
                              5000 Balance Pending
                            </SelectItem>
                          </SelectContent>
                        </Select>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                  <FormField
                    control={form.control}
                    name="paymentMode"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Payment Mode</FormLabel>
                        <Select
                          onValueChange={field.onChange}
                          value={field.value}
                        >
                          <FormControl>
                            <SelectTrigger>
                              <SelectValue placeholder="Select payment mode" />
                            </SelectTrigger>
                          </FormControl>
                          <SelectContent>
                            {PAYMENT_MODE_OPTIONS.map((option) => (
                              <SelectItem key={option} value={option}>
                                {option}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  {shouldShowTransactionField && (
                    <FormField
                      control={form.control}
                      name="transactionIdUtr"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>
                            {transactionFieldLabel}
                            {isTransactionRequired ? " *" : " (Optional)"}
                          </FormLabel>
                          <FormControl>
                            <Input
                              placeholder="Enter transaction reference number"
                              {...field}
                            />
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                  )}

                  {shouldShowPaymentProofField && (
                    <FormField
                      control={form.control}
                      name="paymentProof"
                      render={({ field }) => (
                        <FormItem className="md:col-span-2">
                          <FormLabel>
                            Payment Screenshot Proof
                            {isPaymentProofRequired ? " *" : " (Optional)"}
                          </FormLabel>
                          <FormControl>
                            <div className="space-y-3">
                              <label className="flex flex-col items-center justify-center w-full h-32 border-2 border-slate-300 border-dashed rounded-lg cursor-pointer bg-slate-50 hover:bg-slate-100 transition">
                                <div className="flex flex-col items-center justify-center pt-5 pb-6">
                                  <FileText className="w-8 h-8 mb-3 text-slate-400" />
                                  <p className="mb-2 text-sm text-slate-500">
                                    <span className="font-semibold">
                                      Click to upload
                                    </span>{" "}
                                    or drag and drop
                                  </p>
                                  <p className="text-xs text-slate-500">
                                    PNG, JPG up to 5MB
                                  </p>
                                </div>
                                <Input
                                  key={paymentUploadKey}
                                  type="file"
                                  className="hidden"
                                  accept="image/*"
                                  onChange={(e) => {
                                    field.onChange(e.target.files);
                                    if (e.target.files && e.target.files[0]) {
                                      const reader = new FileReader();
                                      reader.onloadend = () => {
                                        setPaymentProofPreview(
                                          reader.result as string,
                                        );
                                      };
                                      reader.readAsDataURL(e.target.files[0]);
                                    }
                                  }}
                                />
                              </label>

                              {paymentProofPreview && (
                                <div className="flex items-start gap-3 rounded-lg border bg-white p-3">
                                  <img
                                    src={paymentProofPreview}
                                    alt="Payment proof preview"
                                    className="h-20 w-20 rounded-md border object-cover"
                                  />
                                  <div className="flex-1">
                                    <p className="text-sm font-medium text-slate-700">
                                      Uploaded image preview
                                    </p>
                                    <p className="text-xs text-slate-500">
                                      If this is wrong, remove it and upload
                                      again.
                                    </p>
                                  </div>
                                  <Button
                                    type="button"
                                    variant="ghost"
                                    size="icon"
                                    onClick={() => {
                                      setPaymentProofPreview(null);
                                      form.setValue("paymentProof", undefined);
                                      setPaymentUploadKey((prev) => prev + 1);
                                    }}
                                    aria-label="Remove uploaded payment screenshot"
                                  >
                                    <X className="h-4 w-4" />
                                  </Button>
                                </div>
                              )}
                            </div>
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                  )}
                </div>
              </div>
            </div>

            <Button
              type="submit"
              className="w-full h-14 text-lg font-semibold rounded-xl bg-blue-600 hover:bg-blue-700 shadow-md transition-all hover:scale-[1.01]"
              disabled={isSubmitting}
            >
              {isSubmitting ? "Processing..." : "Submit Booking Request"}
            </Button>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}
