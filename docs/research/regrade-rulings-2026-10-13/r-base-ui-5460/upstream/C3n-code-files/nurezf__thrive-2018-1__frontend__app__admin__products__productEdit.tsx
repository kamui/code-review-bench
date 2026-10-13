"use client";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { useState, useEffect } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { z } from "zod";
import { Button } from "@base-ui/react";
import axios from "axios";
import { Product } from "./page";
import { toast } from "sonner";

type Category = {
  category_id: string;
  name: string;
};

const productEditSchema = z
  .object({
    name: z.string().min(3, "Name must be at least 3 characters"),
    description: z
      .string()
      .min(10, "Description must be at least 10 characters"),
    price: z.number().positive("Price must be greater than 0"),
    originalPrice: z
      .number()
      .positive("Original price must be greater than 0")
      .optional(),
    discountPercentage: z
      .number()
      .min(0, "Discount percentage cannot be negative")
      .max(100, "Discount percentage cannot exceed 100")
      .optional(),
    stock: z
      .number()
      .int("Stock must be a whole number")
      .nonnegative("Stock cannot be negative"),
    categoryId: z.string().min(1, "Category is required"),
  })
  .refine((data) => !data.originalPrice || data.originalPrice >= data.price, {
    message: "Original price must be greater than or equal to price",
    path: ["originalPrice"],
  });

interface ProductEditProps {
  product: Product;
}

export default function ProductEdit({ product }: ProductEditProps) {
  const [name, setName] = useState(product.name || "");
  const [description, setDescription] = useState(product.description || "");
  const [price, setPrice] = useState(Number(product.price) || 0);
  const [originalPrice, setOriginalPrice] = useState<number | "">(
    product.original_price ? Number(product.original_price) : "",
  );
  const [discountPercentage, setDiscountPercentage] = useState<number | "">(
    product.discount_percentage ? Number(product.discount_percentage) : "",
  );
  const [stock, setStock] = useState(product.stock || 0);
  const [categoryId, setCategoryId] = useState<string>(
    product.category_id || "",
  );
  const [categories, setCategories] = useState<Category[]>([]);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    const fetchCategories = async () => {
      const response = await fetch("http://localhost:8000/api/categories");
      const data = await response.json();
      setCategories(data);
    };
    fetchCategories();
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrors({});

    const parsed = productEditSchema.safeParse({
      name: name.trim(),
      description: description.trim(),
      price,
      originalPrice: originalPrice === "" ? undefined : originalPrice,
      discountPercentage:
        discountPercentage === "" ? undefined : discountPercentage,
      stock,
      categoryId,
    });

    if (!parsed.success) {
      const formattedErrors: Record<string, string> = {};
      parsed.error.errors.forEach((issue) => {
        if (issue.path[0]) {
          formattedErrors[String(issue.path[0])] = issue.message;
        }
      });
      setErrors(formattedErrors);
      toast.error("Please fix the highlighted validation errors.");
      return;
    }

    try {
      await axios.put(
        `http://localhost:8000/api/product/${product.product_id}`,
        {
          name: parsed.data.name,
          description: parsed.data.description,
          price: parsed.data.price,
          original_price:
            parsed.data.originalPrice !== undefined
              ? parsed.data.originalPrice
              : null,
          discount_percentage:
            parsed.data.discountPercentage !== undefined
              ? parsed.data.discountPercentage
              : null,
          stock: parsed.data.stock,
          category_id: parsed.data.categoryId,
        },
      );

      toast.success("Product updated successfully!");
      setIsOpen(false);
      window.location.reload();
    } catch (error: unknown) {
      console.error("Error updating product:", error);
      const message =
        axios.isAxiosError(error) && error.response?.data?.error
          ? String(error.response.data.error)
          : "Failed to update product.";
      toast.error(message);
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogTrigger
        className={cn(buttonVariants({ variant: "outline", size: "sm" }))}
      >
        Edit
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Edit Product</DialogTitle>
          <DialogDescription>Update product details.</DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit}>
          <div className="grid gap-4 py-4">
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="name" className="text-right">
                Name
              </Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="col-span-3"
              />
            </div>
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="description" className="text-right">
                Description
              </Label>
              <Input
                id="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="col-span-3"
              />
            </div>
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="price" className="text-right">
                Price
              </Label>
              <div className="col-span-3">
                <Input
                  id="price"
                  type="number"
                  min="0"
                  step="0.01"
                  value={price}
                  onChange={(e) => setPrice(Number(e.target.value))}
                  className="w-full"
                />
                {errors.price && (
                  <p className="mt-1 text-sm text-destructive">
                    {errors.price}
                  </p>
                )}
              </div>
            </div>
            <div className="grid grid-cols-4 items-center gap-4">
              <Label
                htmlFor="originalPrice"
                className="text-right whitespace-nowrap overflow-hidden text-ellipsis"
              >
                Original Price
              </Label>
              <div className="col-span-3">
                <Input
                  id="originalPrice"
                  type="number"
                  min="0"
                  step="0.01"
                  value={originalPrice}
                  onChange={(e) =>
                    setOriginalPrice(
                      e.target.value === "" ? "" : Number(e.target.value),
                    )
                  }
                  className="w-full"
                />
                {errors.originalPrice && (
                  <p className="mt-1 text-sm text-destructive">
                    {errors.originalPrice}
                  </p>
                )}
              </div>
            </div>
            <div className="grid grid-cols-4 items-center gap-4">
              <Label
                htmlFor="discountPercentage"
                className="text-right whitespace-nowrap overflow-hidden text-ellipsis"
              >
                Discount %
              </Label>
              <div className="col-span-3">
                <Input
                  id="discountPercentage"
                  type="number"
                  min="0"
                  max="100"
                  step="1"
                  value={discountPercentage}
                  onChange={(e) =>
                    setDiscountPercentage(
                      e.target.value === "" ? "" : Number(e.target.value),
                    )
                  }
                  className="w-full"
                />
                {errors.discountPercentage && (
                  <p className="mt-1 text-sm text-destructive">
                    {errors.discountPercentage}
                  </p>
                )}
              </div>
            </div>
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="stock" className="text-right">
                Stock
              </Label>
              <div className="col-span-3">
                <Input
                  id="stock"
                  type="number"
                  min="0"
                  step="1"
                  value={stock}
                  onChange={(e) => setStock(Number(e.target.value))}
                  className="w-full"
                />
                {errors.stock && (
                  <p className="mt-1 text-sm text-destructive">
                    {errors.stock}
                  </p>
                )}
              </div>
            </div>
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="category" className="text-right">
                Category
              </Label>
              <div className="col-span-3">
                <Select
                  value={categoryId}
                  onValueChange={(value: string) => setCategoryId(value)}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Select a category" />
                  </SelectTrigger>
                  <SelectContent>
                    {categories.map((category) => (
                      <SelectItem
                        key={category.category_id}
                        value={category.category_id}
                      >
                        {category.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {errors.categoryId && (
                  <p className="mt-1 text-sm text-destructive">
                    {errors.categoryId}
                  </p>
                )}
              </div>
            </div>
          </div>
        </form>
        <Button
          type="submit"
          className="bg-black text-white hover:bg-black-600 p-2 rounded-md"
          onClick={handleSubmit}
        >
          Save Changes
        </Button>
      </DialogContent>
    </Dialog>
  );
}
