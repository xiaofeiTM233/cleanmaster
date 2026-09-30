import { useEffect, useMemo, useState } from "react";
import "./App.css";
import { api } from "./lib/api";
import { fmtSize } from "./lib/format";
import RulesPanel from "./components/RulesPanel";
import ScanResults from "./components/ScanResults";
import type { CleanResult, DiscoveredItem, RulesPayload } from "./types";

type Busy = null | "scan" | "clean" | "blockers" | "autoruns";

const ACTION_LABEL: Record<string, string> = {
  deleted: "已删除",
  cleaned_contents: "已清空",
  skipped_occupied: "占用跳过",
  refused: "已拒绝",
  not_found: "不存在",
  failed: "失败",
};

/** 默认勾选：规则默认启用 且 非「疑似」目录 */
function defaultSelection(
  items: DiscoveredItem[],
  rules: { id: string; defaultEnabled: boolean }[],
): Set<string> {
  const ruleMap = new Map(rules.map((r) => [r.id, r]));
  const sel = new Set<string>();
  for (const it of items) {
    const rule = ruleMap.get(it.ruleId);
    if ((rule ? rule.defaultEnabled : true) && it.status !== "suspected") {
      sel.add(it.path);
    }
  }
  return sel;
}

function App() {
  const [payload, setPayload] = useState<RulesPayload | null>(null);
  const [disabled, setDisabled] = useState<Set<string>>(new Set());
  const [items, setItems] = useState<DiscoveredItem[]>([]);
  const [totalSize, setTotalSize] = useState(0);
  const [scanWarnings, setScanWarnings] = useState<string[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [occupied, setOccupied] = useState<Record<string, string[]>>({});
  const [autorunRefs, setAutorunRefs] = useState<string[]>([]);
  const [cleanResult, setCleanResult] = useState<CleanResult | null>(null);
  const [exportReport, setExportReport] = useState<string[]>([]);
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState("");
  const [showRules, setShowRules] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const [p, s] = await Promise.all([api.rulesList(), api.settingsGet()]);
        let d = new Set(s.disabledRules);
        if (d.size === 0) {
          // 首次运行：以规则自带的 defaultEnabled 生成初始勾选并持久化
          d = new Set(p.rules.filter((r) => !r.defaultEnabled).map((r) => r.id));
          await api.settingsSet({ disabledRules: [...d] });
        } else {
          // 规则集变化时同步清理失效的 id
          const known = new Set(p.rules.map((r) => r.id));
          const filtered = [...d].filter((id) => known.has(id));
          if (filtered.length !== d.size) {
            d = new Set(filtered);
            await api.settingsSet({ disabledRules: filtered });
          }
        }
        setPayload(p);
        setDisabled(d);
      } catch (e) {
        setError(String(e));
      }
    })();
  }, []);

  const groups = useMemo(() => {
    const map = new Map<string, DiscoveredItem[]>();
    for (const it of items) {
      const arr = map.get(it.category);
      if (arr) arr.push(it);
      else map.set(it.category, [it]);
    }
    return [...map.entries()];
  }, [items]);

  const selectedSize = useMemo(
    () =>
      items
        .filter((i) => selected.has(i.path))
        .reduce((sum, i) => sum + i.size, 0),
    [items, selected],
  );

  function applyScan(res: {
    items: DiscoveredItem[];
    totalSize: number;
    warnings: string[];
  }) {
    setItems(res.items);
    setTotalSize(res.totalSize);
    setScanWarnings(res.warnings);
    setSelected(defaultSelection(res.items, payload?.rules ?? []));
  }

  async function handleScan() {
    setBusy("scan");
    setError("");
    setOccupied({});
    setAutorunRefs([]);
    setCleanResult(null);
    try {
      applyScan(await api.scan());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  async function handleClean() {
    if (selected.size === 0) return;
    setBusy("clean");
    setError("");
    setOccupied({});
    try {
      setCleanResult(await api.clean([...selected]));
      applyScan(await api.scan());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  async function handleCheckBlockers() {
    if (selected.size === 0) return;
    setBusy("blockers");
    setError("");
    try {
      const res = await api.checkBlockers([...selected]);
      const map: Record<string, string[]> = {};
      for (const b of res) map[b.path] = b.processes;
      setOccupied(map);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  async function handleCheckAutoruns() {
    if (selected.size === 0) return;
    setBusy("autoruns");
    setError("");
    try {
      setAutorunRefs(await api.checkAutoruns([...selected]));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  }

  async function handleToggleRule(id: string, enabled: boolean) {
    const next = new Set(disabled);
    if (enabled) next.delete(id);
    else next.add(id);
    setDisabled(next);
    try {
      await api.settingsSet({ disabledRules: [...next] });
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleReloadRules() {
    setError("");
    try {
      setPayload(await api.rulesList());
      setExportReport([]);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleExportRules() {
    setError("");
    try {
      const [report, p] = await Promise.all([api.exportBuiltinRules(), api.rulesList()]);
      setExportReport(report);
      setPayload(p);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <main className="container">
      <header className="app-header">
        <div>
          <h1>CleanMaster</h1>
          <p className="subtitle">
            插件化清理规则引擎 · 临时文件 / 包管理器缓存 / 开发工具缓存 / 应用更新缓存
          </p>
        </div>
        <button
          className="btn"
          onClick={() => setShowRules((v) => !v)}
          aria-expanded={showRules}
        >
          {showRules ? "收起规则管理" : "规则管理"}
        </button>
      </header>

      {showRules && payload && (
        <RulesPanel
          rules={payload.rules}
          disabled={disabled}
          userRulesDir={payload.userRulesDir}
          warnings={payload.warnings}
          exportReport={exportReport}
          onToggle={handleToggleRule}
          onReload={handleReloadRules}
          onExport={handleExportRules}
        />
      )}

      <div className="toolbar">
        <button className="btn btn-primary" onClick={handleScan} disabled={busy !== null}>
          {busy === "scan" ? "扫描中…" : "扫描"}
        </button>
        <button
          className="btn btn-danger"
          onClick={handleClean}
          disabled={busy !== null || selected.size === 0}
        >
          {busy === "clean" ? "清理中…" : "清理所勾选"}
        </button>
        <button
          className="btn"
          onClick={handleCheckBlockers}
          disabled={busy !== null || selected.size === 0}
        >
          {busy === "blockers" ? "检测中…" : "检测占用"}
        </button>
        <button
          className="btn"
          onClick={handleCheckAutoruns}
          disabled={busy !== null || selected.size === 0}
        >
          {busy === "autoruns" ? "检测中…" : "自启动引用"}
        </button>
        <span className="stats">
          {items.length > 0
            ? `共 ${items.length} 项，合计 ${fmtSize(totalSize)}；已勾选 ${selected.size} 项（约 ${fmtSize(selectedSize)}）`
            : "尚未扫描"}
        </span>
      </div>

      {error && <div className="notice warn">{error}</div>}

      {scanWarnings.length > 0 && (
        <div className="notice info">{scanWarnings.join("；")}</div>
      )}

      {autorunRefs.length > 0 && (
        <div className="notice info">
          以下启动项仍引用勾选的目录（删除后相关软件的更新功能会失效/自动重建）：
          <ul className="autorun-list">
            {autorunRefs.map((r) => (
              <li key={r}>{r}</li>
            ))}
          </ul>
        </div>
      )}

      <ScanResults
        groups={groups}
        selected={selected}
        occupied={occupied}
        onTogglePath={(path) => {
          const next = new Set(selected);
          if (next.has(path)) next.delete(path);
          else next.add(path);
          setSelected(next);
        }}
        onToggleGroup={(groupItems, checked) => {
          const next = new Set(selected);
          for (const it of groupItems) {
            if (checked) next.add(it.path);
            else next.delete(it.path);
          }
          setSelected(next);
        }}
      />

      {cleanResult && (
        <section className="log-panel">
          <h2>
            清理结果：释放 {fmtSize(cleanResult.freed)}，成功 {cleanResult.deleted}{" "}
            项，失败/跳过 {cleanResult.failed} 项
          </h2>
          <ul className="log-list">
            {cleanResult.results.map((r) => (
              <li key={r.path} className={r.ok ? "ok" : "bad"}>
                <span className={`badge action-badge`}>
                  {ACTION_LABEL[r.action] ?? r.action}
                </span>
                <span className="log-path">{r.path}</span>
                <span className="log-msg">{r.message}</span>
              </li>
            ))}
          </ul>
        </section>
      )}

      <footer className="app-footer">
        安全说明：删除前会重新校验路径；受保护目录（用户目录、系统根目录、盘符根目录等）一律拒绝；
        被占用的文件自动跳过。
      </footer>
    </main>
  );
}

export default App;
