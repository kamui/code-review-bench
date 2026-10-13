import {
  Ingredient,
  PantryFormData,
  PantryIngredientFormData,
  PantryUpdateFormData,
} from "@/utils/interfaces";
import { SubmitHandler, useFieldArray, useFormContext } from "react-hook-form";
import { Autocomplete } from "@base-ui/react/autocomplete";
import { useIngredientsStore } from "@/providers/ingredient-store-provider";
import { useShallow } from "zustand/react/shallow";
import { useEffect, useMemo, useRef, useState } from "react";
import { Drawer, Field, Separator } from "@base-ui/react";
import { Input } from "@/components/ui/input";
import { GroceryFormIngredientUnitSelect } from "@/components/ui/unit-select";
import { PriceDisplay } from "@/components/ui/price-display";
import { BagDeleteIcon } from "@/components/icons/grocery-bag/delete";
import { usePantryStore } from "@/providers/pantry-store-provider";
import { CircleResetIcon } from "@/components/icons/circle-reset-icon";
import { AddFridge } from "@/components/icons/fridge/add";

export const PantryForm = () => {
  const { control, handleSubmit, reset, formState, getValues } =
    useFormContext<PantryFormData>();
  const { fields, append, remove } = useFieldArray({
    control,
    name: "ingredients",
  });

  const { pantryIngredients, setPantryIngredients, pantryVersion } =
    usePantryStore(
      useShallow(
        ({ pantryIngredients, setPantryIngredients, pantryVersion }) => ({
          pantryIngredients,
          setPantryIngredients,
          pantryVersion,
        }),
      ),
    );

  const handleIngredientSelect = (ingredient: Ingredient) => {
    const pantryIngredients = getValues("ingredients");
    const indexOfFoundIngredient = pantryIngredients.findIndex(
      ({ ingredientPublicId }) => {
        return ingredientPublicId === ingredient.publicId;
      },
    );
    if (indexOfFoundIngredient === -1)
      append({
        name: ingredient.name,
        ingredientPublicId: ingredient.publicId,
        unit: ingredient.unit,
        quantity: undefined as unknown as number,
        capacity: undefined as unknown as number,
      });
  };

  const onResetHandler = () => {
    reset({ ingredients: pantryIngredients });
  };

  const filterChangedData = ({
    ingredients,
  }: PantryFormData): PantryUpdateFormData => {
    const originalIngredients = formState.defaultValues?.ingredients ?? [];

    const newIngredients: PantryIngredientFormData[] = [];
    const updatedIngredients: PantryIngredientFormData[] = [];
    const deletedIngredientIds: string[] = [];

    //   If ingredient.publicId exists in originalIngredients but not in ingredientChanges: deleted ingredient
    for (const originalIngredient of originalIngredients) {
      if (
        originalIngredient &&
        originalIngredient.publicId &&
        !ingredients.some(
          ({ publicId }) => publicId === originalIngredient?.publicId,
        )
      ) {
        deletedIngredientIds.push(originalIngredient.publicId);
      }
    }

    for (const dirtyIngredient of ingredients) {
      //   If no ingredient.publicId: new ingredient
      if (!dirtyIngredient.publicId) newIngredients.push(dirtyIngredient);
      else {
        //   If ingredient.publicId exists in originalIngredients: do difference check to see if updated
        const existingIngredient = originalIngredients.find(
          (originalIngredient) =>
            originalIngredient?.publicId === dirtyIngredient.publicId,
        );

        if (existingIngredient) {
          if (
            JSON.stringify(dirtyIngredient) !==
            JSON.stringify(existingIngredient)
          ) {
            updatedIngredients.push(dirtyIngredient);
          }
        }
      }
    }

    return {
      deletedIngredientIds,
      newIngredients,
      updatedIngredients,
    };
  };

  const onSaveHandler: SubmitHandler<PantryFormData> = async (pantryData) => {
    // "use server";

    const changedFields = filterChangedData(pantryData);

    const submitResponse = await fetch(`/api/pantry`, {
      method: "PATCH",
      body: JSON.stringify(changedFields, (_key, value) =>
        value === undefined ? null : value,
      ),
      headers: {
        "Content-Type": "application/json",
      },
    });

    if (submitResponse.status === 200 || submitResponse.status === 401) {
      const response = await submitResponse.json();
      const { pantryIngredients } = response;
      setPantryIngredients(pantryIngredients);
    }
    return;
  };

  const prevVersionRef = useRef<number | null>(null);
  useEffect(() => {
    if (prevVersionRef.current === pantryVersion) return;
    reset({ ingredients: pantryIngredients });
    prevVersionRef.current = pantryVersion;
  }, [pantryVersion, pantryIngredients, reset]);

  return (
    <div className="relative flex h-full flex-col font-medium">
      <form className={"flex h-full flex-col"}>
        <PantryIngredientSearch onSelect={handleIngredientSelect} />
        {fields.length > 0 ? (
          <div className="p-4 pt-0">
            <div
              className={
                "flex flex-col gap-4 lg:rounded-md lg:border lg:border-gray-200 lg:p-4"
              }
            >
              {fields.map(({ id, name }, index) => (
                <PantryIngredientRow
                  key={id}
                  index={index}
                  onRemove={() => remove(index)}
                  ingredientName={name}
                />
              ))}
            </div>
          </div>
        ) : null}

        {fields.length === 0 ? <EmptyPantry /> : null}

        <div className={"mt-auto"}>
          <Separator
            orientation="horizontal"
            className="h-px bg-gray-200 dark:bg-neutral-700"
          />

          {/* Clear pantry button */}
          <div className="flex justify-end gap-4 p-4">
            <button
              type="button"
              onClick={onResetHandler}
              className="group flex h-10 w-full cursor-pointer items-center justify-center gap-x-2 rounded-md border border-gray-200 px-4 font-medium tracking-widest hover:bg-red-500 hover:text-white lg:w-auto"
            >
              <CircleResetIcon className={"group-hover:fill-white"} />
              Restore
            </button>

            {/* Save pantry button */}
            <button
              type="button"
              onClick={handleSubmit(onSaveHandler)}
              className="group flex h-10 w-full cursor-pointer items-center justify-center gap-x-2 rounded-md border border-gray-200 bg-blue-500 px-4 font-medium tracking-widest text-white hover:bg-green-500 lg:w-auto"
            >
              <AddFridge className={"fill-white"} />
              Save pantry
            </button>
          </div>
        </div>
      </form>
    </div>
  );
};

