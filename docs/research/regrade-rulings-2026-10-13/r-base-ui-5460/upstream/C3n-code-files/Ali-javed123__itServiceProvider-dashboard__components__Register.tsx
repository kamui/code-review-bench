"use client";

import React from "react";
import {
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldSeparator,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import FormikInput from "@/lib/useFields";
import { Button } from "@base-ui/react";
import {
  Formik,
  FastField,
  Form,
  ErrorMessage,
  FormikHelpers,
} from "formik";
import { toFormikValidationSchema } from "zod-formik-adapter";
import { useRouter } from "next/navigation";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  registerSchema,
  type RegisterFormValues,
  type RegisterPayload,
} from "../validation/register.validation";

const Register = () => {
  const router = useRouter();

  const initialValues: RegisterFormValues = {
    email: "",
    password: "",
    name: "",
    age: "",
    gender: "male",
  };

  const validationSchema = toFormikValidationSchema(registerSchema);

  const handleSubmit = async (
    values: RegisterFormValues,
    { setSubmitting }: FormikHelpers<RegisterFormValues>
  ) => {
    try {
      // ✅ Sirf yahan convert karo
      const payload: RegisterPayload = {
        ...values,
        age: Number(values.age),
      };

      console.log("Register Payload:", payload);

      // const response = await api.post("/register", payload);
      // router.push("/login");
    } catch (error) {
      console.error("Registration error:", error);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-6">
      <div className="w-full max-w-md">
        <Formik<RegisterFormValues>
          initialValues={initialValues}
          validationSchema={validationSchema}
          onSubmit={handleSubmit}
        >
          {({ isSubmitting, errors, touched,values,setFieldValue }) => (
            <Form className="space-y-6">
              {/* Name */}
              <FieldGroup>
                <FieldLabel htmlFor="name">Name</FieldLabel>
                <FastField
                  id="name"
                  name="name"
                  type="text"
                  placeholder="Enter your name"
                  className="w-full rounded-md border px-3 py-2"
                />
                <ErrorMessage
                  name="name"
                  component="p"
                  className="text-sm text-red-500"
                />
              </FieldGroup>

              {/* Email */}
              <FieldGroup>
                <FieldLabel htmlFor="email">Email</FieldLabel>
                <FastField
                  id="email"
                  name="email"
                  type="email"
                  placeholder="Enter your email"
                  className="w-full rounded-md border px-3 py-2"
                />
                <ErrorMessage
                  name="email"
                  component="p"
                  className="text-sm text-red-500"
                />
              </FieldGroup>

              {/* Password */}
              <FieldGroup>
                <FieldLabel htmlFor="password">Password</FieldLabel>
                <FastField
                  id="password"
                  name="password"
                  type="password"
                  placeholder="Enter your password"
                  className="w-full rounded-md border px-3 py-2"
                />
                {/* ✅ Hint sirf tab dikhao jab error na ho */}
                {!errors.password && (
                  <FieldDescription>
                    Password must contain at least 8 characters, one uppercase
                    letter and one number.
                  </FieldDescription>
                )}
                <ErrorMessage
                  name="password"
                  component="p"
                  className="text-sm text-red-500"
                />
              </FieldGroup>

              {/* Age */}
              <FieldGroup>
                <FieldLabel htmlFor="age">Age</FieldLabel>
                <FastField
                  id="age"
                  name="age"
                  type="number"
                  min={18}
                  placeholder="Enter your age"
                  className="w-full rounded-md border px-3 py-2"
                />
                <ErrorMessage
                  name="age"
                  component="p"
                  className="text-sm text-red-500"
                />
              </FieldGroup>

              {/* Gender */}
<FieldGroup>
  <FieldLabel htmlFor="gender">Gender</FieldLabel>

  <Select
    name="gender"
    value={values.gender}
    onValueChange={(value) => {
      setFieldValue("gender", value);
    }}
  >
    <SelectTrigger className="w-full">
      <SelectValue placeholder="Select Gender" />
    </SelectTrigger>

    <SelectContent>
      <SelectItem value="male">Male</SelectItem>
      <SelectItem value="female">Female</SelectItem>
    </SelectContent>
  </Select>

  <ErrorMessage
    name="gender"
    component="p"
    className="text-sm text-red-500"
  />
</FieldGroup>
              <FieldSeparator />

              <Button
                type="submit"
                disabled={isSubmitting}
                className="w-full py-2 bg-white hover:bg-gray-300 rounded-md text-black"
              >
                {isSubmitting ? "Creating Account..." : "Create Account"}
              </Button>
            </Form>
          )}
        </Formik>
      </div>
    </div>
  );
};

export default Register;