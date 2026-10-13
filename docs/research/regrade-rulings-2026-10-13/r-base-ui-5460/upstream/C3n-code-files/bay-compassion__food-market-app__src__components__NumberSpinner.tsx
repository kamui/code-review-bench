import { NumberField } from '@base-ui/react/number-field';
import styled from '@emotion/styled';
import { Button, FormLabel, IconButton, InputAdornment, OutlinedInput } from '@mui/material';
import { useId } from 'react';

export type NumberSpinnerProps = {
	label: string;
	disabled?: boolean;
	value: number | string;
	onChange: (value: number | string) => void;
	/** The lowest count the buttons will step down to. Typed entry is clamped to it as well. */
	min?: number;
	max?: number;
	/** How far one press of a step button moves the value. Defaults to 1. */
	step?: number;
	required?: boolean;
	/** Shown in the input while it is empty, e.g. what leaving an optional count out means. */
	placeholder?: string;
	/**
	 * Accessible name for a button that empties the field. The button is offered only when this is
	 * set, for a count that may be left out, and only while the field has a value to clear.
	 */
	clearLabel?: string;
	/** Optional helper text shown under the label, e.g. clarifying what to include in the count. */
	hint?: string;
	/** Accessible name for the step-down button, which shows only a glyph. */
	decrementLabel: string;
	/** Accessible name for the step-up button. */
	incrementLabel: string;
};

/** The height every control in the row shares, matching the outlined inputs elsewhere in the form. */
const controlSize = 58;

const Field = styled.div`
	display: grid;
	gap: 8px;
	color: var(--color-text);

	/* The label's own bottom margin would compound with this grid's gap. */
	> label {
		margin-bottom: 0;
	}
`;

const Hint = styled.p`
	margin: -4px 0 0;
	color: var(--color-text-muted);
	font-size: 14px;
	line-height: 1.5;
`;

/**
 * The three controls sit in their own tracks rather than butted into one joined pill: separate
 * 58px boxes with the form's usual border and radius are what the rest of the guest form looks
 * like, and the grid mirrors on its own under `dir="rtl"`.
 */
const Group = styled(NumberField.Group)`
	display: grid;
	grid-template-columns: ${controlSize}px minmax(0, 1fr) ${controlSize}px;
	gap: 8px;
`;

const StepButton = styled(Button)`
	min-width: 0;
	height: ${controlSize}px;
	padding: 0;
	color: var(--color-text);
	border: 2px solid var(--color-border);
	border-radius: var(--radius-md);
	background: var(--color-background);

	&:hover {
		border-color: var(--color-border);
		background: var(--color-surface-soft);
	}

	/* Reached at the ends of the range, where the button stays visible but stops being an action. */
	&.Mui-disabled {
		color: var(--color-text-subtle);
		border-color: var(--color-border);
		opacity: 0.55;
	}
`;

const SpinnerInput = styled(OutlinedInput)`
	min-width: 0;

	input {
		padding: 0 8px;
		font-weight: 700;
		text-align: center;
	}
`;

/**
 * Material's own `Remove` and `Add` glyphs, inlined rather than pulling in `@mui/icons-material`.
 * Guest screens ride in the initial chunk that stands between a guest and the queue, and two paths
 * are not worth a package.
 */
function StepGlyph({ direction }: { direction: 'up' | 'down' }) {
	return (
		<svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" focusable="false">
			<path
				fill="currentColor"
				d={direction === 'up' ? 'M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z' : 'M19 13H5v-2h14v2z'}
			/>
		</svg>
	);
}

/** Material's `Clear` glyph, inlined for the same reason. */
function ClearGlyph() {
	return (
		<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" focusable="false">
			<path
				fill="currentColor"
				d="M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
			/>
		</svg>
	);
}

/** The form keeps an unanswered count as `''`, where Base UI spells the same absence `null`. */
function toNumericValue(value: number | string): number | null {
	if (value === '') {
		return null;
	}

	const parsed = Number(value);

	return Number.isFinite(parsed) ? parsed : null;
}

/**
 * A labelled count with a step-down and step-up button either side of the number itself — MUI's
 * number spinner, which is Base UI's `NumberField` wearing this app's outlined input and buttons.
 *
 * Typing works as well as stepping: the field takes digits directly and clamps them to `min` and
 * `max` on the way out, so a large household is a few keystrokes rather than a long press. It is a
 * plain `type="text"` input under `inputMode="numeric"` rather than `type="number"`, which is what
 * lets it reject stray characters instead of silently accepting them.
 */
export function NumberSpinner({
	label,
	disabled,
	value,
	onChange,
	min,
	max,
	step,
	required = false,
	placeholder,
	clearLabel,
	hint,
	decrementLabel,
	incrementLabel,
}: NumberSpinnerProps) {
	const inputId = useId();
	const labelId = useId();

	return (
		<NumberField.Root
			disabled={disabled}
			value={toNumericValue(value)}
			min={min}
			max={max}
			step={step}
			required={required}
			// Reported back in the form's own vocabulary, so the store and `GuestFormState` keep
			// holding `number | ''` and neither has to learn about Base UI.
			onValueChange={(next) => onChange(next ?? '')}
			render={(props) => (
				<Field ref={props.ref} className="number-spinner">
					{props.children}
				</Field>
			)}
		>
			{/* `required` is deliberately left off the label, matching text fields: the control
			    carries it for validation, and the design marks required fields by their copy
			    rather than an asterisk. */}
			<FormLabel id={labelId} htmlFor={inputId}>
				{label}
			</FormLabel>
			{hint ? <Hint className="number-spinner-hint">{hint}</Hint> : null}
			{/* Named after the field, because the two buttons are not: a form asking for three
			    separate counts would otherwise offer three identical "decrease" buttons with
			    nothing to tell them apart. */}
			<Group className="number-spinner-group" aria-labelledby={labelId}>
				<NumberField.Decrement
					render={<StepButton variant="outlined" aria-label={decrementLabel} />}
				>
					<StepGlyph direction="down" />
				</NumberField.Decrement>
				<NumberField.Input
					id={inputId}
					render={(props, state) => (
						<SpinnerInput
							className="number-spinner-input"
							// `InputBase` overwrites four of the props it is handed: the input's `ref`,
							// and its `onFocus`, `onBlur`, and `onChange`, which it wraps in handlers of
							// its own. Those four have to arrive by the props that compose them back in,
							// or Base UI never hears about a keystroke. Everything else it leaves alone,
							// so the rest — `onKeyDown`, `onPaste`, `type`, `inputMode` — rides in
							// through the slot.
							inputRef={props.ref}
							value={state.inputValue}
							onFocus={props.onFocus}
							onBlur={props.onBlur}
							onChange={props.onChange}
							placeholder={placeholder}
							endAdornment={
								clearLabel && value !== '' ? (
									<InputAdornment position="end">
										<IconButton
											aria-label={clearLabel}
											edge="end"
											size="small"
											onClick={() => onChange('')}
										>
											<ClearGlyph />
										</IconButton>
									</InputAdornment>
								) : undefined
							}
							slotProps={{ input: props }}
						/>
					)}
				/>
				<NumberField.Increment
					render={<StepButton variant="outlined" aria-label={incrementLabel} />}
				>
					<StepGlyph direction="up" />
				</NumberField.Increment>
			</Group>
		</NumberField.Root>
	);
}
