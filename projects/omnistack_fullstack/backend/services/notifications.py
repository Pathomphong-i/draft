"""
Notifications & Alerting Dispatcher
"""
import time
from models import get_db

class NotificationViewSet:
    def handle_get(self, subpath, query, body, headers):
        user = query.get("user", ["default"])[0]
        with get_db() as conn:
            cur = conn.cursor()
            cur.execute("SELECT * FROM notifications WHERE target_user = ? ORDER BY id DESC", (user,))
            alerts = [dict(r) for r in cur.fetchall()]
            return {"unread_count": len([a for a in alerts if not a["read_status"]]), "alerts": alerts}, 200

    def handle_post(self, subpath, query, body, headers):
        user = body.get("user", "developer")
        title = body.get("title", "System Notification")
        msg = body.get("body", "")
        with get_db() as conn:
            cur = conn.cursor()
            cur.execute(
                "INSERT INTO notifications (target_user, title, body, created_at) VALUES (?, ?, ?, ?)",
                (user, title, msg, time.time())
            )
            conn.commit()
            return {"id": cur.lastrowid, "status": "dispatched"}, 201
