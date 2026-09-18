// Orders UI Component
export function renderOrdersTable(orders) {
  return orders.map(o => `
    <tr>
      <td>${o.order_number}</td>
      <td>$${o.total_amount.toFixed(2)}</td>
      <td><span class="status-pill ${o.status}">${o.status}</span></td>
    </tr>
  `).join("");
}
