"""
Order Processing Service & ViewSet
"""
import time
import json
from models import get_db

class OrderViewSet:
    def handle_get(self, subpath, query, body, headers):
        with get_db() as conn:
            cur = conn.cursor()
            if subpath:
                order_num = subpath[0]
                cur.execute("SELECT * FROM orders WHERE order_number = ?", (order_num,))
                ord_row = cur.fetchone()
                if ord_row:
                    res = dict(ord_row)
                    res["items"] = json.loads(res["items_json"])
                    return res, 200
                return {"detail": "Order not found"}, 404
            cur.execute("SELECT * FROM orders ORDER BY id DESC LIMIT 50")
            rows = []
            for r in cur.fetchall():
                d = dict(r)
                d["items"] = json.loads(d["items_json"])
                rows.append(d)
            return {"results": rows}, 200

    def handle_post(self, subpath, query, body, headers):
        user_id = body.get("user_id", 1)
        items = body.get("items", [])
        total = sum(float(i.get("price", 0)) * int(i.get("qty", 1)) for i in items)
        order_num = f"ORD-{int(time.time()*1000)}"
        with get_db() as conn:
            cur = conn.cursor()
            cur.execute(
                "INSERT INTO orders (order_number, user_id, total_amount, status, items_json, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                (order_num, user_id, total, "processing", json.dumps(items), time.time())
            )
            order_id = cur.lastrowid
            # Also create invoice record
            cur.execute(
                "INSERT INTO invoices (invoice_id, order_id, amount, status, created_at) VALUES (?, ?, ?, ?, ?)",
                (f"INV-{order_num}", order_id, total, "unpaid", time.time())
            )
            conn.commit()
            return {"id": order_id, "order_number": order_num, "total": total, "status": "processing"}, 201
