#!/usr/bin/env python3
"""Read-only probe of an iTop server for iTop Wrap (PLAN.md, Task 0).

What it does:
  - Asks for the server URL, username and password (the password is typed
    at a hidden prompt and never written anywhere).
  - Sends only read operations: list_operations, core/check_credentials,
    core/get. Anything else is refused before a request is made.
  - Writes redacted copies to Tests/ITopKitTests/Fixtures/: every free-text
    value is replaced, ids are renumbered, and HTML keeps its tags only.
  - Writes schema.json: attribute codes and value types per class, so the
    app can be built against your real data model.

Raw responses are kept in memory only and are never saved.

Usage:
  python3 scripts/probe-itop.py            # probe your server
  python3 scripts/probe-itop.py --selftest # redaction self-test, no network
"""
import getpass
import json
import os
import re
import ssl
import sys
import urllib.parse
import urllib.request

READ_ONLY_OPS = {"list_operations", "core/check_credentials", "core/get"}
REST_VERSION = "1.4"
OUT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "Tests", "ITopKitTests", "Fixtures")

# Values kept as-is: short enums and codes that are not personal data.
KEEP_KEYS = {
    "status", "operational_status", "priority", "impact", "urgency", "finalclass",
    "request_type", "origin", "resolution_code", "escalation_flag", "tto_escalation_deadline",
    "ttr_escalation_deadline", "sla_tto_passed", "sla_ttr_passed", "class", "code", "message_type",
    "version", "verb", "business_criticality", "type", "obj_class",
}
# Digits-only values that are still personal or identifying (phone 0612345678, employee numbers, serials).
PERSONAL_NUMBER_KEYS = ("phone", "mobile", "fax", "employee_number", "serial", "asset_number", "iban", "badge")
# Linked lists that can hold hundreds of rows; a fixture only needs a few.
LIST_CAP = 3
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}( \d{2}:\d{2}:\d{2})?$")
NUM_RE = re.compile(r"^-?\d+(\.\d+)?$")
TAG_RE = re.compile(r"(<[^>]+>)")


class Redactor:
    def __init__(self):
        self.ids = {}

    def fake_id(self, real):
        real = str(real)
        if real in ("0", ""):
            return real
        if real not in self.ids:
            self.ids[real] = str(1000 + len(self.ids) + 1)
        return self.ids[real]

    def text(self, value):
        if "<" in value and ">" in value:
            parts = TAG_RE.split(value)
            out = []
            for p in parts:
                if TAG_RE.fullmatch(p or ""):
                    # keep the tag name only, drop attributes (links, image sources, styles)
                    m = re.match(r"<\s*(/?)\s*([a-zA-Z0-9]+)", p)
                    out.append("<%s%s>" % (m.group(1), m.group(2).lower()) if m else "")
                elif p.strip():
                    out.append("[text]")
                else:
                    out.append(p)
            return "".join(out)
        return "[text]" if value.strip() else value

    def value(self, key, v):
        if isinstance(v, dict):
            return {k: self.value(k, x) for k, x in v.items()}
        if isinstance(v, list):
            return [self.value(key, x) for x in v[:LIST_CAP]]
        if not isinstance(v, str):
            return v
        k = (key or "").lower()
        if any(s in k for s in PERSONAL_NUMBER_KEYS):
            return "[number]" if v.strip() else v
        if k == "id" or k.endswith("_id") or k in ("key", "contactid"):
            return self.fake_id(v) if NUM_RE.match(v) else v
        if k in KEEP_KEYS or DATE_RE.match(v) or NUM_RE.match(v):
            return v
        return self.text(v)

    def response(self, resp):
        out = self.value(None, resp)
        objs = out.get("objects") if isinstance(out, dict) else None
        if isinstance(objs, dict):
            renamed = {}
            for k, o in objs.items():
                cls, _, num = k.partition("::")
                renamed["%s::%s" % (cls, self.fake_id(num))] = o
            out["objects"] = renamed
        return out


def schema_of(resp):
    """Attribute codes and value shapes per class, with enum values kept."""
    result = {}
    for o in (resp.get("objects") or {}).values():
        cls = o.get("class", "?")
        fields = result.setdefault(cls, {})
        for k, v in (o.get("fields") or {}).items():
            if isinstance(v, list):
                inner = sorted({kk for x in v if isinstance(x, dict) for kk in x})
                shape = "list of {%s}" % ", ".join(inner) if inner else "list"
            elif isinstance(v, dict):
                shape = "object {%s}" % ", ".join(sorted(v))
            elif isinstance(v, str) and DATE_RE.match(v):
                shape = "date"
            elif isinstance(v, str) and NUM_RE.match(v):
                shape = "number (string)"
            else:
                shape = type(v).__name__
            if k in KEEP_KEYS and isinstance(v, str) and v:
                shape += " = %s" % v
            fields[k] = shape
    return result


