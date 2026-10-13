'use client';
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Combobox, ComboboxContent, ComboboxInput, ComboboxItem, ComboboxLabel } from "@/components/ui/combobox";
import { Label } from "@/components/ui/label";
import { MultiSelect } from "@/components/ui/multi-select";
import { useAuth } from "@/context/auth-context";
import { api } from "@/lib/eden";
import { Input } from "@base-ui/react/input"
import { env } from "@repo/env/web";
import { useForm, type StandardSchemaV1 } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRouter } from "next/navigation";
import { useState } from "react";
import z from "zod";
import posthog from "posthog-js";

declare global {
  interface Window {
    Razorpay: any;
  }
}

interface FormSchema {
    fullName: string;
    age: number | undefined;
    profession: string;
    collegeName: string;
    otherProfession: string;
    gender: string;
    contactNumber: string;
    bloodGroup: string;
    city: string;
    state: string;
    education: string;
    involvement: string[];
    areaOfInterest: string[];
    contribute: string;
    termsAccepted: boolean;
}

const formSchema: z.ZodType<FormSchema> = z.object({
    fullName: z.string().min(2, "Please enter your full name"),
    age: z.preprocess(
        (val) => (val === "" || val === null ? undefined : Number(val)),
        z.union([z.number().min(2, "Age too young"), z.undefined()])
    ),
    profession: z.string().min(2, "Please select a profession"),
    collegeName: z.string().default(""),
    otherProfession: z.string().default(""),
    gender: z.string().min(3, "Please select a valid gender value"),
    contactNumber: z.string().min(10, "Please enter a valid contact number"),
    bloodGroup: z.string().min(2, "Please select a valid blood group"),
    city: z.string().min(2, "Please enter a valid city"),
    state: z.string().min(3, "Please enter a valid state"),
    education: z.string().min(5, "Please select a valid education level"),
    involvement: z.array(z.string()).min(1, "Please select a valid wings of involvement"),
    areaOfInterest: z.array(z.string()).min(1, "Please select an area of interest"),
    contribute: z.preprocess((val) => (val === true ? "yes" : "no"), z.string()),
    termsAccepted: z.literal(true, {
        error: () => ({ message: "You must accept the terms and conditions" }),
    }),
}).superRefine((data, ctx) => {
    if (data.profession === "Student" && (!data.collegeName || data.collegeName.length < 5)) {
        ctx.addIssue({
            code: z.ZodIssueCode.custom,
            message: "Please input a valid college name",
            path: ["collegeName"],
        });
    }
    if (data.profession === "other" && !data.otherProfession) {
        ctx.addIssue({
            code: z.ZodIssueCode.custom,
            message: "Please specify your profession",
            path: ["otherProfession"],
        });
    }
});

