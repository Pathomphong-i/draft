"""
Full-Stack Integration Test Suite
Verifies all 10 agent modules in the converged mainline.
"""
import unittest
import sys
import os

# Add backend to sys.path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "backend"))

from models import init_db, get_db
from services.auth import AuthViewSet
from services.catalog import CatalogViewSet
from services.orders import OrderViewSet
from services.billing import BillingViewSet
from services.analytics import AnalyticsViewSet
from services.search import SearchViewSet
from services.notifications import NotificationViewSet
from services.devtools import DevToolsViewSet

class TestOmniStackConvergedMainline(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with get_db() as conn:
            cur = conn.cursor()
            for tbl in ["users", "catalog_items", "orders", "invoices", "telemetry_events", "notifications"]:
                cur.execute(f"DROP TABLE IF EXISTS {tbl}")
            conn.commit()
        init_db()

    def test_01_auth_register_and_login(self):
        auth = AuthViewSet()
        reg_resp, code = auth.handle_post(["register"], {}, {
            "username": "alice_test",
            "email": "alice@omnistack.io",
            "password": "secret_pass_123"
        }, {})
        self.assertEqual(code, 201)
        self.assertIn("token", reg_resp)

        login_resp, lcode = auth.handle_post(["login"], {}, {
            "username": "alice_test",
            "password": "secret_pass_123"
        }, {})
        self.assertEqual(lcode, 200)
        self.assertIn("token", login_resp)

    def test_02_catalog_items(self):
        cat = CatalogViewSet()
        c_resp, code = cat.handle_post([], {}, {
            "sku": "SKU-TEST-01",
            "name": "Cloud Hypervisor Node",
            "category": "compute",
            "price": 199.99
        }, {})
        self.assertEqual(code, 201)

        list_resp, lcode = cat.handle_get([], {"category": ["compute"]}, {}, {})
        self.assertEqual(lcode, 200)
        self.assertGreaterEqual(len(list_resp["results"]), 1)

    def test_03_orders_checkout(self):
        ord_vs = OrderViewSet()
        resp, code = ord_vs.handle_post([], {}, {
            "user_id": 1,
            "items": [{"sku": "SKU-TEST-01", "price": 199.99, "qty": 2}]
        }, {})
        self.assertEqual(code, 201)
        self.assertEqual(resp["status"], "processing")
        self.assertEqual(resp["total"], 399.98)

    def test_04_billing_invoices(self):
        bill = BillingViewSet()
        resp, code = bill.handle_get([], {}, {}, {})
        self.assertEqual(code, 200)
        self.assertIn("invoices", resp)

    def test_05_analytics_metrics(self):
        ana = AnalyticsViewSet()
        resp, code = ana.handle_get([], {}, {}, {})
        self.assertEqual(code, 200)
        self.assertEqual(resp["system_metrics"]["active_agents"], 10)

    def test_06_search_inverted(self):
        search = SearchViewSet()
        resp, code = search.handle_get([], {"q": ["hypervisor"]}, {}, {})
        self.assertEqual(code, 200)
        self.assertGreaterEqual(resp["count"], 1)

    def test_07_notifications(self):
        notif = NotificationViewSet()
        post_resp, pcode = notif.handle_post([], {}, {
            "user": "alice_test",
            "title": "Welcome to OmniStack",
            "body": "Your cluster is initialized."
        }, {})
        self.assertEqual(pcode, 201)

    def test_08_devtools_health(self):
        dev = DevToolsViewSet()
        resp, code = dev.handle_get([], {}, {}, {})
        self.assertEqual(code, 200)
        self.assertEqual(resp["health"], "OK")

if __name__ == "__main__":
    unittest.main()