class Client:
    def __init__(self, base, user, pwd):
        self.url = base.rstrip("/") + "/webservices/rest.php?version=" + REST_VERSION
        self.user, self.pwd = user, pwd
        self.ctx = ssl.create_default_context()  # TLS is always verified

    def call(self, payload):
        if payload.get("operation") not in READ_ONLY_OPS:
            raise SystemExit("Refused: %r is not a read operation." % payload.get("operation"))
        body = urllib.parse.urlencode({
            "auth_user": self.user, "auth_pwd": self.pwd, "json_data": json.dumps(payload),
        }).encode()
        req = urllib.request.Request(self.url, data=body, method="POST",
                                     headers={"Content-Type": "application/x-www-form-urlencoded"})
        with urllib.request.urlopen(req, context=self.ctx, timeout=30) as r:
            data = json.loads(r.read().decode("utf-8"))
        if data.get("code", 0) != 0:
            print("  iTop said: code %s, %s" % (data.get("code"), data.get("message")), file=sys.stderr)
        return data

    def get(self, cls, oql, fields="*", limit=1):
        return self.call({"operation": "core/get", "class": cls, "key": oql,
                          "output_fields": fields, "limit": str(limit)})


def first(resp, n=1):
    objs = resp.get("objects") or {}
    keys = list(objs)[:n]
    trimmed = dict(resp)
    trimmed["objects"] = {k: objs[k] for k in keys} if objs else objs
    return trimmed


def save(name, data):
    os.makedirs(OUT_DIR, exist_ok=True)
    path = os.path.join(OUT_DIR, name)
    with open(path, "w") as f:
        json.dump(data, f, indent=2, ensure_ascii=False, sort_keys=True)
        f.write("\n")
    print("  saved %s" % os.path.relpath(path))


def oql_str(s):
    return "'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'"