export const VolunteerForm = () => {
    const { state, dispatch } = useAuth();
    const userId = state.user?.id;
    const userRole = state.user?.userRole;
    const router = useRouter();
    const [successMessage, setSuccessMessage] = useState("");
    const [errorMessage, setErrorMessage] = useState("");
    const queryClient = useQueryClient();

    if (userRole === "PATRON") {
        router.push("/dashboard")
    }

    const educationOptions = [
        { value: "Intermediate", label: "Intermediate" },
        { value: "Graduate", label: "Graduate" },
        { value: "Post Graduate", label: "Post Graduate" },
    ];

    const { data, isLoading, error } = useQuery({
        queryKey: ['professionOptions', userId],
        queryFn: async () => {
            if (!userId) throw new Error("No User ID");

            const { data: professionOptions, error } = await api.options.professions.get({
                $query: { userId: userId },
                $headers: {},
                $fetch: {
                    credentials: "include"
                }
            });

            if (error) {
                throw new Error(error.message || "Professions options failed to fetch");
            }
            return professionOptions;
        },
        enabled: !!userId,
    });

    const { data: involvementOptions, isLoading: involvementOptionsLoading, error: involvementOptionsError } = useQuery({
        queryKey: ['involvementOptions', userId],
        queryFn: async () => {
            if (!userId) throw new Error("No User ID");

            const { data: involvementOptionsData, error } = await api.options.involvement.get({
                $query: { userId: userId },
                $headers: {},
                $fetch: {
                    credentials: "include"
                }
            });

            if (error) {
                throw new Error(error.message || "Involvement options failed to fetch");
            }
            return involvementOptionsData;
        },
        enabled: !!userId,
    });

    const { data: interestOptions, isLoading: interestOptionsLoading, error: interestOptionsError } = useQuery({
        queryKey: ['interestOptions', userId],
        queryFn: async () => {
            if (!userId) throw new Error("No User ID");

            const { data: interestOptionsData, error } = await api.options.interest.get({
                $query: { userId: userId },
                $headers: {},
                $fetch: {
                    credentials: "include"
                }
            });

            if (error) {
                throw new Error(error.message || "Ares of interest options failed to fetch");
            }
            return interestOptionsData;
        },
        enabled: !!userId,
    });

    const mutation = useMutation({
        mutationFn: async (value: FormSchema) => {
            if (!userId) {
                throw new Error("User ID is missing. Please log in again");
            }

            const { data, error } = await api.volunteer.submit.post({
                fullName: value.fullName,
                age: value.age!,
                profession: value.profession,
                collegeName: value.collegeName,
                otherProfession: value.otherProfession,
                gender: value.gender,
                contactNumber: value.contactNumber,
                bloodGroup: value.bloodGroup,
                city: value.city,
                state: value.state,
                education: value.education,
                involvement: value.involvement,
                areaOfInterest: value.areaOfInterest,
                contribute: value.contribute,
                termsAccepted: value.termsAccepted,
                $query: {
                    userId: userId!
                },
                $headers: {},
                $fetch: {
                    credentials: "include"
                }
            });

            if (error) {
                throw new Error(error.message || "An unknown API error occurred.");
            }
            return data;
        },
        onSuccess: async ( ctx, data, variables) => {
            setErrorMessage('');
            setSuccessMessage("Registration Successful!");

            dispatch({
                type: "UPDATE_USER",
                payload: {
                    userRole: ctx.membershipDetails?.roleName,
                    memberCode: ctx.membershipDetails?.memberCode
                },
            });

            queryClient.invalidateQueries({ queryKey: ['userMemberships', userId] });
            queryClient.invalidateQueries({ queryKey: ['userRole', userId] });

            const contribuationData = ctx.wantsToContribute;

            if (contribuationData) {
                setSuccessMessage("Initiating Payment process...");

                const userEmail = state.user?.email;

                try {
                    const { data: checkoutSession, error } = await api.payments.checkout.post({
                        amount: "99",
                        frequency: "Monthly",
                        otherAmount: "",
                        $query: { userId: userId! },
                        $headers: {},
                        $fetch: {
                            credentials: "include"
                        }
                    });
                
                    if (error) {
                        let errMsg = "Could not initiate payment";
                        if (error.message) {
                            errMsg = error.message;
                        }
                        setErrorMessage(errMsg);
                        return;
                    }
                
                    let paymentIdConfig: Record<string, string | undefined>;
                    if (checkoutSession.type === "subscription") {
                        paymentIdConfig = { subscription_id: checkoutSession.id };
                    } else {
                        paymentIdConfig = { order_id: checkoutSession.id };
                    }

                    const options = {
                        key: env.NEXT_PUBLIC_RAZORPAY_KEY_ID,
                        amount: 99 * 100,
                        currency: "INR",
                        name: data.fullName,
                        frequency: "Monthly",
                        order_id: checkoutSession.id,
                        ...paymentIdConfig,
                        handler: function ( response: any) {
                            console.log("Payment ID: ", response.razorpay_payment_id);
                            setSuccessMessage("Payment is successful!! Redirecting...");

                            // Capture volunteer payment success event
                            posthog.capture("volunteer_payment_success", {
                                amount: 99,
                                currency: "INR",
                                frequency: "Monthly",
                                payment_id: response.razorpay_payment_id,
                            });

                            router.push('/dashboard');
                        },
                        prefill: {
                            name: data.fullName,
                            contact: data.contactNumber,
                            email: userEmail
                        }
                    };
                
                    const rzp = new (window).Razorpay(options);
                
                    rzp.on('payment.failed', function (response: any) {
                        setErrorMessage("Payment failed:" + response.error.description);

                        // Capture volunteer payment failed event
                        posthog.capture("volunteer_payment_failed", {
                            amount: 99,
                            currency: "INR",
                            frequency: "Monthly",
                            error_code: response.error.code,
                            error_description: response.error.description,
                        });
                    });
                
                    rzp.open();
                } catch (error: any) {
                    console.error(error);
                    let catchMsg = "Could not initiate payment";
                    if (error.message) {
                        catchMsg = error.message;
                    }
                    setErrorMessage(catchMsg);
                }
            } else {
                form.reset();
                router.push('/dashboard');
            }
        },
        onError: (error: Error) => {
            setSuccessMessage("");
            console.error(error);
            setErrorMessage(error.message || "An unexpected error occurred during submission.");
        }
    })

    const form = useForm({
        defaultValues: {
            fullName: "",
            age: undefined as number | undefined,
            profession: "",
            collegeName: "",
            otherProfession: "",
            gender: "",
            contactNumber: "",
            bloodGroup: "",
            city: "",
            state: "",
            education: "",
            involvement: [] as string[],
            areaOfInterest: [] as string[],
            contribute: "no",
            termsAccepted: false
        } as FormSchema,
        onSubmit: async ({ value }) => {
            console.log("submitting", value)

            // Capture volunteer registration submitted event
            posthog.capture("volunteer_registration_submitted", {
                profession: value.profession,
                gender: value.gender,
                city: value.city,
                state: value.state,
                education: value.education,
                involvement_count: value.involvement.length,
                interest_count: value.areaOfInterest.length,
                wants_to_contribute: value.contribute === "yes",
            });

            mutation.mutate(value);
        },
        validators: {
            onSubmit: formSchema as StandardSchemaV1<FormSchema, FormSchema>,
        }
    });

    return (
        <div className="flex w-full min-h-screen gap-6 p-10">
            <form
                onSubmit={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    form.handleSubmit();
                }}
                className="flex flex-1 space-y-4 space-x-6"
            >
                <div className="flex-1 border-r">
                    <div className="text-[38px] font-bold text-[#DB7A05] font-poppins leading-8.25">Volunteer</div>
                    <div className="text-[28px] font-bold text-[#604D00] font-poppins leading-8.25 mt-3">Personal Information</div>

                    <div>
                        <form.Field name="fullName">
                            {(field) => (
                                <div className="space-y-2 flex flex-row items-center">
                                    <Input
                                        id={field.name}
                                        name={field.name}
                                        type="text"
                                        value={field.state.value}
                                        onChange={(e) => field.handleChange(e.target.value)}
                                        placeholder="Full Name"
                                        className="border-0 border-b border-input rounded-none shadow-none focus-visible:ring-0 focus-visible:ring-offset-0 focus-visible:border-primary px-3 font-poppins font-medium text-[14px] mt-2 leading-8.25"
                                        required
                                    />
                                    <div className="font-poppins text-[14px] text-[#604D00]/50">(Will be used in certificates)</div>
                                    {field.state.meta.errors.map((error) => (
                                        <p key={error?.message} className="text-red-500 font-poppins">
                                            {error?.message}
                                        </p>
                                    ))}
                                </div>
                            )}
                        </form.Field>
                    </div>

                    <div>
                        <form.Field name="age">
                            {(field) => (
                                <div className="space-y-2">
                                    <Input
                                        id={field.name}
                                        name={field.name}
                                        type="number"
                                        value={field.state.value}
                                        placeholder="Age"
                                        onBlur={field.handleBlur}
                                        onChange={(e) => {
                                            const val = e.target.value;
                                            field.handleChange(val === "" ? undefined : Number(val) as any);
                                        }}
                                        className="border-0 border-b border-input rounded-none shadow-none focus-visible:ring-0 focus-visible:ring-offset-0 focus-visible:border-primary px-3 font-poppins font-medium text-[14px] mt-4 leading-8.25"
                                        required
                                    />
                                    {field.state.meta.errors.map((error) => (
                                        <p key={error?.message} className="text-red-500 font-poppins">
                                            {error?.message}
                                        </p>
                                    ))}
                                </div>
                            )}
                        </form.Field>
                    </div>

                    <div>
                        <form.Field name="gender">
                            {(field) => (
                                <div className="space-y-2">
                                    <Combobox
                                        id={field.name}
                                        name={field.name}
                                        value={field.state.value}
                                        onValueChange={(val) => field.handleChange(val ?? "")}
                                        required
                                    >
                                        <ComboboxInput placeholder="Gender" className="border-0 border-b rounded-none shadow-none px-0 focus:ring-0 focus:ring-offset-0 focus:border-b-2 focus:border-primary font-poppins w-43.75 font-medium text-[14px] mt-4" />
                                        <ComboboxContent className="font-poppins">
                                            <ComboboxItem value="Male">Male</ComboboxItem>
                                            <ComboboxItem value="Female">Female</ComboboxItem>
                                            <ComboboxItem value="Prefer not to say">Prefer not to say</ComboboxItem>
                                        </ComboboxContent>
                                    </Combobox>
                                    {field.state.meta.errors.map((error) => (
                                        <p key={error?.message} className="text-red-500 font-poppins">
                                            {error?.message}
                                        </p>
                                    ))}
                                </div>
                            )}
                        </form.Field>
                    </div>

                    <div>
                        <form.Field name="profession">
                            {(field) => {
                                const studentOption = data?.find((opt) => opt.name === "Student");
                                const isStudent = String(field.state.value) === String(studentOption?.name);

                                return (
                                    <div className="space-y-2">
                                        <Combobox
                                            id={field.name}
                                            name={field.name}
                                            value={field.state.value}
                                            onValueChange={(val) => {
                                                field.handleChange(val ?? "");
                                                if (studentOption && val !== String(studentOption.id)) {
                                                    form.setFieldValue('collegeName', '');
                                                }

                                                if (val !== "other") {
                                                    form.setFieldValue('otherProfession', '');
                                                }
                                            }}
                                            required
                                        >
                                            <ComboboxInput placeholder="Profession" className="border-0 border-b rounded-none shadow-none px-0 focus:ring-0 focus:ring-offset-0 focus:border-b-2 focus:border-primary font-poppins font-medium text-[14px] w-43.75 mt-4" />
                                            <ComboboxContent className="font-poppins">
                                                {isLoading ? (
                                                    <ComboboxItem value="loading" disabled>Loading...</ComboboxItem>
                                                        ) : (
                                                            data?.map((option) => (
                                                                <ComboboxItem key={option.id} value={option.name}>
                                                                    {option.name}
                                                                </ComboboxItem>
                                                            ))
                                                        )
                                                }
                                                <ComboboxItem value="other">Other</ComboboxItem>
                                            </ComboboxContent>
                                        </Combobox>
                                        {field.state.meta.errors.map((error) => (
                                            <p key={error?.message} className="text-red-500 font-poppins">
                                                {error?.message}
                                            </p>
                                        ))}

                                        {isStudent && (
                                            <form.Field name="collegeName">
                                                {(subField) => (
                                                    <div className="mt-4 animate-in fade-in slide-in-from-top-1">
                                                        <Input
                                                            placeholder="Enter your College Name"
                                                            value={subField.state.value}
                                                            onChange={(e) => subField.handleChange(e.target.value)}
                                                            onBlur={subField.handleBlur}
                                                            className="border-0 border-b rounded-none shadow-none px-0 focus-visible:ring-0 focus-visible:border-primary font-poppins font-medium text-[14px]"
                                                        />
                                                        {field.state.meta.errors.map((error) => (
                                                            <p key={error?.message} className="text-red-500 font-poppins">
                                                                {error?.message}
                                                            </p>
                                                        ))}
                                                    </div>
                                                )}
                                            </form.Field>
                                        )}

                                        {field.state.value === "other" && (
                                            <form.Field name="otherProfession">
                                                {(field) => (
                                                    <div className="mt-4 animate-in fade-in slide-in-from-top-1">
                                                        <Input
                                                            placeholder="Specify your profession"
                                                            value={field.state.value}
                                                            onChange={(e) => field.handleChange(e.target.value)}
                                                            onBlur={async () => {
                                                                field.handleBlur();
                                                                if (field.state.value) {
                                                                    await api.options.professions.post({
                                                                        name: field.state.value,
                                                                        $query: { userId: userId },
                                                                        $headers: {}
                                                                    });
                                                                }
                                                            }}
                                                            className="border-0 border-b rounded-none shadow-none px-0 focus-visible:ring-0 focus-visible:border-primary"
                                                        />
                                                        {field.state.meta.errors.map((error) => (
                                                            <p key={error?.message} className="text-red-500 font-poppins">
                                                                {error?.message}
                                                            </p>
                                                        ))}
                                                    </div>
                                                )}
                                            </form.Field>
                                        )}
                                    </div>
                                )
                            }}
                        </form.Field>
                    </div>

                    <div className="mb-10">
                        <form.Field name="bloodGroup">
                            {(field) => (
                                <div className="space-y-2">
                                    <Combobox
                                        id={field.name}
                                        name={field.name}
                                        value={field.state.value}
                                        onValueChange={(val) => field.handleChange(val ?? "")}
                                        required
                                    >
                                        <ComboboxInput placeholder="Blood Group" className="border-0 border-b rounded-none shadow-none px-0 focus:ring-0 focus:ring-offset-0 focus:border-b-2 focus:border-primary font-poppins font-medium text-[14px] w-43.75 mt-4" />
                                        <ComboboxContent className="font-poppins">
                                            <ComboboxItem value="A+">A+</ComboboxItem>
                                            <ComboboxItem value="A-">A-</ComboboxItem>
                                            <ComboboxItem value="B+">B+</ComboboxItem>
                                            <ComboboxItem value="B-">B-</ComboboxItem>
                                            <ComboboxItem value="AB+">AB+</ComboboxItem>
                                            <ComboboxItem value="AB-">AB-</ComboboxItem>
                                            <ComboboxItem value="O+">O+</ComboboxItem>
                                            <ComboboxItem value="O-">O-</ComboboxItem>
                                        </ComboboxContent>
                                    </Combobox>
                                    {field.state.meta.errors.map((error) => (
                                        <p key={error?.message} className="text-red-500 font-poppins">
                                            {error?.message}
                                        </p>
                                    ))}
                                </div>
                            )}
                        </form.Field>
                    </div>

                    <div className="text-[28px] font-bold text-[#604D00] font-poppins leading-8.25 mt-3">Contact Information</div>

                    <div>
                        <form.Field name="city">
                            {(field) => (
                                <div className="space-y-2">
                                    <Input
                                        id={field.name}
                                        name={field.name}
                                        value={field.state.value}
                                        placeholder="City"
                                        onChange={(e) => field.handleChange(e.target.value)}
                                        className="border-0 border-b border-input rounded-none shadow-none focus-visible:ring-0 focus-visible:ring-offset-0 focus-visible:border-primary px-3 font-poppins font-medium text-[14px] mt-2 leading-8.25"
                                        required
                                    />
                                    {field.state.meta.errors.map((error) => (
                                        <p key={error?.message} className="text-red-500 font-poppins">
                                            {error?.message}
                                        </p>
                                    ))}
                                </div>
                            )}
                        </form.Field>
                    </div>

                    <div>
                        <form.Field name="state">
                            {(field) => (
                                <div className="space-y-2">
                                    <Combobox
                                        id={field.name}
                                        name={field.name}
                                        value={field.state.value}
                                        onValueChange={(val) => field.handleChange(val ?? "")}
                                        required
                                    >
                                        <ComboboxInput placeholder="State" className="border-0 border-b rounded-none shadow-none px-0 focus:ring-0 focus:ring-offset-0 focus:border-b-2 focus:border-primary font-poppins font-medium text-[14px] w-43.75 mt-4" />
                                        <ComboboxContent className="font-poppins">
                                            <ComboboxItem value="Andhra Pradesh">Andhra Pradesh</ComboboxItem>
                                            <ComboboxItem value="Arunachal Pradesh">Arunachal Pradesh</ComboboxItem>
                                            <ComboboxItem value="Assam">Assam</ComboboxItem>
                                            <ComboboxItem value="Bihar">Bihar</ComboboxItem>
                                            <ComboboxItem value="Chattisgarh">Chattisgarh</ComboboxItem>
                                            <ComboboxItem value="Goa">Goa</ComboboxItem>
                                            <ComboboxItem value="Gujarat">Gujarat</ComboboxItem>
                                            <ComboboxItem value="Haryana">Haryana</ComboboxItem>
                                            <ComboboxItem value="Himachal Pradesh">Himachal Pradesh</ComboboxItem>
                                            <ComboboxItem value="Jharkhand">Jharkhand</ComboboxItem>
                                            <ComboboxItem value="Karnataka">Karnataka</ComboboxItem>
                                            <ComboboxItem value="Kerala">Kerala</ComboboxItem>
                                            <ComboboxItem value="Madhya Pradesh">Madhya Pradesh</ComboboxItem>
                                            <ComboboxItem value="Maharashtra">Maharashtra</ComboboxItem>
                                            <ComboboxItem value="Manipur">Manipur</ComboboxItem>
                                            <ComboboxItem value="Meghalaya">Meghalaya</ComboboxItem>
                                            <ComboboxItem value="Mizoram">Mizoram</ComboboxItem>
                                            <ComboboxItem value="Nagaland">Nagaland</ComboboxItem>
                                            <ComboboxItem value="Odisha">Odisha</ComboboxItem>
                                            <ComboboxItem value="Punjab">Punjab</ComboboxItem>
                                            <ComboboxItem value="Rajasthan">Rajasthan</ComboboxItem>
                                            <ComboboxItem value="Sikkim">Sikkim</ComboboxItem>
                                            <ComboboxItem value="Tamil Nadu">Tamil Nadu</ComboboxItem>
                                            <ComboboxItem value="Telangana">Telangana</ComboboxItem>
                                            <ComboboxItem value="Tripura">Tripura</ComboboxItem>
                                            <ComboboxItem value="Uttar Pradesh">Uttar Pradesh</ComboboxItem>
                                            <ComboboxItem value="Uttarakhand">Uttarakhand</ComboboxItem>
                                            <ComboboxItem value="West Bengal">West Bengal</ComboboxItem>
                                        </ComboboxContent>
                                    </Combobox>
                                    {field.state.meta.errors.map((error) => (
                                        <p key={error?.message} className="text-red-500 font-poppins">
                                            {error?.message}
                                        </p>
                                    ))}
                                </div>
                            )}

                        </form.Field>
                    </div>
                    
                    <div>
                        <form.Field name="contactNumber">
                            {(field) => (
                                <div className="space-y-2">
                                    <Input
                                        id={field.name}
                                        name={field.name}
                                        value={field.state.value}
                                        placeholder="Contact Number"
                                        onBlur={field.handleBlur}
                                        onChange={(e) => field.handleChange(e.target.value)}
                                        className="border-0 border-b border-input rounded-none shadow-none focus-visible:ring-0 focus-visible:ring-offset-0 focus-visible:border-primary px-3 font-poppins font-medium text-[14px] mt-2 leading-8.25"
                                        required
                                        type="tel"
                                    />
                                    {field.state.meta.errors.map((error) => (
                                        <p key={error?.message} className="text-red-500 font-poppins">
                                            {error?.message}
                                        </p>
                                    ))}
                                </div>
                            )}
                        </form.Field>
                    </div>
            </div>

            <div className="flex-1 mt-20">
                <div>
                    <form.Field name="involvement">
                        {(field) => (
                            <div className="space-y-2">
                                <MultiSelect
                                    placeholder="Select wings of involvement"
                                    options={involvementOptions?.map((option) => ({
                                        value: String(option.id),
                                        label: `${option.name} (${option.purpose})`
                                    })) || []}
                                    value={field.state.value}
                                    onChange={(val) => field.handleChange(val)}
                                    className="mt-2 px-2 font-poppins"
                                />
                                {field.state.meta.errors.map((error) => (
                                    <p key={error?.message} className="text-red-500 font-poppins">
                                        {error?.message}
                                    </p>
                                ))}
                            </div>
                        )}
                    </form.Field>
                </div>
                    
                <div>
                    <form.Field name="areaOfInterest">
                        {(field) => (
                            <div className="space-y-2">
                                <MultiSelect
                                    placeholder="Select areas of interest"
                                    options={interestOptions?.map((option) => ({
                                        value: String(option.id),
                                        label: `${option.name}`
                                    })) || []}
                                    value={field.state.value}
                                    onChange={(val) => field.handleChange(val)}
                                    className="mt-2 px-2 font-poppins"
                                />
                                {field.state.meta.errors.map((error) => (
                                    <p key={error?.message} className="text-red-500 font-poppins">
                                        {error?.message}
                                    </p>
                                ))}
                            </div>
                        )}
                    </form.Field>
                </div>

                <div>
                    <form.Field name="education">
                        {(field) => (
                            <div className="space-y-2">
                                <Combobox
                                    id={field.name}
                                    name={field.name}
                                    value={field.state.value}
                                    onValueChange={(val) => field.handleChange(val ?? "")}
                                    required
                                    items={educationOptions}
                                >
                                    <ComboboxInput placeholder="Education" className="border-0 border-b rounded-none shadow-none px-0 focus:ring-0 focus:ring-offset-0 focus:border-b-2 focus:border-primary font-poppins font-medium text-[14px] mt-4" />
                                    <ComboboxContent className="font-poppins">
                                        {educationOptions.map((opt) => (
                                            <ComboboxItem key={opt.value} value={opt.value}>
                                                {opt.label}
                                            </ComboboxItem>
                                        ))}
                                    </ComboboxContent>
                                </Combobox>
                                {field.state.meta.errors.map((error) => (
                                    <p key={error?.message} className="text-red-500 font-poppins">
                                        {error?.message}
                                    </p>
                                ))}
                            </div>
                        )}
                    </form.Field>
                </div>

                <p className="mt-20 font-poppins font-bold text-[28px] leading-8.25 text-[#604D00]">Declaration</p>
                <form.Field name="termsAccepted">
                        {(field) => (
                            <div className="flex flex-row items-start mt-5">
                                <Checkbox
                                    id={field.name}
                                    name={field.name}
                                    checked={field.state.value}
                                    onCheckedChange={(checked) => field.handleChange(checked)}
                                    onBlur={field.handleBlur}
                                    className="mr-2 mt-1"
                                />
                                <Label htmlFor={field.name} className="font-poppins font-medium text-[14px] leading-6 text-[#604D004D] cursor-pointer">
                                    I confirm that the information provided is true to the best of my knowledge and I accept the <a href="/terms" className="text-primary hover:underline" target="_blank" rel="noopener noreferrer">terms and conditions</a>.
                                </Label>
                                {field.state.meta.errors.map((error) => (
                                    <p key={error?.message} className="text-red-500 font-poppins">
                                        {error?.message}
                                    </p>
                                ))}
                            </div>
                        )}
                    </form.Field>

                <form.Subscribe>
                    {(state) => (
                        <div>
                            <Button
                                variant="outline"
                                type="submit"
                                className="bg-linear-to-r mt-20 from-[#DB7A04] to-[#F1980F] text-white hover:text-white font-semibold text-[15px] rounded-[40px] py-5 w-full font-poppins hover:cursor-pointer"
                                disabled={mutation.isPending || !form.state.canSubmit}
                            >
                                {state.isSubmitting ? "Submitting..." : "Submit"}
                            </Button>
                        </div>
                    )}
                </form.Subscribe>
            </div>
            {successMessage && (
                <p className="text-xs text-green-600 font-poppins mt-2 flex justify-center items-center">{successMessage}</p>
            )}
            {errorMessage && (
                <p className="text-red-500 text-xs font-poppins mt-2 flex justify-center items-center">{errorMessage}</p>
            )}
            </form>
        </div>
    )
}