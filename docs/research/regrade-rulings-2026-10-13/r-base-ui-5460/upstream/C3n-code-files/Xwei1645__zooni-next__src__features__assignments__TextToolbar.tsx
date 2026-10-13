import { useEffect, useRef, useState } from "react";
import { Bold, Check, ChevronDown, Italic, Minus, MoreHorizontal, Palette, Plus, Redo2, Strikethrough, Type, Underline, Undo2 } from "lucide-react";
import { Popover } from "@base-ui/react/popover";
import { Select as SelectPrimitive } from "@base-ui/react/select";
import { invoke } from "@tauri-apps/api/core";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getSelection,
  $isRangeSelection,
  CAN_REDO_COMMAND,
  CAN_UNDO_COMMAND,
  COMMAND_PRIORITY_CRITICAL,
  FORMAT_TEXT_COMMAND,
  REDO_COMMAND,
  UNDO_COMMAND,
  mergeRegister,
} from "lexical";
import {
  $getSelectionStyleValueForProperty,
  $patchStyleText,
} from "@lexical/selection";
import { Button } from "@/components/ui/button";
import { ColorPicker } from "@/components/ui/color-picker";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";
import { fontFamilyStack } from "@/lib/appearance";
import { logError } from "@/lib/logger";
import {
  DEFAULT_FONT_SIZE,
  MAX_FONT_SIZE,
  MIN_FONT_SIZE,
  normalizeTextColor,
  quoteFontFamily,
  unquoteFontFamily,
} from "./editor-utils";

