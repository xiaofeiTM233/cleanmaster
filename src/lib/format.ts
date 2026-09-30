/** 字节数格式化（对齐原 Python 脚本的 fmt_size） */
export function fmtSize(n: number): string {
  let v = n;
  for (const unit of ["B", "KB", "MB", "GB"]) {
    if (v < 1024 || unit === "GB") return `${v.toFixed(1)} ${unit}`;
    v /= 1024;
  }
  return `${v.toFixed(1)} GB`;
}
