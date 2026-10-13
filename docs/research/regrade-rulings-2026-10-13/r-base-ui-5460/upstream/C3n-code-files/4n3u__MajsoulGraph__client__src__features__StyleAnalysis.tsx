import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { Popover } from "@base-ui/react/popover";
import type { ProcessedStyleStats, StyleIntensity, StyleLabel } from "@shared/styleAnalysis";
import { EChart } from "../charts/EChart";
import { buildStyleChartOptions } from "../charts/styleChartOptions";
import { Button, ProgressBar, TextField } from "../components/BaseControls";
import { useErrorToast } from "../components/ErrorToasts";
import { PlayerAccountPicker, type PlayerAccount } from "../components/PlayerAccountPicker";
import { ToolCredit } from "../components/ToolCredit";

type StyleResponse = {
  player: {
    id: number;
    nickname: string;
  };
  processed: ProcessedStyleStats;
  point: {
    x: number;
    y: number;
  };
  analysis: {
    intensity: StyleIntensity;
    style: StyleLabel;
    distance: number;
    slope: number;
  };
};

type SearchResponse = {
  players?: PlayerAccount[];
};

const statLabels: Array<{ key: keyof ProcessedStyleStats; label: string }> = [
  { key: "horyuRate", label: "화료율" },
  { key: "houjuRate", label: "방총율" },
  { key: "furoRate", label: "후로율" },
  { key: "riichiRate", label: "리치율" },
  { key: "damaRate", label: "다마율" },
  { key: "averageScore", label: "평균 화료 타점" },
  { key: "avgHoryuTurn", label: "평균 화료순" },
  { key: "avgHoujuScore", label: "평균 방총 타점" },
  { key: "ryukyokuRate", label: "유국 텐파이율" },
  { key: "riichiTurn", label: "리치순" },
  { key: "riichiFirstRate", label: "리치 선제율" },
  { key: "riichiChaseRate", label: "리치 추격률" }
];

const styleCredits = [
  { label: "@yuraku_urame", href: "https://x.com/yuraku_urame" },
  { label: "@AmaeKoromo_MajS", href: "https://amae-koromo.sapk.ch/" }
] as const;

async function parseStyleResponse(response: Response): Promise<StyleResponse> {
  let body: StyleResponse | null = null;

  try {
    body = (await response.json()) as StyleResponse;
  } catch {
    body = null;
  }

  if (!response.ok || !body) {
    throw new Error("스타일 분석에 실패했습니다.");
  }

  return body;
}

async function parseSearchResponse(response: Response): Promise<SearchResponse> {
  let body: SearchResponse | null = null;

  try {
    body = (await response.json()) as SearchResponse;
  } catch {
    body = null;
  }

  if (!response.ok || !body) {
    throw new Error("닉네임 검색에 실패했습니다.");
  }

  return body;
}

async function fetchStyle(url: string, signal: AbortSignal): Promise<Response> {
  try {
    return await fetch(url, { signal });
  } catch {
    throw new Error("스타일 분석에 실패했습니다.");
  }
}

function formatPoint(value: number): string {
  return value.toFixed(2);
}

function formatStyleSummary(result: StyleResponse): string {
  return `${result.analysis.intensity} ${result.analysis.style}(${formatPoint(result.point.x)},${formatPoint(result.point.y)})`;
}

