"use client"

import {
    Dialog,
    DialogClose,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog"
import {
    Select,
    SelectContent,
    SelectGroup,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldLabel, FieldSeparator } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import GlassButton from "./navbarButton"
import { UserRound, Trash, Settings } from "lucide-react";

import { useState } from "react"
import { AlertDialogRoot, Form } from "@base-ui/react";
import { updatePlannerAction, deleteAllPlannerEntriesAction } from "@/lib/actions/planner"
import { updatePlannerSchema } from "@/lib/validations/planner";
import { updateNameSchema, updatePasswordSchema } from "@/lib/validations/user";
import { updatePasswordAction, deleteUserAction } from "@/lib/actions/auth";
import { authClient } from "@/lib/auth-client";
import { useRouter } from "next/navigation";
import { redirect } from "next/navigation";


type SettingsDialogProps = {
    plannerData: {
        startDay: number,
        length: number,
    }
}

export function SettingsDialog({ plannerData }: SettingsDialogProps) {

    const [startDay, setStartDay] = useState(plannerData.startDay);
    const [length, setLength] = useState(plannerData.length.toString());

    const [isOpen, setIsOpen] = useState<boolean>(false);

    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState<boolean>(false)

    // weekday items for dropdown
    const weekdays = [
        { label: "Montag", value: 1 },
        { label: "Dienstag", value: 2 },
        { label: "Mittwoch", value: 3 },
        { label: "Donnerstag", value: 4 },
        { label: "Freitag", value: 5 },
        { label: "Samstag", value: 6 },
        { label: "Sonntag", value: 7 },
    ]

    async function handleSubmit() {
        setIsLoading(true);
        setErrorMessage(null);

        const result = updatePlannerSchema.safeParse({
            startDay: Number(startDay),
            length: Number(length),
        })

        if (!result.success) {
            setErrorMessage(result.error.issues[0].message)
            setIsLoading(false);
            return;
        }

        if (plannerData.startDay === startDay && plannerData.length === Number(length)) {
            setIsOpen(false);
            setIsLoading(false);
            return;
        }

        await updatePlannerAction(result.data);
        setIsOpen(false);
        setIsLoading(false);
    };

    return (
        <Dialog open={isOpen} onOpenChange={setIsOpen}>
            <form>
                <DialogTrigger render={<GlassButton icon={<Settings />} />} />
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Listeneinstellungen</DialogTitle>
                        <DialogDescription>
                            Hier kannst du den Starttag und die Länge der Liste einstellen.
                        </DialogDescription>
                    </DialogHeader>
                    <FieldGroup>
                        <Field>
                            <Label htmlFor="startDay">Starttag</Label>
                            <Select items={weekdays} value={startDay} onValueChange={(event) => setStartDay(Number(event))}>
                                <SelectTrigger className="w-full">
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectGroup>
                                        {weekdays.map((weekday) => (
                                            <SelectItem key={weekday.value} value={weekday.value}>
                                                {weekday.label}
                                            </SelectItem>
                                        ))}
                                    </SelectGroup>
                                </SelectContent>
                            </Select>
                        </Field>

                        <Field>
                            <Label htmlFor="length">Länge</Label>
                            <Input id="length" name="length" type="number" max={28} value={length} onChange={(event) => setLength(event.target.value)}></Input>
                            {errorMessage && (<p role="alert" aria-live="polite" className="text-sm text-red-600">{errorMessage}</p>)}
                        </Field>
                    </FieldGroup>
                    <DialogFooter>
                        <DialogClose render={<Button variant="outline">Schließen</Button>} />
                        <Button variant="color" onClick={handleSubmit}>{isLoading ? "Wird gespeichert" : "Speichern"}</Button>
                    </DialogFooter>
                </DialogContent>
            </form>
        </Dialog>
    )
}

export function ProfileDialog() {

    const [isOpen, setIsOpen] = useState<boolean>(false);
    const [errorMessage, setErrorMessage] = useState<string | null>(null);
    const [isNameChangeOpen, setIsNameChangeOpen] = useState<boolean>(false);
    const [isPWChangeOpen, setIsPWChangeOpen] = useState<boolean>(false);
    const [isAccDeleteOpen, setIsAccDeleteOpen] = useState<boolean>(false);

    const [isLoading, setIsLoading] = useState<boolean>(false);

    const { data: session } = authClient.useSession()
    const [name, setName] = useState(session?.user.name ?? "")

    // load current username on name edit if not yet done
    async function toggleNameEdit() {
        setErrorMessage(null);

        if (!isNameChangeOpen) {
            setName(session?.user.name ?? "")
        }
        setIsNameChangeOpen(!isNameChangeOpen)
    }

    // save name changes
    async function saveName() {
        setIsLoading(true);
        setErrorMessage(null);

        if (name === session?.user.name) {
            setIsLoading(false);
            setIsNameChangeOpen(false);
            return;
        }

        const result = updateNameSchema.safeParse({ name })

        if (!result.success) {
            setErrorMessage(result.error.issues[0].message)
            setIsLoading(false);
            return;
        }

        await authClient.updateUser({ name: result.data.name })
        setIsLoading(false);
        setIsNameChangeOpen(false);
    }

    // save password changes
    async function savePasswordChanges(formData: FormData) {
        setIsLoading(true);
        setErrorMessage(null);

        const data = updatePasswordSchema.safeParse({
            currentPassword: formData.get("currentPassword"),
            newPassword: formData.get("newPassword")
        });

        if (data.error) {
            setErrorMessage(data.error.issues[0].message)
            setIsLoading(false);
            return;
        }

        const result = await updatePasswordAction({
            currentPassword: data.data.currentPassword,
            newPassword: data.data.newPassword
        })

        if (result.error) {
            setErrorMessage(result.error)
            setIsLoading(false);
            return;
        }

        setIsLoading(false);
        setIsPWChangeOpen(false);
    }

    // delete user profile
    async function deleteUserProfile() {
        setIsLoading(true)
        setErrorMessage(null)

        const result = await deleteUserAction()

        if (!result.success) {
            setErrorMessage(result.error)
            setIsLoading(false)
            setIsAccDeleteOpen(false)
            return;
        }

        setIsLoading(false)
        redirect("/")
    }

    // logout
    const router = useRouter();
    async function logout() {
        await authClient.signOut();
        router.push("/")
    }

    return (
        <>
            <Dialog open={isOpen} onOpenChange={() => {
                setIsOpen(!isOpen)
                setErrorMessage(null)
            }}>
                <form>
                    <DialogTrigger render={<GlassButton icon={<UserRound />} />} />
                    <DialogContent initialFocus={false} className="sm:max-w-sm">
                        <DialogHeader>
                            <DialogTitle>Hallo {session?.user.name}!</DialogTitle>
                            <DialogDescription>
                                Hier kannst du Änderungen an deinem Konto vornehmen.
                            </DialogDescription>
                        </DialogHeader>

                        {(!isNameChangeOpen && !isPWChangeOpen) && (
                            <FieldGroup>
                                <Field>
                                    <Button variant="outline" onClick={toggleNameEdit}>Name ändern</Button>
                                    <Button variant="outline" onClick={() => {
                                        setErrorMessage(null)
                                        setIsPWChangeOpen(!isPWChangeOpen)
                                    }}>Passwort ändern</Button>
                                </Field>
                                <Field>
                                    <Button className="w-full" variant="warning" onClick={() => setIsAccDeleteOpen(true)}>Konto löschen</Button>
                                    {errorMessage && (<p role="alert" aria-live="polite" className="text-sm text-red-600">{errorMessage}</p>)}
                                </Field>
                                <FieldSeparator />
                                <DialogFooter>
                                    <DialogClose render={<Button variant="outline">Schließen</Button>} />
                                    <Button variant="color" onClick={logout}>Ausloggen</Button>
                                </DialogFooter>
                            </FieldGroup>
                        )}

                        {isNameChangeOpen && (
                            <FieldGroup>
                                <Field>
                                    <Input value={name} onChange={(event) => setName(event.target.value.trim())}></Input>
                                    {errorMessage && (<p role="alert" aria-live="polite" className="text-sm text-red-600">{errorMessage}</p>)}
                                </Field>
                                <Field>
                                    <Button variant="color" onClick={saveName}>{isLoading ? "Wird gespeichert" : "Speichern"}</Button>
                                    <Button variant="outline" onClick={() => {
                                        setErrorMessage(null);
                                        setIsNameChangeOpen(false);
                                    }}>Abbrechen</Button>
                                </Field>
                            </FieldGroup>
                        )}

                        {isPWChangeOpen && (
                            <Form action={savePasswordChanges}>
                                <FieldGroup>
                                    <Field>
                                        <FieldLabel>Altes Passwort eingeben</FieldLabel>
                                        <Input name="currentPassword" type="password" autoComplete="current-password" placeholder="********"></Input>
                                    </Field>
                                    <Field>
                                        <FieldLabel>Neues Passwort eingeben</FieldLabel>
                                        <Input name="newPassword" type="password" autoComplete="new-password" placeholder="********"></Input>
                                    </Field>
                                    <Field>
                                        {errorMessage && (<p role="alert" aria-live="polite" className="text-sm text-red-600">{errorMessage}</p>)}
                                        <Button variant="color" type="submit">{isLoading ? "Wird gespeichert" : "Speichern"}</Button>
                                        <Button variant="outline" onClick={() => {
                                            setErrorMessage(null);
                                            setIsPWChangeOpen(false);
                                        }}>Abbrechen</Button>
                                    </Field>
                                </FieldGroup>
                            </Form>
                        )}
                    </DialogContent>
                </form>
            </Dialog>

            <AlertDialog open={isAccDeleteOpen} onOpenChange={setIsAccDeleteOpen}>
                <form>
                    <AlertDialogContent>
                        <AlertDialogHeader>
                            <AlertDialogTitle>Möchtest du dein Konto vollständig löschen?</AlertDialogTitle>
                            <AlertDialogDescription>
                                Diese Aktion kann nicht rückgängig gemacht werden. Dein Planer und alle personenbezogenen Daten werden vollständig gelöscht.
                            </AlertDialogDescription>
                        </AlertDialogHeader>
                        <AlertDialogFooter>
                            <AlertDialogCancel>Abbrechen</AlertDialogCancel>
                            <AlertDialogAction onClick={deleteUserProfile}>{isLoading ? "Wird gelöscht" : "Konto unwiderruflich löschen"}</AlertDialogAction>
                        </AlertDialogFooter>
                    </AlertDialogContent>
                </form>
            </AlertDialog>
        </>
    )
}

export function DeleteDialog() {

    const [isOpen, setIsOpen] = useState<boolean>(false);
    const [isLoading, setIsLoading] = useState<boolean>(false)

    async function handleSubmit() {
        setIsLoading(true);
        const result = await deleteAllPlannerEntriesAction()

        if (result.success) {
            setIsOpen(false);
            setIsLoading(false);
        }
    }

    return (
        <AlertDialog open={isOpen} onOpenChange={setIsOpen}>
            <form>
                <AlertDialogTrigger render={<GlassButton icon={<Trash />} />} />
                <AlertDialogContent>
                    <AlertDialogHeader>
                        <AlertDialogTitle>Möchtest du alle Einträge löschen?</AlertDialogTitle>
                        <AlertDialogDescription>
                            Diese Aktion kann nicht rückgängig gemacht werden.
                        </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                        <AlertDialogCancel>Abbrechen</AlertDialogCancel>
                        <AlertDialogAction onClick={handleSubmit}>{isLoading ? "Wird gelöscht" : "Alle Einträge löschen"}</AlertDialogAction>
                    </AlertDialogFooter>
                </AlertDialogContent>
            </form>
        </AlertDialog>
    )
}