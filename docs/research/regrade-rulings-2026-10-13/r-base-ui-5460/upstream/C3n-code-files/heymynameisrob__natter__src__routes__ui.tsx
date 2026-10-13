import * as React from "react";
import { createFileRoute } from "@tanstack/react-router";
import { z } from "zod";
import { Form } from "@base-ui-components/react/form";
import { Button, buttonStyles } from "@/components/Button";
import {
  Field,
  FieldLabel,
  FieldDescription,
  FieldError,
  Fieldset,
  FieldsetLegend,
} from "@/components/Field";
import { Input } from "@/components/Input";
import { Label } from "@/components/Label";
import { Avatar } from "@/components/Avatar";
import { Checkbox } from "@/components/Checkbox";
import { RadioGroup, RadioGroupItem } from "@/components/Radio";
import { Tooltip, TooltipProvider } from "@/components/Tooltip";
import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/Dialog";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/Popover";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuCheckboxItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuLabel,
} from "@/components/DropdownMenu";
import { isDev } from "@/lib/flags";
import { ThemeToggle } from "@/components/ThemeProvider";

export const Route = createFileRoute("/ui")({
  loader: () => {
    if (!isDev) {
      throw new Response("Not Found", { status: 404 });
    }
    return {};
  },
  component: UIPage,
});

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-4">
      <h2 className="text-xl font-semibold text-primary">{title}</h2>
      <div className="p-2 -mx-2">{children}</div>
    </section>
  );
}

