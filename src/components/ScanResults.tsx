import type { DiscoveredItem } from "../types";
import { fmtSize } from "../lib/format";

interface Props {
  groups: [string, DiscoveredItem[]][];
  selected: Set<string>;
  occupied: Record<string, string[]>;
  onTogglePath: (path: string) => void;
  onToggleGroup: (items: DiscoveredItem[], checked: boolean) => void;
}

const STATUS_LABEL: Record<string, string> = {
  normal: "普通",
  confirmed: "确认",
  suspected: "疑似",
};

export default function ScanResults({
  groups,
  selected,
  occupied,
  onTogglePath,
  onToggleGroup,
}: Props) {
  if (groups.length === 0) {
    return <div className="empty">点击「扫描」开始检查可清理项</div>;
  }
  return (
    <div className="groups">
      {groups.map(([category, items]) => {
        const allChecked = items.every((i) => selected.has(i.path));
        const sizeSum = items.reduce((s, i) => s + i.size, 0);
        return (
          <section className="group-card" key={category}>
            <header className="group-header">
              <label className="group-check">
                <input
                  type="checkbox"
                  checked={allChecked}
                  onChange={(e) => onToggleGroup(items, e.target.checked)}
                />
                <strong>{category}</strong>
              </label>
              <span className="group-meta">
                {items.length} 项 · {fmtSize(sizeSum)}
              </span>
            </header>
            <div className="group-items">
              {items.map((it) => (
                <label className="item-row" key={it.path}>
                  <input
                    type="checkbox"
                    checked={selected.has(it.path)}
                    onChange={() => onTogglePath(it.path)}
                  />
                  <span className={`badge status-${it.status}`}>
                    {STATUS_LABEL[it.status] ?? it.status}
                  </span>
                  <span className="item-main">
                    <span className="item-name">
                      {it.ruleName}
                      {it.source === "user" ? "（用户规则）" : ""}
                    </span>
                    <span className="item-path" title={it.path}>
                      {it.path}
                    </span>
                    {it.signatures.length > 0 && (
                      <span className="item-sig">
                        特征：{it.signatures.join(", ")}
                      </span>
                    )}
                    {(occupied[it.path]?.length ?? 0) > 0 && (
                      <span className="item-warn">
                        ⚠ 占用：{(occupied[it.path] ?? []).join(", ")}
                      </span>
                    )}
                  </span>
                  <span className="item-size">{fmtSize(it.size)}</span>
                </label>
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}
