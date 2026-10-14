'use client';

import { Input as BaseInput } from '@base-ui/react/input';
import * as stylex from '@stylexjs/stylex';
import { border, color, font, radius, space, text } from '@ultima/tokens/tokens.stylex';
import type { PartProps } from '@ultima/ui/lib/component';
import { Popover, type PopoverPopupProps, type PopoverPortalProps, type PopoverPositionerProps } from '@ultima/ui/popover';
import {
  createContext,
  use,
  useCallback,
  useState,
  type ComponentProps,
  type PointerEvent as ReactPointerEvent,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';

const HEX = /^#[0-9a-fA-F]{6}$/;
const HEX_PATTERN = '#[0-9A-Fa-f]{6}';
const DEFAULT_VALUE = '#000000';

const styles = stylex.create({
  swatch: {
    appearance: 'none',
    backgroundImage: 'none',
    borderColor: color['--ult-color-border-strong'],
    borderRadius: radius['--ult-radius-md'],
    borderStyle: 'solid',
    borderWidth: border.hairline,
    boxSizing: 'border-box',
    cursor: { default: 'pointer', ':disabled': 'default', ':is([data-disabled])': 'default' },
    display: 'inline-block',
    flexShrink: 0,
    fontFamily: font['--ult-font-sans'],
    lineHeight: font['--ult-font-leading-none'],
    margin: 0,
    opacity: { default: 1, ':disabled': 0.5, ':is([data-disabled])': 0.5 },
    padding: 0,
    ':focus-visible': {
      outline: `${border.focus} solid ${color['--ult-color-border-focus']}`,
      outlineOffset: border.focusOffset,
    },
  },
  input: {
    appearance: 'none',
    backgroundColor: color['--ult-color-surface-sunken'],
    borderColor: {
      default: color['--ult-color-border-strong'],
      ':is([aria-invalid="true"], [data-invalid])': color['--ult-color-danger-border'],
    },
    borderRadius: radius['--ult-radius-md'],
    borderStyle: 'solid',
    borderWidth: border.hairline,
    boxSizing: 'border-box',
    color: color['--ult-color-text'],
    fontFamily: font['--ult-font-sans'],
    lineHeight: font['--ult-font-leading-none'],
    margin: 0,
    opacity: { default: 1, ':disabled': 0.5, ':is([data-disabled])': 0.5 },
    width: '100%',
    '::placeholder': { color: color['--ult-color-text-subtle'] },
    ':focus-visible': {
      outline: `${border.focus} solid ${color['--ult-color-border-focus']}`,
      outlineOffset: border.focusOffset,
    },
  },
  popup: {
    minWidth: `calc(${space['--ult-space-12']} * 4)`,
  },
  picker: {
    display: 'flex',
    flexDirection: 'column',
    gap: space['--ult-space-4'],
  },
  area: {
    aspectRatio: 1,
    borderColor: color['--ult-color-border'],
    borderRadius: radius['--ult-radius-md'],
    borderStyle: 'solid',
    borderWidth: border.hairline,
    boxSizing: 'border-box',
    cursor: 'crosshair',
    margin: 0,
    overflow: 'hidden',
    position: 'relative',
    touchAction: 'none',
    userSelect: 'none',
    width: '100%',
    ':focus-visible': {
      outline: `${border.focus} solid ${color['--ult-color-border-focus']}`,
      outlineOffset: border.focusOffset,
    },
  },
  areaThumb: {
    backgroundColor: color['--ult-color-surface-raised'],
    borderColor: color['--ult-color-border-strong'],
    borderRadius: radius['--ult-radius-full'],
    borderStyle: 'solid',
    borderWidth: border.hairline,
    boxSizing: 'border-box',
    height: space['--ult-space-4'],
    pointerEvents: 'none',
    position: 'absolute',
    transform: 'translate(-50%, -50%)',
    width: space['--ult-space-4'],
  },
  hue: {
    appearance: 'none',
    accentColor: color['--ult-color-surface-raised'],
    borderRadius: radius['--ult-radius-full'],
    boxSizing: 'border-box',
    display: 'block',
    height: space['--ult-space-3'],
    margin: 0,
    width: '100%',
    backgroundImage:
      'linear-gradient(to right, hsl(0 100% 50%), hsl(60 100% 50%), hsl(120 100% 50%), hsl(180 100% 50%), hsl(240 100% 50%), hsl(300 100% 50%), hsl(360 100% 50%))',
    ':focus-visible': {
      outline: `${border.focus} solid ${color['--ult-color-border-focus']}`,
      outlineOffset: border.focusOffset,
    },
  },
  channels: {
    display: 'flex',
    flexDirection: 'column',
    gap: space['--ult-space-3'],
  },
  channelRow: {
    alignItems: 'center',
    display: 'grid',
    gap: space['--ult-space-3'],
    gridTemplateColumns: 'auto 1fr',
  },
  rgbRow: {
    display: 'grid',
    gap: space['--ult-space-3'],
    gridTemplateColumns: 'repeat(3, 1fr)',
  },
  channel: {
    display: 'flex',
    flexDirection: 'column',
    gap: space['--ult-space-2'],
  },
  label: {
    color: color['--ult-color-text'],
    fontFamily: font['--ult-font-sans'],
    fontSize: text['--ult-text-3'],
    fontWeight: font['--ult-font-weight-medium'],
    lineHeight: font['--ult-font-leading-none'],
    margin: 0,
  },
  channelInput: {
    appearance: 'none',
    backgroundColor: color['--ult-color-surface-sunken'],
    borderColor: color['--ult-color-border-strong'],
    borderRadius: radius['--ult-radius-sm'],
    borderStyle: 'solid',
    borderWidth: border.hairline,
    boxSizing: 'border-box',
    color: color['--ult-color-text'],
    fontFamily: font['--ult-font-mono'],
    fontSize: text['--ult-text-3'],
    height: space['--ult-space-9'],
    lineHeight: font['--ult-font-leading-none'],
    margin: 0,
    paddingInline: space['--ult-space-3'],
    width: '100%',
    ':focus-visible': {
      outline: `${border.focus} solid ${color['--ult-color-border-focus']}`,
      outlineOffset: border.focusOffset,
    },
  },
});

const swatchSizes = stylex.create({
  sm: {
    height: space['--ult-space-9'],
    width: space['--ult-space-9'],
  },
  md: {
    height: space['--ult-space-10'],
    width: space['--ult-space-10'],
  },
  lg: {
    height: space['--ult-space-11'],
    width: space['--ult-space-11'],
  },
});

const inputSizes = stylex.create({
  sm: {
    fontSize: text['--ult-text-4'],
    height: space['--ult-space-9'],
    paddingInline: space['--ult-space-4'],
  },
  md: {
    fontSize: text['--ult-text-5'],
    height: space['--ult-space-10'],
    paddingInline: space['--ult-space-5'],
  },
  lg: {
    fontSize: text['--ult-text-5'],
    height: space['--ult-space-11'],
    paddingInline: space['--ult-space-6'],
  },
});

const paints = stylex.create({
  fill: (backgroundColor: string) => ({
    backgroundColor,
  }),
  area: (hue: number) => ({
    backgroundImage: `linear-gradient(to top, hsl(0 0% 0%), transparent), linear-gradient(to right, hsl(0 0% 100%), hsl(${hue} 100% 50%))`,
  }),
  thumb: (x: number, y: number) => ({
    insetInlineStart: `${x}%`,
    insetBlockStart: `${y}%`,
  }),
});

export type ColorFieldSize = keyof typeof swatchSizes;

type ColorFieldValue = {
  value: string;
  setValue: (value: string) => void;
  disabled: boolean;
};

const SizeContext = createContext<ColorFieldSize>('md');
const ValueContext = createContext<ColorFieldValue | null>(null);

function useValue(): ColorFieldValue {
  return use(ValueContext) ?? { value: DEFAULT_VALUE, setValue: () => {}, disabled: false };
}

function useColorField(): ColorFieldValue {
  const context = use(ValueContext);
  if (!context) throw new Error('useColorField must be used inside ColorField.Root');
  return context;
}

export type ColorFieldRootProps = PartProps<ComponentProps<typeof Popover.Root>> & {
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
  size?: ColorFieldSize;
  disabled?: boolean;
};

export type ColorFieldSwatchProps = PartProps<ComponentProps<typeof Popover.Trigger>>;

export type ColorFieldInputProps = Omit<PartProps<ComponentProps<typeof BaseInput>>, 'size'>;

export type ColorFieldPortalProps = PopoverPortalProps;

export type ColorFieldPositionerProps = PopoverPositionerProps;

export type ColorFieldPopupProps = PopoverPopupProps;

export type ColorFieldPickerProps = PartProps<ComponentProps<'div'>>;

function Root({
  value,
  defaultValue = DEFAULT_VALUE,
  onValueChange,
  size = 'md',
  disabled = false,
  style,
  ...props
}: ColorFieldRootProps) {
  const [uncontrolled, setUncontrolled] = useState(defaultValue);
  const resolved = value ?? uncontrolled;
  const setValue = useCallback(
    (next: string) => {
      setUncontrolled(next);
      onValueChange?.(next);
    },
    [onValueChange],
  );

  return (
    <SizeContext value={size}>
      <ValueContext value={{ value: resolved, setValue, disabled }}>
        <Popover.Root {...props} {...stylex.props(style)} />
      </ValueContext>
    </SizeContext>
  );
}

function Swatch({ style, disabled, ...props }: ColorFieldSwatchProps) {
  const size = use(SizeContext);
  const { value, disabled: rootDisabled } = useValue();
  return (
    <Popover.Trigger
      {...props}
      disabled={disabled ?? rootDisabled}
      {...stylex.props(styles.swatch, swatchSizes[size], paints.fill(value), style)}
    />
  );
}

function Input({
  style,
  disabled,
  value: valueProp,
  defaultValue: _defaultValue,
  onValueChange,
  ...props
}: ColorFieldInputProps) {
  const size = use(SizeContext);
  const { value, setValue, disabled: rootDisabled } = useValue();
  const committed = typeof valueProp === 'string' ? valueProp : value;
  const [draft, setDraft] = useState(committed);
  const [lastCommitted, setLastCommitted] = useState(committed);
  if (committed !== lastCommitted) {
    setLastCommitted(committed);
    setDraft(committed);
  }

  return (
    <BaseInput
      {...props}
      disabled={disabled ?? rootDisabled}
      maxLength={7}
      pattern={HEX_PATTERN}
      spellCheck={false}
      value={draft}
      onValueChange={(next, eventDetails) => {
        setDraft(next);
        if (HEX.test(next)) setValue(next.toLowerCase());
        onValueChange?.(next, eventDetails);
      }}
      {...stylex.props(styles.input, inputSizes[size], style)}
    />
  );
}

function Popup({
  style,
  'aria-label': ariaLabel,
  'aria-labelledby': ariaLabelledBy,
  ...props
}: ColorFieldPopupProps) {
  return (
    <Popover.Popup
      {...props}
      aria-label={ariaLabel ?? (ariaLabelledBy ? undefined : 'Color picker')}
      aria-labelledby={ariaLabelledBy}
      style={[styles.popup, style]}
    />
  );
}

function Picker({ style, ...props }: ColorFieldPickerProps) {
  const { value, setValue, disabled } = useValue();
  const rgb = hexToRgb(value) ?? { r: 0, g: 0, b: 0 };
  const hsv = rgbToHsv(rgb);
  const hsl = rgbToHsl(rgb);
  const [hue, setHue] = useState(hsv.h);
  const [hexDraft, setHexDraft] = useState(value);
  const [lastHex, setLastHex] = useState(value);
  if (hsv.s > 0 && Math.abs(hue - hsv.h) > 0.5) {
    setHue(hsv.h);
  }
  if (value !== lastHex) {
    setLastHex(value);
    setHexDraft(value);
  }

  const commitHsv = (next: { h: number; s: number; v: number }) => {
    setHue(wrapHue(next.h));
    setValue(rgbToHex(hsvToRgb(next)));
  };

  const onAreaPointer = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (disabled || event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const next = pointerToSv(event, event.currentTarget);
    commitHsv({ h: hue, s: next.s, v: next.v });
  };

  const onAreaKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (disabled) return;
    const step = event.shiftKey ? 0.1 : 0.01;
    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      commitHsv({ h: hue, s: clamp(hsv.s - step, 0, 1), v: hsv.v });
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      commitHsv({ h: hue, s: clamp(hsv.s + step, 0, 1), v: hsv.v });
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      commitHsv({ h: hue, s: hsv.s, v: clamp(hsv.v - step, 0, 1) });
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      commitHsv({ h: hue, s: hsv.s, v: clamp(hsv.v + step, 0, 1) });
    }
  };

  return (
    <div {...props} {...stylex.props(styles.picker, style)}>
      <div
        aria-label="Saturation and brightness"
        aria-orientation="vertical"
        aria-valuemax={100}
        aria-valuemin={0}
        aria-valuenow={Math.round(hsv.s * 100)}
        aria-valuetext={`Saturation ${Math.round(hsv.s * 100)}%, brightness ${Math.round(hsv.v * 100)}%`}
        role="slider"
        tabIndex={disabled ? -1 : 0}
        onKeyDown={onAreaKeyDown}
        onPointerDown={onAreaPointer}
        onPointerMove={(event) => {
          if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
          const next = pointerToSv(event, event.currentTarget);
          commitHsv({ h: hue, s: next.s, v: next.v });
        }}
        {...stylex.props(styles.area, paints.area(hue))}
      >
        <span aria-hidden="true" {...stylex.props(styles.areaThumb, paints.thumb(hsv.s * 100, (1 - hsv.v) * 100))} />
      </div>
      <input
        aria-label="Hue"
        disabled={disabled}
        max={360}
        min={0}
        step={1}
        type="range"
        value={Math.round(hue)}
        onChange={(event) => commitHsv({ h: Number(event.currentTarget.value), s: hsv.s, v: hsv.v })}
        {...stylex.props(styles.hue)}
      />
      <div {...stylex.props(styles.channels)}>
        <label {...stylex.props(styles.channelRow)}>
          <span {...stylex.props(styles.label)}>HEX</span>
          <input
            aria-label="HEX"
            disabled={disabled}
            maxLength={7}
            spellCheck={false}
            value={hexDraft}
            onChange={(event) => {
              const next = event.currentTarget.value;
              setHexDraft(next);
              if (HEX.test(next)) setValue(next.toLowerCase());
            }}
            {...stylex.props(styles.channelInput)}
          />
        </label>
        <div {...stylex.props(styles.rgbRow)}>
          <Channel
            disabled={disabled}
            label="R"
            max={255}
            name="Red"
            value={Math.round(rgb.r)}
            onCommit={(r) => setValue(rgbToHex({ r, g: rgb.g, b: rgb.b }))}
          />
          <Channel
            disabled={disabled}
            label="G"
            max={255}
            name="Green"
            value={Math.round(rgb.g)}
            onCommit={(g) => setValue(rgbToHex({ r: rgb.r, g, b: rgb.b }))}
          />
          <Channel
            disabled={disabled}
            label="B"
            max={255}
            name="Blue"
            value={Math.round(rgb.b)}
            onCommit={(b) => setValue(rgbToHex({ r: rgb.r, g: rgb.g, b }))}
          />
        </div>
        <div {...stylex.props(styles.rgbRow)}>
          <Channel
            disabled={disabled}
            label="H"
            max={360}
            name="HSL hue"
            value={Math.round(hsl.h)}
            onCommit={(h) => setValue(rgbToHex(hslToRgb({ h, s: hsl.s, l: hsl.l })))}
          />
          <Channel
            disabled={disabled}
            label="S"
            max={100}
            name="HSL saturation"
            value={Math.round(hsl.s * 100)}
            onCommit={(s) => setValue(rgbToHex(hslToRgb({ h: hsl.h, s: s / 100, l: hsl.l })))}
          />
          <Channel
            disabled={disabled}
            label="L"
            max={100}
            name="HSL lightness"
            value={Math.round(hsl.l * 100)}
            onCommit={(l) => setValue(rgbToHex(hslToRgb({ h: hsl.h, s: hsl.s, l: l / 100 })))}
          />
        </div>
      </div>
    </div>
  );
}

