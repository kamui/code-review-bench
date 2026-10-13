import React, { useEffect } from "react";
import { useForm } from "react-hook-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Button, Field, Input, Textarea } from "../ui";
import { toast } from "sonner";
import {
  createReminder,
  getContactMethods,
  createContactMethod,
  updateReminder,
  Reminder,
  UpdateReminderRequest,
} from "../../api/reminders";
import {
  getCurrentDateTimeString,
  getDateTimeStringInMinutes,
  getTomorrowDateTimeString,
} from "../../utils/datetime";
import { DEFAULT_USER_ID, TIME_PRESETS } from "../../constants";
import {
  ReminderTypeSelector,
  TimePresetButtons,
  RepeatIntervalSelector,
  ContactMethodSelector,
  MessageInput,
} from ".";
import { formatPhoneNumber } from "../ui/PhoneInput";
import { Link } from "react-router-dom";

interface ReminderFormData {
  body?: string;
  contactMethodID?: number;
  isRepeating?: boolean;
  intervalDays?: number;
  intervalHours?: number;
  intervalMinutes?: number;
  startTime?: string;
  newContactMethodType?: "phone" | "email";
  newContactMethodValue?: string;
  newContactMethodDescription?: string;
}

interface ReminderFormProps {
  reminderID?: number;
  onSuccess?: () => void;
  initialData?: Partial<ReminderFormData>;
}