function UIPage() {
  const [checkboxChecked, setCheckboxChecked] = React.useState(false);
  const [radioValue, setRadioValue] = React.useState("option1");
  const [menuCheckbox1, setMenuCheckbox1] = React.useState(true);
  const [menuCheckbox2, setMenuCheckbox2] = React.useState(false);
  const [menuRadioValue, setMenuRadioValue] = React.useState("radio1");
  const [formErrors, setFormErrors] = React.useState<Record<string, string | string[]>>({});
  const [formLoading, setFormLoading] = React.useState(false);

  const formSchema = z.object({
    username: z
      .string()
      .min(3, "Username must be at least 3 characters")
      .regex(/^[a-zA-Z0-9_]+$/, "Only letters, numbers, and underscores"),
    email: z
      .string()
      .email("Please enter a valid email")
      .refine(val => !val.endsWith("@example.com"), "Example domain not allowed"),
    password: z.string().min(8, "Password must be at least 8 characters"),
    age: z.coerce.number().min(18, "Must be at least 18").max(120, "Must be 120 or under"),
  });

  async function handleFormSubmit(formValues: Form.Values) {
    setFormLoading(true);
    await new Promise(resolve => setTimeout(resolve, 1000));
    const result = formSchema.safeParse(formValues);

    if (!result.success) {
      const fieldErrors = z.flattenError(result.error).fieldErrors;
      const errors: Record<string, string | string[]> = {};
      for (const [key, value] of Object.entries(fieldErrors)) {
        if (value) errors[key] = value;
      }
      setFormErrors(errors);
      setFormLoading(false);
      return;
    }

    setFormErrors({});
    setFormLoading(false);
  }

  return (
    <main className="flex flex-col gap-8 py-16 max-w-4xl mx-auto">
      <header className="flex items-center justify-between gap-4">
        <h1 className="text-3xl font-bold text-primary">UI Components</h1>
        <ThemeToggle />
      </header>

      <Section title="Button">
        <div className="space-y-4">
          <div className="flex flex-wrap gap-3">
            <Button variant="primary">Primary</Button>
            <Button variant="secondary">Secondary</Button>
            <Button variant="accent">Accent</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="destructive">Destructive</Button>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <Button size="sm">Small</Button>
            <Button size="md">Medium</Button>
            <Button size="lg">Large</Button>
            <Button size="icon">
              <span>+</span>
            </Button>
          </div>
          <div className="flex flex-wrap gap-3">
            <Button disabled>Disabled</Button>
          </div>
        </div>
      </Section>

      <Section title="Input">
        <div className="space-y-4">
          <Field>
            <FieldLabel>Email</FieldLabel>
            <Input type="email" placeholder="Enter your email" />
          </Field>
          <Field>
            <FieldLabel>Password</FieldLabel>
            <Input type="password" placeholder="Enter your password" />
            <FieldDescription>Must be at least 8 characters</FieldDescription>
          </Field>
          <Field>
            <FieldLabel>Disabled</FieldLabel>
            <Input disabled placeholder="Disabled input" />
          </Field>
        </div>
      </Section>

      <Section title="Checkbox">
        <div className="flex items-center gap-3">
          <Checkbox
            id="terms"
            checked={checkboxChecked}
            onCheckedChange={checked => setCheckboxChecked(checked as boolean)}
          />
          <Label htmlFor="terms">Accept terms and conditions</Label>
        </div>
      </Section>

      <Section title="Radio">
        <RadioGroup value={radioValue} onValueChange={value => setRadioValue(value as string)}>
          <div className="flex items-center gap-3">
            <RadioGroupItem value="option-1" />
            <Label htmlFor="option-1">Option 1</Label>
          </div>
          <div className="flex items-center gap-3">
            <RadioGroupItem value="option-2" />
            <Label htmlFor="option-2">Option 2</Label>
          </div>
          <div className="flex items-center gap-3">
            <RadioGroupItem value="option3" />
            <Label>Option 3</Label>
          </div>
        </RadioGroup>
      </Section>

      <Section title="Avatar">
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            <Avatar src="https://i.pravatar.cc/150?img=1" fallback="JD" size="xs" />
            <Avatar src="https://i.pravatar.cc/150?img=2" fallback="JD" size="sm" />
            <Avatar src="https://i.pravatar.cc/150?img=3" fallback="JD" size="md" />
            <Avatar src="https://i.pravatar.cc/150?img=4" fallback="JD" size="lg" />
            <Avatar src="https://i.pravatar.cc/150?img=5" fallback="JD" size="xl" />
          </div>
          <div className="flex items-center gap-3">
            <Avatar
              src="https://i.pravatar.cc/150?img=6"
              fallback="AB"
              size="lg"
              variant="circle"
            />
            <Avatar
              src="https://i.pravatar.cc/150?img=7"
              fallback="CD"
              size="lg"
              variant="rounded"
            />
          </div>
        </div>
      </Section>

      <Section title="Tooltip">
        <div className="flex gap-4">
          <Tooltip content="This is a tooltip">
            <Button>Hover me</Button>
          </Tooltip>
          <Tooltip content="Top tooltip" side="top">
            <Button variant="ghost">Top</Button>
          </Tooltip>
          <Tooltip content="Bottom tooltip" side="bottom">
            <Button variant="ghost">Bottom</Button>
          </Tooltip>
          <Tooltip content="Left tooltip" side="left">
            <Button variant="ghost">Left</Button>
          </Tooltip>
          <Tooltip content="Right tooltip" side="right">
            <Button variant="ghost">Right</Button>
          </Tooltip>
        </div>
      </Section>

      <Section title="Dialog">
        <Dialog>
          <DialogTrigger className={buttonStyles({ variant: "secondary", size: "md" })}>
            Open Dialog
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Dialog Title</DialogTitle>
              <DialogDescription>
                This is a dialog description. It provides context about the dialog content.
              </DialogDescription>
            </DialogHeader>
            <div className="py-4">
              <p className="text-primary">Dialog content goes here.</p>
            </div>
            <DialogFooter>
              <Button variant="ghost">Cancel</Button>
              <Button variant="primary">Confirm</Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </Section>

      <Section title="Popover">
        <Popover>
          <PopoverTrigger className={buttonStyles({ variant: "secondary", size: "md" })}>
            Open popover
          </PopoverTrigger>
          <PopoverContent align="start" className="p-4">
            <div className="space-y-2">
              <h4 className="font-medium text-primary">Popover Title</h4>
              <p className="text-sm text-secondary">
                This is the popover content. You can put anything here.
              </p>
            </div>
          </PopoverContent>
        </Popover>
      </Section>

      <Section title="Dropdown Menu">
        <DropdownMenu>
          <DropdownMenuTrigger className={buttonStyles({ variant: "secondary", size: "md" })}>
            Open menu
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            <DropdownMenuLabel>My Account</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem>Profile</DropdownMenuItem>
            <DropdownMenuItem>Settings</DropdownMenuItem>
            <DropdownMenuItem>Billing</DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuCheckboxItem
              checked={menuCheckbox1}
              onCheckedChange={checked => setMenuCheckbox1(checked as boolean)}
            >
              Show notifications
            </DropdownMenuCheckboxItem>
            <DropdownMenuCheckboxItem
              checked={menuCheckbox2}
              onCheckedChange={checked => setMenuCheckbox2(checked as boolean)}
            >
              Enable sounds
            </DropdownMenuCheckboxItem>
            <DropdownMenuSeparator />
            <DropdownMenuRadioGroup
              value={menuRadioValue}
              onValueChange={value => setMenuRadioValue(value as string)}
            >
              <DropdownMenuRadioItem value="radio1">Light</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="radio2">Dark</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="radio3">System</DropdownMenuRadioItem>
            </DropdownMenuRadioGroup>
            <DropdownMenuSeparator />
            <DropdownMenuItem>Log out</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </Section>

      <Section title="Fieldset">
        <Fieldset>
          <FieldsetLegend>Account Information</FieldsetLegend>
          <Field>
            <FieldLabel>Full Name</FieldLabel>
            <Input placeholder="John Doe" />
          </Field>
          <Field>
            <FieldLabel>Email</FieldLabel>
            <Input type="email" placeholder="john@example.com" />
            <FieldDescription>We'll never share your email</FieldDescription>
          </Field>
        </Fieldset>
      </Section>

      <Section title="Form">
        <Form
          className="flex w-full max-w-80 flex-col gap-4"
          errors={formErrors}
          onFormSubmit={handleFormSubmit}
        >
          <Field name="username">
            <FieldLabel>Username</FieldLabel>
            <Input name="username" placeholder="john_doe" />
            <FieldDescription>Letters, numbers, and underscores only</FieldDescription>
            <FieldError />
          </Field>
          <Field name="email">
            <FieldLabel>Email</FieldLabel>
            <Input name="email" type="email" placeholder="john@company.com" />
            <FieldError />
          </Field>
          <Field name="password">
            <FieldLabel>Password</FieldLabel>
            <Input name="password" type="password" placeholder="••••••••" />
            <FieldDescription>Must be at least 8 characters</FieldDescription>
            <FieldError />
          </Field>
          <Field name="age">
            <FieldLabel>Age</FieldLabel>
            <Input name="age" type="number" placeholder="25" />
            <FieldError />
          </Field>
          <Button type="submit" disabled={formLoading}>
            {formLoading ? "Submitting..." : "Submit"}
          </Button>
        </Form>
      </Section>
    </main>
  );
}
