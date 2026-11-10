#!/usr/bin/env python3
"""Read-only probe for Task 8b: how does iTop store a web stopwatch that's still running?

Run it while the stopwatch is running on a test ticket, then again after it stops.
It finds the ticket's Activity and YOUR TimeSpent records on it, and prints and saves
ONLY their shape: origin, whether start/end are set, duration, and minutes since start/end.
No titles, descriptions, names or dates are saved.

Usage:
  python3 -I scripts/probe-stopwatch.py            # probe your server
  python3 -I scripts/probe-stopwatch.py --selftest # offline checks, no network
"""
import datetime
import getpass
import json
import os
import re
import ssl
import sys
import urllib.parse
import urllib.request

READ_ONLY_OPS = {"core/get"}
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "Tests", "ITopKitTests", "Fixtures", "stopwatch-probe.json")
REF = re.compile(r"^(R|I)-\d{1,9}$")


class Client:
    def __init__(self, base, user, pwd):
        self.base = base.rstrip("/")
        self.user, self.pwd = user, pwd
        self.ctx = ssl.create_default_context()

    def call(self, payload):
        if payload.get("operation") not in READ_ONLY_OPS:
            raise SystemExit("Refused: %r is not a read operation." % payload.get("operation"))
        body = urllib.parse.urlencode({"auth_user": self.user, "auth_pwd": self.pwd, "json_data": json.dumps(payload)}).encode()
        req = urllib.request.Request(self.base + "/webservices/rest.php?version=1.4", data=body, method="POST")
        with urllib.request.urlopen(req, context=self.ctx, timeout=60) as r:
            return json.loads(r.read().decode("utf-8"))

    def get(self, cls, key, fields):
        return self.call({"operation": "core/get", "class": cls, "key": key, "output_fields": fields})


def objects(resp):
    return list((resp.get("objects") or {}).values())


def minutes_ago(value, now):
    """Minutes between an iTop date and now; None when empty or unreadable."""
    if not value:
        return None
    try:
        d = datetime.datetime.strptime(value, "%Y-%m-%d %H:%M:%S")
    except ValueError:
        return "unreadable"
    return round((now - d).total_seconds() / 60)


def shape(fields, now):
    return {
        "origin": fields.get("origin"),
        "start_set": bool(fields.get("start_date")),
        "end_set": bool(fields.get("end_date")),
        "end_raw_if_odd": fields.get("end_date") if fields.get("end_date") and not re.match(r"^\d{4}-", fields.get("end_date")) else None,
        "duration": fields.get("duration"),
        "minutes_since_start": minutes_ago(fields.get("start_date"), now),
        "minutes_since_end": minutes_ago(fields.get("end_date"), now),
        "fields": sorted(fields.keys()),
    }


def probe():
    print("iTop Wrap stopwatch probe. Read-only; saves only the shape of your time records.\n", file=sys.stderr)
    base = input("Server URL (https://...): ").strip()
    user = input("Username: ").strip()
    pwd = getpass.getpass("Password (hidden): ")
    ref = input("Test ticket ref (for example R-012345): ").strip().upper()
    if not REF.match(ref):
        raise SystemExit("That doesn't look like a ticket ref.")
    c = Client(base, user, pwd)
    cls = "UserRequest" if ref.startswith("R") else "Incident"
    summary = {"step": input("Is the web stopwatch running right now? (yes/no): ").strip().lower(), "notes": []}

    me = objects(c.get("User", "SELECT User WHERE login = '%s'" % user.replace("\\", "\\\\").replace("'", "\\'"), "contactid"))
    user_id = me[0]["key"] if me else "0"
    t = objects(c.get(cls, "SELECT %s WHERE ref = '%s'" % (cls, ref), "id"))
    if not t:
        raise SystemExit("Ticket not found, or your account can't see it.")
    ticket_id = t[0]["key"]
    acts = objects(c.get("Activity", "SELECT Activity WHERE obj_class = '%s' AND obj_id = %d" % (cls, int(ticket_id)), "obj_class"))
    summary["activities_on_ticket"] = len(acts)
    now = datetime.datetime.now()
    summary["note_on_time"] = "minutes are measured against this Mac's clock; iTop's server clock or timezone may differ"
    records = []
    for a in acts:
        spent = objects(c.get("TimeSpent", "SELECT TimeSpent WHERE activity_id = %d AND user_id = %d" % (int(a["key"]), int(user_id)),
                              "origin,start_date,end_date,duration"))
        spent.sort(key=lambda o: int(o["key"]))
        records += [shape(o.get("fields") or {}, now) for o in spent[-5:]]
    summary["your_latest_timespent_on_ticket"] = records
    if not records:
        summary["notes"].append("No TimeSpent of yours on this ticket yet. If the stopwatch is running, it may not be a TimeSpent until it stops.")

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    existing = []
    if os.path.exists(OUT):
        try:
            existing = json.load(open(OUT))
            existing = existing if isinstance(existing, list) else [existing]
        except ValueError:
            existing = []
    with open(OUT, "w") as f:
        json.dump(existing + [summary], f, indent=2)
        f.write("\n")
    print(json.dumps(summary, indent=2))
    print("\nAdded this run to %s. Only read operations were sent." % os.path.relpath(OUT), file=sys.stderr)


def selftest():
    now = datetime.datetime(2026, 11, 11, 12, 0, 0)
    s = shape({"origin": "stopwatch", "start_date": "2026-11-11 11:30:00", "end_date": "", "duration": "0", "title": "secret"}, now)
    assert s["minutes_since_start"] == 30 and s["end_set"] is False and s["minutes_since_end"] is None, s
    assert "secret" not in json.dumps({k: v for k, v in s.items() if k != "fields"}), "titles must never be saved"
    assert not REF.match("R-1 OR 1=1") and REF.match("R-079496")
    try:
        Client("https://itop.example.com", "u", "p").call({"operation": "core/create"})
        raise AssertionError("write not refused")
    except SystemExit as e:
        assert "Refused" in str(e)
    print("selftest passed: shape only, refs checked, writes refused")


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else probe())
