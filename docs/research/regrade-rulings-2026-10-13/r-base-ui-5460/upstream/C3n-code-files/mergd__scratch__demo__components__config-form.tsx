import { Checkbox } from '@base-ui/react/checkbox';
import { Collapsible } from '@base-ui/react/collapsible';
import { Input } from '@base-ui/react/input';
import { Select } from '@base-ui/react/select';
import { CaretDown, CaretRight } from '@phosphor-icons/react';
import { COLOR_OPTIONS, type AnnotationColorId } from '@fldr/scratch';

export type PlaygroundConfig = {
  primaryColor: AnnotationColorId;
  enableCopy: boolean;
  enableMailFeedback: boolean;
  isDevelopment: boolean;
  feedbackUrl: string;
  mailto: string;
  webhookUrl: string;
  feedbackUserId: string;
  feedbackPlan: string;
  guideTitle: string;
  guideBody: string;
  idleTimeoutMs: number;
};

type ConfigFormProps = {
  config: PlaygroundConfig;
  showAdvanced: boolean;
  onShowAdvancedChange: (open: boolean) => void;
  onChange: <K extends keyof PlaygroundConfig>(
    key: K,
    value: PlaygroundConfig[K],
  ) => void;
};

const colorItems = COLOR_OPTIONS.map((color) => ({
  label: color.label,
  value: color.id,
}));

function ColorOptionLabel({ colorId }: { colorId: AnnotationColorId }) {
  const color = COLOR_OPTIONS.find((option) => option.id === colorId);
  if (!color) return null;

  return (
    <>
      <span
        className="color-swatch"
        style={{ backgroundColor: color.srgb }}
        aria-hidden
      />
      <span>{color.label}</span>
    </>
  );
}

export function ConfigForm({
  config,
  showAdvanced,
  onShowAdvancedChange,
  onChange,
}: ConfigFormProps) {
  return (
    <>
      <div className="config-grid">
        <label className="field">
          <span>Accent color</span>
          <Select.Root
            items={colorItems}
            value={config.primaryColor}
            onValueChange={(value) => {
              if (value) onChange('primaryColor', value);
            }}
          >
            <Select.Trigger className="control select-trigger">
              <Select.Value className="select-value">
                {(value) =>
                  value ? <ColorOptionLabel colorId={value} /> : null
                }
              </Select.Value>
              <Select.Icon className="select-icon">
                <CaretDown size={12} weight="bold" aria-hidden />
              </Select.Icon>
            </Select.Trigger>
            <Select.Portal>
              <Select.Positioner
                className="select-positioner"
                side="bottom"
                align="start"
                alignItemWithTrigger={false}
                sideOffset={4}
              >
                <Select.Popup className="select-popup">
                  <Select.List className="select-list">
                    {COLOR_OPTIONS.map((color) => (
                      <Select.Item
                        key={color.id}
                        value={color.id}
                        className="select-item"
                      >
                        <Select.ItemIndicator
                          keepMounted
                          className="select-item-indicator"
                        >
                          ✓
                        </Select.ItemIndicator>
                        <span
                          className="color-swatch"
                          style={{ backgroundColor: color.srgb }}
                          aria-hidden
                        />
                        <Select.ItemText>{color.label}</Select.ItemText>
                      </Select.Item>
                    ))}
                  </Select.List>
                </Select.Popup>
              </Select.Positioner>
            </Select.Portal>
          </Select.Root>
        </label>

        <label className="field">
          <span>Feedback endpoint</span>
          <Input
            className="control control-opaque"
            type="text"
            value={config.feedbackUrl}
            onChange={(event) => onChange('feedbackUrl', event.target.value)}
            placeholder="/api/feedback"
          />
        </label>

        <label className="field checkbox-field">
          <Checkbox.Root
            className="checkbox-root"
            checked={config.enableCopy}
            onCheckedChange={(checked) => onChange('enableCopy', checked)}
          >
            <Checkbox.Indicator className="checkbox-indicator" />
          </Checkbox.Root>
          <span>Allow copying</span>
        </label>

        <label className="field checkbox-field">
          <Checkbox.Root
            className="checkbox-root"
            checked={config.enableMailFeedback}
            onCheckedChange={(checked) =>
              onChange('enableMailFeedback', checked)
            }
          >
            <Checkbox.Indicator className="checkbox-indicator" />
          </Checkbox.Root>
          <span>Enable send</span>
        </label>

        <label className="field checkbox-field">
          <Checkbox.Root
            className="checkbox-root"
            checked={config.isDevelopment}
            onCheckedChange={(checked) => onChange('isDevelopment', checked)}
          >
            <Checkbox.Indicator className="checkbox-indicator" />
          </Checkbox.Root>
          <span>Always on (dev)</span>
        </label>
      </div>

      <Collapsible.Root
        className="advanced-disclosure"
        open={showAdvanced}
        onOpenChange={onShowAdvancedChange}
      >
        <Collapsible.Trigger className="advanced-trigger">
          <CaretRight
            size={12}
            weight="bold"
            className="advanced-caret"
            aria-hidden
          />
          Advanced
        </Collapsible.Trigger>
        <Collapsible.Panel className="advanced-panel">
          <div className="config-grid advanced-grid">
            <label className="field">
              <span>Email address</span>
              <Input
                className="control"
                type="text"
                value={config.mailto}
                onChange={(event) => onChange('mailto', event.target.value)}
                placeholder="team@example.com"
              />
            </label>

            <label className="field">
              <span>Webhook URL</span>
              <Input
                className="control"
                type="text"
                value={config.webhookUrl}
                onChange={(event) => onChange('webhookUrl', event.target.value)}
                placeholder="https://..."
              />
            </label>

            <label className="field">
              <span>Context user ID</span>
              <Input
                className="control"
                type="text"
                value={config.feedbackUserId}
                onChange={(event) =>
                  onChange('feedbackUserId', event.target.value)
                }
              />
            </label>

            <label className="field">
              <span>Context plan</span>
              <Input
                className="control"
                type="text"
                value={config.feedbackPlan}
                onChange={(event) =>
                  onChange('feedbackPlan', event.target.value)
                }
              />
            </label>

            <label className="field field-wide">
              <span>Guide title</span>
              <Input
                className="control"
                type="text"
                value={config.guideTitle}
                onChange={(event) =>
                  onChange('guideTitle', event.target.value)
                }
                placeholder="Optional — reset guide cookie to preview"
              />
            </label>

            <label className="field field-wide">
              <span>Guide body</span>
              <Input
                className="control textarea"
                render={<textarea rows={2} />}
                value={config.guideBody}
                onChange={(event) =>
                  onChange('guideBody', event.target.value)
                }
                placeholder="Optional custom guide body"
              />
            </label>

            <label className="field">
              <span>Idle dismiss (ms)</span>
              <Input
                className="control"
                type="number"
                min={0}
                step={1000}
                value={config.idleTimeoutMs}
                onChange={(event) =>
                  onChange(
                    'idleTimeoutMs',
                    Number.parseInt(event.target.value, 10) || 0,
                  )
                }
                placeholder="30000"
              />
            </label>
          </div>
        </Collapsible.Panel>
      </Collapsible.Root>
    </>
  );
}
