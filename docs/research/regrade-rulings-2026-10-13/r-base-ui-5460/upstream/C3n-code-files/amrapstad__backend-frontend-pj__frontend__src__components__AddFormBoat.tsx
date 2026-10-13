'use client';
import * as React from 'react';
import { useState, useEffect } from 'react'
import { Field } from '@base-ui/react/field';
import { Button } from '@base-ui/react/button';
import { Form } from '@base-ui/react/form';
import styles from '../css/addform.module.css';
import fieldStyles from '../css/addform.module.css';
import submitButtonStyles from '../css/other.module.css'


export default function AddFormBoat({ isNew }: { isNew: boolean }) {
    const [loading, setLoading] = useState(false);
    const [items, setItems] = useState<any[]>([])
    const [errors, setErrors] = React.useState<Record<string, string>>({});
    const [searchKeyword, setSearchKeyword] = useState("");


    useEffect(() => {
        const timer = setTimeout(() => {
            handleBoatSearch(searchKeyword);
        }, 300);

        return () => clearTimeout(timer);
    }, [searchKeyword]);

    async function handleBoatSearch(keyword: string) {
        setErrors({})
        setLoading(true)
        try {
            const url = keyword
                ? `/api/boats/search?keyword=${encodeURIComponent(keyword)}`
                : `/api/boats/search`
            const res = await fetch(url)
            if (!res.ok) { setErrors({ server: await res.text() }); setItems([]); return }
            setItems(await res.json())
        } catch (err: any) { setErrors({ server: err.message || 'An error occurred ' }) }
        finally { setLoading(false) }
    }

    const handleSubmit = async (event: React.SubmitEvent<HTMLFormElement>) => {
        event.preventDefault();
        const formData = new FormData(event.currentTarget);
        setLoading(true);
        setErrors({});

        try {
            const rRes = await fetch('/api/boat', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    boatName: formData.get('boat_name'),
                    modelYear: Number(formData.get('model_year')),
                })
            });
            if (!rRes.ok) throw new Error(await rRes.text() || 'Failed to create boat');

            alert("Success!");
        } catch (err: any) {
            setErrors({ server: err.message || 'An error occurred' });
            alert(err.message);
        } finally {
            setLoading(false);
        }
    };


    return (
        <>
            {isNew ? (
                <>
                    <h3>Boat</h3>
                    <Form onSubmit={handleSubmit} errors={errors}>
                        <Field.Root name="boat_name" className={fieldStyles.Field}>
                            <Field.Label className={fieldStyles.Label}>Boat name</Field.Label>
                            <Field.Control
                                type="text"
                                required
                                defaultValue=""
                                placeholder="Boat name"
                                className={fieldStyles.Input}
                            />
                            <Field.Error className={fieldStyles.Error} />
                        </Field.Root>
                        <Field.Root name="model_year" className={fieldStyles.Field}>
                            <Field.Label className={fieldStyles.Label}>Year Model</Field.Label>
                            <Field.Control
                                type="number"
                                required
                                defaultValue=""
                                placeholder="Year Model"
                                className={fieldStyles.Input}
                            />
                            <Field.Error className={fieldStyles.Error} />
                        </Field.Root>
                        <Button type="submit" disabled={loading} focusableWhenDisabled className={submitButtonStyles.Button_Custome}>
                            {loading ? 'Submitting...' : 'Submit'}
                        </Button>
                    </Form>
                </>
            ) : (
                <>
                    <Field.Root name="boat_name" className={styles.Field}>
                        <Field.Label className={styles.Label}>Boat name</Field.Label>
                        <Field.Control
                            type="text"
                            required
                            defaultValue={searchKeyword}
                            placeholder="Search for an existing boat..."
                            className={styles.Input}
                            onChange={(e) => {
                                setSearchKeyword(e.target.value);
                            }}
                        />
                        <Field.Error className={styles.Error} />
                    </Field.Root>
                </>
            )}
        </>
    );
}
