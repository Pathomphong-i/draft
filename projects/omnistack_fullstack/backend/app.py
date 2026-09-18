"""
OmniStack Cloud Core Backend API Framework
Modeled after Django REST Framework (DRF) patterns: ViewSets, Routers, Request/Response, Status Codes.
"""
from http.server import HTTPServer, BaseHTTPRequestHandler
import urllib.parse
import json
import os
import sys

from models import init_db, get_db

try:
    from services.auth import AuthViewSet
except ImportError:
    AuthViewSet = None

try:
    from services.catalog import CatalogViewSet
except ImportError:
    CatalogViewSet = None

try:
    from services.orders import OrderViewSet
except ImportError:
    OrderViewSet = None

try:
    from services.billing import BillingViewSet
except ImportError:
    BillingViewSet = None

try:
    from services.analytics import AnalyticsViewSet
except ImportError:
    AnalyticsViewSet = None

try:
    from services.search import SearchViewSet
except ImportError:
    SearchViewSet = None

try:
    from services.notifications import NotificationViewSet
except ImportError:
    NotificationViewSet = None

try:
    from services.devtools import DevToolsViewSet
except ImportError:
    DevToolsViewSet = None


class Response:
    def __init__(self, data=None, status=200, headers=None):
        self.data = data
        self.status = status
        self.headers = headers or {"Content-Type": "application/json"}

class DefaultRouter:
    """DRF-style SimpleRouter / DefaultRouter mapping URL prefixes to ViewSets"""
    def __init__(self):
        self.registry = {}

    def register(self, prefix, viewset_cls, basename=None):
        self.registry[prefix.strip("/")] = viewset_cls

router = DefaultRouter()

if AuthViewSet: router.register("auth", AuthViewSet)
if CatalogViewSet: router.register("catalog", CatalogViewSet)
if OrderViewSet: router.register("orders", OrderViewSet)
if BillingViewSet: router.register("billing", BillingViewSet)
if AnalyticsViewSet: router.register("analytics", AnalyticsViewSet)
if SearchViewSet: router.register("search", SearchViewSet)
if NotificationViewSet: router.register("notifications", NotificationViewSet)
if DevToolsViewSet: router.register("devtools", DevToolsViewSet)


class OmniStackHandler(BaseHTTPRequestHandler):
    def _send_response_obj(self, resp: Response):
        self.send_response(resp.status)
        for k, v in resp.headers.items():
            self.send_header(k, v)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, Authorization")
        self.end_headers()
        if resp.data is not None:
            payload = json.dumps(resp.data, indent=2).encode("utf-8")
            self.wfile.write(payload)

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, Authorization")
        self.end_headers()

    def _parse_request(self):
        parsed = urllib.parse.urlparse(self.path)
        query = urllib.parse.parse_qs(parsed.query)
        body = {}
        content_len = int(self.headers.get("Content-Length", 0))
        if content_len > 0:
            raw_body = self.rfile.read(content_len).decode("utf-8")
            try:
                body = json.loads(raw_body)
            except Exception:
                body = {"raw": raw_body}
        return parsed.path, query, body

    def _dispatch(self, method):
        path, query, body = self._parse_request()
        if path == "/" or path == "/health":
            return self._send_response_obj(Response({
                "service": "OmniStack Cloud API",
                "version": "1.0.0",
                "status": "healthy",
                "endpoints": list(router.registry.keys())
            }))

        parts = [p for p in path.strip("/").split("/") if p]
        if len(parts) >= 2 and parts[0] == "api":
            prefix = parts[1]
            subpath = parts[2:] if len(parts) > 2 else []
            if prefix in router.registry:
                viewset_cls = router.registry[prefix]
                viewset = viewset_cls()
                handler_name = getattr(viewset, f"handle_{method.lower()}", None)
                if handler_name:
                    resp = handler_name(subpath, query, body, self.headers)
                    if isinstance(resp, tuple):
                        data = resp[0]
                        code = resp[1] if len(resp) > 1 else 200
                        resp = Response(data=data, status=code)
                    return self._send_response_obj(resp)
                else:
                    return self._send_response_obj(Response({"detail": f"Method {method} not allowed"}, 405))

        self._send_response_obj(Response({"detail": f"Not found: {path}"}, 404))

    def do_GET(self): self._dispatch("GET")
    def do_POST(self): self._dispatch("POST")
    def do_PUT(self): self._dispatch("PUT")
    def do_DELETE(self): self._dispatch("DELETE")

def run_server(port=8000):
    init_db()
    server_address = ("", port)
    httpd = HTTPServer(server_address, OmniStackHandler)
    print(f"[*] OmniStack Cloud REST API running at http://localhost:{port}/")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        httpd.server_close()

if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
    run_server(port)
