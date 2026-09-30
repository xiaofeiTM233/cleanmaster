import { useMemo } from "react";
import type { Rule } from "../types";

interface Props {
  rules: Rule[];
  disabled: Set<string>;
  userRulesDir: string;
  warnings: string[];
  exportReport: string[];
  onToggle: (id: string, enabled: boolean) => void;
  onToggleMany: (ids: string[], enabled: boolean) => void;
  onReload: () => void;
  onExport: () => void;
}

const RISK_LABEL: Record<string, { text: string; cls: string }> = {
  low: { text: "低风险", cls: "risk-low" },
  medium: { text: "中风险", cls: "risk-medium" },
  high: { text: "高风险", cls: "risk-high" },
};

export default function RulesPanel({
  rules,
  disabled,
  userRulesDir,
  warnings,
  exportReport,
  onToggle,
  onToggleMany,
  onReload,
  onExport,
}: Props) {
  const groups = useMemo(() => {
    const map = new Map<string, Rule[]>();
    for (const r of rules) {
      const arr = map.get(r.category);
      if (arr) arr.push(r);
      else map.set(r.category, [r]);
    }
    return [...map.entries()];
  }, [rules]);

  return (
    <section className="rules-panel">
      <h2 className="rules-title">规则管理</h2>
      <header className="rules-actions">
        <div className="rules-dir" title={userRulesDir}>
          用户规则目录：{userRulesDir}
        </div>
        <div className="rules-btns">
          <button className="btn" onClick={onExport}>
            导出内置规则
          </button>
          <button className="btn" onClick={onReload}>
            重新加载
          </button>
        </div>
      </header>
      <p className="rules-hint">
        把 .json 规则文件放入用户规则目录即可扩展清理能力，无需更新软件；与内置规则同
        id 时覆盖内置项。勾选决定扫描时包含哪些规则。
      </p>
      {warnings.length > 0 && (
        <div className="notice warn">{warnings.join("；")}</div>
      )}
      {exportReport.length > 0 && (
        <div className="notice ok">{exportReport.join("；")}</div>
      )}
      {groups.map(([category, rs]) => (
        <div className="rules-group" key={category}>
          <div className="rules-group-title">
            <span>{category}</span>
            <span className="rules-group-btns">
              <button
                className="btn btn-mini"
                onClick={() => onToggleMany(rs.map((r) => r.id), true)}
              >
                全选
              </button>
              <button
                className="btn btn-mini"
                onClick={() => onToggleMany(rs.map((r) => r.id), false)}
              >
                全不选
              </button>
            </span>
          </div>
          {rs.map((r) => {
            const risk = RISK_LABEL[r.risk] ?? { text: r.risk, cls: "risk-low" };
            return (
              <label className="rule-row" key={r.id} title={r.description}>
                <input
                  type="checkbox"
                  checked={!disabled.has(r.id)}
                  onChange={(e) => onToggle(r.id, e.target.checked)}
                />
                <span className="rule-name">{r.name}</span>
                <span className={`badge ${risk.cls}`}>{risk.text}</span>
                <span className={`badge source-${r.source}`}>
                  {r.source === "user" ? "用户" : "内置"}
                </span>
              </label>
            );
          })}
        </div>
      ))}
    </section>
  );
}