export function TextToolbar({ onFontSizeStep }: { onFontSizeStep: () => void }) {
  const [editor] = useLexicalComposerContext();
  const [systemFonts, setSystemFonts] = useState<string[]>([]);
  const [fontFamily, setFontFamily] = useState("");
  const [fontSize, setFontSize] = useState(DEFAULT_FONT_SIZE);
  const [fontSizeInput, setFontSizeInput] = useState(String(DEFAULT_FONT_SIZE));
  const [isBold, setIsBold] = useState(false);
  const [isItalic, setIsItalic] = useState(false);
  const [isStrikethrough, setIsStrikethrough] = useState(false);
  const [isUnderline, setIsUnderline] = useState(false);
  const [textColor, setTextColor] = useState<string | null>(null);
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const [toolbarWidth, setToolbarWidth] = useState(Number.POSITIVE_INFINITY);

  useEffect(() => {
    const node = toolbarRef.current;

    if (!node) {
      return;
    }

    const observer = new ResizeObserver((entries) => {
      const entry = entries[entries.length - 1];
      setToolbarWidth(entry.contentRect.width);
    });
    observer.observe(node);

    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    let active = true;

    void invoke<string[]>("list_system_fonts")
      .then((fonts) => {
        if (active) {
          setSystemFonts(fonts);
        }
      })
      .catch((error) => {
        logError("assignment-editor.list-system-fonts", error);
        if (active) {
          setSystemFonts([]);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    return mergeRegister(
      editor.registerCommand(CAN_UNDO_COMMAND, (payload) => {
        setCanUndo(payload);
        return false;
      }, COMMAND_PRIORITY_CRITICAL),
      editor.registerCommand(CAN_REDO_COMMAND, (payload) => {
        setCanRedo(payload);
        return false;
      }, COMMAND_PRIORITY_CRITICAL),
    );
  }, [editor]);

  useEffect(() => {
    return editor.registerUpdateListener(({ editorState }) => {
      editorState.read(() => {
        const selection = $getSelection();

        if ($isRangeSelection(selection)) {
          const value = Number.parseInt(
            $getSelectionStyleValueForProperty(selection, "font-size", `${DEFAULT_FONT_SIZE}px`),
            10,
          );
          const nextFontSize = Number.isFinite(value) ? value : DEFAULT_FONT_SIZE;
          setFontSize(nextFontSize);
          setFontSizeInput(String(nextFontSize));
          setFontFamily(unquoteFontFamily(
            $getSelectionStyleValueForProperty(selection, "font-family", ""),
          ));
          setIsBold(selection.hasFormat("bold"));
          setIsItalic(selection.hasFormat("italic"));
          setIsStrikethrough(selection.hasFormat("strikethrough"));
          setIsUnderline(selection.hasFormat("underline"));
          const resolvedTextColor = $getSelectionStyleValueForProperty(selection, "color", "");
          setTextColor(resolvedTextColor ? normalizeTextColor(resolvedTextColor) : null);
        } else {
          setIsBold(false);
          setIsItalic(false);
          setIsStrikethrough(false);
          setIsUnderline(false);
          setTextColor(null);
        }
      });
    });
  }, [editor]);

  // 程序化更新选区样式时，若编辑器原本未聚焦（例如在输入框/颜色弹层中提交），
  // 防止编辑器在这次更新中抢走焦点、导致焦点跳到正文；工具栏按钮（编辑器已聚焦）
  // 的场景不受影响。编辑器抢占焦点可能发生在异步阶段（微任务/下一帧），
  // 因此延迟到下一帧再校正，确保覆盖“按 Enter + 自动补齐”这类情况。
  function applySelectionStyle(update: () => void) {
    const root = editor.getRootElement();
    const wasEditorFocused = root ? document.activeElement === root : false;
    editor.update(update);
    if (root && !wasEditorFocused) {
      // 编辑器可能在异步阶段（微任务/下一帧）才抢走焦点，延迟到下一帧并
      // 再兜底一次，确保“按 Enter + 自动补齐”等场景下焦点不跳到正文。
      const release = () => {
        if (document.activeElement === root) {
          root.blur();
        }
      };
      requestAnimationFrame(release);
      window.setTimeout(release, 0);
    }
  }

  function setSelectionFontSize(nextFontSize: number, updateInput = true) {
    const clampedFontSize = Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, nextFontSize));
    setFontSize(clampedFontSize);
    if (updateInput) {
      setFontSizeInput(String(clampedFontSize));
    }
    applySelectionStyle(() => {
      const selection = $getSelection();

      if ($isRangeSelection(selection)) {
        $patchStyleText(selection, { "font-size": `${clampedFontSize}px` });
      }
    });
  }

  // 只在失焦/按 Enter 时把输入框的值应用到选区，避免输入过程中每个
  // 按键都触发 editor.update() 而让编辑器抢走输入框焦点。
  function commitFontSizeInput() {
    const numericValue = Number(fontSizeInput);

    if (
      Number.isInteger(numericValue)
      && numericValue >= MIN_FONT_SIZE
      && numericValue <= MAX_FONT_SIZE
    ) {
      setSelectionFontSize(numericValue);
    } else {
      setFontSizeInput(String(fontSize));
    }
  }

  function setSelectionFontFamily(nextFontFamily: string) {
    setFontFamily(nextFontFamily);
    applySelectionStyle(() => {
      const selection = $getSelection();

      if ($isRangeSelection(selection)) {
        $patchStyleText(selection, {
          "font-family": nextFontFamily ? quoteFontFamily(nextFontFamily) : null,
        });
      }
    });
  }

  function setSelectionTextColor(nextTextColor: string | null) {
    const normalizedTextColor = nextTextColor ? normalizeTextColor(nextTextColor) : null;
    setTextColor(normalizedTextColor);
    applySelectionStyle(() => {
      const selection = $getSelection();

      if ($isRangeSelection(selection)) {
        $patchStyleText(selection, { color: normalizedTextColor });
      }
    });
  }

  function preserveSelection(event: React.MouseEvent) {
    event.preventDefault();
  }

  const formatControls = [
    { key: "bold", label: "粗体", icon: <Bold aria-hidden="true" />, active: isBold, toggle: () => { editor.dispatchCommand(FORMAT_TEXT_COMMAND, "bold"); setIsBold((value) => !value); } },
    { key: "italic", label: "斜体", icon: <Italic aria-hidden="true" />, active: isItalic, toggle: () => { editor.dispatchCommand(FORMAT_TEXT_COMMAND, "italic"); setIsItalic((value) => !value); } },
    { key: "strikethrough", label: "删除线", icon: <Strikethrough aria-hidden="true" />, active: isStrikethrough, toggle: () => { editor.dispatchCommand(FORMAT_TEXT_COMMAND, "strikethrough"); setIsStrikethrough((value) => !value); } },
    { key: "underline", label: "下划线", icon: <Underline aria-hidden="true" />, active: isUnderline, toggle: () => { editor.dispatchCommand(FORMAT_TEXT_COMMAND, "underline"); setIsUnderline((value) => !value); } },
  ];

  const hideFont = toolbarWidth < 600;
  const hideFormat = toolbarWidth < 460;
  const showMore = hideFont || hideFormat;

  return (
    <div ref={toolbarRef} className="assignment-editor-toolbar" role="toolbar" aria-label="正文格式">
      <div className="assignment-font-size" aria-label="字号">
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="assignment-editor-tool"
          aria-label="减小字号"
          onMouseDown={preserveSelection}
          onClick={() => {
            onFontSizeStep();
            setSelectionFontSize(fontSize - 1);
          }}
        >
          <Minus aria-hidden="true" />
        </Button>
        <input
          type="number"
          className="assignment-font-size-input"
          min={MIN_FONT_SIZE}
          max={MAX_FONT_SIZE}
          value={fontSizeInput}
          aria-label="字号"
          onChange={(event) => {
            setFontSizeInput(event.target.value);
          }}
          onBlur={commitFontSizeInput}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              // 按 Enter 只取消输入框焦点（会触发 onBlur 提交），
              // 不触发表单提交/关闭对话框，也不把焦点移到正文。
              event.preventDefault();
              event.currentTarget.blur();
            }
          }}
        />
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="assignment-editor-tool"
          aria-label="增大字号"
          onMouseDown={preserveSelection}
          onClick={() => {
            onFontSizeStep();
            setSelectionFontSize(fontSize + 1);
          }}
        >
          <Plus aria-hidden="true" />
        </Button>
      </div>
      <Popover.Root>
        <Popover.Trigger
          render={
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="assignment-editor-tool assignment-text-color-trigger"
              aria-label="文本颜色"
              style={{ "--assignment-text-color": textColor ?? "var(--card-foreground)" } as React.CSSProperties}
            />
          }
          onMouseDown={preserveSelection}
        >
          <Palette aria-hidden="true" />
        </Popover.Trigger>
        <Popover.Portal>
          <Popover.Positioner
            className="assignment-text-color-positioner"
            side="bottom"
            align="start"
            sideOffset={8}
          >
            <Popover.Popup
              className="assignment-text-color-picker"
              initialFocus={false}
              finalFocus={false}
            >
              <ColorPicker value={textColor} onValueChange={setSelectionTextColor} />
            </Popover.Popup>
          </Popover.Positioner>
        </Popover.Portal>
      </Popover.Root>
      {!hideFormat && (
        <div className="assignment-toolbar-format-buttons">
          {formatControls.map((format) => (
            <Button
              key={format.key}
              type="button"
              variant="ghost"
              size="icon-sm"
              className="assignment-editor-tool"
              aria-label={format.label}
              aria-pressed={format.active}
              onMouseDown={preserveSelection}
              onClick={format.toggle}
            >
              {format.icon}
            </Button>
          ))}
        </div>
      )}
      {!hideFont && (
        <Select
          value={fontFamily}
          onValueChange={(value) => setSelectionFontFamily(value ?? "")}
        >
          <SelectPrimitive.Trigger
            type="button"
            className="assignment-font-family-trigger"
            aria-label={fontFamily ? `字体：${fontFamily}` : "字体"}
            style={{ fontFamily: fontFamily ? fontFamilyStack(fontFamily) : undefined }}
            onMouseDown={preserveSelection}
          >
            <span className="assignment-font-family-value">
              {fontFamily || "默认字体"}
            </span>
            <ChevronDown aria-hidden="true" className="assignment-font-family-chevron" />
          </SelectPrimitive.Trigger>
          <SelectContent
            className="assignment-font-family-content"
            positionerClassName="assignment-font-family-positioner"
            side="bottom"
            align="start"
            sideOffset={8}
            finalFocus={false}
            onMouseDown={preserveSelection}
          >
            <SelectItem value="" className="assignment-font-family-item">
              默认字体
            </SelectItem>
            {systemFonts.map((font) => (
              <SelectItem
                key={font}
                value={font}
                className="assignment-font-family-item"
                style={{ fontFamily: fontFamilyStack(font) }}
              >
                {font}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}
      {showMore && (
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="assignment-editor-tool assignment-toolbar-more-button"
                aria-label="更多选项"
              />
            }
            onMouseDown={preserveSelection}
          >
            <MoreHorizontal aria-hidden="true" />
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="start"
            sideOffset={8}
            className="assignment-toolbar-more-menu"
            positionerClassName="assignment-toolbar-more-positioner"
            onMouseDown={preserveSelection}
          >
            {hideFormat && formatControls.map((format) => (
              <DropdownMenuItem
                key={format.key}
                className="assignment-toolbar-more-item"
                data-active={format.active || undefined}
                onMouseDown={preserveSelection}
                onClick={format.toggle}
              >
                {format.icon}
                <span>{format.label}</span>
                {format.active && <Check aria-hidden="true" className="ml-auto size-4" />}
              </DropdownMenuItem>
            ))}
            {hideFont && hideFormat && <DropdownMenuSeparator />}
            {hideFont && (
              <DropdownMenuSub>
                <DropdownMenuSubTrigger
                  className="assignment-toolbar-more-item"
                  onMouseDown={preserveSelection}
                >
                  <Type aria-hidden="true" />
                  <span>字体</span>
                  <span
                    className="assignment-toolbar-more-current"
                    style={{ fontFamily: fontFamily ? fontFamilyStack(fontFamily) : undefined }}
                  >
                    {fontFamily || "默认字体"}
                  </span>
                </DropdownMenuSubTrigger>
                <DropdownMenuSubContent
                  className="assignment-font-family-submenu"
                  positionerClassName="assignment-toolbar-more-positioner"
                  onMouseDown={preserveSelection}
                >
                  <ScrollArea
                    className="assignment-font-family-submenu-scroll"
                    verticalScrollbarClassName="data-vertical:!w-1.5"
                  >
                    <DropdownMenuItem
                      className="assignment-font-family-item"
                      onClick={() => setSelectionFontFamily("")}
                    >
                      默认字体
                      {!fontFamily && <Check aria-hidden="true" className="ml-auto size-4" />}
                    </DropdownMenuItem>
                    {systemFonts.map((font) => (
                      <DropdownMenuItem
                        key={font}
                        className="assignment-font-family-item"
                        style={{ fontFamily: fontFamilyStack(font) }}
                        onClick={() => setSelectionFontFamily(font)}
                      >
                        {font}
                        {fontFamily === font && <Check aria-hidden="true" className="ml-auto size-4" />}
                      </DropdownMenuItem>
                    ))}
                  </ScrollArea>
                </DropdownMenuSubContent>
              </DropdownMenuSub>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      )}
      <div className="assignment-editor-toolbar-actions">
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="assignment-editor-tool"
          aria-label="撤销"
          disabled={!canUndo}
          onMouseDown={preserveSelection}
          onClick={() => {
            editor.dispatchCommand(UNDO_COMMAND, undefined);
          }}
        >
          <Undo2 aria-hidden="true" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="assignment-editor-tool"
          aria-label="重做"
          disabled={!canRedo}
          onMouseDown={preserveSelection}
          onClick={() => {
            editor.dispatchCommand(REDO_COMMAND, undefined);
          }}
        >
          <Redo2 aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}
