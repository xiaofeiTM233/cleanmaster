import type { DiscoveredItem } from "../types";
import { fmtSize } from "../lib/format";

interface Props {
  items: DiscoveredItem[];
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** 清理确认对话框：按规则汇总勾选项，确认后才开始删除 */
export default function ConfirmDialog({ items, busy, onConfirm, onCancel }: Props) {
  const groups = new Map<string, DiscoveredItem[]>();
  for (const it of items) {
    const arr = groups.get(it.ruleName);
    if (arr) arr.push(it);
    else groups.set(it.ruleName, [it]);
  }
  const total = items.reduce((s, i) => s + i.size, 0);

  return (
    <div
      className="modal-overlay"
      onClick={busy ? undefined : onCancel}
      role="presentation"
    >
      <div
        className="modal-card"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <h2>确认清理</h2>
        <p className="confirm-summary">
          即将删除 <strong>{items.length}</strong> 项，预计释放{" "}
          <strong>{fmtSize(total)}</strong>。删除后无法恢复，请核对以下内容：
        </p>
        <ul className="confirm-groups">
          {[...groups.entries()].map(([ruleName, its]) => (
            <li key={ruleName}>
              <div className="confirm-rule">
                <span className="confirm-rule-name">{ruleName}</span>
                <span className="confirm-rule-meta">
                  {its.length} 项 · {fmtSize(its.reduce((s, i) => s + i.size, 0))}
                </span>
              </div>
              <ul className="confirm-paths">
                {its.map((it) => (
                  <li key={it.path} title={it.path}>
                    {it.path}
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ul>
        <div className="modal-actions">
          <button className="btn" onClick={onCancel} disabled={busy}>
            取消
          </button>
          <button className="btn btn-danger" onClick={onConfirm} disabled={busy}>
            {busy ? "清理中…" : "确认清理"}
          </button>
        </div>
      </div>
    </div>
  );
}
