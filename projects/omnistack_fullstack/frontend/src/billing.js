// Billing Ledger UI
export function renderInvoices(invs) {
  return invs.map(i => `<div class="invoice-item">${i.invoice_id}: $${i.amount} [${i.status}]</div>`).join("");
}
