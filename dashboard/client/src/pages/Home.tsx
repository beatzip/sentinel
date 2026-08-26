/** Radar Room design: tactical cartography, Signal Amber accents, evidence-first hierarchy, no simulated match data. */
// Radar Room: dark tactical cartography with signal amber reserved for verified evidence.
import { useEffect, useMemo, useState } from "react";
import { startLogin } from "@/const";
import { useAuth } from "@/_core/hooks/useAuth";
import { trpc } from "@/lib/trpc";
import ReplayViewer from "@/components/ReplayViewer";
import {
  Activity,
  AlertTriangle,
  ArrowUpRight,
  BookOpenCheck,
  ChevronRight,
  CircleDot,
  ClipboardCheck,
  Crosshair,
  Database,
  FileSearch,
  Layers3,
  Radar,
  RefreshCw,
  Sparkles,
  UsersRound,
} from "lucide-react";

type ReportSummary = { id: string; map: string; rounds: number; anomaly_score: number };
type Evidence = { tick?: number; category?: string; confidence?: number; description?: string };
type Player = { steam_id: number; name: string; team: string; scores: { overall: number }; evidence: Evidence[]; summary: string };
type DeathExplanation = { tick: number; summary: string; facts: string[] };
type RoundStory = { headline: string; result: string; deaths: DeathExplanation[] };
type Round = { round_number: number; t_score: number; ct_score: number; story: RoundStory };
type Report = { metadata: { map_name: string; total_rounds: number; duration_seconds: number; tick_rate: number }; players: Player[]; overall_anomaly: number; rounds?: Round[] };
type DossierMatch = { report_id: string; map: string; player: Player; provenance: { engine_version: string; demo_parser_version: string; map_asset_version: string }; reanalysis: { required: boolean; reasons: string[] } };
type PlayerDossier = { steam_id: number; name: string; matches_observed: number; flagged_matches: number; confidence: { recurrence: number; status: string; level: string }; matches: DossierMatch[] };

const API_URL = (import.meta.env.VITE_SENTINEL_API_URL as string | undefined)?.replace(/\/$/, "") || "http://127.0.0.1:8787";
const navItems = [
  [Radar, "Обзор", "overview"],
  [Activity, "Повтор", "replay"],
  [BookOpenCheck, "Раунды", "rounds"],
  [FileSearch, "Доказательства", "evidence"],
  [UsersRound, "Игроки", "players"],
  [Database, "Архив", "archive"],
] as const;

function scoreTone(score: number) {
  if (score >= 0.7) return "critical";
  if (score >= 0.4) return "watch";
  return "clear";
}

function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

