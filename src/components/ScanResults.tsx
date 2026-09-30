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

/** 组内按规则（ruleId）二次分组，保持发现顺序 */
function ruleSubgroups(items: DiscoveredItem[]): [string, string, DiscoveredItem[]][] {
  const map = new Map<string, DiscoveredItem[]>();
  const names = new Map<string, string>();
  for (const it of items) {
    names.set(it.ruleId, it.ruleName);
    const arr = map.get(it.ruleId);
    if (arr) arr.push(it);
    else map.set(it.ruleId, [it]);
  }
  return [...map.entries()].map(([id, its]) => [id, names.get(id) ?? id, its]);
}

/** ref 回调：设置 checkbox 的半选状态 */
function setIndeterminate(all: boolean, some: boolean) {
  return (el: HTMLInputElement | null) => {
    if (el) el.indeterminate = !all && some;
  };
}

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
        const someChecked = items.some((i) => selected.has(i.path));
        const sizeSum = items.reduce((s, i) => s + i.size, 0);
        return (
          <section className="group-card" key={category}>
            <header className="group-header">
              <label className="group-check">
                <input
                  type="checkbox"
                  ref={setIndeterminate(allChecked, someChecked)}
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
              {ruleSubgroups(items).map(([ruleId, ruleName, ruleItems]) => {
                const rAll = ruleItems.every((i) => selected.has(i.path));
                const rSome = ruleItems.some((i) => selected.has(i.path));
                const rSize = ruleItems.reduce((s, i) => s + i.size, 0);
                return (
                  <div className="rule-block" key={ruleId}>
                    <label className="rule-subheader" title={`一键勾选/取消「${ruleName}」的全部条目`}>
                      <input
                        type="checkbox"
                        ref={setIndeterminate(rAll, rSome)}
                        checked={rAll}
                        onChange={(e) => onToggleGroup(ruleItems, e.target.checked)}
                      />
                      <span className="rule-subname">{ruleName}</span>
                      <span className="rule-submeta">
                        {ruleItems.length} 项 · {fmtSize(rSize)}
                      </span>
                    </label>
                    {ruleItems.map((it) => (
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
                );
              })}
            </div>
          </section>
        );
      })}
    </div>
  );
}