def probe():
    print("iTop Wrap read-only probe. Nothing is written to iTop.\n", file=sys.stderr)
    base = input("Server URL (https://...): ").strip()
    if not base.lower().startswith("https://"):
        if input("This isn't https, so your password is sent unencrypted. Continue? [y/N] ").strip().lower() != "y":
            return 1
    user = input("Username: ").strip()
    pwd = getpass.getpass("Password (hidden): ")
    c = Client(base, user, pwd)
    red = Redactor()
    schema = {}
    notes = []
    ur_key = "0"

    def step(label, fn):
        print("- %s" % label, file=sys.stderr)
        try:
            return fn()
        except urllib.error.URLError as e:
            raise SystemExit("  Can't reach the server: %s\n  (If it's a self-signed certificate, trust its CA in Keychain Access. TLS is never turned off.)" % e)

    ops = step("list_operations", lambda: c.call({"operation": "list_operations"}))
    save("list_operations.json", {"code": ops.get("code"), "version": REST_VERSION,
                                  "operations": [o.get("verb") for o in ops.get("operations", [])]})  # verbs only, not redacted or capped
    cred = step("core/check_credentials", lambda: c.call({"operation": "core/check_credentials", "user": user, "password": pwd}))
    save("check_credentials.json", {"code": cred.get("code"), "authorized": cred.get("authorized")})
    if not cred.get("authorized"):
        print("\nNot authorized for REST. Ask your admin for the REST Services User profile.", file=sys.stderr)
        return 1

    me = step("your user record", lambda: c.get("User", "SELECT User WHERE login = %s" % oql_str(user), "*", 1))
    me_obj = next(iter((me.get("objects") or {}).values()), None)
    contact = (me_obj or {}).get("fields", {}).get("contactid", "0")
    user_key = (me_obj or {}).get("key", "0")
    profiles = [p.get("profile") for p in (me_obj or {}).get("fields", {}).get("profile_list", []) if isinstance(p, dict)]
    schema.update(schema_of(me))
    save("user.json", red.response(me))

    person = step("your person and teams", lambda: c.get("Person", "SELECT Person WHERE id = %s" % int(contact or 0), "*", 1))
    schema.update(schema_of(person))
    save("person.json", red.response(person))
    pf = next(iter((person.get("objects") or {}).values()), {}).get("fields", {})
    teams = pf.get("team_list", [])
    notes.append("teams readable: %s (%d)" % ("yes" if isinstance(teams, list) else "no", len(teams) if isinstance(teams, list) else 0))

    for cls in ("UserRequest", "Incident"):
        t = step("one open %s assigned to you" % cls, lambda cls=cls: c.get(
            cls, "SELECT %s WHERE agent_id = %d AND status NOT IN ('resolved','closed')" % (cls, int(contact or 0)), "*", 1))
        if not t.get("objects"):
            t = step("  none, any open %s you can see" % cls, lambda cls=cls: c.get(
                cls, "SELECT %s WHERE status NOT IN ('resolved','closed')" % cls, "*", 1))
        t = first(t)
        if cls == "UserRequest" and t.get("objects"):
            ur_key = next(iter(t["objects"].values())).get("key", "0")
        schema.update(schema_of(t))
        save("%s.json" % cls.lower(), red.response(t))
        if not t.get("objects"):
            notes.append("no %s visible" % cls)

    ts = step("one of your TimeSpent records", lambda: c.get("TimeSpent", "SELECT TimeSpent WHERE user_id = %d" % int(user_key or 0), "*", 1))
    if ts.get("code", 0) != 0:
        notes.append("TimeSpent not readable: %s" % ts.get("message"))
    ts = first(ts)
    schema.update(schema_of(ts))
    save("timespent.json", red.response(ts))

    # TimeSpent points at an Activity (activity_id), and the Activity points at the ticket.
    ts_obj = next(iter((ts.get("objects") or {}).values()), {})
    act_id = int(ts_obj.get("fields", {}).get("activity_id") or 0)
    act = step("the Activity behind that TimeSpent", lambda: c.get("Activity", "SELECT Activity WHERE id = %d" % act_id, "*", 1))
    if act.get("code", 0) != 0:
        notes.append("Activity not readable: %s" % act.get("message"))
    act = first(act)
    schema.update(schema_of(act))
    save("activity.json", red.response(act))
    act_ur = step("the Activity of the open UserRequest, if it has one", lambda: c.get(
        "Activity", "SELECT Activity WHERE obj_class = 'UserRequest' AND obj_id = %d" % int(ur_key or 0), "id", 5))
    n = len(act_ur.get("objects") or {})
    notes.append("Activities found for the open UserRequest: %d%s" % (n, "" if act_ur.get("code", 0) == 0 else " (query failed: %s)" % act_ur.get("message")))

    ci = step("one CI", lambda: c.get("FunctionalCI", "SELECT FunctionalCI", "*", 1))
    ci = first(ci)
    schema.update(schema_of(ci))
    save("ci.json", red.response(ci))

    summary = {"rest_version": REST_VERSION, "operations": len(ops.get("operations", [])),
               "profiles": profiles, "notes": notes, "classes": schema}
    save("schema.json", summary)
    print("\nDone. Only read operations were sent. Check the files above before sharing them.", file=sys.stderr)
    return 0


def selftest():
    r = Redactor()
    sample = {"code": 0, "objects": {"UserRequest::4521": {"class": "UserRequest", "key": "4521", "fields": {
        "title": "Jane Doe can't reach vpn.acme.com", "status": "assigned", "agent_id": "77", "phone": "0612345678",
        "caller_id_friendlyname": "Jane Doe", "start_date": "2026-11-10 09:12:00", "priority": "2",
        "public_log": {"entries": [{"date": "2026-11-10 09:12:00", "user_login": "jdoe", "user_id": "12",
                                    "message": "Hi", "message_html": '<p>Call me on <a href="tel:+3312345">+33 12345</a></p>'}]},
        "contacts_list": [{"contact_id": "77", "contact_id_friendlyname": "John Smith"}]}}}}
    out = r.response(sample)
    flat = json.dumps(out)
    for leak in ("Jane", "acme", "jdoe", "John", "12345", "4521", "tel:", "0612345678"):
        assert leak not in flat, "leaked %s: %s" % (leak, flat)
    o = out["objects"]["UserRequest::1001"]["fields"]
    assert o["status"] == "assigned" and o["priority"] == "2" and o["start_date"].startswith("2026")
    assert o["public_log"]["entries"][0]["message_html"] == "<p>[text]<a>[text]</a></p>", o["public_log"]
    assert o["agent_id"] == o["contacts_list"][0]["contact_id"], "ids must map consistently"
    try:
        Client("https://x", "u", "p").call({"operation": "core/update"})
        raise AssertionError("write not refused")
    except SystemExit as e:
        assert "Refused" in str(e)
    print("selftest passed: no leaks, enums and dates kept, HTML tags kept, ids consistent, writes refused")
    return 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else probe())