export default function Home() {
  const { isAuthenticated } = useAuth();
  const [reports, setReports] = useState<ReportSummary[]>([]);
  const [report, setReport] = useState<Report | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [connection, setConnection] = useState<"loading" | "ready" | "offline">("loading");
  const [loadingReport, setLoadingReport] = useState(false);
  const [dossier, setDossier] = useState<PlayerDossier | null>(null);
  const [loadingDossier, setLoadingDossier] = useState(false);
  const [activeView, setActiveView] = useState<"overview" | "replay">("overview");
  const [activeNav, setActiveNav] = useState("overview");

  async function loadReports() {
    setConnection("loading");
    try {
      const response = await fetch(`${API_URL}/v1/reports`);
      if (!response.ok) throw new Error("report list unavailable");
      const nextReports = (await response.json()) as ReportSummary[];
      setReports(nextReports);
      setConnection("ready");
      if (nextReports[0] && !selectedId) setSelectedId(nextReports[0].id);
    } catch {
      setReports([]);
      setConnection("offline");
    }
  }

  async function loadDossier(steamId: number) {
    setLoadingDossier(true);
    try {
      const response = await fetch(`${API_URL}/v1/players/${steamId}/dossier`);
      if (!response.ok) throw new Error("dossier unavailable");
      setDossier((await response.json()) as PlayerDossier);
    } catch {
      setDossier(null);
    } finally {
      setLoadingDossier(false);
    }
  }

  useEffect(() => {
    void loadReports();
  }, []);

  useEffect(() => {
    if (!selectedId) {
      setReport(null);
      return;
    }
    let cancelled = false;
    setLoadingReport(true);
    fetch(`${API_URL}/v1/reports/${selectedId}`)
      .then((response) => (response.ok ? response.json() : Promise.reject(new Error("report unavailable"))))
      .then((nextReport: Report) => !cancelled && setReport(nextReport))
      .catch(() => !cancelled && setReport(null))
      .finally(() => !cancelled && setLoadingReport(false));
    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  const players = useMemo(
    () => [...(report?.players ?? [])].sort((left, right) => right.scores.overall - left.scores.overall),
    [report],
  );
  const evidence = useMemo(() => players.flatMap((player) => player.evidence.map((item) => ({ ...item, player: player.name }))).slice(0, 6), [players]);
  const activeScore = report?.overall_anomaly ?? 0;
  const rounds = report?.rounds ?? [];
  const summaryFacts = useMemo(() => [
    ...rounds.flatMap((round) => [
      { id: `round-${round.round_number}`, text: `${round.story?.headline ?? `Round ${round.round_number}`}. ${round.story?.result ?? "Result recorded."}` },
      ...(round.story?.deaths ?? []).map((death, index) => ({ id: `death-${round.round_number}-${index}`, text: `Tick ${death.tick}: ${death.summary}` })),
    ]),
    ...evidence.map((event, index) => ({ id: `evidence-${index}`, text: `Tick ${event.tick ?? "unknown"}: ${event.player} — ${event.category ?? "behavioral signal"}; ${event.description ?? "details recorded"}` })),
  ].slice(0, 80), [rounds, evidence]);
  const summaryMutation = trpc.summary.generate.useMutation();

  function requestSummary() {
    if (!isAuthenticated) {
      startLogin();
      return;
    }
    if (!report || !selectedId || !summaryFacts.length) return;
    summaryMutation.mutate({
      reportId: selectedId,
      map: report.metadata.map_name,
      overallRisk: report.overall_anomaly,
      facts: summaryFacts,
    });
  }

  return (
    <main className="radar-shell">
      <aside className="command-rail">
        <div className="brand-lockup">
          <img src="/manus-storage/sentinel-radar-logo_dcde56d8.png" alt="Sentinel" className="brand-mark" />
          <div><strong>SENTINEL</strong><span>RADAR ROOM</span></div>
        </div>
        <nav aria-label="Разделы панели">
          {navItems.map(([Icon, label, target]) => (
            <button className={`rail-item ${activeNav === target ? "is-active" : ""}`} key={label} type="button" onClick={() => { setActiveNav(target); setActiveView(target === "replay" ? "replay" : "overview"); const section = target === "evidence" ? "evidence-rail" : target === "players" ? "dossier" : target === "archive" ? "archive" : target === "rounds" ? "round-story" : null; if (section) window.requestAnimationFrame(() => document.getElementById(section)?.scrollIntoView({ behavior: "smooth", block: "center" })); }}>
              <Icon size={18} strokeWidth={1.8} /><span>{label}</span>
            </button>
          ))}
        </nav>
        <div className="rail-footer"><span className="status-dot" data-state={connection} /><span>{connection === "ready" ? "API на связи" : connection === "loading" ? "Проверяем API" : "API недоступен"}</span></div>
      </aside>

      <section className="analysis-surface">
        <header className="surface-header">
          <div><p className="eyebrow">ПОВЕДЕНЧЕСКАЯ АНАЛИТИКА · CS2</p><h1>{activeView === "replay" ? "Интерактивный повтор" : "Карта наблюдений"}</h1></div>
          <button type="button" onClick={() => void loadReports()} className="refresh-action"><RefreshCw size={16} /> Синхронизировать</button>
        </header>

        {activeView === "replay" ? <ReplayViewer apiUrl={API_URL} reportId={selectedId} onClose={() => { setActiveView("overview"); setActiveNav("overview"); }} /> : <>
        <section className="hero-command" style={{ backgroundImage: "linear-gradient(90deg, rgba(7,19,19,.96) 0%, rgba(7,19,19,.70) 56%, rgba(7,19,19,.20) 100%), url('/manus-storage/sentinel-command-background_4f088e6f.jpg')" }}>
          <div className="hero-copy"><p className="eyebrow amber">АКТИВНЫЙ КОНТУР</p><div className="telemetry-line"><span>RPT / {selectedId ?? "PENDING"}</span><span>VERIFY / {report ? "SOURCE-BOUND" : "AWAITING"}</span></div><h2>{report ? report.metadata.map_name.replace("de_", "").toUpperCase() : "Ожидание отчёта"}</h2><p>{report ? `${report.metadata.total_rounds} раундов · ${report.metadata.tick_rate} тик/с · ${Math.round(report.metadata.duration_seconds / 60)} мин анализа` : "Подключите Sentinel API к каталогу JSON-отчётов, чтобы открыть временную шкалу и доказательства."}</p></div>
          <div className="hero-directives"><div><span>01 / SOURCE</span><strong>{connection === "ready" ? "API LINKED" : "LOCAL API"}</strong></div><div><span>02 / NEXT CHECK</span><strong>{report ? "REVIEW EVIDENCE" : "CONNECT REPORT"}</strong></div><div><span>03 / CONFIDENCE</span><strong>{report ? "REPORT-BOUND" : "PENDING"}</strong></div></div>
          <div className={`radar-score ${scoreTone(activeScore)}`}><span>ОБЩИЙ РИСК</span><strong>{formatPercent(activeScore)}</strong><small>{report ? "на основе отчёта" : "нет данных"}</small></div>
          <div className="coordinate-stamp">SECTOR / {selectedId ?? "—"}<br />SOURCE / {API_URL.replace(/^https?:\/\//, "")}</div>
        </section>

        <section className="metric-strip" aria-label="Ключевые показатели">
          <Metric icon={Layers3} label="Отчётов в архиве" value={String(reports.length).padStart(2, "0")} note="доступно через API" />
          <Metric icon={UsersRound} label="Игроков в матче" value={String(players.length).padStart(2, "0")} note={report ? "профили загружены" : "ожидается отчёт"} />
          <Metric icon={ClipboardCheck} label="Сигналов" value={String(evidence.length).padStart(2, "0")} note="событий для проверки" />
          <Metric icon={Crosshair} label="Состояние контура" value={connection === "ready" ? "LIVE" : "IDLE"} note={connection === "ready" ? "данные доступны" : "локальный режим"} />
        </section>

        <section className="workspace-grid">
          <article className="panel dossier-panel" id="dossier">
            <div className="panel-heading"><div><p className="eyebrow">ДОСЬЕ МАТЧА</p><h3>Профили риска</h3></div><span className="panel-index">SECTOR / 01</span></div>
            {players.length ? <div className="player-list">{players.map((player, index) => <button className="player-row" type="button" onClick={() => void loadDossier(player.steam_id)} key={player.steam_id}><span className="rank">{String(index + 1).padStart(2, "0")}</span><div className="player-name"><strong>{player.name}</strong><small>{player.team} · {player.steam_id}</small></div><div className="score-bar"><i style={{ width: `${Math.min(player.scores.overall * 100, 100)}%` }} /></div><strong className={`risk-value ${scoreTone(player.scores.overall)}`}>{formatPercent(player.scores.overall)}</strong><ChevronRight size={16} /></button>)}</div> : <EmptyState icon={UsersRound} title="Нет профилей для отображения" body="Первая запись появится после загрузки JSON-отчёта из API." />}
            {loadingDossier ? <p className="dossier-status">Загрузка local dossier…</p> : dossier ? <section className="local-dossier"><header><div><p className="eyebrow amber">LOCAL DOSSIER / {dossier.steam_id}</p><h4>{dossier.name}</h4></div><strong>{dossier.confidence.status}</strong></header><div className="dossier-metrics"><span><small>МАТЧЕЙ</small><b>{dossier.matches_observed}</b></span><span><small>ПОВТОРЯЕМОСТЬ</small><b>{formatPercent(dossier.confidence.recurrence)}</b></span><span><small>FLAGGED</small><b>{dossier.flagged_matches}</b></span></div><ol>{dossier.matches.map((match) => <li key={match.report_id}><div><strong>{match.map}</strong><small>RPT / {match.report_id}</small></div><b className={scoreTone(match.player.scores.overall)}>{formatPercent(match.player.scores.overall)}</b><span>{match.reanalysis.required ? `REANALYZE / ${match.reanalysis.reasons.join(", ")}` : `ENGINE / ${match.provenance.engine_version || "recorded"}`}</span></li>)}</ol><p>Источник: только локально опубликованные Sentinel reports; внешние профили и reputation signals не используются.</p></section> : null}
          </article>

          <article className="panel timeline-panel" id="evidence-rail" style={{ backgroundImage: "linear-gradient(180deg, rgba(12,28,28,.88), rgba(12,28,28,.98)), url('/manus-storage/sentinel-route-map_6c357b2d.jpg')" }}>
            <div className="panel-heading"><div><p className="eyebrow">ВРЕМЕННАЯ ШКАЛА</p><h3>Цепочка доказательств</h3></div><span className="panel-index active-index">SECTOR / 02</span></div>
            {evidence.length ? <ol className="event-list">{evidence.map((event, index) => <li key={`${event.player}-${event.tick}-${index}`}><span className="event-node" /><div><small>TICK {event.tick ?? "—"} · {event.player}</small><strong>{event.category ?? "Поведенческий сигнал"}</strong><p>{event.description ?? "Детали доступны в исходном отчёте."}</p></div><b>{formatPercent(event.confidence ?? 0)}</b></li>)}</ol> : <EmptyEvidenceRail />}
          </article>

          <article className="panel archive-panel" id="archive">
            <div className="panel-heading"><div><p className="eyebrow">АРХИВ</p><h3>Доступные отчёты</h3></div><span className="panel-index">SECTOR / 03</span></div>
            {reports.length ? <div className="report-list">{reports.map((item) => <button type="button" onClick={() => setSelectedId(item.id)} className={`report-row ${item.id === selectedId ? "selected" : ""}`} key={item.id}><span><CircleDot size={14} />{item.map}</span><small>{item.rounds} RD</small><strong>{formatPercent(item.anomaly_score)}</strong></button>)}</div> : <EmptyState icon={Database} title={connection === "offline" ? "Нет соединения с API" : "Архив пока пуст"} body={connection === "offline" ? "Запустите sentinel-api и задайте VITE_SENTINEL_API_URL для этой панели." : "Отчёты будут перечислены автоматически."} />}
          </article>

          <article className="panel field-panel" style={{ backgroundImage: "linear-gradient(90deg, rgba(13,29,28,.92), rgba(13,29,28,.62)), url('/manus-storage/sentinel-evidence-field_b3cb2976.jpg')" }}>
            <div><p className="eyebrow amber">ПРОТОКОЛ ПРОВЕРКИ</p><h3>Вердикт требует контекста.</h3><p>Sentinel показывает наблюдения и источники, но не заменяет ручную оценку матча.</p></div><div className="field-tag"><AlertTriangle size={15} />{loadingReport ? "Загрузка отчёта" : report ? "Проверить evidence" : "Ожидается источник"}</div><ArrowUpRight className="field-arrow" size={22} />
          </article>

          <article className="panel ai-summary-panel">
            <div className="panel-heading"><div><p className="eyebrow amber">OPTIONAL AI / FACTS ONLY</p><h3>Структурированная сводка</h3></div><Sparkles size={18} /></div>
            <p>Генерируется только по привязанным evidence facts. Сводка не меняет verdict и не добавляет непроверенные причины.</p>
            <button className="summary-action" type="button" disabled={!report || !summaryFacts.length || summaryMutation.isPending} onClick={requestSummary}><Sparkles size={14} />{summaryMutation.isPending ? "Генерация…" : isAuthenticated ? "Сформировать summary" : "Войти для summary"}</button>
            {summaryMutation.data ? <div className="summary-output"><strong>{summaryMutation.data.overview}</strong><ol>{summaryMutation.data.observations.map((observation) => <li key={`${observation.factId}-${observation.text}`}><span>{observation.factId}</span>{observation.text}</li>)}</ol><small>Ограничения: {summaryMutation.data.limitations}</small></div> : null}
            {summaryMutation.error ? <p className="summary-error">Summary недоступен. Исходный report и replay остаются доступны.</p> : null}
          </article>

          <article className="panel round-story-panel" id="round-story">
            <div className="panel-heading"><div><p className="eyebrow amber">ROUND STORY</p><h3>Фактическая история раундов</h3></div><span className="panel-index">SECTOR / 04</span></div>
            {rounds.length ? <div className="round-story-list">{rounds.map((round) => <section className="round-entry" key={round.round_number}><header><span>RD {String(round.round_number).padStart(2, "0")}</span><strong>{round.story?.headline ?? `Раунд ${round.round_number}`}</strong><small>{round.story?.result ?? `${round.t_score}–${round.ct_score}`}</small></header>{round.story?.deaths?.length ? <ol>{round.story.deaths.map((death) => <li key={`${round.round_number}-${death.tick}-${death.summary}`}><time>TICK {death.tick}</time><p>{death.summary}</p>{death.facts.length ? <div>{death.facts.map((fact) => <span key={fact}>{fact.replaceAll("_", " ")}</span>)}</div> : null}</li>)}</ol> : <p className="round-empty">В этом раунде нет roster-resolved deaths.</p>}</section>)}</div> : <EmptyState icon={BookOpenCheck} title="Round Story ожидает отчёт" body="Появится после анализа демо, содержащего round context и roster kill feed." />}
          </article>
        </section>
        </>}
      </section>
    </main>
  );
}

function Metric({ icon: Icon, label, value, note }: { icon: typeof Activity; label: string; value: string; note: string }) {
  return <article className="metric"><Icon size={17} /><div><span>{label}</span><strong>{value}</strong><small>{note}</small></div></article>;
}

function EmptyState({ icon: Icon, title, body }: { icon: typeof Activity; title: string; body: string }) {
  return <div className="empty-state"><Icon size={23} /><div><strong>{title}</strong><p>{body}</p><small>SOURCE / SENTINEL API · VERIFY / PENDING</small></div></div>;
}

function EmptyEvidenceRail() {
  return <div className="empty-evidence-rail"><div className="rail-node active"><i /><span>OBSERVE</span><small>ожидается источник</small></div><div className="rail-node"><i /><span>TRACE</span><small>нет событий</small></div><div className="rail-node"><i /><span>VERIFY</span><small>ручная проверка</small></div><div className="rail-source">SOURCE / SENTINEL API<br />SECTOR / 02 · CONFIDENCE / PENDING</div></div>;
}
