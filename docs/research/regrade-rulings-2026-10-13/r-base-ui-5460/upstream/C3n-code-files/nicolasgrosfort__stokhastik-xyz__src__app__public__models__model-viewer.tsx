"use client";

import { ScanSearch } from "@/components/admin/scan-search";
import { AudioToggle } from "@/components/common/audio-toggle";
import {
  parseAnnotations,
  resolveAnnotationAssets,
} from "@/components/common/annotations";
import {
  CameraPosition,
  CameraSample,
  Model,
  ModelAnnotation,
} from "@/components/common/model";
import { Collapsible, Select, Slider, Toolbar } from "@base-ui/react";
import {
  parseAsArrayOf,
  parseAsFloat,
  parseAsJson,
  parseAsString,
  useQueryState,
} from "nuqs";
import {
  ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { ErrorBoundary } from "react-error-boundary";

type ModelExtension = "glb" | "ply";

const emptyAnnotation: ModelAnnotation = {
  text: "Nouvelle annotation",
  point: [0, 0, 0],
  labelOffset: [0.5, 0.5, 0],
};

const axisLabels = ["X", "Y", "Z"] as const;

function AxisRow({
  value,
  onChange,
}: {
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <Slider.Root
      value={value}
      min={-100}
      max={100}
      step={0.01}
      onValueChange={(next) => onChange(next as number)}
      className="min-w-0 flex-1"
    >
      <Slider.Control className="flex w-full touch-none items-center py-1 select-none">
        <Slider.Track className="relative h-1 w-full bg-foreground/20 select-none">
          <Slider.Indicator className="bg-foreground select-none" />
          <Slider.Thumb
            aria-label="Coordonnée"
            className="size-3 border border-foreground bg-background select-none focus:outline-none has-focus-visible:outline has-focus-visible:outline-offset-2 has-focus-visible:outline-foreground"
          />
        </Slider.Track>
      </Slider.Control>
    </Slider.Root>
  );
}

function VectorField({
  label,
  vector,
  onChange,
}: {
  label: string;
  vector: [number, number, number];
  onChange: (axis: 0 | 1 | 2, value: number) => void;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      <label className="text-foreground/70">{label}</label>
      {vector.map((coordinate, axis) => (
        <div key={axis} className="flex items-center gap-1">
          <span className="w-2.5 shrink-0 text-foreground/50">
            {axisLabels[axis]}
          </span>
          <AxisRow
            value={coordinate}
            onChange={(value) => onChange(axis as 0 | 1 | 2, value)}
          />
          <input
            type="number"
            step={0.05}
            value={coordinate}
            onChange={(event) =>
              onChange(axis as 0 | 1 | 2, parseFloat(event.target.value) || 0)
            }
            className="w-12 min-w-0 shrink-0 border border-foreground/40 bg-background px-1 py-0.5 tabular-nums focus:outline-none focus:border-foreground"
          />
        </div>
      ))}
    </div>
  );
}

function SidePanel({
  side,
  title,
  defaultOpen = false,
  children,
}: {
  side: "left" | "right";
  title: string;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  return (
    <Collapsible.Root
      defaultOpen={defaultOpen}
      className={`absolute inset-y-0 z-10 flex items-stretch ${
        side === "left" ? "left-0" : "right-0 flex-row-reverse"
      }`}
    >
      <Collapsible.Trigger className="group flex shrink-0 cursor-pointer flex-col items-center justify-center gap-2 border border-foreground bg-background/60 px-1.5 py-4 font-mono text-[10px] uppercase shadow-md backdrop-blur-sm sm:text-xs">
        <span className="transition-transform group-data-[panel-open]:rotate-180">
          {side === "left" ? "▸" : "◂"}
        </span>
        <span className="[writing-mode:vertical-rl]">{title}</span>
      </Collapsible.Trigger>
      <Collapsible.Panel className="h-full w-(--collapsible-panel-width) overflow-hidden transition-[width] duration-200 ease-out data-starting-style:w-0 data-ending-style:w-0">
        <div className="flex h-full w-72 max-w-[70vw] flex-col gap-2 overflow-y-auto border border-foreground bg-background/60 shadow-md backdrop-blur-sm p-2 font-mono text-[10px] uppercase sm:text-xs">
          {children}
        </div>
      </Collapsible.Panel>
    </Collapsible.Root>
  );
}

function AnnotationsPanel({
  annotations,
  setAnnotations,
  placing,
  onTogglePlacing,
  selectedIndex,
  onSelectIndex,
}: {
  annotations: ModelAnnotation[];
  setAnnotations: (annotations: ModelAnnotation[]) => void;
  placing: boolean;
  onTogglePlacing: () => void;
  selectedIndex: number | null;
  onSelectIndex: (index: number | null) => void;
}) {
  const updateAnnotation = (index: number, patch: Partial<ModelAnnotation>) =>
    setAnnotations(
      annotations.map((annotation, i) =>
        i === index ? { ...annotation, ...patch } : annotation,
      ),
    );

  const updateVector = (
    index: number,
    key: "point" | "labelOffset",
    axis: 0 | 1 | 2,
    value: number,
  ) => {
    const current = annotations[index][key] ?? [0, 0, 0];
    const next: [number, number, number] = [...current];
    next[axis] = value;
    updateAnnotation(index, { [key]: next });
  };

  const removeAnnotation = (index: number) => {
    setAnnotations(annotations.filter((_, i) => i !== index));
    if (selectedIndex === index) onSelectIndex(null);
  };

  const addAnnotation = () =>
    setAnnotations([...annotations, { ...emptyAnnotation }]);

  const [copied, setCopied] = useState(false);

  const copyAnnotations = () => {
    navigator.clipboard.writeText(JSON.stringify(annotations)).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    });
  };

  return (
    <SidePanel side="left" title="Annotations">
      <div className="flex items-center justify-between gap-2">
        <span className="text-foreground/70">Annotations</span>
        <button
          type="button"
          onClick={addAnnotation}
          className="cursor-pointer border border-foreground px-2 py-0.5"
        >
          + Ajouter
        </button>
      </div>
      <button
        type="button"
        onClick={onTogglePlacing}
        aria-pressed={placing}
        data-pressed={placing}
        className="cursor-pointer border border-foreground px-2 py-0.5 data-[pressed=true]:bg-foreground data-[pressed=true]:text-background"
      >
        {placing ? "Cliquer sur le modèle…" : "+ Placer au clic"}
      </button>
      {annotations.length > 0 && (
        <button
          type="button"
          onClick={copyAnnotations}
          className="cursor-pointer border border-foreground px-2 py-0.5"
        >
          {copied ? "Copié !" : "Copier"}
        </button>
      )}
      {annotations.length === 0 && (
        <p className="text-foreground/50 normal-case">
          Aucune annotation pour ce modèle.
        </p>
      )}
      {annotations.map((annotation, index) => (
        <div
          key={index}
          className="flex flex-col gap-1 border border-foreground/40 p-2"
        >
          <div className="flex items-center gap-1">
            <input
              type="text"
              value={annotation.text}
              onChange={(event) =>
                updateAnnotation(index, { text: event.target.value })
              }
              className="min-w-0 flex-1 border border-foreground/40 bg-background px-1 py-0.5 normal-case focus:outline-none focus:border-foreground"
            />
            <button
              type="button"
              onClick={() =>
                onSelectIndex(selectedIndex === index ? null : index)
              }
              aria-pressed={selectedIndex === index}
              data-pressed={selectedIndex === index}
              aria-label="Déplacer dans l'espace 3D"
              className="shrink-0 cursor-pointer border border-foreground px-1.5 py-0.5 data-[pressed=true]:bg-foreground data-[pressed=true]:text-background"
            >
              ✥
            </button>
            <button
              type="button"
              onClick={() => removeAnnotation(index)}
              aria-label="Supprimer l'annotation"
              className="shrink-0 cursor-pointer border border-foreground px-1.5 py-0.5"
            >
              ×
            </button>
          </div>
          <VectorField
            label="Point"
            vector={annotation.point}
            onChange={(axis, value) =>
              updateVector(index, "point", axis, value)
            }
          />
          <VectorField
            label="Label"
            vector={annotation.labelOffset ?? [0.5, 0.5, 0]}
            onChange={(axis, value) =>
              updateVector(index, "labelOffset", axis, value)
            }
          />
          <div className="flex flex-col gap-0.5">
            <label className="text-foreground/70">Modèle du label</label>
            <input
              type="text"
              placeholder="ex: chaise.glb"
              value={annotation.labelModel ?? ""}
              onChange={(event) =>
                updateAnnotation(index, {
                  labelModel: event.target.value || undefined,
                })
              }
              className="min-w-0 border border-foreground/40 bg-background px-1 py-0.5 normal-case focus:outline-none focus:border-foreground"
            />
          </div>
          {annotation.labelModel && (
            <div className="flex items-center gap-1">
              <span className="w-2.5 shrink-0 text-foreground/50">Scale</span>
              <AxisRow
                value={annotation.labelModelScale ?? 0.2}
                onChange={(value) =>
                  updateAnnotation(index, { labelModelScale: value })
                }
              />
              <input
                type="number"
                step={0.05}
                value={annotation.labelModelScale ?? 0.2}
                onChange={(event) =>
                  updateAnnotation(index, {
                    labelModelScale: parseFloat(event.target.value) || 0.2,
                  })
                }
                className="w-12 min-w-0 shrink-0 border border-foreground/40 bg-background px-1 py-0.5 tabular-nums focus:outline-none focus:border-foreground"
              />
            </div>
          )}
        </div>
      ))}
    </SidePanel>
  );
}

