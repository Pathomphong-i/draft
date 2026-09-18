"""
Product Catalog Service & ViewSet
"""
import time
from models import get_db, Serializer, Field

class CatalogItemSerializer(Serializer):
    fields = {
        "sku": Field(str, required=True),
        "name": Field(str, required=True),
        "category": Field(str, required=True),
        "price": Field(float, required=True),
        "stock": Field(int, required=False, default=100)
    }

class CatalogViewSet:
    def handle_get(self, subpath, query, body, headers):
        category = query.get("category", [None])[0]
        with get_db() as conn:
            cur = conn.cursor()
            if subpath:
                sku = subpath[0]
                cur.execute("SELECT * FROM catalog_items WHERE sku = ?", (sku,))
                item = cur.fetchone()
                if item: return dict(item), 200
                return {"detail": "Product not found"}, 404
            
            if category:
                cur.execute("SELECT * FROM catalog_items WHERE category = ?", (category,))
            else:
                cur.execute("SELECT * FROM catalog_items")
            return {"results": [dict(r) for r in cur.fetchall()]}, 200

    def handle_post(self, subpath, query, body, headers):
        serializer = CatalogItemSerializer(data=body)
        if not serializer.is_valid():
            return {"errors": serializer.errors}, 400
        d = serializer.validated_data
        with get_db() as conn:
            cur = conn.cursor()
            cur.execute(
                "INSERT INTO catalog_items (sku, name, category, price, stock, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                (d["sku"], d["name"], d["category"], d["price"], d["stock"], time.time())
            )
            conn.commit()
            return {"id": cur.lastrowid, **d}, 201