const ReminderForm: React.FC<ReminderFormProps> = ({
  reminderID,
  onSuccess,
  initialData,
}) => {
  const {
    register,
    handleSubmit,
    reset,
    setValue,
    watch,
    formState: {},
  } = useForm<ReminderFormData>({
    defaultValues: {
      body: initialData?.body || "",
      isRepeating: initialData?.isRepeating || false,
      contactMethodID: initialData?.contactMethodID || 0,
      intervalDays: initialData?.intervalDays || 0,
      intervalHours: initialData?.intervalHours || 1,
      intervalMinutes: initialData?.intervalMinutes || 0,
      startTime: initialData?.startTime || "",
      newContactMethodType: "email",
    },
  });

  const isRepeating = watch("isRepeating");
  const newContactMethodType = watch("newContactMethodType");

  const { data: contactMethods = [] } = useQuery({
    queryKey: ["contactMethods"],
    queryFn: () => getContactMethods(),
  });

  useEffect(() => {
    if (watch("contactMethodID") === 0 && contactMethods?.length) {
      setValue("contactMethodID", contactMethods[0].id);
    }
  }, [contactMethods]);

  const createMutation = useMutation({
    mutationFn: createReminder,
    onSuccess: () => {
      reset();
      toast.success("Reminder created");
      onSuccess?.();
    },
    onError: (error) => {
      toast.error("Failed to create reminder", {
        description: error.message,
      });
    },
  });

  const updateMutation = useMutation({
    mutationFn: (updateRequest: UpdateReminderRequest) =>
      updateReminder(reminderID!, updateRequest),
    onSuccess: () => {
      reset();
      toast.success("Reminder updated");
      onSuccess?.();
    },
    onError: (error) => {
      toast.error("Failed to update reminder", {
        description: error.message,
      });
    },
  });

  const setStartTimeToNow = () => {
    setValue("startTime", getCurrentDateTimeString());
  };

  const setStartTimeIn = (minutes: number) => {
    setValue("startTime", getDateTimeStringInMinutes(minutes));
  };

  const setStartTimeTomorrow = (hour: number, minute = 0) => {
    setValue("startTime", getTomorrowDateTimeString(hour, minute));
  };

  const onSubmit = async (data: ReminderFormData) => {
    let periodMinutes = 0;
    if (data.isRepeating) {
      const days = data.intervalDays || 0;
      const hours = data.intervalHours || 0;
      const minutes = data.intervalMinutes || 0;
      periodMinutes = days * 24 * 60 + hours * 60 + minutes;
    }

    const contactMethodId = data.contactMethodID;
    if (!contactMethodId) {
      console.error("No contact method selected or created");
      return;
    }

    if (reminderID) {
      return updateMutation.mutate({
        body: data.body || "",
        start_time: new Date(data.startTime!).toISOString(),
        is_repeating: data.isRepeating || false,
        period_minutes: periodMinutes,
        contact_method_id: contactMethodId,
      });
    }

    createMutation.mutate({
      body: data.body || "",
      start_time: new Date(data.startTime!).toISOString(),
      is_repeating: data.isRepeating || false,
      period_minutes: periodMinutes,
      contact_method_id: contactMethodId,
    });
  };

  return (
    <div className="flex flex-col h-full bg-white">
      <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col h-full">
        {/* Main content area with scroll */}
        <div className="flex-1 overflow-y-auto space-y-4 p-4">
          <div>
            <ReminderTypeSelector
              value={isRepeating || false}
              onChange={(value) => setValue("isRepeating", value)}
            />
            <Field.Root>
              <Field.Control
                render={
                  <Input type="datetime-local" {...register("startTime")} />
                }
              />
            </Field.Root>

            {/* Quick Time Buttons */}
            <div className="mt-3">
              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={setStartTimeToNow}
                  className="rounded-full flex-1"
                >
                  Now
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => setStartTimeIn(TIME_PRESETS.FIFTEEN_MINUTES)}
                  className="rounded-full flex-1"
                >
                  15m
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => setStartTimeIn(TIME_PRESETS.ONE_HOUR)}
                  className="rounded-full flex-1"
                >
                  1h
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    setStartTimeTomorrow(
                      TIME_PRESETS.TOMORROW_9AM.hour,
                      TIME_PRESETS.TOMORROW_9AM.minute
                    )
                  }
                  className="rounded-full flex-1"
                >
                  Tmr 9am
                </Button>
              </div>
            </div>
          </div>

          {isRepeating && (
            <div className="space-y-4">
              <label className="text-start block text-sm font-medium text-gray-700 mb-2">
                Repeats every
              </label>
              <div className="grid grid-cols-3 gap-4">
                <Field.Root>
                  <Field.Control
                    render={
                      <Input
                        type="number"
                        {...register("intervalMinutes", {
                          min: 0,
                          max: 59,
                          valueAsNumber: true,
                        })}
                        placeholder="0"
                        min={0}
                        max={59}
                        onFocus={(e) => e.target.select()}
                      />
                    }
                  />
                  <Field.Description>Minutes</Field.Description>
                </Field.Root>

                <Field.Root>
                  <Field.Control
                    render={
                      <Input
                        type="number"
                        {...register("intervalHours", {
                          min: 0,
                          max: 23,
                          valueAsNumber: true,
                        })}
                        placeholder="0"
                        min={0}
                        max={23}
                        onFocus={(e) => e.target.select()}
                      />
                    }
                  />
                  <Field.Description>Hours</Field.Description>
                </Field.Root>

                <Field.Root>
                  <Field.Control
                    render={
                      <Input
                        type="number"
                        {...register("intervalDays", {
                          min: 0,
                          valueAsNumber: true,
                        })}
                        placeholder="0"
                        min={0}
                        onFocus={(e) => e.target.select()}
                      />
                    }
                  />
                  <Field.Description>Days</Field.Description>
                </Field.Root>
              </div>
            </div>
          )}
        </div>

        {/* Sticky footer */}
        <div className="border-t border-gray-200 bg-gray-100 p-4 space-y-3 rounded-b-lg">
          <label className="flex  justify-between items-center text-sm font-medium text-gray-700 rounded-lg">
            <span>Contact method</span>
            <Field.Root>
              {!!contactMethods?.length && (
                <div className="space-y-3">
                  <div>
                    <select
                      {...register("contactMethodID", {
                        valueAsNumber: true,
                      })}
                      value={watch("contactMethodID")}
                      className="bg-white text-sm w-full px-2 py-1 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                    >
                      <option value="0">Select a contact method</option>
                      {contactMethods?.map((method) => (
                        <option key={method.id} value={method.id}>
                          {method.description}{" "}
                          {method.type === "email" && `| ${method.value}`}
                          {method.type === "phone" &&
                            `| ${formatPhoneNumber(method.value)
                              .replaceAll("(", "")
                              .replaceAll(")", "")}`}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
              )}

              {contactMethods?.length === 0 && (
                <div className="space-y-3 mt-3">
                  {contactMethods?.length === 0 && (
                    <p className="text-sm text-gray-600">
                      No contact methods found. Create your first one:
                    </p>
                  )}

                  <div className="flex gap-3">
                    <label className="flex items-center gap-2">
                      <input
                        type="radio"
                        value="email"
                        {...register("newContactMethodType")}
                        className="w-4 h-4 text-blue-600 border-gray-300 focus:ring-blue-500"
                      />
                      <span className="text-sm">Email</span>
                    </label>
                    <label className="flex items-center gap-2">
                      <input
                        type="radio"
                        value="phone"
                        {...register("newContactMethodType")}
                        className="w-4 h-4 text-blue-600 border-gray-300 focus:ring-blue-500"
                      />
                      <span className="text-sm">Phone</span>
                    </label>
                  </div>

                  <Input
                    {...register("newContactMethodValue")}
                    placeholder={
                      newContactMethodType === "email"
                        ? "user@example.com"
                        : "+1234567890"
                    }
                    type={newContactMethodType === "email" ? "email" : "tel"}
                  />

                  <Input
                    {...register("newContactMethodDescription")}
                    placeholder="Description (e.g., Personal email, Work phone)"
                  />
                </div>
              )}
            </Field.Root>
          </label>

          {(!contactMethods?.length || watch("contactMethodID") === 0) && (
            <p className="text-sm text-gray-600 text-end">
              Don't see the right contact method? Create one{" "}
              <Link
                to="/settings"
                className="text-blue-500"
                target="_blank"
                rel="noopener noreferrer"
              >
                here
              </Link>
              .
            </p>
          )}
          <Field.Root>
            <Field.Control
              render={
                <Textarea
                  {...register("body")}
                  className="w-full"
                  placeholder="Message"
                  rows={3}
                />
              }
            />
          </Field.Root>

          <Button
            type="submit"
            disabled={
              createMutation.isPending ||
              updateMutation.isPending ||
              !watch("body") ||
              !watch("startTime") ||
              !watch("contactMethodID")
            }
            className="w-full"
          >
            {reminderID
              ? updateMutation.isPending
                ? "Saving..."
                : "Save"
              : createMutation.isPending
              ? "Creating..."
              : "Create Reminder"}
          </Button>
        </div>
      </form>
    </div>
  );
};

export default ReminderForm;
