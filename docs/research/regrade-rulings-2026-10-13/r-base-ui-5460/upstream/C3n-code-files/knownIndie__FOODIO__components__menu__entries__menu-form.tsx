"use client"

import { Toggle, ToggleGroup } from "@base-ui/react"
import { useForm } from "@tanstack/react-form"
import {
  ChevronLeft,
  Copy,
  Eye,
  FlaskConical,
  IndianRupee,
  Leaf,
  Plus,
  Trash2,
  Utensils,
} from "lucide-react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import { useId, useState } from "react"
import * as z from "zod"
import type { MenuSaveResponse } from "@/lib/dashboard-menu/schema"
import { Badge } from "../../ui/badge"
import { Button } from "../../ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "../../ui/card"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "../../ui/field"
import { Input } from "../../ui/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "../../ui/select"
import { Separator } from "../../ui/separator"
import { Switch } from "../../ui/switch"
import { Textarea } from "../../ui/textarea"
import {
  formatMenuItemOption,
  type MenuItemAvailability,
  menuItemAvailabilities,
  menuItemCuisines,
  menuItemFoodTypes,
} from "./menuFormData"

const menuItemSchema = z.object({
  formId: z.string(),
  name: z.string().trim().min(1, "Item name is required").max(120),
  description: z.string().trim().max(2000),
  priceInRupees: z
    .number()
    .min(0.01, "Price must be at least ₹0.01")
    .max(21_474_836.47, "Price is too large")
    .refine(
      (value) => Math.abs(value * 100 - Math.round(value * 100)) < 0.000001,
      "Price must have at most two decimal places"
    ),
  isVeg: z.boolean(),
  isAvailable: z.boolean(),
  isActive: z.boolean(),
  foodTypes: z
    .array(z.enum(menuItemFoodTypes))
    .min(1, "Choose at least one food type"),
  cuisines: z.array(z.enum(menuItemCuisines)).min(1, "Choose a cuisine"),
  timings: z
    .array(z.enum(menuItemAvailabilities))
    .min(1, "Choose at least one serving time")
    .refine(
      (values) => values.length === 1 || !values.includes("ALL_DAY"),
      "All day cannot be combined with another serving time"
    ),
  caloriesKcal: z
    .number()
    .int()
    .min(0, "Calories cannot be negative")
    .max(2_147_483_647)
    .nullable(),
})

const menuFormSchema = z.object({
  menuItems: z
    .array(menuItemSchema)
    .min(1, "Add at least one menu item")
    .max(50, "Save at most 50 dishes at a time"),
})

type MenuItemFormValue = z.infer<typeof menuItemSchema>

// this is the test data
const testMenuItemDetails: Omit<MenuItemFormValue, "formId"> = {
  name: "Margherita Pizza",
  description:
    "Stone-baked pizza with tomato sauce, fresh mozzarella, and basil.",
  priceInRupees: 349,
  isVeg: true,
  isAvailable: true,
  isActive: true,
  foodTypes: ["PIZZA"],
  cuisines: ["ITALIAN"],
  timings: ["ALL_DAY"],
  caloriesKcal: 620,
}

const showTestDetails = process.env.NODE_ENV === "development"

const emptyMenuItem = (formId: string): MenuItemFormValue => ({
  formId,
  name: "",
  description: "",
  priceInRupees: 100,
  isVeg: true,
  isAvailable: true,
  isActive: true,
  foodTypes: [],
  cuisines: ["NORTH_INDIAN"],
  timings: ["LUNCH", "DINNER"],
  caloriesKcal: null,
})

function normalizeAvailability(
  previous: MenuItemAvailability[],
  next: MenuItemAvailability[]
) {
  const addedAllDay = !previous.includes("ALL_DAY") && next.includes("ALL_DAY")

  return addedAllDay
    ? ["ALL_DAY" as const]
    : next.filter((v) => v !== "ALL_DAY")
}

const toggleClassName =
  "h-8 rounded-2xl border border-border bg-background px-3 text-sm font-medium transition-colors hover:bg-muted data-pressed:border-primary data-pressed:bg-primary data-pressed:text-primary-foreground"