const formatCoordinate = (value: number) => Math.round(value * 100) / 100;

const formatVector3 = ([x, y, z]: CameraPosition) =>
  `${formatCoordinate(x)}, ${formatCoordinate(y)}, ${formatCoordinate(z)}`;

const formatCurveCode = (name: string, points: CameraPosition[]) => {
  const lines = points
    .map(([x, y, z]) => `    new THREE.Vector3(${x}, ${y}, ${z}),`)
    .join("\n");
  return `const ${name} = new THREE.CatmullRomCurve3(\n  [\n${lines}\n  ],\n  true,\n);`;
};

function CameraPathPanel({
  liveSample,
  capturedSamples,
  onCapture,
  onRemove,
  onClear,
}: {
  liveSample: CameraSample | null;
  capturedSamples: CameraSample[];
  onCapture: () => void;
  onRemove: (index: number) => void;
  onClear: () => void;
}) {
  const [copied, setCopied] = useState(false);

  const copyCode = () => {
    const code = [
      formatCurveCode(
        "cameraPositions",
        capturedSamples.map((sample) => sample.position),
      ),
      formatCurveCode(
        "cameraLookAts",
        capturedSamples.map((sample) => sample.lookAt),
      ),
    ].join("\n\n");
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    });
  };

  return (
    <SidePanel side="right" title="Caméra">
      <div className="flex items-center justify-between gap-2">
        <span className="text-foreground/70">Caméra</span>
        <button
          type="button"
          onClick={onCapture}
          disabled={!liveSample}
          className="cursor-pointer border border-foreground px-2 py-0.5 disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Capturer (Ctrl+C)
        </button>
      </div>
      {liveSample ? (
        <div className="flex flex-col gap-0.5 text-foreground/70 normal-case">
          <span className="tabular-nums">Pos {formatVector3(liveSample.position)}</span>
          <span className="tabular-nums">Look {formatVector3(liveSample.lookAt)}</span>
        </div>
      ) : (
        <span className="text-foreground/50 normal-case">
          Active le mode FPS pour suivre la caméra.
        </span>
      )}
      {capturedSamples.length > 0 && (
        <>
          <div className="flex flex-col gap-1">
            {capturedSamples.map((sample, index) => (
              <div
                key={index}
                className="flex items-start justify-between gap-1 border border-foreground/40 px-1.5 py-1 normal-case"
              >
                <div className="flex flex-col gap-0.5 tabular-nums">
                  <span>Pos {formatVector3(sample.position)}</span>
                  <span>Look {formatVector3(sample.lookAt)}</span>
                </div>
                <button
                  type="button"
                  onClick={() => onRemove(index)}
                  aria-label="Supprimer le point"
                  className="shrink-0 cursor-pointer border border-foreground px-1.5"
                >
                  ×
                </button>
              </div>
            ))}
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={copyCode}
              className="cursor-pointer border border-foreground px-2 py-0.5"
            >
              {copied ? "Copié !" : "Copier le code"}
            </button>
            <button
              type="button"
              onClick={onClear}
              className="cursor-pointer border border-foreground px-2 py-0.5"
            >
              Vider
            </button>
          </div>
        </>
      )}
    </SidePanel>
  );
}

