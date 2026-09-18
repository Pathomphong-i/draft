"""
Billing & Ledger Service & ViewSet
"""
import time
from models import get_db

class BillingViewSet:
    def handle_get(self, subpath, query, body, headers):
        with get_db() as conn:
            cur = conn.cursor()
            cur.execute("SELECT * FROM invoices ORDER BY id DESC")
            invoices = [dict(r) for r in cur.fetchall()]
            total_revenue = sum(inv["amount"] for inv in invoices if inv["status"] == "paid")
            unpaid_balance = sum(inv["amount"] for inv in invoices if inv["status"] == "unpaid")
            return {
                "invoices": invoices,
                "summary": {"total_revenue": total_revenue, "unpaid_balance": unpaid_balance}
            }, 200

    def handle_post(self, subpath, query, body, headers):
        action = subpath[0] if subpath else "pay"
        if action == "pay":
            inv_id = body.get("invoice_id")
            method = body.get("payment_method", "credit_card")
            with get_db() as conn:
                cur = conn.cursor()
                cur.execute("UPDATE invoices SET status = 'paid', payment_method = ? WHERE invoice_id = ?", (method, inv_id))
                conn.commit()
                return {"invoice_id": inv_id, "status": "paid", "payment_method": method}, 200
        return {"detail": "Action not supported"}, 400