export function MenuForm({
  restaurantId,
  restaurantName,
}: {
  restaurantId?: string
  restaurantName?: string
} = {}) {
  const initialItemId = useId()
  const router = useRouter()
  const [message, setMessage] = useState<{
    type: "success" | "error"
    text: string
  }>()
  const form = useForm({
    defaultValues: {
      menuItems: [emptyMenuItem(initialItemId)],
    },
    validators: {
      onSubmit: menuFormSchema,
    },
    onSubmit: async ({ value }) => {
      setMessage(undefined)
      if (!restaurantId) {
        setMessage({
          type: "error",
          text: "Open a restaurant you own before saving menu items.",
        })
        return
      }
      const menuItems = value.menuItems.map(
        ({ formId: _formId, ...item }) => item
      )
      try {
        const response = await fetch(
          `/api/restaurants/${restaurantId}/menu/add`,
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ menuItems }),
          }
        )
        const result = (await response.json()) as MenuSaveResponse
        if (!response.ok || !result.success) {
          setMessage({
            type: "error",
            text: result.success
              ? "Could not save menu items."
              : [result.error, ...(result.issues ?? []).slice(0, 3)].join(" "),
          })
          return
        }
        form.reset({ menuItems: [emptyMenuItem(crypto.randomUUID())] })
        setMessage({
          type: "success",
          text: `Saved ${result.createdCount} ${result.createdCount === 1 ? "dish" : "dishes"}.`,
        })
        router.refresh()
      } catch {
        setMessage({
          type: "error",
          text: "Could not confirm the save. Your entries are still here. Check saved dishes before retrying.",
        })
      }
    },
  })
  const addItem = () => {
    if (form.getFieldValue("menuItems").length >= 50) return
    form.pushFieldValue("menuItems", emptyMenuItem(crypto.randomUUID()))
  }
  const removeItem = (index: number) =>
    form.removeFieldValue("menuItems", index)
  const fillTestDetails = () => {
    setMessage(undefined)
    const currentItems = form.getFieldValue("menuItems")
    const firstItemId = currentItems[0]?.formId ?? crypto.randomUUID()

    form.setFieldValue("menuItems", [
      { formId: firstItemId, ...testMenuItemDetails },
      ...currentItems.slice(1),
    ])
  }

  return (
    <main className="min-h-screen rounded-4xl bg-muted/30 px-4 py-8 text-foreground sm:px-6 lg:px-10">
      <div className="mx-auto flex max-w-6xl flex-col gap-2">
        <header className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
          <div className="flex flex-col gap-3">
            <Button
              nativeButton={false}
              variant="ghost"
              size="sm"
              className="w-fit text-muted-foreground"
              render={<Link href="/dashboard" />}
            >
              <ChevronLeft data-icon="inline-start" /> Back To Dashboard
            </Button>
            <div className="flex flex-col gap-2 ">
              <Badge variant="secondary">Menu management</Badge>
            </div>
            <h1 className="text-balance text-3xl font-semibold tracking-tight sm:text-4xl">
              Add menu items
            </h1>
            <p className="max-w-xl text-pretty leading-6 text-muted-foreground">
              {restaurantId
                ? `Save dishes${restaurantName ? ` for ${restaurantName}` : ""}. Add up to 50 in one batch.`
                : "Preview only. Open a restaurant you own to save dishes."}
            </p>
          </div>
          {showTestDetails ? (
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <Button
                  type="button"
                  variant="secondary"
                  className="w-fit"
                  onClick={fillTestDetails}
                  disabled={isSubmitting}
                >
                  <FlaskConical data-icon="inline-start" /> Fill test details
                </Button>
              )}
            </form.Subscribe>
          ) : null}
        </header>
        <form
          noValidate
          onSubmit={(event) => {
            event.preventDefault()
            event.stopPropagation()
            if (form.state.isSubmitting) return
            void form.handleSubmit()
          }}
          className="flex flex-col gap-6"
        >
          {message ? (
            <p
              role={message.type === "error" ? "alert" : "status"}
              className={
                message.type === "error"
                  ? "text-sm text-destructive"
                  : "text-sm"
              }
            >
              {message.text}
            </p>
          ) : null}
          <form.Subscribe selector={(state) => state.isSubmitting}>
            {(isSubmitting) => (
              <fieldset disabled={isSubmitting} className="min-w-0 space-y-6">
                <form.Field name="menuItems" mode="array">
                  {(items) => (
                    <>
                      <FieldError errors={items.state.meta.errors} />
                      {items.state.value.map((item, index) => (
                        <Card key={item.formId}>
                          <CardHeader className="flex flex-col items-start justify-between gap-4 sm:flex-row">
                            <div>
                              <CardTitle>Dish {index + 1}</CardTitle>
                              <CardDescription>
                                Required details are validated as you type.
                              </CardDescription>
                            </div>
                            <div className="flex w-full flex-wrap gap-2 sm:w-auto sm:justify-end">
                              <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                onClick={() => {
                                  const item = items.state.value[index]

                                  if (!item || items.state.value.length >= 50)
                                    return

                                  void form.insertFieldValue(
                                    "menuItems",
                                    index + 1,
                                    {
                                      ...item,
                                      formId: crypto.randomUUID(),
                                      foodTypes: [...item.foodTypes],
                                      cuisines: [...item.cuisines],
                                      timings: [...item.timings],
                                    }
                                  )
                                }}
                                disabled={items.state.value.length >= 50}
                                aria-label={`Duplicate dish ${index + 1}`}
                              >
                                <Copy data-icon="inline-start" /> Duplicate
                              </Button>
                              <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={() => removeItem(index)}
                                disabled={items.state.value.length === 1}
                                aria-label={`Remove dish ${index + 1}`}
                              >
                                <Trash2 data-icon="inline-start" /> Remove
                              </Button>
                            </div>
                          </CardHeader>
                          <CardContent className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_20rem]">
                            <div className="flex flex-col gap-6">
                              <FieldGroup>
                                <form.Field
                                  name={`menuItems[${index}].name`}
                                  validators={{
                                    onChange: ({ value }) =>
                                      menuItemSchema.shape.name.safeParse(value)
                                        .success
                                        ? undefined
                                        : { message: "Item name is required" },
                                  }}
                                >
                                  {(field) => (
                                    <Field
                                      data-invalid={
                                        field.state.meta.isTouched &&
                                        field.state.meta.errors.length > 0
                                      }
                                    >
                                      <FieldLabel htmlFor={`name-${index}`}>
                                        Item name{" "}
                                        <span className="text-destructive">
                                          *
                                        </span>
                                      </FieldLabel>
                                      <Input
                                        id={`name-${index}`}
                                        value={field.state.value}
                                        onBlur={field.handleBlur}
                                        onChange={(e) =>
                                          field.handleChange(e.target.value)
                                        }
                                        placeholder="e.g. Paneer Tikka Masala"
                                        aria-invalid={
                                          field.state.meta.errors.length > 0
                                        }
                                      />
                                      <FieldError
                                        errors={field.state.meta.errors}
                                      />
                                    </Field>
                                  )}
                                </form.Field>
                                <form.Field
                                  name={`menuItems[${index}].description`}
                                >
                                  {(field) => (
                                    <Field>
                                      <FieldLabel
                                        htmlFor={`description-${index}`}
                                      >
                                        Description
                                      </FieldLabel>
                                      <Textarea
                                        id={`description-${index}`}
                                        value={field.state.value}
                                        onBlur={field.handleBlur}
                                        onChange={(e) =>
                                          field.handleChange(e.target.value)
                                        }
                                        placeholder="Tell guests what makes this dish special..."
                                        className="min-h-28 resize-y"
                                      />
                                      <FieldDescription>
                                        Keep it concise. One or two sentences
                                        works best.
                                      </FieldDescription>
                                    </Field>
                                  )}
                                </form.Field>
                                <div className="grid gap-5 sm:grid-cols-2">
                                  <form.Field
                                    name={`menuItems[${index}].priceInRupees`}
                                    validators={{
                                      onChange: ({ value }) =>
                                        value > 0
                                          ? undefined
                                          : {
                                              message:
                                                "Enter a price greater than zero",
                                            },
                                    }}
                                  >
                                    {(field) => (
                                      <Field
                                        data-invalid={
                                          field.state.meta.errors.length > 0
                                        }
                                      >
                                        <FieldLabel htmlFor={`price-${index}`}>
                                          Price{" "}
                                          <span className="text-destructive">
                                            *
                                          </span>
                                        </FieldLabel>
                                        <div className="relative">
                                          <IndianRupee className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
                                          <Input
                                            id={`price-${index}`}
                                            type="number"
                                            min="0.01"
                                            step="0.01"
                                            value={field.state.value || ""}
                                            onBlur={field.handleBlur}
                                            onChange={(e) =>
                                              field.handleChange(
                                                Number(e.target.value)
                                              )
                                            }
                                            className="pl-9"
                                          />
                                          <FieldError
                                            errors={field.state.meta.errors}
                                          />
                                        </div>
                                        <FieldDescription>
                                          Stored as paise in your database.
                                        </FieldDescription>
                                      </Field>
                                    )}
                                  </form.Field>
                                  <form.Field
                                    name={`menuItems[${index}].caloriesKcal`}
                                  >
                                    {(field) => (
                                      <Field>
                                        <FieldLabel
                                          htmlFor={`calories-${index}`}
                                        >
                                          Calories
                                        </FieldLabel>
                                        <Input
                                          id={`calories-${index}`}
                                          type="number"
                                          min="0"
                                          value={field.state.value ?? ""}
                                          onChange={(e) =>
                                            field.handleChange(
                                              e.target.value === ""
                                                ? null
                                                : Number(e.target.value)
                                            )
                                          }
                                          placeholder="420"
                                        />
                                        <FieldError
                                          errors={field.state.meta.errors}
                                        />
                                      </Field>
                                    )}
                                  </form.Field>
                                </div>
                              </FieldGroup>
                              <Separator />
                              <FieldSet>
                                <FieldLegend>Classify your dish</FieldLegend>
                                <FieldDescription>
                                  These labels help guests discover the right
                                  dish.
                                </FieldDescription>
                                <FieldGroup className="mt-2">
                                  <form.Field
                                    name={`menuItems[${index}].cuisines`}
                                    validators={{
                                      onChange: ({ value }) =>
                                        value.length
                                          ? undefined
                                          : { message: "Choose a cuisine" },
                                    }}
                                  >
                                    {(field) => (
                                      <Field>
                                        <FieldLabel
                                          htmlFor={`cuisine-${index}`}
                                        >
                                          Primary cuisine
                                        </FieldLabel>
                                        <Select
                                          value={field.state.value[0]}
                                          onValueChange={(value) =>
                                            field.handleChange(
                                              value ? [value] : []
                                            )
                                          }
                                        >
                                          <SelectTrigger
                                            id={`cuisine-${index}`}
                                            className="w-full"
                                          >
                                            <SelectValue placeholder="Choose a cuisine" />
                                          </SelectTrigger>
                                          <SelectContent>
                                            <SelectGroup>
                                              <SelectLabel>Cuisine</SelectLabel>
                                              {menuItemCuisines.map(
                                                (cuisine) => (
                                                  <SelectItem
                                                    key={cuisine}
                                                    value={cuisine}
                                                  >
                                                    {formatMenuItemOption(
                                                      cuisine
                                                    )}
                                                  </SelectItem>
                                                )
                                              )}
                                            </SelectGroup>
                                          </SelectContent>
                                        </Select>
                                        <FieldError
                                          errors={field.state.meta.errors}
                                        />
                                      </Field>
                                    )}
                                  </form.Field>
                                  <form.Field
                                    name={`menuItems[${index}].foodTypes`}
                                    validators={{
                                      onChange: ({ value }) =>
                                        value.length
                                          ? undefined
                                          : {
                                              message:
                                                "Choose at least one food type",
                                            },
                                    }}
                                  >
                                    {(field) => (
                                      <Field>
                                        <FieldLabel>Food type</FieldLabel>
                                        <ToggleGroup
                                          multiple
                                          value={field.state.value}
                                          onValueChange={field.handleChange}
                                          className="flex flex-wrap justify-start gap-2"
                                        >
                                          {menuItemFoodTypes.map((type) => (
                                            <Toggle
                                              key={type}
                                              value={type}
                                              aria-label={type}
                                              className={toggleClassName}
                                            >
                                              {formatMenuItemOption(type)}
                                            </Toggle>
                                          ))}
                                        </ToggleGroup>
                                        <FieldError
                                          errors={field.state.meta.errors}
                                        />
                                      </Field>
                                    )}
                                  </form.Field>
                                  <form.Field
                                    name={`menuItems[${index}].timings`}
                                    validators={{
                                      onChange: ({ value }) =>
                                        value.length
                                          ? undefined
                                          : {
                                              message:
                                                "Choose at least one serving time",
                                            },
                                    }}
                                  >
                                    {(field) => (
                                      <Field>
                                        <FieldLabel>Serving times</FieldLabel>
                                        <ToggleGroup
                                          multiple
                                          value={field.state.value}
                                          onValueChange={(value) =>
                                            field.handleChange(
                                              normalizeAvailability(
                                                field.state.value,
                                                value
                                              )
                                            )
                                          }
                                          className="flex flex-wrap justify-start gap-2"
                                        >
                                          {menuItemAvailabilities.map(
                                            (time) => (
                                              <Toggle
                                                key={time}
                                                value={time}
                                                aria-label={time}
                                                className={toggleClassName}
                                              >
                                                {formatMenuItemOption(time)}
                                              </Toggle>
                                            )
                                          )}
                                        </ToggleGroup>
                                        <FieldError
                                          errors={field.state.meta.errors}
                                        />
                                      </Field>
                                    )}
                                  </form.Field>
                                </FieldGroup>
                              </FieldSet>
                            </div>
                            <aside className="flex flex-col gap-6">
                              <Card>
                                <CardHeader>
                                  <CardTitle className="text-base">
                                    Visibility & dietary
                                  </CardTitle>
                                  <CardDescription>
                                    Control how this item appears to guests.
                                  </CardDescription>
                                </CardHeader>
                                <CardContent>
                                  <FieldGroup>
                                    <form.Field
                                      name={`menuItems[${index}].isVeg`}
                                    >
                                      {(field) => (
                                        <Field orientation="horizontal">
                                          <FieldContent>
                                            <FieldLabel
                                              htmlFor={`veg-${index}`}
                                            >
                                              <Leaf className="size-4 text-muted-foreground" />
                                              Vegetarian
                                            </FieldLabel>
                                            <FieldDescription className="pl-6">
                                              Show the green veg marker
                                            </FieldDescription>
                                          </FieldContent>
                                          <Switch
                                            id={`veg-${index}`}
                                            checked={field.state.value}
                                            onCheckedChange={field.handleChange}
                                          />
                                        </Field>
                                      )}
                                    </form.Field>
                                    <form.Field
                                      name={`menuItems[${index}].isAvailable`}
                                    >
                                      {(field) => (
                                        <Field orientation="horizontal">
                                          <FieldContent>
                                            <FieldLabel
                                              htmlFor={`available-${index}`}
                                            >
                                              <Utensils className="size-4 text-muted-foreground" />
                                              Available to order
                                            </FieldLabel>
                                            <FieldDescription className="pl-6">
                                              Guests can order this dish
                                            </FieldDescription>
                                          </FieldContent>
                                          <Switch
                                            id={`available-${index}`}
                                            checked={field.state.value}
                                            onCheckedChange={field.handleChange}
                                          />
                                        </Field>
                                      )}
                                    </form.Field>
                                    <form.Field
                                      name={`menuItems[${index}].isActive`}
                                    >
                                      {(field) => (
                                        <Field orientation="horizontal">
                                          <FieldContent>
                                            <FieldLabel
                                              htmlFor={`active-${index}`}
                                            >
                                              <Eye className="size-4 text-muted-foreground " />
                                              Active on menu
                                            </FieldLabel>
                                            <FieldDescription className="pl-6">
                                              Keep this item visible
                                            </FieldDescription>
                                          </FieldContent>
                                          <Switch
                                            id={`active-${index}`}
                                            checked={field.state.value}
                                            onCheckedChange={field.handleChange}
                                          />
                                        </Field>
                                      )}
                                    </form.Field>
                                  </FieldGroup>
                                </CardContent>
                              </Card>
                            </aside>
                          </CardContent>
                          <CardFooter className="border-t bg-muted/20">
                            <span className="text-sm text-muted-foreground">
                              Item {index + 1} of {items.state.value.length}
                            </span>
                          </CardFooter>
                        </Card>
                      ))}
                    </>
                  )}
                </form.Field>
                <div className="flex flex-col-reverse gap-3 sm:flex-row sm:justify-end">
                  <Button type="button" variant="outline" onClick={addItem}>
                    <Plus data-icon="inline-start" /> Add another dish
                  </Button>
                  <form.Subscribe
                    selector={(state) => [state.canSubmit, state.isSubmitting]}
                  >
                    {([canSubmit, isSubmitting]) => (
                      <Button
                        type="submit"
                        disabled={!restaurantId || !canSubmit || isSubmitting}
                      >
                        {isSubmitting ? "Submitting..." : "Submit menu items"}
                      </Button>
                    )}
                  </form.Subscribe>
                </div>
              </fieldset>
            )}
          </form.Subscribe>
        </form>
      </div>
    </main>
  )
}
