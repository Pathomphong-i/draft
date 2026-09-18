"""
OmniStack Cloud Core Data Models & Store
Simulates Django ORM models with fields, serialization, and SQLite persistence.
"""
import sqlite3
import json
import time
import os

DB_PATH = os.path.join(os.path.dirname(__file__), "omnistack.db")

def get_db():
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    return conn

def init_db():
    with get_db() as conn:
        cursor = conn.cursor()
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT UNIQUE NOT NULL,
                email TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT DEFAULT 'developer',
                created_at REAL NOT NULL
            );
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS catalog_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sku TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                category TEXT NOT NULL,
                price REAL NOT NULL,
                stock INTEGER NOT NULL,
                created_at REAL NOT NULL
            );
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS orders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                order_number TEXT UNIQUE NOT NULL,
                user_id INTEGER NOT NULL,
                total_amount REAL NOT NULL,
                status TEXT DEFAULT 'pending',
                items_json TEXT NOT NULL,
                created_at REAL NOT NULL
            );
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS invoices (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                invoice_id TEXT UNIQUE NOT NULL,
                order_id INTEGER NOT NULL,
                amount REAL NOT NULL,
                status TEXT DEFAULT 'unpaid',
                payment_method TEXT,
                created_at REAL NOT NULL
            );
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS telemetry_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                timestamp REAL NOT NULL
            );
        """)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS notifications (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                target_user TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                read_status INTEGER DEFAULT 0,
                created_at REAL NOT NULL
            );
        """)
        conn.commit()

class Field:
    def __init__(self, field_type=str, required=True, default=None):
        self.field_type = field_type
        self.required = required
        self.default = default

    def validate(self, value):
        if value is None:
            if self.required:
                raise ValueError("Field is required")
            return self.default
        try:
            return self.field_type(value)
        except (ValueError, TypeError) as e:
            raise ValueError(f"Invalid type: expected {self.field_type.__name__}, got {type(value).__name__}")

class Serializer:
    """DRF-style Serializer with field validation and dict conversion"""
    fields = {}

    def __init__(self, instance=None, data=None):
        self.instance = instance
        self.data = data
        self.validated_data = {}
        self.errors = {}

    def is_valid(self):
        self.errors.clear()
        self.validated_data.clear()
        if not self.data:
            self.errors["non_field_errors"] = ["No data provided"]
            return False

        for field_name, field in self.fields.items():
            raw = self.data.get(field_name)
            try:
                self.validated_data[field_name] = field.validate(raw)
            except ValueError as err:
                self.errors[field_name] = [str(err)]

        return len(self.errors) == 0

    def serialize(self):
        if self.instance is None:
            return {}
        if isinstance(self.instance, sqlite3.Row):
            return dict(self.instance)
        if isinstance(self.instance, dict):
            return self.instance
        if hasattr(self.instance, "__dict__"):
            return {k: v for k, v in self.instance.__dict__.items() if not k.startswith("_")}
        return str(self.instance)