const EmptyPantry = () => {
  return (
    <div className={"flex h-full flex-col gap-4"}>
      <div
        className={
          "flex h-2/3 flex-col justify-center p-4 text-center font-medium"
        }
      >
        <Drawer.Description className={"text-xl font-bold"}>
          Your pantry is empty.
        </Drawer.Description>
      </div>
    </div>
  );
};

// TODO: reuse for searching other lists; ingredients calculator name input on home page,
const PantryIngredientSearch = ({
  onSelect,
}: {
  onSelect: (ingredient: Ingredient) => void;
}) => {
  const [inputValue, setInputValue] = useState("");

  const ingredients = useIngredientsStore(
    useShallow(({ ingredients }) => ingredients),
  );

  const filteredItems = useMemo(() => {
    const q = inputValue.toLowerCase().trim();
    if (!q) return ingredients;

    return ingredients
      .filter((i) => i.name.toLowerCase().includes(q))
      .sort((a, b) => {
        const aName = a.name.toLowerCase();
        const bName = b.name.toLowerCase();
        if (aName === q && bName !== q) return -1;
        if (bName === q && aName !== q) return 1;
        if (aName.startsWith(q) && !bName.startsWith(q)) return -1;
        if (bName.startsWith(q) && !aName.startsWith(q)) return 1;
        return aName.localeCompare(bName);
      });
  }, [ingredients, inputValue]);

  const handleValueChange = (value: string) => {
    const match = ingredients.find(
      (i) => i.name.toLowerCase() === value.toLowerCase(),
    );
    if (match) {
      onSelect(match);
      setInputValue(""); // clear after a successful selection
    } else {
      setInputValue(value);
    }
  };

  return (
    <Autocomplete.Root
      value={inputValue}
      onValueChange={handleValueChange}
      items={filteredItems}
      itemToStringValue={(item) => item.name}
      mode="inline"
      openOnInputClick
      autoHighlight
    >
      <div className={"flex flex-row gap-4 p-4"}>
        <Autocomplete.Input
          placeholder="Add to pantry..."
          type="search"
          className={
            "text-md flex min-h-10 w-full rounded-md border border-gray-200 px-[15px] font-medium outline-none placeholder:font-normal placeholder:text-gray-400"
          }
        />
        <Autocomplete.Portal>
          <Autocomplete.Positioner sideOffset={10} align="start">
            <Autocomplete.Popup className="text-md z-50 w-[var(--anchor-width)] max-w-[var(--available-width)] rounded-md border border-gray-200 bg-white font-medium shadow-lg outline-none">
              <Autocomplete.Empty>
                <p className="m-2 px-3 py-2 text-sm text-gray-400">
                  No ingredients found
                </p>
              </Autocomplete.Empty>
              <Autocomplete.List className="max-h-60 overflow-y-auto">
                {(item: Ingredient) => (
                  <Autocomplete.Item
                    key={item.publicId}
                    value={item}
                    className="m-2 cursor-pointer rounded-md px-3 py-2 text-sm font-medium capitalize hover:bg-blue-500 hover:text-white data-[highlighted]:bg-blue-500 data-[highlighted]:text-white"
                  >
                    {item.name}
                  </Autocomplete.Item>
                )}
              </Autocomplete.List>
            </Autocomplete.Popup>
          </Autocomplete.Positioner>
        </Autocomplete.Portal>
      </div>
    </Autocomplete.Root>
  );
};

