// Catalog UI View
export function renderCatalog(items) {
  return items.map(it => `
    <div class="catalog-card">
      <div class="sku">${it.sku}</div>
      <h4>${it.name}</h4>
      <div class="price">$${it.price.toFixed(2)}</div>
      <span class="badge">${it.category}</span>
    </div>
  `).join("");
}