const subscribeNever = () => () => {};

export function ModelViewer({ isAdmin }: { isAdmin: boolean }) {
  // The camera distance comes from the URL and yields long-decimal slider
  // percentages, which never hydrate cleanly: render that slider client-side.
  const isClient = useSyncExternalStore(
    subscribeNever,
    () => true,
    () => false,
  );
  const [file, setFile] = useQueryState("file", parseAsString.withDefault(""));
  const [modelFiles, setModelFiles] = useState<string[]>([]);
  const [extensionFilter, setExtensionFilter] = useState<Set<ModelExtension>>(
    new Set(["ply"]),
  );

  useEffect(() => {
    fetch("/api/assets/models")
      .then((res) => res.json())
      .then((data: { files: string[] }) => setModelFiles(data.files ?? []))
      .catch(() => setModelFiles([]));
  }, []);

  const toggleExtensionFilter = (extension: ModelExtension) => {
    setExtensionFilter((previous) => {
      const next = new Set(previous);
      if (next.has(extension)) {
        next.delete(extension);
      } else {
        next.add(extension);
      }
      return next;
    });
  };

  // Un résultat de recherche doit rester visible même si son format est masqué.
  const selectSearchResult = useCallback(
    (selected: string) => {
      const extension = selected
        .split(".")
        .pop()
        ?.toLowerCase() as ModelExtension;
      setExtensionFilter((previous) =>
        previous.has(extension) ? previous : new Set(previous).add(extension),
      );
      setFile(selected);
    },
    [setFile],
  );

  const filteredModelFiles =
    extensionFilter.size === 0
      ? modelFiles
      : modelFiles.filter((modelFile) =>
          extensionFilter.has(
            modelFile.split(".").pop()?.toLowerCase() as ModelExtension,
          ),
        );

  useEffect(() => {
    if (!file && filteredModelFiles.length > 0) {
      setFile(filteredModelFiles[0]);
    }
  }, [file, filteredModelFiles, setFile]);

  const [position, setPosition] = useQueryState(
    "position",
    parseAsString.withDefault("0.5"),
  );
  const [camera, setCamera] = useQueryState(
    "camera",
    parseAsArrayOf(parseAsFloat, ",").withOptions({ history: "replace" }),
  );
  const [stopRotation, setStopRotation] = useQueryState(
    "stopRotation",
    parseAsString.withDefault("false"),
  );

  const [enablePan, setEnablePan] = useQueryState(
    "enablePan",
    parseAsString.withDefault("false"),
  );
  const [fpsMode, setFpsMode] = useState(false);
  const [liveSample, setLiveSample] = useState<CameraSample | null>(null);
  const [capturedSamples, setCapturedSamples] = useState<CameraSample[]>([]);
  const liveSampleRef = useRef<CameraSample | null>(null);
  liveSampleRef.current = liveSample;

  const captureSample = useCallback(() => {
    const sample = liveSampleRef.current;
    if (!sample) return;
    setCapturedSamples((previous) => [...previous, sample]);
  }, []);

  useEffect(() => {
    if (!isAdmin || !fpsMode) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.code !== "KeyC") return;
      if (event.target instanceof HTMLElement && event.target.closest("input, textarea")) {
        return;
      }
      event.preventDefault();
      captureSample();
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isAdmin, fpsMode, captureSample]);
  const [pointSize, setPointSize] = useQueryState(
    "pointSize",
    parseAsString.withDefault("0.01"),
  );
  const [audio] = useQueryState("audio", parseAsString);
  const [annotations, setAnnotations] = useQueryState(
    "annotations",
    parseAsJson(parseAnnotations)
      .withDefault([])
      .withOptions({ history: "replace" }),
  );
  const [placingAnnotation, setPlacingAnnotation] = useState(false);
  const [selectedAnnotationIndex, setSelectedAnnotationIndex] = useState<
    number | null
  >(null);

  const handlePlaceAnnotation = useCallback(
    (point: [number, number, number]) => {
      setAnnotations([...annotations, { ...emptyAnnotation, point }]);
    },
    [annotations, setAnnotations],
  );

  const handleAnnotationPointChange = useCallback(
    (index: number, point: [number, number, number]) => {
      setAnnotations(
        annotations.map((annotation, i) =>
          i === index ? { ...annotation, point } : annotation,
        ),
      );
    },
    [annotations, setAnnotations],
  );

  const handleSelectAnnotation = useCallback((index: number) => {
    setSelectedAnnotationIndex((previous) => (previous === index ? null : index));
  }, []);

  const stopRotationValue = stopRotation === "true";
  const enablePanValue = enablePan === "true";
  const pointSizeValue = parseFloat(pointSize) || 0.01;

  const modelUrl = file ? `/api/assets/models/${file}` : null;

  const cameraPosition: CameraPosition | undefined =
    camera?.length === 3 && camera.every(Number.isFinite)
      ? [camera[0], camera[1], camera[2]]
      : undefined;

  const modelPosition = position ? parseFloat(position) : 0.5;

  // The POS slider is the distance along the (1,1,1) diagonal; once the user
  // has orbited, show the distance of the actual camera instead.
  const displayedPosition = cameraPosition
    ? Math.round((Math.hypot(...cameraPosition) / Math.sqrt(3)) * 10) / 10
    : modelPosition;

  const resolvedAnnotations = resolveAnnotationAssets(annotations);

  return (
    <section className="h-dvh w-screen min-h-0 flex flex-col items-center fixed top-0 left-0 right-0 bottom-0 bg-background">
      <Toolbar.Root className="absolute top-[calc(0.5rem+env(safe-area-inset-top))] left-1/2 -translate-x-1/2 z-10 flex max-w-[calc(100vw-1rem)] flex-nowrap items-center gap-2 overflow-x-auto scrollbar-none border border-foreground bg-background/60 shadow-md backdrop-blur-sm p-1 font-mono text-[10px] whitespace-nowrap uppercase sm:text-xs">
        <Toolbar.Group className="flex items-center gap-1">
          <label className="px-1 text-foreground/70">POS</label>
          {isClient ? (
            <Slider.Root
              value={displayedPosition}
              min={0.1}
              max={100}
              step={0.1}
              onValueChange={(value) => {
                setCamera(null);
                setPosition(String(value));
              }}
            >
              <Slider.Control className="flex w-12 touch-none items-center py-1 select-none">
                <Slider.Track className="relative h-1 w-full bg-foreground/20 select-none">
                  <Slider.Indicator className="bg-foreground select-none" />
                  <Slider.Thumb
                    aria-label="Position"
                    className="size-3 border border-foreground bg-background select-none focus:outline-none has-focus-visible:outline has-focus-visible:outline-offset-2 has-focus-visible:outline-foreground"
                  />
                </Slider.Track>
              </Slider.Control>
            </Slider.Root>
          ) : (
            <div className="h-5 w-12" />
          )}
          <span className="w-8 text-right tabular-nums text-foreground/70 select-none">
            {displayedPosition.toFixed(1)}
          </span>
        </Toolbar.Group>
        <Toolbar.Separator className="h-4 w-px bg-foreground" />
        <Toolbar.Group className="flex items-center gap-1">
          <label className="px-1 text-foreground/70">SIZE</label>
          <Slider.Root
            value={pointSizeValue}
            min={0.001}
            max={0.1}
            step={0.001}
            onValueChange={(value) => setPointSize(String(value))}
          >
            <Slider.Control className="flex w-12 touch-none items-center py-1 select-none">
              <Slider.Track className="relative h-1 w-full bg-foreground/20 select-none">
                <Slider.Indicator className="bg-foreground select-none" />
                <Slider.Thumb
                  aria-label="Taille du point"
                  className="size-3 border border-foreground bg-background select-none focus:outline-none has-focus-visible:outline has-focus-visible:outline-offset-2 has-focus-visible:outline-foreground"
                />
              </Slider.Track>
            </Slider.Control>
          </Slider.Root>
          <span className="w-10 text-right tabular-nums text-foreground/70 select-none">
            {pointSizeValue.toFixed(3)}
          </span>
        </Toolbar.Group>
        <Toolbar.Separator className="h-4 w-px bg-foreground" />
        <Toolbar.Group className="flex items-center gap-1">
          <Toolbar.Button
            aria-pressed={stopRotationValue}
            onClick={() =>
              setStopRotation(stopRotationValue ? "false" : "true")
            }
            className="cursor-pointer px-2 py-0.5 data-[pressed=true]:bg-foreground data-[pressed=true]:text-background uppercase"
            data-pressed={stopRotationValue}
          >
            Rotation
          </Toolbar.Button>
          <Toolbar.Button
            aria-pressed={enablePanValue}
            onClick={() => setEnablePan(enablePanValue ? "false" : "true")}
            className="cursor-pointer px-2 py-0.5 data-[pressed=true]:bg-foreground data-[pressed=true]:text-background uppercase"
            data-pressed={enablePanValue}
          >
            Pan
          </Toolbar.Button>
          <Toolbar.Button
            aria-pressed={fpsMode}
            onClick={() => setFpsMode((previous) => !previous)}
            className="cursor-pointer px-2 py-0.5 data-[pressed=true]:bg-foreground data-[pressed=true]:text-background uppercase"
            data-pressed={fpsMode}
          >
            FPS
          </Toolbar.Button>
          {audio && <AudioToggle />}
        </Toolbar.Group>
      </Toolbar.Root>
      <Toolbar.Root className="absolute bottom-[calc(0.5rem+env(safe-area-inset-bottom))] left-1/2 -translate-x-1/2 z-10 flex max-w-[calc(100vw-1rem)] flex-nowrap items-center gap-2 overflow-x-auto scrollbar-none border border-foreground bg-background/60 shadow-md backdrop-blur-sm p-1 font-mono text-[10px] whitespace-nowrap uppercase sm:text-xs">
        {isAdmin && (
          <>
            <ScanSearch currentFile={file} onSelect={selectSearchResult} />
            <Toolbar.Separator className="h-4 w-px bg-foreground" />
          </>
        )}
        <Toolbar.Group className="flex items-center gap-1">
          <label className="px-1 text-foreground/70">Model</label>
          <Select.Root
            value={file}
            onValueChange={(value) => setFile(value ?? "")}
            items={filteredModelFiles.map((modelFile) => ({
              label: modelFile,
              value: modelFile,
            }))}
          >
            <Toolbar.Button
              render={<Select.Trigger />}
              className="flex min-w-32 max-w-40 cursor-pointer items-center justify-between gap-2 overflow-hidden  bg-background p-1 font-mono focus:outline focus:-outline-offset-2 focus:outline-foreground"
            >
              <Select.Value placeholder="Choisir…" className="truncate" />
              <Select.Icon className="shrink-0">▾</Select.Icon>
            </Toolbar.Button>
            <Select.Portal>
              <Select.Positioner
                className="z-20 outline-none"
                side="top"
                sideOffset={4}
              >
                <Select.Popup className="max-h-(--available-height) min-w-(--anchor-width) overflow-y-auto border border-foreground bg-background font-mono text-[10px] uppercase sm:text-xs">
                  {filteredModelFiles.map((modelFile) => (
                    <Select.Item
                      key={modelFile}
                      value={modelFile}
                      className="cursor-pointer px-2 py-1 data-highlighted:bg-foreground data-highlighted:text-background"
                    >
                      <Select.ItemText>{modelFile}</Select.ItemText>
                    </Select.Item>
                  ))}
                </Select.Popup>
              </Select.Positioner>
            </Select.Portal>
          </Select.Root>
        </Toolbar.Group>
        <Toolbar.Separator className="h-4 w-px bg-foreground" />
        <Toolbar.Group className="flex items-center gap-1">
          <Toolbar.Button
            aria-pressed={extensionFilter.has("glb")}
            onClick={() => toggleExtensionFilter("glb")}
            className="cursor-pointer px-2 py-0.5 data-[pressed=true]:bg-foreground data-[pressed=true]:text-background"
            data-pressed={extensionFilter.has("glb")}
          >
            GLB
          </Toolbar.Button>
          <Toolbar.Button
            aria-pressed={extensionFilter.has("ply")}
            onClick={() => toggleExtensionFilter("ply")}
            className="cursor-pointer px-2 py-0.5 data-[pressed=true]:bg-foreground data-[pressed=true]:text-background"
            data-pressed={extensionFilter.has("ply")}
          >
            PLY
          </Toolbar.Button>
        </Toolbar.Group>
      </Toolbar.Root>
      {isAdmin && (
        <AnnotationsPanel
          annotations={annotations}
          setAnnotations={(next) => setAnnotations(next)}
          placing={placingAnnotation}
          onTogglePlacing={() => setPlacingAnnotation((previous) => !previous)}
          selectedIndex={selectedAnnotationIndex}
          onSelectIndex={setSelectedAnnotationIndex}
        />
      )}
      {isAdmin && (
        <CameraPathPanel
          liveSample={liveSample}
          capturedSamples={capturedSamples}
          onCapture={captureSample}
          onRemove={(index) =>
            setCapturedSamples((previous) =>
              previous.filter((_, i) => i !== index),
            )
          }
          onClear={() => setCapturedSamples([])}
        />
      )}
      <div className="relative w-full flex-1 min-h-0">
        {modelUrl ? (
          <ErrorBoundary
            key={modelUrl}
            fallback={
              <p className="absolute inset-0 flex items-center justify-center text-xs font-mono text-center p-2 uppercase">
                Modèle introuvable
              </p>
            }
          >
            <Model
              model={modelUrl}
              position={modelPosition}
              rotation={0}
              stopRotation={!stopRotationValue}
              enablePan={enablePanValue}
              pointSize={pointSizeValue}
              annotations={resolvedAnnotations}
              audio={audio ?? undefined}
              cameraPosition={cameraPosition}
              onCameraChange={(next) =>
                setCamera(next.map((value) => Math.round(value * 1000) / 1000))
              }
              fpsMode={fpsMode}
              onCameraFrame={isAdmin ? setLiveSample : undefined}
              placingAnnotation={isAdmin && placingAnnotation}
              onPlaceAnnotation={isAdmin ? handlePlaceAnnotation : undefined}
              selectedAnnotationIndex={isAdmin ? selectedAnnotationIndex : null}
              onSelectAnnotation={isAdmin ? handleSelectAnnotation : undefined}
              onAnnotationPointChange={
                isAdmin ? handleAnnotationPointChange : undefined
              }
            />
          </ErrorBoundary>
        ) : (
          <p className="absolute inset-0 flex items-center justify-center text-xs font-mono text-center p-2 uppercase">
            Ajoute ?file=nom.glb&position=0.5 à l&apos;URL.
          </p>
        )}
      </div>
    </section>
  );
}