const PantryIngredientRow = ({
  index,
  onRemove,
  ingredientName,
}: {
  index: number;
  onRemove: () => void;
  ingredientName: string;
}) => {
  const { register, getFieldState, formState } = useFormContext();

  // TODO: use to display form errors
  const quantityState = getFieldState(
    `ingredients.${index}.quantity`,
    formState,
  );
  const capacityState = getFieldState(
    `ingredients.${index}.capacity`,
    formState,
  );

  return (
    <div className="flex w-full flex-col rounded-md border border-gray-200 p-4 lg:border-none lg:p-0">
      <div className="flex flex-row items-center justify-between gap-4 lg:hidden">
        <p className="flex min-h-10 w-full items-center rounded-md text-xl font-medium capitalize">
          {ingredientName}
        </p>

        <button
          className="group flex h-10 w-auto cursor-pointer items-center justify-center gap-x-2 rounded-md border border-gray-200 p-4 hover:bg-red-500 hover:text-white"
          type="button"
          onClick={onRemove}
        >
          <BagDeleteIcon className="h-6 fill-none stroke-red-500 group-hover:stroke-white" />
        </button>
      </div>

      <div className={"flex flex-row gap-4"}>
        <div className="grid w-full grid-cols-4 gap-x-4 gap-y-2 sm:grid-cols-4 lg:grid-cols-12">
          <Field.Root className="relative col-span-4 hidden sm:col-span-2 lg:col-span-4 lg:inline">
            <Field.Label
              className={`text-xs text-black uppercase opacity-50 ${index !== 0 ? "lg:hidden" : ""}`}
            >
              Name
            </Field.Label>
            <div className="hidden min-h-10 w-full items-center rounded-md text-lg font-medium capitalize lg:flex">
              {ingredientName}
            </div>
          </Field.Root>

          {/* Price — computed from master ingredient, read-only */}
          <div className="col-span-2 sm:col-span-1 lg:col-span-2">
            <Field.Root>
              <Field.Label
                className={`text-xs text-black uppercase opacity-50 ${index !== 0 ? "lg:hidden" : ""}`}
              >
                Price
              </Field.Label>
              <PriceDisplay index={index} />
            </Field.Root>
          </div>

          {/* Quantity */}
          <Field.Root
            className="col-span-2 sm:col-span-1 lg:col-span-2"
            invalid={quantityState.invalid}
            touched={quantityState.isTouched}
            dirty={quantityState.isDirty}
          >
            <Field.Label
              className={`text-xs text-black uppercase opacity-50 ${index !== 0 ? "lg:hidden" : ""}`}
            >
              (Quantity)
            </Field.Label>
            <Input
              placeholder="6"
              type="number"
              {...register(`ingredients.${index}.quantity`, {
                setValueAs: (val) => (val ? Number(val) : undefined),
              })}
            />
            <Field.Error className="mt-1 text-xs text-red-500">
              {quantityState.error?.message}
            </Field.Error>
          </Field.Root>

          {/* Capacity */}
          <Field.Root
            className="col-span-2 sm:col-span-1 lg:col-span-2"
            invalid={capacityState.invalid}
            touched={capacityState.isTouched}
            dirty={capacityState.isDirty}
          >
            <Field.Label
              className={`text-xs text-black uppercase opacity-50 ${index !== 0 ? "lg:hidden" : ""}`}
            >
              Capacity
            </Field.Label>
            <Input
              placeholder="0.710"
              step="0.001"
              type="number"
              {...register(`ingredients.${index}.capacity`, {
                setValueAs: (val) => (val ? Number(val) : undefined),
              })}
            />
            <Field.Error className="mt-1 text-xs text-red-500">
              {capacityState.error?.message}
            </Field.Error>
          </Field.Root>

          {/* Unit */}
          <div className="col-span-2 sm:col-span-1 lg:col-span-2">
            <Field.Root>
              <Field.Label
                className={`text-xs text-black uppercase opacity-50 ${index !== 0 ? "lg:hidden" : ""}`}
              >
                Unit
              </Field.Label>
              <GroceryFormIngredientUnitSelect index={index} />
            </Field.Root>
          </div>
        </div>

        {/* Delete — desktop layout only */}
        <div className="group mt-auto hidden flex-col lg:flex">
          <Field.Root>
            <button
              className="flex h-10 w-full cursor-pointer items-center justify-center rounded-md border border-gray-200 px-2 py-2 font-medium tracking-widest group-hover:bg-red-500 group-hover:text-white"
              type="button"
              onClick={onRemove}
            >
              <BagDeleteIcon className="h-6 fill-none stroke-red-500 group-hover:stroke-white" />
            </button>
          </Field.Root>
        </div>
      </div>
    </div>
  );
};