function formatStat(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

function CountHelpPopover() {
  return (
    <Popover.Root>
      <Popover.Trigger aria-label="대국 수 설명" className="field-help-trigger">
        ?
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Positioner className="base-popover-positioner" sideOffset={6}>
          <Popover.Popup className="base-popover">
            <Popover.Arrow className="base-popover-arrow" />
            <Popover.Title className="base-popover-title">대국 수</Popover.Title>
            <Popover.Description className="base-popover-description">
              숫자를 입력하면 최근 해당 대국 수 범위로 스타일 통계를 분석합니다. 비워두면 전체 기록 기준입니다.
            </Popover.Description>
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}

export function StyleAnalysis() {
  const [nickname, setNickname] = useState("");
  const [count, setCount] = useState("");
  const [playerCandidates, setPlayerCandidates] = useState<PlayerAccount[]>([]);
  const [status, setStatus] = useState("");
  const [result, setResult] = useState<StyleResponse | null>(null);
  const requestIdRef = useRef(0);
  const abortControllerRef = useRef<AbortController | null>(null);
  const showError = useErrorToast();
  const isLoading = Boolean(status);

  useEffect(() => {
    return () => {
      requestIdRef.current += 1;
      abortControllerRef.current?.abort();
    };
  }, []);

  const chartOptions = useMemo(() => {
    return result
      ? buildStyleChartOptions({
          x: result.point.x,
          y: result.point.y,
          nickname: result.player.nickname
        })
      : null;
  }, [result]);

  function validateCount(): string {
    const trimmedCount = count.trim();
    const parsedCount = Number(trimmedCount);
    if (trimmedCount && (!/^\d+$/.test(trimmedCount) || !Number.isSafeInteger(parsedCount) || parsedCount <= 0)) {
      throw new Error("대국 수는 양의 정수로 입력해주세요.");
    }
    return trimmedCount;
  }

  async function analyzePlayer(player: PlayerAccount) {
    const trimmedCount = validateCount();
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    abortControllerRef.current?.abort();
    const abortController = new AbortController();
    abortControllerRef.current = abortController;
    const isCurrentRequest = () => requestIdRef.current === requestId && !abortController.signal.aborted;

    try {
      setResult(null);
      setPlayerCandidates([]);

      if (typeof player.latestTimestamp !== "number") {
        throw new Error("최근 대국 시간이 없는 플레이어입니다.");
      }

      setStatus("loading");
      const params = new URLSearchParams({
        latestTimestamp: String(player.latestTimestamp),
        nickname: player.nickname,
        playerId: String(player.id)
      });
      if (trimmedCount) {
        params.set("count", trimmedCount);
      }

      const body = await parseStyleResponse(
        await fetchStyle(`/api/player-style?${params.toString()}`, abortController.signal)
      );

      if (isCurrentRequest()) {
        setResult(body);
        setStatus("");
        if (abortControllerRef.current === abortController) {
          abortControllerRef.current = null;
        }
      }
    } catch (analysisError) {
      if (isCurrentRequest()) {
        setResult(null);
        setStatus("");
        showError(analysisError instanceof Error ? analysisError.message : "스타일 분석에 실패했습니다.");
        if (abortControllerRef.current === abortController) {
          abortControllerRef.current = null;
        }
      }
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const trimmedNickname = nickname.trim();
    setResult(null);
    setPlayerCandidates([]);

    if (!trimmedNickname) {
      setStatus("");
      showError("닉네임을 입력해주세요.");
      return;
    }

    try {
      validateCount();
    } catch (validationError) {
      setStatus("");
      showError(validationError instanceof Error ? validationError.message : "대국 수는 양의 정수로 입력해주세요.");
      return;
    }

    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    abortControllerRef.current?.abort();
    const abortController = new AbortController();
    abortControllerRef.current = abortController;
    const isCurrentRequest = () => requestIdRef.current === requestId && !abortController.signal.aborted;

    try {
      setStatus("loading");
      const params = new URLSearchParams({ mode: "pl4", nickname: trimmedNickname });

      const body = await parseSearchResponse(
        await fetchStyle(`/api/search-player?${params.toString()}`, abortController.signal)
      );

      if (!isCurrentRequest()) return;

      const players = body.players ?? [];
      if (players.length === 0) {
        throw new Error("플레이어를 찾을 수 없습니다.");
      }

      if (players.length > 1) {
        setStatus("");
        setPlayerCandidates(players);
        if (abortControllerRef.current === abortController) {
          abortControllerRef.current = null;
        }
        return;
      }

      if (isCurrentRequest()) {
        if (abortControllerRef.current === abortController) {
          abortControllerRef.current = null;
        }
      }
      await analyzePlayer(players[0]!);
    } catch (submitError) {
      if (isCurrentRequest()) {
        setResult(null);
        setStatus("");
        showError(submitError instanceof Error ? submitError.message : "스타일 분석에 실패했습니다.");
        if (abortControllerRef.current === abortController) {
          abortControllerRef.current = null;
        }
      }
    }
  }

  return (
    <section className="tool-card style-analysis" aria-labelledby="style-analysis-title">
      <div className="tool-card-header">
        <h2 id="style-analysis-title">사마 스타일 분석</h2>
      </div>

      <form className="style-analysis-form" noValidate onSubmit={handleSubmit}>
        <div className="style-form-grid">
          <TextField
            id="style-nickname-input"
            label="작혼 닉네임"
            name="nickname"
            onValueChange={(value) => {
              setNickname(value);
              setPlayerCandidates([]);
            }}
            disabled={isLoading}
            placeholder="닉네임"
            type="text"
            value={nickname}
          />
          <TextField
            id="style-count-input"
            label="대국 수"
            labelAddon={<CountHelpPopover />}
            name="count"
            onValueChange={(value) => {
              setCount(value);
              setPlayerCandidates([]);
            }}
            disabled={isLoading}
            inputMode="numeric"
            min="1"
            placeholder="전체"
            step="1"
            type="number"
            value={count}
          />
          <Button className="primary-button" type="submit" disabled={isLoading}>
            스타일 분석
          </Button>
        </div>
      </form>

      <PlayerAccountPicker
        disabled={isLoading}
        onSelect={(player) => {
          void analyzePlayer(player);
        }}
        players={playerCandidates}
      />

      {status ? (
        <ProgressBar label="스타일 분석 진행 중" />
      ) : null}

      {result && chartOptions ? (
        <section className="style-results" aria-label="스타일 분석 결과">
          <div className="style-result-head">
            <p className="style-verdict">{formatStyleSummary(result)}</p>
          </div>

          <EChart className="style-chart" option={chartOptions} ariaLabel="스타일 분석 산점도" />

          <dl className="style-stat-list" aria-label="처리된 스타일 통계">
            {statLabels.map((stat) => (
              <div key={stat.key}>
                <dt>{stat.label}</dt>
                <dd>{formatStat(result.processed[stat.key])}</dd>
              </div>
            ))}
          </dl>
        </section>
      ) : null}

      <ToolCredit links={styleCredits} />
    </section>
  );
}
