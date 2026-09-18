// Reusable UI Component Primitives
export function createBadge(text, type = "cyan") {
  return `<span class="badge badge-${type}">${text}</span>`;
}

export function createMetricCard(label, val, trend = "") {
  return `
    <div class="stat-card">
      <div class="stat-label">${label}</div>
      <div class="stat-value">${val}</div>
      ${trend ? `<div class="stat-trend">${trend}</div>` : ""}
    </div>
  `;
}
