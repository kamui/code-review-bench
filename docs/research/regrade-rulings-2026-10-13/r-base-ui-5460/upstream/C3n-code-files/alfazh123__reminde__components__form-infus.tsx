"use client";

import { useState } from "react";
import { Dialog, DialogClose, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";
import { Field, FieldGroup } from "./ui/field";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Button } from "./ui/button";
import { Dialog as DialogPrimitive } from "@base-ui/react/dialog";
import { XIcon } from "lucide-react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { dropFactorList } from "@/app/utils";
import { addDoc, collection } from "firebase/firestore";
import { db } from "@/app/firebase";
import { Separator } from "./ui/separator";

export default function FormInfus({isOpen, onClose, level}: {isOpen: boolean, onClose: () => void, level: string}) {
    const [form, setForm] = useState({
        roomId: "",
        startTime: Date.now(),
        rangeHour: 0,      // Disimpan sebagai angka jam (misal: 4 atau 2.5)
        volume: 0,
        dropFactor: 20,
    })

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();

        // Konversi input ke tipe angka untuk perhitungan
        const volNum = Number(form.volume) || 0;
        const dropNum = Number(form.dropFactor) || 20;
        const hours = Number(form.rangeHour) || 0;

        // Hitung Kecepatan Tetesan (TPM / Hasil)
        // Rumus: (Volume * DropFactor) / (Rentang Jam * 60 menit)
        const resultTpm = hours > 0 
            ? Math.floor((volNum * dropNum) / (hours * 60)) 
            : 0;

        const startDate = new Date(form.startTime);

        const infusData = {
            id: Math.random().toString(36).substr(2, 9), // Generate random ID
            roomId: form.roomId,
            timeStart: startDate,
            rangeTime: hours,             // Rentang waktu dalam bentuk jam
            volume: form.volume,
            dropFactor: form.dropFactor,
            result: resultTpm.toString(), // Hasil TPM yang dihitung otomatis
            createdAt: new Date(),
            category: "infus"
        };

        try {
            // 2. Tentukan nama collection secara dinamis berdasarkan nilai level
            const collectionName = `gdh-level-${level || "1"}`;

            // 3. Simpan ke Firestore (Collection otomatis dibuat jika belum ada)
            await addDoc(collection(db, collectionName), infusData);

            console.log("Data Infus Berhasil Disimpan ke:", collectionName);
            
            // Karena Dashboard Anda menggunakan onSnapshot, data di tabel 
            // akan otomatis sinkron tanpa perlu setInfusData manual lagi.
            onClose(); 
        } catch (error) {
            console.error("Gagal menyimpan data infus:", error);
            alert("Terjadi kesalahan saat menyimpan data.");
        }
    }

    return (
		<Dialog
			open={isOpen}
			onOpenChange={onClose}>
			<DialogContent
				className="sm:max-w-lg"
				showCloseButton={false}>
				<form
					onSubmit={handleSubmit}
					className="flex flex-col gap-2">
					<DialogHeader>
						<DialogTitle>Infus Form (Level {level})</DialogTitle>
					</DialogHeader>
					<Separator />
					<FieldGroup>
						<Field>
							<Label htmlFor="room-id">Nomor Kamar</Label>
							<Input
								id="room-id"
								name="room-id"
								placeholder="Masukkan nomor kamar"
								onChange={(e) =>
									setForm({ ...form, roomId: e.target.value })
								}
								required
							/>
						</Field>
						<Field>
							<Label htmlFor="start-time">Waktu Mulai</Label>
							<Input
								id="start-time"
								name="start-time"
								type="datetime-local"
								onChange={(e) =>
									setForm({
										...form,
										startTime: new Date(
											e.target.value,
										).getTime(),
									})
								}
								required
							/>
						</Field>
						<Field>
							<Label htmlFor="range-time">
								Rentang Waktu (Jam)
							</Label>
							{/* Diubah menjadi type="number" step="any" agar perawat bisa isi angka jam/desimal */}
							<Input
								id="range-time"
								name="range-time"
								type="number"
								step="0.5"
								placeholder="Contoh: 4 (untuk 4 jam)"
								onChange={(e) =>
									setForm({
										...form,
										rangeHour:
											parseFloat(e.target.value) || 0,
									})
								}
								required
							/>
						</Field>
						<Field>
							<Label htmlFor="volume">Volume (mL)</Label>
							<Input
								id="volume"
								name="volume"
								type="number"
								placeholder="Masukkan volume infus"
								onChange={(e) =>
									setForm({
										...form,
										volume: Number(e.target.value),
									})
								}
								required
							/>
						</Field>
						<Field>
							<Label htmlFor="drop-factor">Drop Factor</Label>
							<Select
								items={dropFactorList}
								onValueChange={(value) =>
									setForm((prev) => ({
										...prev,
										dropFactor: Number(value) || 20,
									}))
								}>
								<SelectTrigger>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{dropFactorList.map((item) => (
										<SelectItem
											key={item.value}
											value={item.value}>
											{item.label}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
						</Field>
					</FieldGroup>
					<DialogFooter>
						<DialogClose
							onClick={onClose}
							render={
								<Button
									variant="outline"
									type="button">
									Cancel
								</Button>
							}
						/>
						<Button type="submit">Simpan Infus</Button>
					</DialogFooter>
					<DialogPrimitive.Close
						onClick={onClose}
						data-slot="dialog-close"
						render={
							<Button
								variant="ghost"
								className="absolute top-2 right-2 bg-secondary"
								size="icon-sm"
								type="button"
							/>
						}>
						<XIcon />
						<span className="sr-only">Close</span>
					</DialogPrimitive.Close>
				</form>
			</DialogContent>
		</Dialog>
	);
}