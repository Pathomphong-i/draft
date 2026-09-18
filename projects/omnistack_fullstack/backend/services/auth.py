"""
Auth Service & ViewSet (DRF Architecture)
"""
import hashlib
import time
from models import get_db, Serializer, Field

class UserSerializer(Serializer):
    fields = {
        "username": Field(str, required=True),
        "email": Field(str, required=True),
        "password": Field(str, required=True),
        "role": Field(str, required=False, default="developer")
    }

class AuthViewSet:
    def handle_post(self, subpath, query, body, headers):
        action = subpath[0] if subpath else "login"
        if action == "register":
            serializer = UserSerializer(data=body)
            if not serializer.is_valid():
                return {"errors": serializer.errors}, 400
            data = serializer.validated_data
            pwd_hash = hashlib.sha256(data["password"].encode()).hexdigest()
            with get_db() as conn:
                try:
                    cur = conn.cursor()
                    cur.execute(
                        "INSERT INTO users (username, email, password_hash, role, created_at) VALUES (?, ?, ?, ?, ?)",
                        (data["username"], data["email"], pwd_hash, data.get("role", "developer"), time.time())
                    )
                    user_id = cur.lastrowid
                    conn.commit()
                except Exception as e:
                    return {"detail": f"Registration error: {e}"}, 409
            token = f"jwt_{user_id}_{int(time.time())}"
            return {"token": token, "user": {"id": user_id, "username": data["username"], "role": data["role"]}}, 201

        elif action == "login":
            username = body.get("username")
            password = body.get("password", "")
            pwd_hash = hashlib.sha256(password.encode()).hexdigest()
            with get_db() as conn:
                cur = conn.cursor()
                cur.execute("SELECT id, username, role FROM users WHERE username = ? AND password_hash = ?", (username, pwd_hash))
                user = cur.fetchone()
                if not user:
                    return {"detail": "Invalid username or credentials"}, 401
                token = f"jwt_{user['id']}_{int(time.time())}"
                return {"token": token, "user": dict(user)}, 200

        return {"detail": "Unknown auth action"}, 404

    def handle_get(self, subpath, query, body, headers):
        action = subpath[0] if subpath else "me"
        if action == "users":
            with get_db() as conn:
                cur = conn.cursor()
                cur.execute("SELECT id, username, email, role, created_at FROM users")
                return {"results": [dict(r) for r in cur.fetchall()]}, 200
        return {"user": "current_session_user", "authenticated": True}, 200
