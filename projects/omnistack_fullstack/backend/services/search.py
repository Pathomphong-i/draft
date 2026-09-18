"""
Search Engine Service & ViewSet
"""
from models import get_db

class SearchViewSet:
    def handle_get(self, subpath, query, body, headers):
        q = query.get("q", [""])[0].lower()
        if not q:
            return {"results": []}, 200

        with get_db() as conn:
            cur = conn.cursor()
            # Search catalog items
            cur.execute("SELECT 'product' as type, sku as id, name as title FROM catalog_items WHERE LOWER(name) LIKE ? OR LOWER(category) LIKE ?", (f"%{q}%", f"%{q}%"))
            prod_matches = [dict(r) for r in cur.fetchall()]

            # Search users
            cur.execute("SELECT 'user' as type, username as id, email as title FROM users WHERE LOWER(username) LIKE ? OR LOWER(email) LIKE ?", (f"%{q}%", f"%{q}%"))
            user_matches = [dict(r) for r in cur.fetchall()]

            return {
                "query": q,
                "count": len(prod_matches) + len(user_matches),
                "results": prod_matches + user_matches
            }, 200