function Channel({
  disabled,
  label,
  max,
  name,
  value,
  onCommit,
}: {
  disabled: boolean;
  label: string;
  max: number;
  name: string;
  value: number;
  onCommit: (value: number) => void;
}) {
  return (
    <label {...stylex.props(styles.channel)}>
      <span {...stylex.props(styles.label)}>{label}</span>
      <input
        aria-label={name}
        disabled={disabled}
        inputMode="numeric"
        max={max}
        min={0}
        type="number"
        value={String(value)}
        onChange={(event) => {
          const next = event.currentTarget.value;
          if (next === '') return;
          const parsed = Number(next);
          if (!Number.isFinite(parsed)) return;
          onCommit(clamp(parsed, 0, max));
        }}
        {...stylex.props(styles.channelInput)}
      />
    </label>
  );
}

const ColorField = {
  Root,
  Swatch,
  Input,
  Portal: Popover.Portal,
  Positioner: Popover.Positioner,
  Popup,
  Picker,
};

export {
  ColorField,
  useColorField,
};

type Rgb = { r: number; g: number; b: number };

function clamp(n: number, min: number, max: number) {
  return Math.min(max, Math.max(min, n));
}

function wrapHue(h: number) {
  return ((h % 360) + 360) % 360;
}

function hexToRgb(hex: string): Rgb | null {
  if (!HEX.test(hex)) return null;
  const n = Number.parseInt(hex.slice(1), 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
}

function rgbToHex({ r, g, b }: Rgb) {
  return `#${[r, g, b]
    .map((channel) => clamp(Math.round(channel), 0, 255).toString(16).padStart(2, '0'))
    .join('')}`;
}

function rgbToHsv({ r, g, b }: Rgb) {
  const rr = r / 255;
  const gg = g / 255;
  const bb = b / 255;
  const max = Math.max(rr, gg, bb);
  const min = Math.min(rr, gg, bb);
  const d = max - min;
  const v = max;
  const s = max === 0 ? 0 : d / max;
  let h = 0;
  if (d !== 0) {
    if (max === rr) h = ((gg - bb) / d + (gg < bb ? 6 : 0)) * 60;
    else if (max === gg) h = ((bb - rr) / d + 2) * 60;
    else h = ((rr - gg) / d + 4) * 60;
  }
  return { h, s, v };
}

function hsvToRgb({ h, s, v }: { h: number; s: number; v: number }): Rgb {
  const hue = wrapHue(h);
  const c = v * s;
  const x = c * (1 - Math.abs(((hue / 60) % 2) - 1));
  const m = v - c;
  let rr = 0;
  let gg = 0;
  let bb = 0;
  if (hue < 60) {
    rr = c;
    gg = x;
  } else if (hue < 120) {
    rr = x;
    gg = c;
  } else if (hue < 180) {
    gg = c;
    bb = x;
  } else if (hue < 240) {
    gg = x;
    bb = c;
  } else if (hue < 300) {
    rr = x;
    bb = c;
  } else {
    rr = c;
    bb = x;
  }
  return { r: (rr + m) * 255, g: (gg + m) * 255, b: (bb + m) * 255 };
}

function rgbToHsl({ r, g, b }: Rgb) {
  const rr = r / 255;
  const gg = g / 255;
  const bb = b / 255;
  const max = Math.max(rr, gg, bb);
  const min = Math.min(rr, gg, bb);
  const l = (max + min) / 2;
  const d = max - min;
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  let h = 0;
  if (d !== 0) {
    if (max === rr) h = ((gg - bb) / d + (gg < bb ? 6 : 0)) * 60;
    else if (max === gg) h = ((bb - rr) / d + 2) * 60;
    else h = ((rr - gg) / d + 4) * 60;
  }
  return { h, s, l };
}

function hslToRgb({ h, s, l }: { h: number; s: number; l: number }): Rgb {
  const hue = wrapHue(h);
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((hue / 60) % 2) - 1));
  const m = l - c / 2;
  let rr = 0;
  let gg = 0;
  let bb = 0;
  if (hue < 60) {
    rr = c;
    gg = x;
  } else if (hue < 120) {
    rr = x;
    gg = c;
  } else if (hue < 180) {
    gg = c;
    bb = x;
  } else if (hue < 240) {
    gg = x;
    bb = c;
  } else if (hue < 300) {
    rr = x;
    bb = c;
  } else {
    rr = c;
    bb = x;
  }
  return { r: (rr + m) * 255, g: (gg + m) * 255, b: (bb + m) * 255 };
}

function pointerToSv(event: ReactPointerEvent<HTMLDivElement>, el: HTMLDivElement) {
  const rect = el.getBoundingClientRect();
  return {
    s: clamp((event.clientX - rect.left) / rect.width, 0, 1),
    v: clamp(1 - (event.clientY - rect.top) / rect.height, 0, 1),
  };
}
