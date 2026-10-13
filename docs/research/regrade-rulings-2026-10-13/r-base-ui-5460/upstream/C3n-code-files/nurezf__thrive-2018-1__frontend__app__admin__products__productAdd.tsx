"use client";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
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
import { Button } from "@base-ui/react";
import axios from "axios";
import { z } from "zod";
import { toast } from "sonner";

type Category = {
  category_id: string;
  name: string;
};

const productSchema = z
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
    image: z
      .any()
      .refine((value) => value instanceof File, "Image is required"),
    categoryId: z.string().min(1, "Category is required"),
  })
  .refine((data) => !data.originalPrice || data.originalPrice >= data.price, {
    message: "Original price must be greater than or equal to price",
    path: ["originalPrice"],
  });

export default function ProductAdd() {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [price, setPrice] = useState(0);
  const [originalPrice, setOriginalPrice] = useState<number | "">("");
  const [discountPercentage, setDiscountPercentage] = useState<number | "">("");
  const [stock, setStock] = useState(0);
  const [image, setImage] = useState<File | null>(null);
  const [categoryId, setCategoryId] = useState<string>("");
  const [categories, setCategories] = useState<Category[]>([]);
  const [errors, setErrors] = useState<Record<string, string>>({});

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

    const parsed = productSchema.safeParse({
      name: name.trim(),
      description: description.trim(),
      price,
      originalPrice: originalPrice === "" ? undefined : originalPrice,
      discountPercentage:
        discountPercentage === "" ? undefined : discountPercentage,
      stock,
      image,
      categoryId,
    });

    if (!parsed.success) {
      const formattedErrors: Record<string, string> = {};
      parsed.error.issues.forEach((issue) => {
        const field = issue.path[0];
        if (field) {
          formattedErrors[String(field)] = issue.message;
        }
      });
      setErrors(formattedErrors);
      toast.error("Please fix the highlighted validation errors.");
      return;
    }

    try {
      const response = await axios.post("http://localhost:8000/api/product", {
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
      });
      const newProduct = response.data;

      if (parsed.data.image && newProduct.product_id) {
        const formData = new FormData();
        formData.append("images", parsed.data.image);
        try {
          await axios.post(
            `http://localhost:8000/api/product/${newProduct.product_id}/images`,
            formData,
            {
              headers: {
                "Content-Type": "multipart/form-data",
                Authorization: "Bearer dev_token",
              },
            },
          );
        } catch (imgError: unknown) {
          console.error("Image upload failed:", imgError);
          const message =
            axios.isAxiosError(imgError) && imgError.response?.data?.error
              ? String(imgError.response.data.error)
              : "Image upload failed.";
          toast.error(`Product added but image upload failed: ${message}`);
          window.location.reload();
          return;
        }
      }

      toast.success("Product and image added successfully!");
      window.location.reload();
    } catch (error: unknown) {
      console.error("Error creating product:", error);
      const message =
        axios.isAxiosError(error) && error.response?.data?.error
          ? String(error.response.data.error)
          : "Failed to add product.";
      toast.error(message);
    }
  };

  return (
    <Dialog>
      <DialogTrigger className="bg-blue-500 text-amber-50 p-2 rounded-md">
        Add Product
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add Product</DialogTitle>
          <DialogDescription>Add a new product to the store.</DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit}>
          <div className="grid gap-4 py-4">
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="name" className="text-right">
                Name
              </Label>
              <div className="col-span-3">
                <Input
                  id="name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  className="w-full"
                />
                {errors.name && (
                  <p className="mt-1 text-sm text-destructive">{errors.name}</p>
                )}
              </div>
            </div>
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="description" className="text-right">
                Description
              </Label>
              <div className="col-span-3">
                <Input
                  id="description"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  className="w-full"
                />
                {errors.description && (
                  <p className="mt-1 text-sm text-destructive">
                    {errors.description}
                  </p>
                )}
              </div>
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
              <Label htmlFor="image" className="text-right">
                Image
              </Label>
              <div className="col-span-3">
                <Input
                  id="image"
                  type="file"
                  accept="image/*"
                  onChange={(e) => setImage(e.target.files?.[0] || null)}
                  className="w-full"
                />
                {errors.image && (
                  <p className="mt-1 text-sm text-destructive">
                    {errors.image}
                  </p>
                )}
              </div>
            </div>
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="category" className="text-right">
                Categories
              </Label>
              <div className="col-span-3">
                <Select
                  onValueChange={(value: string) => setCategoryId(value)}
                  value={categoryId}
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
          className="w-full bg-black p-2 text-white rounded-md"
          onClick={handleSubmit}
        >
          Add Product
        </Button>
      </DialogContent>
    </Dialog>
  );
}
